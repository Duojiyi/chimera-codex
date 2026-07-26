// Step 7.3 RED — apply a migration inventory transactionally via ports.
//
// Every port here is an in-memory double: no test in this file touches a
// real keychain, a real provider database, or a real config file, which is
// exactly what makes migrate.rs "fully testable with doubles" (Step 7.3
// design note). The two tests that *do* touch real files
// (`read_only_source_*`) exist specifically to prove the source-file
// read-only guarantee end to end, using the already-shipped
// legacy_source/ccswitch_source readers.

use chimera_domain::Provider;
use chimera_migration::ccswitch_source::{CcSwitchSourcePaths, read_ccswitch_inventory};
use chimera_migration::legacy_source::{LegacyProtocol, LegacySourcePaths, read_legacy_inventory};
use chimera_migration::migrate::{
    MigrationCandidate, MigrationError, SourceKind, apply_migration,
    restore_pre_migration_configuration,
};
use chimera_migration::ports::{
    ConfigSnapshot, ConfigSnapshotPort, HealthCheckPort, KeychainReference, KeychainSink,
    PortError, ProviderSink,
};
use serde_json::json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;
use uuid::Uuid;

// ── in-memory port doubles ──────────────────────────────────────────────────

#[derive(Default)]
struct FakeKeychain {
    store: RefCell<HashMap<String, String>>,
    store_calls: RefCell<u32>,
    remove_calls: RefCell<u32>,
    fail_service: RefCell<Option<String>>,
}

impl FakeKeychain {
    fn failing_on(service_name: &str) -> Self {
        let f = Self::default();
        *f.fail_service.borrow_mut() = Some(service_name.to_string());
        f
    }
}

impl KeychainSink for FakeKeychain {
    fn store(&self, service_name: &str, secret: &str) -> Result<KeychainReference, PortError> {
        *self.store_calls.borrow_mut() += 1;
        if self.fail_service.borrow().as_deref() == Some(service_name) {
            return Err(PortError::KeychainWrite(
                "simulated keychain failure".into(),
            ));
        }
        let reference = KeychainReference::new(format!("keychain://test/{service_name}"));
        self.store
            .borrow_mut()
            .insert(reference.as_str().to_string(), secret.to_string());
        Ok(reference)
    }

    fn remove(&self, reference: &KeychainReference) -> Result<(), PortError> {
        *self.remove_calls.borrow_mut() += 1;
        self.store.borrow_mut().remove(reference.as_str());
        Ok(())
    }
}

#[derive(Default)]
struct FakeProviderSink {
    rows: RefCell<HashMap<Uuid, Provider>>,
    insert_calls: RefCell<u32>,
    remove_calls: RefCell<u32>,
    fail_display_name: RefCell<Option<String>>,
}

impl FakeProviderSink {
    fn failing_on_display_name(name: &str) -> Self {
        let f = Self::default();
        *f.fail_display_name.borrow_mut() = Some(name.to_string());
        f
    }
}

impl ProviderSink for FakeProviderSink {
    fn contains(&self, id: Uuid) -> Result<bool, PortError> {
        Ok(self.rows.borrow().contains_key(&id))
    }

    fn insert(&self, provider: Provider) -> Result<(), PortError> {
        *self.insert_calls.borrow_mut() += 1;
        if self.fail_display_name.borrow().as_deref() == Some(provider.display_name.as_str()) {
            return Err(PortError::ProviderInsert("simulated insert failure".into()));
        }
        self.rows.borrow_mut().insert(provider.id, provider);
        Ok(())
    }

    fn remove(&self, id: Uuid) -> Result<(), PortError> {
        *self.remove_calls.borrow_mut() += 1;
        self.rows.borrow_mut().remove(&id);
        Ok(())
    }
}

struct FakeConfigStore {
    bytes: RefCell<Vec<u8>>,
    restore_calls: RefCell<u32>,
    fail_restore: bool,
}

impl FakeConfigStore {
    fn new(initial: &[u8]) -> Self {
        Self {
            bytes: RefCell::new(initial.to_vec()),
            restore_calls: RefCell::new(0),
            fail_restore: false,
        }
    }
}

impl ConfigSnapshotPort for FakeConfigStore {
    fn snapshot(&self) -> Result<ConfigSnapshot, PortError> {
        Ok(ConfigSnapshot::new(self.bytes.borrow().clone()))
    }

    fn restore(&self, snapshot: &ConfigSnapshot) -> Result<(), PortError> {
        *self.restore_calls.borrow_mut() += 1;
        if self.fail_restore {
            return Err(PortError::ConfigRestore("simulated restore failure".into()));
        }
        *self.bytes.borrow_mut() = snapshot.as_bytes().to_vec();
        Ok(())
    }
}

#[derive(Default)]
struct FakeHealthCheck {
    check_calls: RefCell<u32>,
    fail: bool,
}

impl HealthCheckPort for FakeHealthCheck {
    fn check(&self, _provider_ids: &[Uuid]) -> Result<(), PortError> {
        *self.check_calls.borrow_mut() += 1;
        if self.fail {
            return Err(PortError::HealthCheck(
                "simulated health-check failure".into(),
            ));
        }
        Ok(())
    }
}

fn candidate(
    source_id: &str,
    display_name: &str,
    base_url: &str,
    key: Option<&str>,
) -> MigrationCandidate {
    MigrationCandidate::new(
        SourceKind::Legacy,
        source_id,
        display_name,
        base_url,
        LegacyProtocol::Responses,
        false,
        key,
    )
}

// ── golden path ──────────────────────────────────────────────────────────────

#[test]
fn a_clean_migration_stores_the_secret_inserts_the_provider_and_health_checks_it() {
    let candidates = vec![candidate(
        "one",
        "One",
        "https://one.example/v1",
        Some("sk-one"),
    )];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck::default();

    let outcome = apply_migration(&candidates, &keychain, &providers, &config, &health)
        .expect("a clean migration must succeed");

    assert_eq!(outcome.migrated.len(), 1);
    assert_eq!(outcome.migrated[0].source_id, "one");
    assert!(outcome.already_migrated.is_empty());
    assert!(outcome.skipped.is_empty());
    assert_eq!(*keychain.store_calls.borrow(), 1);
    assert_eq!(*providers.insert_calls.borrow(), 1);
    assert_eq!(*health.check_calls.borrow(), 1);
    // Nothing was ever rolled back on a clean run.
    assert_eq!(*providers.remove_calls.borrow(), 0);
    assert_eq!(*keychain.remove_calls.borrow(), 0);
    assert_eq!(*config.restore_calls.borrow(), 0);
}

#[test]
fn a_candidate_with_no_key_is_migrated_without_touching_the_keychain() {
    let candidates = vec![candidate(
        "nokey",
        "No Key",
        "https://nokey.example/v1",
        None,
    )];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck::default();

    let outcome = apply_migration(&candidates, &keychain, &providers, &config, &health).unwrap();

    assert_eq!(outcome.migrated.len(), 1);
    assert_eq!(*keychain.store_calls.borrow(), 0);
}

// ── per-candidate skips (do not abort the transaction) ──────────────────────

#[test]
fn a_chat_completions_candidate_is_skipped_not_migrated_and_does_not_block_others() {
    let candidates = vec![
        MigrationCandidate::new(
            SourceKind::Legacy,
            "chat",
            "Chat",
            "https://chat.example/v1",
            LegacyProtocol::ChatCompletions,
            false,
            Some("sk-chat"),
        ),
        candidate("ok", "OK", "https://ok.example/v1", Some("sk-ok")),
    ];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck::default();

    let outcome = apply_migration(&candidates, &keychain, &providers, &config, &health).unwrap();

    assert_eq!(outcome.migrated.len(), 1);
    assert_eq!(outcome.migrated[0].source_id, "ok");
    assert_eq!(outcome.skipped.len(), 1);
    assert_eq!(outcome.skipped[0].source_id, "chat");
    assert!(
        outcome.skipped[0]
            .reason
            .to_lowercase()
            .contains("chat completions")
    );
}

#[test]
fn a_candidate_with_an_invalid_base_url_is_skipped_not_migrated() {
    let candidates = vec![candidate("bad-url", "Bad URL", "not a url", Some("sk"))];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck::default();

    let outcome = apply_migration(&candidates, &keychain, &providers, &config, &health).unwrap();

    assert!(outcome.migrated.is_empty());
    assert_eq!(outcome.skipped.len(), 1);
    assert_eq!(outcome.skipped[0].source_id, "bad-url");
    // The rejected candidate carried a key -- prove it was never stored.
    assert_eq!(*keychain.store_calls.borrow(), 0);
}

// ── idempotency: re-running does not duplicate providers ───────────────────

#[test]
fn re_running_a_migration_that_already_ran_does_not_duplicate_providers() {
    let candidates = vec![
        candidate("one", "One", "https://one.example/v1", Some("sk-one")),
        candidate("two", "Two", "https://two.example/v1", Some("sk-two")),
    ];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck::default();

    let first = apply_migration(&candidates, &keychain, &providers, &config, &health).unwrap();
    assert_eq!(first.migrated.len(), 2);
    assert_eq!(*providers.insert_calls.borrow(), 2);

    let second = apply_migration(&candidates, &keychain, &providers, &config, &health).unwrap();

    assert!(
        second.migrated.is_empty(),
        "nothing new should be migrated on a re-run"
    );
    assert_eq!(second.already_migrated.len(), 2);
    assert!(second.already_migrated.contains(&"one".to_string()));
    assert!(second.already_migrated.contains(&"two".to_string()));
    // No new inserts, and the health check was not re-run for zero new providers.
    assert_eq!(
        *providers.insert_calls.borrow(),
        2,
        "insert must not be called again"
    );
    assert_eq!(
        *health.check_calls.borrow(),
        1,
        "second run has nothing new to health-check"
    );
    assert_eq!(providers.rows.borrow().len(), 2, "no duplicate rows");
}

// ── failure -> rollback: byte-identical config, nothing stranded ───────────

#[test]
fn snapshot_failure_aborts_before_any_write_at_all() {
    struct FailingSnapshot;
    impl ConfigSnapshotPort for FailingSnapshot {
        fn snapshot(&self) -> Result<ConfigSnapshot, PortError> {
            Err(PortError::ConfigSnapshot(
                "simulated snapshot failure".into(),
            ))
        }
        fn restore(&self, _snapshot: &ConfigSnapshot) -> Result<(), PortError> {
            panic!("restore must never be called when the initial snapshot itself failed");
        }
    }

    let candidates = vec![candidate(
        "one",
        "One",
        "https://one.example/v1",
        Some("sk-one"),
    )];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::default();
    let health = FakeHealthCheck::default();

    let result = apply_migration(
        &candidates,
        &keychain,
        &providers,
        &FailingSnapshot,
        &health,
    );

    assert!(matches!(result, Err(MigrationError::SnapshotFailed(_))));
    assert_eq!(*keychain.store_calls.borrow(), 0);
    assert_eq!(*providers.insert_calls.borrow(), 0);
}

#[test]
fn a_keychain_write_failure_leaves_live_config_byte_identical_to_before() {
    let candidates = vec![candidate(
        "boom",
        "Boom",
        "https://boom.example/v1",
        Some("sk-boom"),
    )];
    let keychain = FakeKeychain::failing_on("migration:Legacy:boom");
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-v0-untouched");
    let health = FakeHealthCheck::default();
    let original_bytes = config.bytes.borrow().clone();

    let result = apply_migration(&candidates, &keychain, &providers, &config, &health);

    assert!(matches!(
        result,
        Err(MigrationError::KeychainWriteFailed(_))
    ));
    assert_eq!(
        *config.bytes.borrow(),
        original_bytes,
        "live config must be byte-identical, not merely similar, after a failed migration"
    );
    assert_eq!(
        *config.restore_calls.borrow(),
        1,
        "rollback must call restore exactly once"
    );
    assert_eq!(*providers.insert_calls.borrow(), 0);
}

#[test]
fn a_provider_insert_failure_removes_the_secret_that_was_already_stored() {
    // Regression target for the "remove anything already stored" stop
    // condition: the secret is written to the keychain *before* the
    // provider row is inserted, so a failure at the insert step must not
    // strand that just-written secret.
    let candidates = vec![candidate(
        "strand",
        "Strand Me Not",
        "https://strand.example/v1",
        Some("sk-strand"),
    )];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::failing_on_display_name("Strand Me Not");
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck::default();

    let result = apply_migration(&candidates, &keychain, &providers, &config, &health);

    assert!(matches!(
        result,
        Err(MigrationError::ProviderInsertFailed(_))
    ));
    assert_eq!(
        *keychain.store_calls.borrow(),
        1,
        "the secret was written once"
    );
    assert_eq!(
        *keychain.remove_calls.borrow(),
        1,
        "the just-written secret must be removed by rollback, not stranded"
    );
    assert!(
        keychain.store.borrow().is_empty(),
        "the keychain must end up empty again"
    );
    assert_eq!(providers.rows.borrow().len(), 0);
}

#[test]
fn a_health_check_failure_rolls_back_every_provider_and_secret_migrated_this_run() {
    let candidates = vec![
        candidate("one", "One", "https://one.example/v1", Some("sk-one")),
        candidate("two", "Two", "https://two.example/v1", Some("sk-two")),
    ];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck {
        check_calls: RefCell::new(0),
        fail: true,
    };
    let original_bytes = config.bytes.borrow().clone();

    let result = apply_migration(&candidates, &keychain, &providers, &config, &health);

    assert!(matches!(result, Err(MigrationError::HealthCheckFailed(_))));
    assert_eq!(
        providers.rows.borrow().len(),
        0,
        "both providers must be rolled back"
    );
    assert!(
        keychain.store.borrow().is_empty(),
        "both secrets must be rolled back"
    );
    assert_eq!(*config.bytes.borrow(), original_bytes);
}

#[test]
fn a_rollback_that_cannot_restore_config_is_reported_distinctly() {
    let candidates = vec![candidate(
        "one",
        "One",
        "https://one.example/v1",
        Some("sk-one"),
    )];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::failing_on_display_name("One");
    let config = FakeConfigStore {
        bytes: RefCell::new(b"config-v0".to_vec()),
        restore_calls: RefCell::new(0),
        fail_restore: true,
    };
    let health = FakeHealthCheck::default();

    let result = apply_migration(&candidates, &keychain, &providers, &config, &health);

    match result {
        Err(MigrationError::RollbackIncomplete(msg)) => {
            assert!(
                msg.contains("could not be restored") || msg.to_lowercase().contains("restore")
            );
        }
        other => panic!("expected RollbackIncomplete, got {other:?}"),
    }
}

// ── explicit "restore pre-upgrade configuration" after success ─────────────

#[test]
fn restore_pre_migration_configuration_is_usable_as_an_explicit_action_after_success() {
    let candidates = vec![candidate(
        "one",
        "One",
        "https://one.example/v1",
        Some("sk-one"),
    )];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-before-migration");
    let health = FakeHealthCheck::default();

    let outcome = apply_migration(&candidates, &keychain, &providers, &config, &health).unwrap();
    assert_eq!(
        *config.restore_calls.borrow(),
        0,
        "success must not call restore on its own"
    );

    // Simulate more changes happening to live config after the migration.
    *config.bytes.borrow_mut() = b"config-after-more-changes".to_vec();

    restore_pre_migration_configuration(&config, &outcome.pre_migration_snapshot)
        .expect("an explicit restore after success must succeed");

    assert_eq!(*config.restore_calls.borrow(), 1);
    assert_eq!(*config.bytes.borrow(), b"config-before-migration".to_vec());
}

// ── secrets never leak ──────────────────────────────────────────────────────

#[test]
fn secrets_never_appear_in_debug_output_of_a_successful_outcome() {
    const SECRET: &str = "sk-must-never-be-printed-anywhere-1234567890";
    let candidates = vec![candidate(
        "one",
        "One",
        "https://one.example/v1",
        Some(SECRET),
    )];
    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck::default();

    let outcome = apply_migration(&candidates, &keychain, &providers, &config, &health).unwrap();

    assert!(!format!("{outcome:?}").contains(SECRET));
    assert!(!format!("{candidates:?}").contains(SECRET));
}

#[test]
fn secrets_never_appear_in_a_migration_error_after_a_failure() {
    const SECRET: &str = "sk-must-never-leak-through-an-error-message";
    let candidates = vec![candidate(
        "boom",
        "Boom",
        "https://boom.example/v1",
        Some(SECRET),
    )];
    let keychain = FakeKeychain::failing_on("migration:Legacy:boom");
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck::default();

    let result = apply_migration(&candidates, &keychain, &providers, &config, &health);
    let err = result.unwrap_err();

    assert!(!format!("{err:?}").contains(SECRET));
    assert!(!format!("{err}").contains(SECRET));
}

// ── the source is never written to (end-to-end, real files) ────────────────

#[test]
fn read_only_source_legacy_settings_file_is_never_modified_by_a_failed_migration() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let original = serde_json::to_vec_pretty(&json!({
        "activeRelayId": "one",
        "relayProfiles": [
            {"id": "one", "name": "One", "upstreamBaseUrl": "https://one.example/v1", "apiKey": "sk-one"},
        ]
    }))
    .unwrap();
    fs::write(&path, &original).unwrap();
    let original_on_disk = fs::read(&path).unwrap();

    let inventory = read_legacy_inventory(&LegacySourcePaths::new(&path)).unwrap();
    let candidates: Vec<MigrationCandidate> = inventory
        .providers
        .iter()
        .map(MigrationCandidate::from_legacy)
        .collect();

    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::failing_on_display_name("One");
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck::default();

    let result = apply_migration(&candidates, &keychain, &providers, &config, &health);
    assert!(
        result.is_err(),
        "this test exercises the failure path on purpose"
    );

    let after = fs::read(&path).unwrap();
    assert_eq!(
        after, original_on_disk,
        "the 1.x source file must never be written to"
    );
}

#[test]
fn read_only_source_ccswitch_config_is_never_modified_by_a_successful_migration() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("cc-switch-config.json");
    let original = serde_json::to_vec_pretty(&json!({
        "apps": {"codex": {"current": "one", "providers": {
            "one": {"name": "One", "settingsConfig": {"baseUrl": "https://one.example/v1", "apiKey": "sk-one"}}
        }}}
    }))
    .unwrap();
    fs::write(&path, &original).unwrap();
    let original_on_disk = fs::read(&path).unwrap();

    let inventory = read_ccswitch_inventory(&CcSwitchSourcePaths::new(&path))
        .unwrap()
        .unwrap();
    let candidates: Vec<MigrationCandidate> = inventory
        .providers
        .iter()
        .map(MigrationCandidate::from_ccswitch)
        .collect();

    let keychain = FakeKeychain::default();
    let providers = FakeProviderSink::default();
    let config = FakeConfigStore::new(b"config-v0");
    let health = FakeHealthCheck::default();

    let result = apply_migration(&candidates, &keychain, &providers, &config, &health);
    assert!(
        result.is_ok(),
        "this test exercises the success path on purpose"
    );

    let after = fs::read(&path).unwrap();
    assert_eq!(
        after, original_on_disk,
        "CC Switch's config must never be written to"
    );
}
