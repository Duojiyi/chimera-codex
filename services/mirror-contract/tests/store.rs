// Step 4.5 RED — CAS promotion at the storage layer.
//
// cas.rs::validate_stable_promotion is a pure comparison: it proves a sequence
// is monotonic but cannot stop two workflows from both reading sequence N,
// both validating, and both writing N+1. Spec 9.4 requires the read-validate-
// write window to be held under an exclusive lock. These tests prove it is.

use mirror_contract::cas::StablePointer;
use mirror_contract::store::{StableStore, StoreError};
use tempfile::tempdir;

fn pointer(seq: u64, version: &str) -> StablePointer {
    StablePointer {
        codex_version: version.to_string(),
        raw_digest: format!("sha256:raw-{version}"),
        manifest_digest: format!("sha256:manifest-{version}"),
        promoted_at: "2026-07-26T00:00:00Z".to_string(),
        sequence: seq,
    }
}

// ── Promotion writes and reads back ──────────────────────────────────────────

#[test]
fn promote_writes_a_pointer_that_reads_back_identically() {
    let tmp = tempdir().unwrap();
    let store = StableStore::new(tmp.path());
    store.initialise().unwrap();

    store.promote(&pointer(1, "26.721")).unwrap();

    let read = store.read_pointer().unwrap().expect("pointer must exist");
    assert_eq!(read.sequence, 1);
    assert_eq!(read.codex_version, "26.721");
}

#[test]
fn a_fresh_store_has_no_pointer_rather_than_erroring() {
    let tmp = tempdir().unwrap();
    let store = StableStore::new(tmp.path());
    store.initialise().unwrap();

    assert!(
        store.read_pointer().unwrap().is_none(),
        "an empty store must report absence, not fail"
    );
}

// ── Monotonicity enforced at the storage layer ───────────────────────────────

#[test]
fn a_lower_sequence_is_refused_even_though_the_file_is_writable() {
    let tmp = tempdir().unwrap();
    let store = StableStore::new(tmp.path());
    store.initialise().unwrap();

    store.promote(&pointer(5, "26.721")).unwrap();

    let err = store
        .promote(&pointer(4, "26.700"))
        .expect_err("a rollback attack must be refused");
    assert!(matches!(err, StoreError::SequenceConflict { .. }));

    // The refused promotion must not have partially applied.
    let read = store.read_pointer().unwrap().unwrap();
    assert_eq!(read.sequence, 5, "the refused write must not have landed");
    assert_eq!(read.codex_version, "26.721");
}

#[test]
fn an_equal_sequence_is_refused_as_stale() {
    let tmp = tempdir().unwrap();
    let store = StableStore::new(tmp.path());
    store.initialise().unwrap();
    store.promote(&pointer(3, "26.721")).unwrap();

    let err = store
        .promote(&pointer(3, "26.722"))
        .expect_err("replaying the same sequence must be refused");
    assert!(matches!(err, StoreError::SequenceConflict { .. }));
}

// ── The lock actually closes the race window ─────────────────────────────────

#[test]
fn promotion_is_refused_while_another_promotion_holds_the_lock() {
    let tmp = tempdir().unwrap();
    let store = StableStore::new(tmp.path());
    store.initialise().unwrap();

    // Hold the lock as a concurrent workflow would.
    let _guard = store.lock_for_test().expect("first holder acquires");

    let err = store
        .promote(&pointer(1, "26.721"))
        .expect_err("a second promotion must not proceed while the lock is held");
    assert!(matches!(err, StoreError::Locked { .. }));
}

#[test]
fn the_lock_is_released_so_a_later_promotion_succeeds() {
    let tmp = tempdir().unwrap();
    let store = StableStore::new(tmp.path());
    store.initialise().unwrap();

    {
        let _guard = store.lock_for_test().unwrap();
    } // released here

    store
        .promote(&pointer(1, "26.721"))
        .expect("the lock must be released when the guard drops");
}

// ── Audit trail ──────────────────────────────────────────────────────────────

#[test]
fn every_accepted_promotion_appends_to_the_log() {
    let tmp = tempdir().unwrap();
    let store = StableStore::new(tmp.path());
    store.initialise().unwrap();

    store.promote(&pointer(1, "26.721")).unwrap();
    store.promote(&pointer(2, "26.722")).unwrap();

    let log = store.read_log().unwrap();
    assert_eq!(log.len(), 2, "each accepted promotion appends one entry");
    assert_eq!(log[0].sequence, 1);
    assert_eq!(log[1].sequence, 2);
}

#[test]
fn a_refused_promotion_is_not_logged_as_if_it_happened() {
    let tmp = tempdir().unwrap();
    let store = StableStore::new(tmp.path());
    store.initialise().unwrap();

    store.promote(&pointer(2, "26.721")).unwrap();
    let _ = store.promote(&pointer(1, "26.700"));

    let log = store.read_log().unwrap();
    assert_eq!(
        log.len(),
        1,
        "a refused promotion must not appear in the log"
    );
    assert_eq!(log[0].sequence, 2);
}
