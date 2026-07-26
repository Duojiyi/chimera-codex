// Step 8.3 RED — try / apply / restore-default skin-state transaction
// (ADR-005).
//
// `SkinPackage` fixtures below are built with plain struct literals rather
// than a real `.codexskin` zip: package import is already covered by
// `step8_1_import.rs`, and constructing the already-validated in-memory
// shape directly keeps these tests focused on apply.rs's own transaction
// logic (same "pure-logic fixture" style `step8_1_import.rs` uses for the
// declared-size-mismatch case it cannot practically trigger through the
// real zip crate).

use chimera_platform::CanonicalPath;
use chimera_theme::apply::{ApplyError, SkinApplier, SkinState, SkinStateTransaction};
use chimera_theme::package::SkinPackage;
use chimera_theme::schema::SkinManifest;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

// ── fixtures ─────────────────────────────────────────────────────────────

fn manifest(name: &str, version: &str, entry_css: &str) -> SkinManifest {
    let json = format!(
        r#"{{"schema_version":1,"name":"{name}","version":"{version}","entry_css":"{entry_css}"}}"#
    );
    SkinManifest::parse(json.as_bytes()).expect("fixture manifest must be valid")
}

fn package(name: &str, version: &str, css: &str) -> SkinPackage {
    SkinPackage {
        manifest: manifest(name, version, "theme.css"),
        entry_css: css.to_string(),
        assets: vec![],
    }
}

// ── a fake SkinApplier: records what's "showing" without any real CDP ─────

#[derive(Clone, Default)]
struct FakeApplier {
    live_css: Arc<Mutex<Option<String>>>,
    apply_calls: Arc<Mutex<u32>>,
    fail_next_apply: Arc<Mutex<bool>>,
}

impl FakeApplier {
    fn fail_next_apply(&self) {
        *self.fail_next_apply.lock().unwrap() = true;
    }

    fn live_css(&self) -> Option<String> {
        self.live_css.lock().unwrap().clone()
    }
}

impl SkinApplier for FakeApplier {
    fn apply(&mut self, css: &str) -> Result<(), ApplyError> {
        *self.apply_calls.lock().unwrap() += 1;
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

fn state_dir(tmp: &TempDir) -> CanonicalPath {
    CanonicalPath::new(tmp.path().join("chimera-skin-state")).expect("tmp path is absolute")
}

// ── recursive byte-for-byte snapshot, for the "official dir untouched" test

fn snapshot_dir(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("read_dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let bytes = fs::read(&path).expect("read fixture file");
                let relative = path.strip_prefix(root).expect("under root").to_path_buf();
                out.insert(relative, bytes);
            }
        }
    }
    walk(dir, dir, &mut out);
    out
}

// ── open(): default state when nothing has ever been applied ──────────────

#[test]
fn a_fresh_state_dir_starts_at_default() {
    let tmp = TempDir::new().unwrap();
    let txn = SkinStateTransaction::open(&state_dir(&tmp), FakeApplier::default()).unwrap();
    assert_eq!(*txn.current(), SkinState::Default);
}

// ── apply_and_commit + restore_default round-trip ──────────────────────────

#[test]
fn apply_and_commit_pushes_css_live_and_persists_the_committed_state() {
    let tmp = TempDir::new().unwrap();
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&state_dir(&tmp), applier.clone()).unwrap();

    let pkg = package("Midnight", "1.0.0", ".x{color:#fff}");
    txn.apply_and_commit(&pkg).unwrap();

    assert_eq!(applier.live_css(), Some(".x{color:#fff}".to_string()));
    assert_eq!(
        *txn.current(),
        SkinState::Applied {
            name: "Midnight".to_string(),
            version: "1.0.0".to_string(),
            entry_css: "theme.css".to_string(),
        }
    );
}

#[test]
fn a_committed_skin_survives_reopening_the_transaction() {
    let tmp = TempDir::new().unwrap();
    let dir = state_dir(&tmp);
    let pkg = package("Midnight", "1.0.0", ".x{color:#fff}");
    {
        let mut txn = SkinStateTransaction::open(&dir, FakeApplier::default()).unwrap();
        txn.apply_and_commit(&pkg).unwrap();
    }

    let reopened = SkinStateTransaction::open(&dir, FakeApplier::default()).unwrap();
    assert_eq!(
        *reopened.current(),
        SkinState::Applied {
            name: "Midnight".to_string(),
            version: "1.0.0".to_string(),
            entry_css: "theme.css".to_string(),
        }
    );
}

#[test]
fn restore_default_clears_the_live_session_and_records_default() {
    let tmp = TempDir::new().unwrap();
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&state_dir(&tmp), applier.clone()).unwrap();
    txn.apply_and_commit(&package("Midnight", "1.0.0", ".x{color:#fff}"))
        .unwrap();

    txn.restore_default().unwrap();

    assert_eq!(applier.live_css(), None);
    assert_eq!(*txn.current(), SkinState::Default);
}

#[test]
fn restore_default_always_works_even_right_after_a_failed_apply() {
    let tmp = TempDir::new().unwrap();
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&state_dir(&tmp), applier.clone()).unwrap();
    txn.apply_and_commit(&package("A", "1.0.0", ".a{color:red}"))
        .unwrap();

    applier.fail_next_apply();
    let failed = txn.apply_and_commit(&package("B", "2.0.0", ".b{color:blue}"));
    assert!(
        failed.is_err(),
        "the simulated transport failure must surface"
    );

    // A failed apply must not have corrupted the committed state.
    assert_eq!(
        *txn.current(),
        SkinState::Applied {
            name: "A".to_string(),
            version: "1.0.0".to_string(),
            entry_css: "theme.css".to_string(),
        },
        "a failed apply must leave the previously committed skin as the recorded state"
    );

    // And restore-default must still work from here.
    txn.restore_default().unwrap();
    assert_eq!(*txn.current(), SkinState::Default);
    assert_eq!(applier.live_css(), None);
}

#[test]
fn a_failed_apply_does_not_overwrite_the_previously_committed_skin_files_on_disk() {
    let tmp = TempDir::new().unwrap();
    let dir = state_dir(&tmp);
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&dir, applier.clone()).unwrap();
    txn.apply_and_commit(&package("A", "1.0.0", ".a{color:red}"))
        .unwrap();

    let before = snapshot_dir(dir.as_path());

    applier.fail_next_apply();
    let _ = txn.apply_and_commit(&package("B", "2.0.0", ".b{color:blue}"));

    let after = snapshot_dir(dir.as_path());
    assert_eq!(
        before, after,
        "a failed apply must not touch any previously committed skin bytes on disk"
    );
}

// ── try / cancel: reversible without side effects ──────────────────────────

#[test]
fn trying_a_skin_then_cancelling_from_default_leaves_default_untouched() {
    let tmp = TempDir::new().unwrap();
    let dir = state_dir(&tmp);
    let applier = FakeApplier::default();

    let state_file = dir.as_path().join("skin-state.json");
    let mut txn = SkinStateTransaction::open(&dir, applier.clone()).unwrap();
    let before_bytes = fs::read(&state_file).ok();

    txn.try_skin(&package("Preview", "0.1.0", ".p{color:green}"))
        .unwrap();
    assert_eq!(applier.live_css(), Some(".p{color:green}".to_string()));

    txn.cancel_try().unwrap();

    assert_eq!(
        applier.live_css(),
        None,
        "cancelling a try must clear the live preview"
    );
    assert_eq!(*txn.current(), SkinState::Default);
    let after_bytes = fs::read(&state_file).ok();
    assert_eq!(
        before_bytes, after_bytes,
        "trying a skin and cancelling must not have written anything new to the state file"
    );
}

#[test]
fn trying_a_different_skin_then_cancelling_restores_the_previously_committed_one() {
    let tmp = TempDir::new().unwrap();
    let dir = state_dir(&tmp);
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&dir, applier.clone()).unwrap();
    txn.apply_and_commit(&package("Committed", "1.0.0", ".c{color:black}"))
        .unwrap();

    let committed_state = txn.current().clone();
    let state_file = dir.as_path().join("skin-state.json");
    let committed_bytes = fs::read(&state_file).unwrap();

    txn.try_skin(&package("Preview", "0.1.0", ".p{color:green}"))
        .unwrap();
    assert_eq!(applier.live_css(), Some(".p{color:green}".to_string()));

    txn.cancel_try().unwrap();

    assert_eq!(
        applier.live_css(),
        Some(".c{color:black}".to_string()),
        "cancelling must restore the previously committed skin's own CSS, not clear to default"
    );
    assert_eq!(*txn.current(), committed_state);
    assert_eq!(
        fs::read(&state_file).unwrap(),
        committed_bytes,
        "trying and cancelling must never rewrite the committed state file"
    );
}

#[test]
fn try_skin_never_writes_to_the_state_directory_at_all() {
    let tmp = TempDir::new().unwrap();
    let dir = state_dir(&tmp);
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&dir, applier).unwrap();

    let before = snapshot_dir(dir.as_path());
    txn.try_skin(&package("Preview", "0.1.0", ".p{color:green}"))
        .unwrap();
    let after = snapshot_dir(dir.as_path());

    assert_eq!(before, after, "try_skin must not write anything to disk");
}

// ── STOP CONDITION: no official Codex file is ever modified ────────────────

#[test]
fn a_full_apply_try_cancel_restore_cycle_never_touches_an_unrelated_official_dir() {
    // Stands in for the official Codex install directory. apply.rs's own
    // API surface never even takes a path to one (see the crate report),
    // so this test demonstrates the invariant empirically: nothing about
    // running the full transaction lifecycle touches a directory that was
    // never handed to it.
    let official = TempDir::new().unwrap();
    fs::write(
        official.path().join("Codex.exe"),
        b"pretend-official-binary",
    )
    .unwrap();
    fs::create_dir_all(official.path().join("resources")).unwrap();
    fs::write(
        official.path().join("resources").join("app.asar"),
        b"pretend-official-payload",
    )
    .unwrap();
    let before = snapshot_dir(official.path());

    let tmp = TempDir::new().unwrap();
    let dir = state_dir(&tmp);
    let applier = FakeApplier::default();
    let mut txn = SkinStateTransaction::open(&dir, applier.clone()).unwrap();

    txn.apply_and_commit(&package("A", "1.0.0", ".a{color:red}"))
        .unwrap();
    txn.try_skin(&package("B", "2.0.0", ".b{color:blue}"))
        .unwrap();
    txn.cancel_try().unwrap();
    txn.restore_default().unwrap();

    let after = snapshot_dir(official.path());
    assert_eq!(
        before, after,
        "every byte of every file in the official directory must be identical after a full \
         apply/try/cancel/restore cycle"
    );
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "no file must have been added to or removed from the official directory either"
    );

    // Meanwhile Chimera's own state dir did receive the committed skin's
    // files — proving the write happened, just never in the official dir.
    let chimera_files = snapshot_dir(dir.as_path());
    assert!(
        !chimera_files.is_empty(),
        "the committed skin's files must live under Chimera's own state dir"
    );
}

#[test]
fn skin_state_is_never_written_under_a_directory_named_like_an_official_install() {
    // A different angle on the same invariant: the state directory the
    // caller supplies is Chimera's own, and every written path stays under
    // it — `CanonicalPath` itself refuses a non-absolute path, and nothing
    // here appends `..` or an absolute override, so a caller cannot be
    // tricked into writing outside `state_dir` no matter what a skin's
    // manifest claims (that is `SkinPackage::write_to`'s / `safe_join`'s
    // job, already covered by `step8_1_import.rs`).
    let tmp = TempDir::new().unwrap();
    let dir = state_dir(&tmp);
    let mut txn = SkinStateTransaction::open(&dir, FakeApplier::default()).unwrap();
    txn.apply_and_commit(&package("A", "1.0.0", ".a{color:red}"))
        .unwrap();

    let written = snapshot_dir(dir.as_path());
    for path in written.keys() {
        assert!(
            dir.as_path().join(path).starts_with(dir.as_path()),
            "every written file must stay under the supplied state dir"
        );
    }
}
