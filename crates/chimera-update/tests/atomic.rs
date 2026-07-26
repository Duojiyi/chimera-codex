// Step 9.2 RED — crash-safe writes for settings / ownership / transaction
// state, plus schema migration and .bak recovery.
//
// The property under test is not "write and read work" — it is what survives
// a crash: the primary is only ever replaced by an atomic rename, and exactly
// one backup generation is kept so external corruption (not a crash in this
// process) still has a fallback. A corrupt primary AND a corrupt backup must
// be a typed refusal, never a panic and never a silent empty default.

use chimera_update::atomic::{AtomicError, AtomicStore, Migratable};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use tempfile::TempDir;

/// A minimal document standing in for settings/ownership/transaction state —
/// this crate's job is the store, not any one document's field list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Doc {
    name: String,
    count: u32,
    /// Added at schema version 2; a version-1 document on disk has no value
    /// for this, which is exactly what `upgrade` below has to supply.
    #[serde(default)]
    nickname: String,
}

impl Migratable for Doc {
    const CURRENT_VERSION: u32 = 2;

    fn upgrade(from_version: u32, value: Value) -> Option<Self> {
        // This test type only ever has to bridge the one step this crate
        // ships with; a real document with more history would loop here.
        if from_version != 1 {
            return None;
        }
        let mut value = value;
        let obj = value.as_object_mut()?;
        obj.entry("nickname").or_insert_with(|| json!(""));
        serde_json::from_value(value).ok()
    }
}

fn store(dir: &TempDir) -> AtomicStore<Doc> {
    AtomicStore::new(dir.path().join("settings.json"))
}

// ── Round trip ───────────────────────────────────────────────────────────

#[test]
fn a_missing_file_reads_as_none() {
    let dir = TempDir::new().unwrap();
    assert_eq!(store(&dir).read().unwrap(), None);
}

#[test]
fn write_then_read_round_trips() {
    let dir = TempDir::new().unwrap();
    let s = store(&dir);
    let doc = Doc {
        name: "alice".into(),
        count: 3,
        nickname: "al".into(),
    };
    s.write(&doc).unwrap();
    assert_eq!(s.read().unwrap(), Some(doc));
}

#[test]
fn a_leftover_tmp_file_from_an_interrupted_write_does_not_affect_a_read() {
    // A crash between "temp file written" and "renamed into place" leaves an
    // orphan `.tmp` file. It must be inert: the primary is untouched, so a
    // read must return exactly what it held before the interrupted write.
    let dir = TempDir::new().unwrap();
    let s = store(&dir);
    let doc = Doc {
        name: "bob".into(),
        count: 1,
        nickname: "".into(),
    };
    s.write(&doc).unwrap();

    fs::write(dir.path().join("settings.json.tmp"), b"not even json").unwrap();

    assert_eq!(s.read().unwrap(), Some(doc));
}

// ── .bak recovery ────────────────────────────────────────────────────────

#[test]
fn a_corrupt_primary_falls_back_to_the_backup() {
    let dir = TempDir::new().unwrap();
    let s = store(&dir);
    let v1 = Doc {
        name: "v1".into(),
        count: 1,
        nickname: "one".into(),
    };
    let v2 = Doc {
        name: "v2".into(),
        count: 2,
        nickname: "two".into(),
    };
    s.write(&v1).unwrap(); // no .bak yet — nothing to preserve
    s.write(&v2).unwrap(); // .bak now holds v1

    fs::write(dir.path().join("settings.json"), b"{ this is not json").unwrap();

    assert_eq!(
        s.read().unwrap(),
        Some(v1),
        "a corrupt primary must recover from the single backup generation"
    );
}

#[test]
fn exactly_one_backup_generation_is_kept_across_many_writes() {
    let dir = TempDir::new().unwrap();
    let s = store(&dir);
    for i in 0..5u32 {
        s.write(&Doc {
            name: format!("v{i}"),
            count: i,
            nickname: String::new(),
        })
        .unwrap();
    }
    // The backup must hold the second-to-last write, not an older one and not
    // a pile of every prior generation.
    let bak_text = fs::read_to_string(dir.path().join("settings.json.bak")).unwrap();
    assert!(
        bak_text.contains("\"v3\""),
        "backup should hold the write immediately before the last one: {bak_text}"
    );
    assert!(
        !bak_text.contains("\"v2\"")
            && !bak_text.contains("\"v1\"")
            && !bak_text.contains("\"v0\""),
        "only one backup generation may exist, found older data: {bak_text}"
    );
}

#[test]
fn a_corrupt_primary_and_a_corrupt_backup_is_a_typed_error_not_a_panic() {
    let dir = TempDir::new().unwrap();
    let s = store(&dir);
    s.write(&Doc {
        name: "a".into(),
        count: 1,
        nickname: "".into(),
    })
    .unwrap();
    s.write(&Doc {
        name: "b".into(),
        count: 2,
        nickname: "".into(),
    })
    .unwrap();

    fs::write(dir.path().join("settings.json"), b"garbage").unwrap();
    fs::write(dir.path().join("settings.json.bak"), b"also garbage").unwrap();

    let err = s.read().unwrap_err();
    assert!(
        matches!(err, AtomicError::Corrupt),
        "expected a typed Corrupt error, got {err:?}"
    );
}

// ── Schema migration ────────────────────────────────────────────────────

#[test]
fn a_document_saved_at_an_older_schema_version_is_upgraded_on_read() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("settings.json");
    // Hand-written to simulate a file an older Chimera actually produced —
    // schema_version 1, no `nickname` field at all.
    let raw = json!({
        "schema_version": 1,
        "data": { "name": "carol", "count": 7 }
    });
    fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();

    let s: AtomicStore<Doc> = AtomicStore::new(&path);
    let doc = s
        .read()
        .unwrap()
        .expect("an old-schema document must still read");
    assert_eq!(doc.name, "carol");
    assert_eq!(doc.count, 7);
    assert_eq!(doc.nickname, "", "upgrade must supply the missing field");
}

#[test]
fn an_unknown_future_schema_version_is_refused_not_guessed_at() {
    // A newer Chimera may have written this. Downgrading it — dropping
    // whatever fields only the newer schema knows about — would lose data
    // silently, so this must fail loudly instead.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("settings.json");
    let raw = json!({
        "schema_version": 99,
        "data": { "name": "future", "count": 1, "nickname": "x", "extra_field_we_do_not_know": true }
    });
    fs::write(&path, serde_json::to_vec(&raw).unwrap()).unwrap();

    let s: AtomicStore<Doc> = AtomicStore::new(&path);
    let err = s.read().unwrap_err();
    match err {
        AtomicError::FutureSchema {
            found,
            max_supported,
        } => {
            assert_eq!(found, 99);
            assert_eq!(max_supported, Doc::CURRENT_VERSION);
        }
        other => panic!("expected FutureSchema, got {other:?}"),
    }
}

#[test]
fn a_current_schema_document_is_read_without_calling_upgrade() {
    // Exercises the equal-version path distinctly from the older-version path.
    let dir = TempDir::new().unwrap();
    let s = store(&dir);
    let doc = Doc {
        name: "same".into(),
        count: 42,
        nickname: "s".into(),
    };
    s.write(&doc).unwrap();
    assert_eq!(s.read().unwrap(), Some(doc));
}

// ── The atomicity the store is named for ────────────────────────────────────
//
// An adversarial review pointed out that replacing the whole tmp-file + fsync +
// rename sequence with a plain `File::create(primary)` left all nine tests
// green. The one property this module exists for had no coverage at all: every
// test checked what `read` returns, and none checked that a *failed* write
// leaves the previous content intact.
//
// The observable difference is what these pin. A direct write truncates the
// primary before it can fail; a staged write cannot touch the primary until
// the staged copy is complete.

/// Make the staging path unusable by putting a directory where the temp file
/// needs to be. `File::create` on a directory fails on every platform.
fn block_the_staging_path(store_path: &std::path::Path) {
    fs::create_dir_all(store_path.with_extension("json.tmp")).unwrap();
}

#[test]
fn a_write_that_cannot_stage_leaves_the_previous_content_intact() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("doc.json");
    let store: AtomicStore<Doc> = AtomicStore::new(&path);

    let first = Doc {
        name: "first".into(),
        count: 1,
        nickname: "one".into(),
    };
    store.write(&first).unwrap();

    block_the_staging_path(&path);

    let second = Doc {
        name: "second".into(),
        count: 2,
        nickname: "two".into(),
    };
    let result = store.write(&second);

    assert!(
        result.is_err(),
        "a write that cannot stage must be reported, not silently skipped"
    );
    assert_eq!(
        store.read().unwrap(),
        Some(first),
        "the previous document was destroyed by a write that never completed"
    );
}

#[test]
fn a_write_that_cannot_stage_does_not_create_the_document_at_all() {
    // The same property on a fresh store: a failed first write must leave
    // nothing behind, not an empty or half-written file that `read` would
    // then report as corrupt for the rest of the install's life.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("doc.json");
    let store: AtomicStore<Doc> = AtomicStore::new(&path);

    block_the_staging_path(&path);
    let result = store.write(&Doc {
        name: "x".into(),
        count: 1,
        nickname: "y".into(),
    });

    assert!(result.is_err());
    assert!(
        !path.exists(),
        "a failed first write created the primary anyway"
    );
    assert_eq!(store.read().unwrap(), None);
}

#[test]
fn a_successful_write_leaves_no_staging_file_behind() {
    // A leftover .tmp is how a later read picks up a document nobody
    // committed, and it is the cheapest signal that the rename actually ran.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("doc.json");
    let store: AtomicStore<Doc> = AtomicStore::new(&path);

    store
        .write(&Doc {
            name: "a".into(),
            count: 1,
            nickname: "b".into(),
        })
        .unwrap();

    assert!(
        !path.with_extension("json.tmp").exists(),
        "the staging file survived a successful write"
    );
}
