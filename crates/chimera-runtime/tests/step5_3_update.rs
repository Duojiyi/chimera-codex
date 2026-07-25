// Steps 5.3/5.4 RED — Runtime directory layout, current pointer, fault injection.
// Spec 8.2-8.3: staging → atomic pointer switch → rollback on failure.
use chimera_runtime::update::{
    RuntimeLayout, UpdatePointer, stage_version, commit_version,
    rollback_to_last_known, UpdateError,
};
use tempfile::tempdir;
use std::fs;

fn make_layout(tmp: &tempfile::TempDir) -> RuntimeLayout {
    RuntimeLayout::new(tmp.path().join("runtime"))
}

// ── Directory layout ──────────────────────────────────────────────────────────

#[test]
fn layout_initialise_creates_required_dirs() {
    let tmp = tempdir().unwrap();
    let layout = make_layout(&tmp);
    layout.initialise().unwrap();
    assert!(layout.versions_dir().exists(), "versions/ must be created");
    assert!(layout.staging_dir().exists(), "staging/ must be created");
    assert!(layout.backup_dir().exists(), "backup/ must be created");
}

// ── Stage + commit ────────────────────────────────────────────────────────────

#[test]
fn stage_and_commit_creates_version_dir_and_current_pointer() {
    let tmp = tempdir().unwrap();
    let layout = make_layout(&tmp);
    layout.initialise().unwrap();

    // Create a fake payload in staging
    let version = "26.721";
    let staged_dir = stage_version(&layout, version).unwrap();
    // Put a fake exe in staging
    fs::write(staged_dir.join("Codex.exe"), b"fake-exe").unwrap();

    let pointer = commit_version(&layout, version, "sha256:abc123").unwrap();
    assert_eq!(pointer.active_version, version);
    assert!(layout.version_dir(version).exists(), "version dir must exist after commit");
    assert!(layout.current_pointer_path().exists(), "current.json must exist");
}

#[test]
fn current_pointer_roundtrips() {
    let tmp = tempdir().unwrap();
    let layout = make_layout(&tmp);
    layout.initialise().unwrap();

    let staged = stage_version(&layout, "26.732").unwrap();
    fs::write(staged.join("Codex.exe"), b"fake").unwrap();
    let committed = commit_version(&layout, "26.732", "sha256:new").unwrap();

    let read_back = layout.read_current_pointer().unwrap().unwrap();
    assert_eq!(read_back.active_version, "26.732");
    assert_eq!(read_back.source_manifest_digest, "sha256:new");
}

// ── Rollback ──────────────────────────────────────────────────────────────────

#[test]
fn rollback_to_last_known_good_restores_previous() {
    let tmp = tempdir().unwrap();
    let layout = make_layout(&tmp);
    layout.initialise().unwrap();

    // Commit version 1 (stable)
    let s1 = stage_version(&layout, "26.721").unwrap();
    fs::write(s1.join("Codex.exe"), b"v1").unwrap();
    commit_version(&layout, "26.721", "sha256:v1").unwrap();

    // Commit version 2 (broken)
    let s2 = stage_version(&layout, "26.732").unwrap();
    fs::write(s2.join("Codex.exe"), b"v2").unwrap();
    commit_version(&layout, "26.732", "sha256:v2").unwrap();

    // Roll back — must return to v1
    let restored = rollback_to_last_known(&layout).unwrap();
    assert_eq!(restored.active_version, "26.721",
        "rollback must restore previous version");

    let current = layout.read_current_pointer().unwrap().unwrap();
    assert_eq!(current.active_version, "26.721");
}

#[test]
fn rollback_with_no_previous_version_returns_error() {
    let tmp = tempdir().unwrap();
    let layout = make_layout(&tmp);
    layout.initialise().unwrap();
    let result = rollback_to_last_known(&layout);
    assert!(result.is_err(), "rollback with no versions must fail");
    assert!(matches!(result.unwrap_err(), UpdateError::NoPreviousVersion));
}

// ── Staging does not touch current pointer ────────────────────────────────────

#[test]
fn staging_does_not_modify_current_pointer() {
    let tmp = tempdir().unwrap();
    let layout = make_layout(&tmp);
    layout.initialise().unwrap();

    // No current pointer yet
    let _staged = stage_version(&layout, "26.800").unwrap();
    assert!(!layout.current_pointer_path().exists(),
        "staging must not create current.json");
}
