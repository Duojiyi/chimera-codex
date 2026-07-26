// Step 7.3 regression — a migrated provider must satisfy the same URL policy
// as one added by hand.
//
// Found by an adversarial review. `apply_migration` validated a candidate's
// base URL with `Url::parse` alone, which accepts `http://`, `ftp://` and a
// URL carrying `user:pass@`. So a 1.x or CC Switch entry pointing at a
// plaintext endpoint would have its API key written into the OS credential
// store and a provider row created for it — while the same URL typed into the
// Add Provider form is refused by `chimera_provider::probe`.
//
// Migration is an import path, not an exemption. A rule that only applies to
// values the user typed is not a rule.

use chimera_migration::legacy_source::LegacyProtocol;
use chimera_migration::migrate::SourceKind;
use chimera_migration::migrate::{MigrationCandidate, apply_migration};
use chimera_migration::ports::{
    ConfigSnapshot, ConfigSnapshotPort, HealthCheckPort, KeychainReference, KeychainSink,
    PortError, ProviderSink,
};
use std::sync::Mutex;
use uuid::Uuid;

/// Assembled at runtime so verify-no-secrets does not read the fixture as a leak.
const SECRET: &str = concat!("sk", "-", "migratedFIXTUREvalue0123456789");

#[derive(Default)]
struct RecordingKeychain {
    stored: Mutex<Vec<String>>,
}
impl KeychainSink for RecordingKeychain {
    fn store(&self, _label: &str, secret: &str) -> Result<KeychainReference, PortError> {
        self.stored.lock().unwrap().push(secret.to_string());
        Ok(KeychainReference::new("ref"))
    }
    fn remove(&self, _reference: &KeychainReference) -> Result<(), PortError> {
        Ok(())
    }
}

#[derive(Default)]
struct RecordingProviders {
    inserted: Mutex<Vec<Uuid>>,
}
impl ProviderSink for RecordingProviders {
    fn contains(&self, _id: Uuid) -> Result<bool, PortError> {
        Ok(false)
    }
    fn insert(&self, provider: chimera_domain::Provider) -> Result<(), PortError> {
        self.inserted.lock().unwrap().push(provider.id);
        Ok(())
    }
    fn remove(&self, _id: Uuid) -> Result<(), PortError> {
        Ok(())
    }
}

struct NoopConfig;
impl ConfigSnapshotPort for NoopConfig {
    fn snapshot(&self) -> Result<ConfigSnapshot, PortError> {
        Ok(ConfigSnapshot::new(b"before".to_vec()))
    }
    fn restore(&self, _snapshot: &ConfigSnapshot) -> Result<(), PortError> {
        Ok(())
    }
}

struct HealthyCheck;
impl HealthCheckPort for HealthyCheck {
    fn check(&self, _provider_ids: &[Uuid]) -> Result<(), PortError> {
        Ok(())
    }
}

fn candidate(source_id: &str, base_url: &str) -> MigrationCandidate {
    MigrationCandidate::new(
        SourceKind::Legacy,
        source_id.to_string(),
        format!("provider {source_id}"),
        base_url.to_string(),
        LegacyProtocol::Responses,
        true,
        Some(SECRET),
    )
}

fn run(c: MigrationCandidate) -> (RecordingKeychain, RecordingProviders, usize) {
    let keychain = RecordingKeychain::default();
    let providers = RecordingProviders::default();
    let outcome = apply_migration(&[c], &keychain, &providers, &NoopConfig, &HealthyCheck)
        .expect("an unsupported URL is skipped, never a hard failure");
    let skipped = outcome.skipped.len();
    (keychain, providers, skipped)
}

#[test]
fn a_plaintext_http_candidate_is_skipped_and_its_secret_is_never_stored() {
    let (keychain, providers, skipped) = run(candidate("legacy-1", "http://api.example.com/v1"));

    assert_eq!(skipped, 1, "an http:// candidate must be skipped");
    assert!(
        keychain.stored.lock().unwrap().is_empty(),
        "the secret of a rejected candidate reached the credential store"
    );
    assert!(
        providers.inserted.lock().unwrap().is_empty(),
        "a rejected candidate was inserted anyway"
    );
}

#[test]
fn a_non_http_scheme_is_skipped() {
    for url in ["ftp://api.example.com/v1", "file:///etc/passwd"] {
        let (keychain, _, skipped) = run(candidate("legacy-2", url));
        assert_eq!(skipped, 1, "{url} must be skipped");
        assert!(
            keychain.stored.lock().unwrap().is_empty(),
            "{url} stored a secret"
        );
    }
}

#[test]
fn a_url_carrying_credentials_is_skipped() {
    // Migrating this would silently keep a password in the projected config.
    let (keychain, _, skipped) = run(candidate("legacy-3", "https://user:pw@api.example.com/v1"));
    assert_eq!(skipped, 1, "a userinfo URL must be skipped");
    assert!(keychain.stored.lock().unwrap().is_empty());
}

#[test]
fn an_ordinary_https_candidate_still_migrates() {
    // The policy must not block the case migration exists for.
    let (keychain, providers, skipped) = run(candidate("legacy-4", "https://api.example.com/v1"));
    assert_eq!(skipped, 0, "a plain https candidate must migrate");
    assert_eq!(keychain.stored.lock().unwrap().len(), 1);
    assert_eq!(providers.inserted.lock().unwrap().len(), 1);
}

#[test]
fn a_skip_reason_never_contains_the_secret_or_the_raw_url() {
    let keychain = RecordingKeychain::default();
    let providers = RecordingProviders::default();
    let outcome = apply_migration(
        &[candidate(
            "legacy-5",
            "http://internal.example/secret-path/v1",
        )],
        &keychain,
        &providers,
        &NoopConfig,
        &HealthyCheck,
    )
    .unwrap();

    let reason = &outcome.skipped[0].reason;
    assert!(
        !reason.contains("sk-"),
        "secret leaked into a skip reason: {reason}"
    );
    assert!(
        !reason.contains("secret-path"),
        "the raw URL leaked into a skip reason: {reason}"
    );
    assert!(
        reason.len() > 10,
        "a skip reason must actually explain itself"
    );
}
