use chimera_platform::CanonicalPath;
use chimera_theme::apply::{ApplyError, SkinApplier, SkinState, SkinStateTransaction};
use chimera_theme::package::SkinPackage;
use chimera_theme::schema::SkinManifest;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

fn manifest(name: &str, version: &str, entry_css: &str) -> SkinManifest {
    let json = format!(
        r#"{{"schema_version":1,"name":"{name}","version":"{version}","entry_css":"{entry_css}"}}"#
    );
    SkinManifest::parse(json.as_bytes()).expect("fixture manifest must be valid")
}

fn package_with_bad_asset(name: &str, version: &str, css: &str) -> SkinPackage {
    use chimera_theme::package::Asset;
    SkinPackage {
        manifest: manifest(name, version, "theme.css"),
        entry_css: css.to_string(),
        assets: vec![Asset {
            name: "../escape.txt".to_string(),
            bytes: b"evil".to_vec(),
        }],
    }
}

#[derive(Clone, Default)]
struct FakeApplier {
    live_css: Arc<Mutex<Option<String>>>,
}
impl SkinApplier for FakeApplier {
    fn apply(&mut self, css: &str) -> Result<(), ApplyError> {
        *self.live_css.lock().unwrap() = Some(css.to_string());
        Ok(())
    }
    fn clear(&mut self) -> Result<(), ApplyError> {
        *self.live_css.lock().unwrap() = None;
        Ok(())
    }
}

#[test]
fn live_state_diverges_from_recorded_state_when_disk_write_fails_after_live_push_succeeds() {
    let tmp = TempDir::new().unwrap();
    let dir = CanonicalPath::new(tmp.path().join("state")).unwrap();
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&dir, applier.clone()).unwrap();

    // Commit a good skin "A" first.
    let good = {
        SkinPackage {
            manifest: manifest("A", "1.0.0", "theme.css"),
            entry_css: ".a{color:red}".to_string(),
            assets: vec![],
        }
    };
    txn.apply_and_commit(&good).unwrap();
    assert_eq!(applier.live_css.lock().unwrap().clone(), Some(".a{color:red}".to_string()));

    // Now attempt to apply "B", whose asset write will fail (unsafe path),
    // AFTER the live push already succeeded.
    let bad = package_with_bad_asset("B", "2.0.0", ".b{color:blue}");
    let result = txn.apply_and_commit(&bad);
    assert!(result.is_err(), "the bad asset write must fail");

    let live_now = applier.live_css.lock().unwrap().clone();
    println!("live css after failed commit: {:?}", live_now);
    println!("txn.current() after failed commit: {:?}", txn.current());

    // The bug: live browser shows B's css, but recorded state still says A.
    assert_eq!(live_now, Some(".b{color:blue}".to_string()), "live session now shows the FAILED package's css");
    assert_eq!(
        *txn.current(),
        SkinState::Applied { name: "A".to_string(), version: "1.0.0".to_string(), entry_css: "theme.css".to_string() },
        "but recorded state still claims the OLD committed skin -- live and recorded have diverged"
    );
}
