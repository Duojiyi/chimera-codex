// Step 9.1 RED — persisted trust state cache, namespaced apart from the
// Codex mirror's own cache (G8/G15).

use chimera_update::cache::{APP_TRUST_CACHE_DIRNAME, CacheError, UpdateCache};
use chimera_update::metadata::{MetaSignature, SignedPayload};
use std::fs;
use tempfile::TempDir;

fn sample() -> SignedPayload {
    SignedPayload {
        payload: r#"{"version":1}"#.to_string(),
        signatures: vec![MetaSignature {
            key_id: "k1".to_string(),
            signature_hex: "aa".to_string(),
        }],
    }
}

#[test]
fn a_fresh_cache_has_no_persisted_root() {
    let dir = TempDir::new().unwrap();
    let cache = UpdateCache::new(dir.path());
    cache.initialise().unwrap();

    assert!(cache.read_root().unwrap().is_none());
}

#[test]
fn a_written_root_round_trips() {
    let dir = TempDir::new().unwrap();
    let cache = UpdateCache::new(dir.path());
    cache.initialise().unwrap();

    cache.write_root(&sample()).unwrap();
    let read = cache.read_root().unwrap().expect("must round-trip");
    assert_eq!(read.payload, sample().payload);
}

#[test]
fn timestamp_snapshot_and_targets_each_round_trip_independently() {
    let dir = TempDir::new().unwrap();
    let cache = UpdateCache::new(dir.path());
    cache.initialise().unwrap();

    cache.write_timestamp(&sample()).unwrap();
    cache.write_snapshot(&sample()).unwrap();
    cache.write_targets(&sample()).unwrap();

    assert!(cache.read_timestamp().unwrap().is_some());
    assert!(cache.read_snapshot().unwrap().is_some());
    assert!(cache.read_targets().unwrap().is_some());
}

#[test]
fn a_corrupt_root_file_fails_closed_instead_of_being_treated_as_absent() {
    // Fail closed (house rule): a corrupt cache must not be silently treated
    // as "no trust state yet", which would let an attacker force a client
    // back to bootstrap-from-bundled-root by merely truncating a file it can
    // write to. It must be a distinct, loud error.
    let dir = TempDir::new().unwrap();
    let cache = UpdateCache::new(dir.path());
    cache.initialise().unwrap();
    cache.write_root(&sample()).unwrap();

    fs::write(cache.root_path_for_test(), b"{ not json at all").unwrap();

    let err = cache.read_root().unwrap_err();
    assert!(matches!(err, CacheError::Corrupt(_)));
}

#[test]
fn the_cache_directory_is_namespaced_and_cannot_collide_with_an_arbitrary_shared_path() {
    // Even if a caller passes the very same base directory the Codex mirror
    // happens to use for its own state, this crate's cache always lives
    // under its own fixed subdirectory name, never at the base directly.
    let dir = TempDir::new().unwrap();
    let cache = UpdateCache::new(dir.path());

    let root_dir = cache.root_dir_for_test();
    assert!(root_dir.ends_with(APP_TRUST_CACHE_DIRNAME));
    assert_ne!(root_dir, dir.path());
}

#[test]
fn initialise_creates_the_cache_directory() {
    let dir = TempDir::new().unwrap();
    let cache = UpdateCache::new(dir.path());
    assert!(!cache.root_dir_for_test().exists());

    cache.initialise().unwrap();
    assert!(cache.root_dir_for_test().exists());
}
