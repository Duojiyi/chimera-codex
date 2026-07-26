// Step 8.3 regression — the live session and the recorded state must never
// disagree about which skin is applied.
//
// Found by an adversarial review of apply.rs. The live push used to happen
// before the disk write, so a failing disk write left the browser showing the
// failed package's CSS while `skin-state.json` still named the previous one.
// Restore-default would then restore the wrong skin, and the user would be
// looking at something the app believed was not applied — a state no retry can
// resolve, because both halves think they are correct.
//
// Kept in its own file rather than folded into step8_3_apply.rs: the property
// is about the relationship BETWEEN the two stores, not about either one, and
// the assertion below deliberately does not care which of the two legal
// outcomes occurs — only that they match.

use chimera_platform::CanonicalPath;
use chimera_theme::apply::{ApplyError, SkinApplier, SkinState, SkinStateTransaction};
use chimera_theme::package::{AssetKind, SkinAsset, SkinPackage};
use chimera_theme::schema::SkinManifest;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

fn package(name: &str, version: &str, css: &str) -> SkinPackage {
    let json = format!(
        r#"{{"schema_version":1,"name":"{name}","version":"{version}","entry_css":"theme.css"}}"#
    );
    SkinPackage {
        manifest: SkinManifest::parse(json.as_bytes()).expect("fixture manifest must be valid"),
        entry_css: css.to_string(),
        assets: vec![],
    }
}

#[derive(Clone, Default)]
struct FakeApplier {
    live_css: Arc<Mutex<Option<String>>>,
    fail_next_apply: Arc<Mutex<bool>>,
}

impl SkinApplier for FakeApplier {
    fn apply(&mut self, css: &str) -> Result<(), ApplyError> {
        if std::mem::take(&mut *self.fail_next_apply.lock().unwrap()) {
            return Err(ApplyError::Io("simulated transport failure".to_string()));
        }
        *self.live_css.lock().unwrap() = Some(css.to_string());
        Ok(())
    }
    fn clear(&mut self) -> Result<(), ApplyError> {
        *self.live_css.lock().unwrap() = None;
        Ok(())
    }
}

/// The invariant, expressed once: whatever state the transaction records, the
/// live session must be showing something consistent with it.
fn assert_consistent(state: &SkinState, live: Option<String>) {
    match state {
        SkinState::Default => assert_eq!(
            live, None,
            "recorded state says Default but the live session is showing a skin"
        ),
        SkinState::Applied { .. } => assert!(
            live.is_some(),
            "recorded state names an applied skin but the live session shows none"
        ),
    }
}

#[test]
fn a_failed_live_push_leaves_the_two_stores_agreeing() {
    let tmp = TempDir::new().unwrap();
    let dir = CanonicalPath::new(tmp.path().join("skin-state")).unwrap();
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&dir, applier.clone()).unwrap();

    txn.apply_and_commit(&package("A", "1.0.0", ".a{color:red}"))
        .unwrap();
    assert_consistent(txn.current(), applier.live_css.lock().unwrap().clone());

    *applier.fail_next_apply.lock().unwrap() = true;
    let result = txn.apply_and_commit(&package("B", "2.0.0", ".b{color:blue}"));

    assert!(result.is_err(), "the failing live push must be reported");
    let live = applier.live_css.lock().unwrap().clone();
    assert_ne!(
        live,
        Some(".b{color:blue}".to_string()),
        "the live session is showing the CSS of a package whose apply failed"
    );
    assert_consistent(txn.current(), live);
}

#[test]
fn the_previous_skin_is_still_what_is_recorded_after_a_failed_live_push() {
    // The weaker "they agree" check above is satisfied by falling back to
    // Default. That is the correct outcome only for a failure the transaction
    // cannot undo. A failed live push happens before anything is published, so
    // the previously committed skin must survive intact.
    let tmp = TempDir::new().unwrap();
    let dir = CanonicalPath::new(tmp.path().join("skin-state")).unwrap();
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&dir, applier.clone()).unwrap();

    txn.apply_and_commit(&package("A", "1.0.0", ".a{color:red}"))
        .unwrap();

    *applier.fail_next_apply.lock().unwrap() = true;
    let _ = txn.apply_and_commit(&package("B", "2.0.0", ".b{color:blue}"));

    match txn.current() {
        SkinState::Applied { name, version, .. } => {
            assert_eq!(name, "A", "the previously committed skin was replaced");
            assert_eq!(version, "1.0.0");
        }
        other => panic!("the previously committed skin was lost: {other:?}"),
    }
    assert_eq!(
        applier.live_css.lock().unwrap().clone(),
        Some(".a{color:red}".to_string()),
        "the live session should still be showing the skin that is still committed"
    );
}

/// A package whose asset name escapes its destination, so `write_to` refuses.
///
/// This is the failure the original bug needed: one that happens *after* a
/// successful live push under the old ordering. Constructed directly rather
/// than imported from a zip, because `import_codexskin` correctly rejects this
/// name and the package could never exist in memory by that route — which is
/// the point. `write_to` is defence in depth for a package assembled any other
/// way, and this test exercises it.
fn package_with_unwritable_asset(name: &str, version: &str, css: &str) -> SkinPackage {
    let mut pkg = package(name, version, css);
    pkg.assets.push(SkinAsset {
        name: "../escape.png".to_string(),
        bytes: vec![0u8; 4],
        kind: AssetKind::Png,
    });
    pkg
}

#[test]
fn a_disk_failure_after_a_successful_live_push_is_impossible() {
    // The original defect, stated as the property that prevents it: the live
    // session must not be showing a package whose commit failed on disk.
    //
    // Under the old ordering the browser was updated first, so this exact
    // sequence left it showing B's CSS while the recorded state still said A.
    // Staging to disk before touching the live session removes the window
    // rather than compensating for it.
    let tmp = TempDir::new().unwrap();
    let dir = CanonicalPath::new(tmp.path().join("skin-state")).unwrap();
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&dir, applier.clone()).unwrap();

    txn.apply_and_commit(&package("A", "1.0.0", ".a{color:red}"))
        .unwrap();

    let result = txn.apply_and_commit(&package_with_unwritable_asset(
        "B",
        "2.0.0",
        ".b{color:blue}",
    ));
    assert!(result.is_err(), "an unwritable asset must fail the commit");

    let live = applier.live_css.lock().unwrap().clone();
    assert_eq!(
        live,
        Some(".a{color:red}".to_string()),
        "the live session is showing a package whose commit failed"
    );
    match txn.current() {
        SkinState::Applied { name, .. } => assert_eq!(name, "A"),
        other => panic!("the previously committed skin was lost: {other:?}"),
    }
    assert_consistent(txn.current(), live);
}

#[test]
fn restore_default_reaches_a_consistent_state_from_a_failed_apply() {
    let tmp = TempDir::new().unwrap();
    let dir = CanonicalPath::new(tmp.path().join("skin-state")).unwrap();
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&dir, applier.clone()).unwrap();

    *applier.fail_next_apply.lock().unwrap() = true;
    let _ = txn.apply_and_commit(&package("A", "1.0.0", ".a{color:red}"));

    txn.restore_default()
        .expect("restore must work after a failed apply");

    assert!(matches!(txn.current(), SkinState::Default));
    assert_consistent(txn.current(), applier.live_css.lock().unwrap().clone());
}
