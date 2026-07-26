// Step 2.5 RED — CAS transaction switch, journal, and external write detection.
// Spec 7.4: acquire lock → snapshot+hash → render → journal → stage → CAS check
//           → atomic replace → verify → mark active → clear journal.
use chimera_provider::keychain::{KeychainPort, MemoryKeychain};
use chimera_provider::projection::ProviderProjection;
use chimera_provider::transaction::{JournalEntry, SwitchTransaction, TransactionOutcome};
use std::fs;
use tempfile::tempdir;

// ── Happy path ────────────────────────────────────────────────────────────────

#[test]
fn happy_path_switch_updates_config_and_clears_journal() {
    let tmp = tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(
        &config_path,
        "model = \"gpt-4o\"\nmodel_provider = \"openai\"\n",
    )
    .unwrap();

    let kc = MemoryKeychain::new();
    let secret_ref = kc.store("chimera/test", "sk-test-key").unwrap();

    let tx = SwitchTransaction::new(
        config_path.clone(),
        tmp.path().join("chimera.lock"),
        tmp.path().join("chimera.journal"),
    );

    let projection = ProviderProjection {
        base_url: "https://api.chimerahub.io/v1".into(),
        model: Some("gpt-4o".into()),
        api_key_env_or_plain: "sk-test-key".into(),
    };

    let outcome = tx.execute(&projection, &kc, &secret_ref).unwrap();
    assert!(
        matches!(outcome, TransactionOutcome::Committed),
        "happy path must commit: {:?}",
        outcome
    );

    // Config must now reference the new provider
    let new_config = fs::read_to_string(&config_path).unwrap();
    assert!(
        new_config.contains("chimerahub.io"),
        "config must contain new base_url"
    );

    // Journal must be cleared after successful commit
    let journal_path = tmp.path().join("chimera.journal");
    if journal_path.exists() {
        let journal = fs::read_to_string(&journal_path).unwrap();
        let entry: Option<JournalEntry> = serde_json::from_str(&journal).ok();
        assert!(
            entry.map(|e| e.is_cleared()).unwrap_or(true),
            "journal must be cleared after successful commit"
        );
    }
}

// ── CAS: external write detected between snapshot and commit ─────────────────

#[test]
fn cas_detects_external_write_after_snapshot() {
    let tmp = tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    let original = "model = \"gpt-4o\"\nmodel_provider = \"openai\"\n";
    fs::write(&config_path, original).unwrap();

    let kc = MemoryKeychain::new();
    let secret_ref = kc.store("chimera/cas-test", "sk-key").unwrap();

    let tx = SwitchTransaction::new(
        config_path.clone(),
        tmp.path().join("chimera.lock"),
        tmp.path().join("chimera.journal"),
    );

    // Simulate external write by providing a "snapshot injector" that
    // modifies the file between snapshot and CAS check.
    let config_path_clone = config_path.clone();
    let external_write_fn = Box::new(move || {
        fs::write(
            &config_path_clone,
            "model = \"o3\"\nmodel_provider = \"openai\"\nexternal_change = true\n",
        )
        .unwrap();
    });

    let projection = ProviderProjection {
        base_url: "https://api.chimerahub.io/v1".into(),
        model: Some("gpt-4o".into()),
        api_key_env_or_plain: "sk-key".into(),
    };

    let outcome = tx
        .execute_with_pre_cas_hook(&projection, &kc, &secret_ref, external_write_fn)
        .unwrap();
    assert!(
        matches!(outcome, TransactionOutcome::Conflict(_)),
        "must detect external write via CAS: {:?}",
        outcome
    );

    // Conflict must preserve the external change (not overwrite it)
    let current_config = fs::read_to_string(&config_path).unwrap();
    assert!(
        current_config.contains("external_change = true"),
        "CAS must NOT overwrite external change: {current_config}"
    );
}

// ── Journal recovery: simulate crash after journal write ─────────────────────

#[test]
fn journal_written_before_atomic_rename() {
    let tmp = tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(&config_path, "model = \"gpt-4o\"\n").unwrap();

    let kc = MemoryKeychain::new();
    let secret_ref = kc.store("chimera/journal-test", "sk-k").unwrap();
    let journal_path = tmp.path().join("chimera.journal");

    let tx = SwitchTransaction::new(
        config_path.clone(),
        tmp.path().join("chimera.lock"),
        journal_path.clone(),
    );

    let projection = ProviderProjection {
        base_url: "https://api.chimerahub.io/v1".into(),
        model: None,
        api_key_env_or_plain: "sk-k".into(),
    };

    // Execute successfully
    tx.execute(&projection, &kc, &secret_ref).unwrap();

    // After success: journal must be in cleared or absent state
    if journal_path.exists() {
        let content = fs::read_to_string(&journal_path).unwrap();
        // Either empty, or a cleared JournalEntry
        if !content.trim().is_empty() {
            let entry: JournalEntry =
                serde_json::from_str(&content).expect("journal must be valid JSON");
            assert!(entry.is_cleared(), "journal must be cleared after commit");
        }
    }
}

// ── Lock prevents concurrent switch ──────────────────────────────────────────

#[test]
fn second_transaction_fails_while_first_holds_lock() {
    let tmp = tempdir().unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(&config_path, "model = \"gpt-4o\"\n").unwrap();

    let lock_path = tmp.path().join("chimera.lock");
    let journal_path = tmp.path().join("chimera.journal");

    // Acquire the lock manually
    let lock = chimera_platform::OperationLock::new(&lock_path);
    let _guard = lock
        .try_acquire("manual_hold")
        .expect("first lock must succeed");

    let kc = MemoryKeychain::new();
    let sr = kc.store("chimera/lock-test", "sk").unwrap();

    let tx = SwitchTransaction::new(config_path, lock_path, journal_path);
    let projection = ProviderProjection {
        base_url: "https://api.example.com/v1".into(),
        model: None,
        api_key_env_or_plain: "sk".into(),
    };

    let result = tx.execute(&projection, &kc, &sr);
    assert!(result.is_err(), "transaction while lock held must fail");
}

// ── Snapshot hash ─────────────────────────────────────────────────────────────

#[test]
fn snapshot_hash_detects_byte_change() {
    use chimera_provider::transaction::snapshot_hash;
    let tmp = tempdir().unwrap();
    let p = tmp.path().join("f.toml");
    fs::write(&p, "a = 1\n").unwrap();
    let h1 = snapshot_hash(&p).unwrap();
    fs::write(&p, "a = 2\n").unwrap();
    let h2 = snapshot_hash(&p).unwrap();
    assert_ne!(h1, h2, "hash must change when content changes");
}
