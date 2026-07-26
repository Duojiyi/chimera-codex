//! Step 7.3 seams — narrow port traits for every side effect `migrate`
//! performs, so migration logic is fully testable with in-memory doubles.
//!
//! LAYERING (scripts/verify-v2-architecture.mjs, G1): this crate is layer 2
//! (adapter) and must not depend on `chimera-provider`, `chimera-runtime`, or
//! `chimera-theme` — those are sibling adapters. It therefore cannot reuse
//! `chimera_provider::keychain::KeychainPort` or its provider repository
//! directly, so it defines its own minimal seams here instead. The desktop
//! shell adapts its real keychain/provider-store/config-file implementations
//! to satisfy these traits — see this crate's final report, "Integration
//! needed", for exactly what that wiring looks like.

use chimera_domain::Provider;
use std::fmt;
use thiserror::Error;
use uuid::Uuid;

/// A handle a keychain implementation hands back after storing a secret.
/// Not secret itself (it is a lookup key, e.g. `keychain://chimera/<name>`)
/// — mirrors `chimera_provider::keychain::SecretRef`'s Debug/Display
/// behaviour (this crate cannot depend on chimera-provider, so the pattern
/// is duplicated rather than shared; see `lib.rs`). Safe to keep in a
/// migration report or log line.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct KeychainReference(String);

impl KeychainReference {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The reference string itself. Never the secret it points at.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for KeychainReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KeychainReference({})", self.0)
    }
}

impl fmt::Display for KeychainReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Every error a port can return. One flat enum shared by all four ports
/// (rather than one per trait) because `migrate` handles them identically:
/// any of them aborts the transaction and triggers rollback.
///
/// The wrapped `String` is caller-supplied — produced by whatever backs the
/// port in the desktop shell — and is surfaced to the user as-is, so it MUST
/// NOT be a raw Rust error, an absolute path, a username, or key material.
/// Implementors of these traits are responsible for sanitizing before an
/// error crosses this boundary.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PortError {
    #[error("could not store the migrated secret in the system keychain: {0}")]
    KeychainWrite(String),
    #[error("could not remove a migrated secret from the system keychain: {0}")]
    KeychainRemove(String),
    #[error("could not check for an already-migrated provider: {0}")]
    ProviderLookup(String),
    #[error("could not add the migrated provider: {0}")]
    ProviderInsert(String),
    #[error("could not remove a migrated provider: {0}")]
    ProviderRemove(String),
    #[error("could not read the current configuration before migrating: {0}")]
    ConfigSnapshot(String),
    #[error("could not restore the configuration: {0}")]
    ConfigRestore(String),
    #[error("the migrated providers failed their health check: {0}")]
    HealthCheck(String),
}

/// Stores exactly one migrated secret and hands back a reference. Narrower
/// than `chimera_provider::keychain::KeychainPort`: `retrieve` is
/// deliberately omitted because `migrate` only ever writes forward or
/// deletes what it just wrote — it never needs to read a secret back out.
pub trait KeychainSink {
    /// Store `secret` under `service_name`, returning a reference safe to
    /// keep around (never the secret itself).
    fn store(&self, service_name: &str, secret: &str) -> Result<KeychainReference, PortError>;

    /// Undo a `store` — used only to unwind a partially-applied migration.
    /// Must be idempotent: removing a reference that is already gone is
    /// success, not an error (mirrors `chimera_provider`'s `OsKeychain::
    /// delete`), so a rollback retry after a partial failure never gets
    /// stuck re-failing on work it already finished.
    fn remove(&self, reference: &KeychainReference) -> Result<(), PortError>;
}

/// Inserts/removes provider rows and answers "has this id already been
/// migrated" so re-running a migration is a no-op, not a duplicate. Takes
/// full `chimera_domain::Provider` values — a layer-0 type, not an adapter
/// type — rather than inventing a parallel shape this crate would have to
/// keep in sync by hand.
pub trait ProviderSink {
    /// True if a provider with this id already exists. `migrate` derives a
    /// deterministic id per source candidate (see
    /// `migrate::deterministic_provider_id`) specifically so this check
    /// makes re-running idempotent without the sink needing to know
    /// anything about 1.x or CC Switch source ids.
    fn contains(&self, id: Uuid) -> Result<bool, PortError>;

    /// Insert one provider row.
    fn insert(&self, provider: Provider) -> Result<(), PortError>;

    /// Remove a provider row by id — used only to unwind a partially-applied
    /// migration. Must be idempotent (see `KeychainSink::remove`).
    fn remove(&self, id: Uuid) -> Result<(), PortError>;
}

/// Byte-exact, opaque capture of the live configuration. `migrate` never
/// inspects the bytes — it only ever compares two snapshots for equality (to
/// prove a failed migration left config untouched) or hands one back to
/// `restore`. Keeping it opaque means this crate never needs to know the
/// desktop shell's on-disk config shape — and, just as importantly, never
/// handles it richly enough to accidentally fold a raw secret into it:
/// secrets live only behind `KeychainSink`, referenced by a
/// [`KeychainReference`] string, never embedded in a snapshot.
#[derive(Clone, PartialEq, Eq)]
pub struct ConfigSnapshot(Vec<u8>);

impl ConfigSnapshot {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for ConfigSnapshot {
    // Never print the captured bytes: whatever the desktop shell's config
    // format turns out to be, a byte dump in a log or report is exactly the
    // kind of accidental leak this crate's redaction rules exist to
    // prevent, and the bytes are meaningless for debugging without the
    // shell's own schema anyway.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConfigSnapshot({} bytes)", self.0.len())
    }
}

/// Captures and restores the entire live configuration around a migration
/// attempt, so a failure can put things back byte-for-byte instead of
/// "close enough".
pub trait ConfigSnapshotPort {
    /// Capture the current live configuration right now.
    fn snapshot(&self) -> Result<ConfigSnapshot, PortError>;

    /// Restore a previously captured configuration. Called both by
    /// `migrate`'s automatic rollback on failure and by
    /// `restore_pre_migration_configuration` as an explicit, later,
    /// user-invoked undo of a *successful* migration.
    fn restore(&self, snapshot: &ConfigSnapshot) -> Result<(), PortError>;
}

/// Confirms the newly migrated providers are actually usable before the
/// migration is considered committed. A side effect (a real implementation
/// makes a network call), so it gets its own narrow port rather than being
/// folded into `ProviderSink`.
pub trait HealthCheckPort {
    /// Check the given migrated provider ids. Any failure aborts the whole
    /// migration and triggers rollback — `migrate` does not need
    /// per-provider granularity to make that decision.
    fn check(&self, provider_ids: &[Uuid]) -> Result<(), PortError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_snapshot_debug_never_prints_the_captured_bytes() {
        const SENTINEL: &str = "sk-should-not-appear-in-debug";
        let snapshot = ConfigSnapshot::new(SENTINEL.as_bytes().to_vec());

        let rendered = format!("{snapshot:?}");

        assert!(!rendered.contains(SENTINEL));
        assert_eq!(snapshot.as_bytes(), SENTINEL.as_bytes());
    }

    #[test]
    fn keychain_reference_round_trips_its_value() {
        let r = KeychainReference::new("keychain://chimera/example");

        assert_eq!(r.as_str(), "keychain://chimera/example");
        assert_eq!(format!("{r}"), "keychain://chimera/example");
        assert_eq!(
            format!("{r:?}"),
            "KeychainReference(keychain://chimera/example)"
        );
    }
}
