//! Step 7.3 — apply a read-only inventory (from `legacy_source` or
//! `ccswitch_source`) to the live install, transactionally.
//!
//! Order of operations, matching this task's design exactly:
//!   1. snapshot the current live config FIRST (before any write happens)
//!   2. store each candidate's secret via [`ports::KeychainSink`]
//!   3. insert its provider row via [`ports::ProviderSink`]
//!   4. health-check the newly migrated providers
//!   5. on ANY failure: remove everything this run stored/inserted and
//!      restore the pre-migration snapshot, so live config ends up
//!      byte-identical to before — never merely "close enough".
//!
//! Two properties make the transaction safe to reason about:
//!   - `MigrationCandidate` carries no source path or handle of any kind, so
//!     this module has no way to open `settings.json` or CC Switch's config
//!     for writing even by accident — the "source is read-only" guarantee
//!     holds structurally, not just by convention.
//!   - Every provider id is derived deterministically from
//!     `(source_kind, source_id)` (see `deterministic_provider_id`), so
//!     re-running a migration against a `ProviderSink` that already has
//!     those rows is a plain, cheap no-op instead of a duplicate insert.

use crate::ccswitch_source::CcSwitchProviderCandidate;
use crate::legacy_source::{LegacyProtocol, LegacyProviderCandidate};
use crate::ports::{
    ConfigSnapshot, ConfigSnapshotPort, HealthCheckPort, KeychainReference, KeychainSink,
    ProviderSink,
};
use crate::secret::RedactedSecret;
use chimera_domain::{Provider, ProviderHealth, ProviderKind, ProviderProtocol};
use std::sync::LazyLock;
use thiserror::Error;
use url::Url;
use uuid::Uuid;

/// Which migration source a candidate came from. Folded into the
/// deterministic provider id so a 1.x profile and a CC Switch provider that
/// happen to share a source id never collide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKind {
    Legacy,
    CcSwitch,
}

/// One provider candidate to migrate, reduced to a source-agnostic shape so
/// `apply_migration` has a single transactional code path for both 1.x and
/// CC Switch providers instead of duplicating it per source.
#[derive(Debug, Clone)]
pub struct MigrationCandidate {
    pub source_kind: SourceKind,
    pub source_id: String,
    pub display_name: String,
    pub base_url: String,
    pub protocol: LegacyProtocol,
    pub is_selected: bool,
    /// Private even from this crate's own callers: the only way in is
    /// `new`/`from_legacy`/`from_ccswitch`, which all route the raw value
    /// through `RedactedSecret` immediately so there is never a moment where
    /// a plain `String` holding a real key exists as a struct field.
    key: Option<RedactedSecret>,
}

impl MigrationCandidate {
    /// Construct directly — e.g. from a test double, or a future migration
    /// source that doesn't fit `LegacyProviderCandidate`/
    /// `CcSwitchProviderCandidate`. `key` is wrapped in `RedactedSecret`
    /// immediately so it is never held as a plain `String`.
    pub fn new(
        source_kind: SourceKind,
        source_id: impl Into<String>,
        display_name: impl Into<String>,
        base_url: impl Into<String>,
        protocol: LegacyProtocol,
        is_selected: bool,
        key: Option<&str>,
    ) -> Self {
        Self {
            source_kind,
            source_id: source_id.into(),
            display_name: display_name.into(),
            base_url: base_url.into(),
            protocol,
            is_selected,
            key: key.map(RedactedSecret::new),
        }
    }

    /// Build from a 1.x relay profile already read by `legacy_source`.
    pub fn from_legacy(candidate: &LegacyProviderCandidate) -> Self {
        Self {
            source_kind: SourceKind::Legacy,
            source_id: candidate.source_id.clone(),
            display_name: candidate.display_name.clone(),
            base_url: candidate.base_url.clone(),
            protocol: candidate.protocol,
            is_selected: candidate.is_active,
            key: candidate.reveal_key().map(RedactedSecret::new),
        }
    }

    /// Build from a CC Switch provider already read by `ccswitch_source`.
    pub fn from_ccswitch(candidate: &CcSwitchProviderCandidate) -> Self {
        Self {
            source_kind: SourceKind::CcSwitch,
            source_id: candidate.source_id.clone(),
            display_name: candidate.display_name.clone(),
            base_url: candidate.base_url.clone(),
            protocol: candidate.protocol,
            is_selected: candidate.is_current,
            key: candidate.reveal_key().map(RedactedSecret::new),
        }
    }
}

/// Namespace UUID for provider ids derived from a migration source.
/// `Uuid::new_v5` needs *some* stable namespace; deriving it from a literal
/// tag (rather than hard-coding an opaque UUID constant that would need a
/// comment explaining where it came from) keeps the derivation legible.
static MIGRATION_NAMESPACE: LazyLock<Uuid> =
    LazyLock::new(|| Uuid::new_v5(&Uuid::NAMESPACE_URL, b"chimera-migration.source-provider"));

/// Deterministic provider id for one migration source candidate. The same
/// `(source_kind, source_id)` always yields the same id — see the
/// module doc comment for why that is what makes re-running idempotent.
fn deterministic_provider_id(source_kind: SourceKind, source_id: &str) -> Uuid {
    let name = format!("{source_kind:?}:{source_id}");
    Uuid::new_v5(&MIGRATION_NAMESPACE, name.as_bytes())
}

/// Errors that abort the whole migration transaction. Every variant's
/// wrapped `String` comes from a `ports::PortError`'s `Display` (already
/// caller-sanitized — see that type's docs) or from this module's own fixed,
/// non-secret text; none of them ever interpolates a candidate's key.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MigrationError {
    #[error("could not read the current configuration before migrating: {0}")]
    SnapshotFailed(String),
    #[error("could not check for an already-migrated provider: {0}")]
    ProviderLookupFailed(String),
    #[error("could not store a migrated secret in the system keychain: {0}")]
    KeychainWriteFailed(String),
    #[error("could not add a migrated provider: {0}")]
    ProviderInsertFailed(String),
    #[error("the migrated providers failed their health check: {0}")]
    HealthCheckFailed(String),
    #[error(
        "migration failed and some changes could not be fully undone — restart the app before \
         trying again: {0}"
    )]
    RollbackIncomplete(String),
}

/// One provider actually inserted by this run (as opposed to one recognised
/// as already migrated, or skipped as unsupported).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigratedProvider {
    pub source_kind: SourceKind,
    pub source_id: String,
    pub provider_id: Uuid,
}

/// One candidate this run declined to migrate, with an actionable, non-fatal
/// reason. Never a raw Rust error, path, or key value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedCandidate {
    pub source_id: String,
    pub reason: String,
}

/// Result of a run that completed — fully, or with some candidates skipped
/// or recognised as already migrated — without needing to roll back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationOutcome {
    pub migrated: Vec<MigratedProvider>,
    /// Source ids `ProviderSink::contains` already had a row for. Not an
    /// error — a normal outcome of re-running a migration that already ran.
    pub already_migrated: Vec<String>,
    pub skipped: Vec<SkippedCandidate>,
    /// Captured immediately before this run touched anything. Kept on the
    /// success outcome (not just used internally on failure) so
    /// `restore_pre_migration_configuration` can be offered as an explicit,
    /// later action — e.g. a "restore pre-upgrade configuration" menu item —
    /// long after this call returns.
    pub pre_migration_snapshot: ConfigSnapshot,
}

/// Apply `candidates` to the live install through the given ports,
/// transactionally. See the module doc comment for the exact order of
/// operations and the rollback guarantee.
pub fn apply_migration(
    candidates: &[MigrationCandidate],
    keychain: &dyn KeychainSink,
    providers: &dyn ProviderSink,
    config: &dyn ConfigSnapshotPort,
    health: &dyn HealthCheckPort,
) -> Result<MigrationOutcome, MigrationError> {
    // Snapshot FIRST: every later failure path depends on this already
    // existing so it can restore byte-identical config. Nothing above this
    // line can possibly have written anything.
    let pre_migration_snapshot = config
        .snapshot()
        .map_err(|e| MigrationError::SnapshotFailed(e.to_string()))?;

    let mut migrated = Vec::new();
    let mut already_migrated = Vec::new();
    let mut skipped = Vec::new();
    // Everything actually written this run, in write order, so a rollback
    // can unwind it in reverse. `Option<KeychainReference>` because a
    // keyless candidate has nothing to remove from the keychain.
    let mut applied: Vec<(Uuid, Option<KeychainReference>)> = Vec::new();

    for candidate in candidates {
        // v2's provider engine only ever commits to the Responses API (see
        // `chimera_provider::probe` doc comment) — a Chat Completions
        // candidate is skipped, never silently imported as if it spoke
        // Responses. Skipping one candidate is not a transaction failure.
        if candidate.protocol == LegacyProtocol::ChatCompletions {
            skipped.push(SkippedCandidate {
                source_id: candidate.source_id.clone(),
                reason: "uses the Chat Completions protocol, which v2 does not support \
                         (only the OpenAI Responses API)"
                    .to_string(),
            });
            continue;
        }

        // `Url::parse` alone is not a policy. It accepts `http://`, `ftp://`
        // and a URL carrying `user:pass@` — so an imported 1.x or CC Switch
        // entry pointing at a plaintext endpoint would have had its API key
        // written to the credential store and a provider row created, while
        // the identical URL typed into the Add Provider form is refused.
        //
        // Migration is an import path, not an exemption. A rule that only
        // applies to values the user typed is not a rule. Mirrors
        // `chimera_provider::probe::validate_provider_url` — deliberately
        // re-stated rather than imported, since adapter crates may not depend
        // on each other (G1, enforced by verify-v2-architecture).
        let base_url = match Url::parse(&candidate.base_url) {
            Ok(url) if url.scheme() != "https" => {
                skipped.push(SkippedCandidate {
                    source_id: candidate.source_id.clone(),
                    reason: "used a non-HTTPS endpoint and was not migrated. Re-add it manually \
                             if the endpoint supports HTTPS."
                        .to_string(),
                });
                continue;
            }
            Ok(url) if !url.username().is_empty() || url.password().is_some() => {
                // Migrating this would carry a password into the projected
                // config, where nothing else in v2 ever puts one.
                skipped.push(SkippedCandidate {
                    source_id: candidate.source_id.clone(),
                    reason: "embedded a username or password in its URL, which v2 does not \
                             support. Re-add it manually using the API key field."
                        .to_string(),
                });
                continue;
            }
            Ok(url) if url.fragment().is_some() => {
                skipped.push(SkippedCandidate {
                    source_id: candidate.source_id.clone(),
                    reason: "had a URL fragment, which is not valid for an API endpoint"
                        .to_string(),
                });
                continue;
            }
            Ok(url) => url,
            Err(_) => {
                skipped.push(SkippedCandidate {
                    source_id: candidate.source_id.clone(),
                    reason: "had an invalid base URL and was not migrated".to_string(),
                });
                continue;
            }
        };

        let provider_id = deterministic_provider_id(candidate.source_kind, &candidate.source_id);

        match providers.contains(provider_id) {
            Ok(true) => {
                already_migrated.push(candidate.source_id.clone());
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                let err = MigrationError::ProviderLookupFailed(e.to_string());
                return Err(roll_back_and_report(
                    err,
                    &applied,
                    providers,
                    keychain,
                    config,
                    &pre_migration_snapshot,
                ));
            }
        }

        let secret_ref = match &candidate.key {
            Some(secret) => {
                let service_name = format!(
                    "migration:{:?}:{}",
                    candidate.source_kind, candidate.source_id
                );
                match keychain.store(&service_name, secret.reveal()) {
                    Ok(r) => Some(r),
                    Err(e) => {
                        let err = MigrationError::KeychainWriteFailed(e.to_string());
                        return Err(roll_back_and_report(
                            err,
                            &applied,
                            providers,
                            keychain,
                            config,
                            &pre_migration_snapshot,
                        ));
                    }
                }
            }
            None => None,
        };

        let provider = Provider {
            id: provider_id,
            display_name: candidate.display_name.clone(),
            kind: ProviderKind::Custom,
            base_url,
            protocol: ProviderProtocol::Responses,
            secret_ref: secret_ref.as_ref().map(|r| r.as_str().to_string()),
            selected_model: None,
            discovered_models: Vec::new(),
            health: ProviderHealth::Unknown,
        };

        if let Err(e) = providers.insert(provider) {
            // The secret (if any) was already stored above — record it in
            // `applied` *before* returning so rollback removes it too,
            // instead of stranding an orphaned keychain entry.
            applied.push((provider_id, secret_ref));
            let err = MigrationError::ProviderInsertFailed(e.to_string());
            return Err(roll_back_and_report(
                err,
                &applied,
                providers,
                keychain,
                config,
                &pre_migration_snapshot,
            ));
        }

        applied.push((provider_id, secret_ref));
        migrated.push(MigratedProvider {
            source_kind: candidate.source_kind,
            source_id: candidate.source_id.clone(),
            provider_id,
        });
    }

    // Only check what this run actually inserted — already-migrated rows
    // were health-checked by whichever run first inserted them, and calling
    // out to the port for zero providers would be a pointless side effect.
    if !migrated.is_empty() {
        let migrated_ids: Vec<Uuid> = migrated.iter().map(|m| m.provider_id).collect();
        if let Err(e) = health.check(&migrated_ids) {
            let err = MigrationError::HealthCheckFailed(e.to_string());
            return Err(roll_back_and_report(
                err,
                &applied,
                providers,
                keychain,
                config,
                &pre_migration_snapshot,
            ));
        }
    }

    Ok(MigrationOutcome {
        migrated,
        already_migrated,
        skipped,
        pre_migration_snapshot,
    })
}

/// Explicit, user-invoked undo of a previously *successful* migration:
/// restores the exact configuration captured before that migration ran.
///
/// Deliberately a separate entry point from the rollback performed inside
/// `apply_migration` (rather than being the only way to reach
/// `ConfigSnapshotPort::restore`), so a caller can invoke it at any later
/// time — e.g. a "restore pre-upgrade configuration" menu item — without a
/// failed `apply_migration` call being required to reach it.
pub fn restore_pre_migration_configuration(
    config: &dyn ConfigSnapshotPort,
    snapshot: &ConfigSnapshot,
) -> Result<(), MigrationError> {
    config
        .restore(snapshot)
        .map_err(|e| MigrationError::RollbackIncomplete(e.to_string()))
}

/// Unwind everything in `applied` (in reverse order) and restore
/// `pre_migration_snapshot`, then fold the result into whichever
/// `MigrationError` gets returned. Every failure path in `apply_migration`
/// calls this, so there is exactly one place that decides how a rollback
/// failure gets reported instead of repeating that decision at each call
/// site.
///
/// Best-effort on the removals: a single stuck removal must not stop the
/// config restore from being attempted, since restoring byte-identical live
/// config is the more load-bearing guarantee of the two.
fn roll_back_and_report(
    original: MigrationError,
    applied: &[(Uuid, Option<KeychainReference>)],
    providers: &dyn ProviderSink,
    keychain: &dyn KeychainSink,
    config: &dyn ConfigSnapshotPort,
    pre_migration_snapshot: &ConfigSnapshot,
) -> MigrationError {
    let mut residual: Vec<String> = Vec::new();

    for (provider_id, secret_ref) in applied.iter().rev() {
        if let Err(e) = providers.remove(*provider_id) {
            residual.push(format!(
                "a partially-migrated provider could not be removed: {e}"
            ));
        }
        if let Some(reference) = secret_ref {
            if let Err(e) = keychain.remove(reference) {
                residual.push(format!(
                    "a partially-migrated secret could not be removed: {e}"
                ));
            }
        }
    }

    if let Err(e) = config.restore(pre_migration_snapshot) {
        residual.push(format!("the configuration could not be restored: {e}"));
    }

    if residual.is_empty() {
        original
    } else {
        MigrationError::RollbackIncomplete(format!("{original}; {}", residual.join("; ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_provider_id_is_stable_for_the_same_source() {
        let a = deterministic_provider_id(SourceKind::Legacy, "one");
        let b = deterministic_provider_id(SourceKind::Legacy, "one");
        assert_eq!(a, b);
    }

    #[test]
    fn deterministic_provider_id_differs_by_source_kind() {
        // Same source_id, different source: must not collide, or a 1.x
        // profile and a CC Switch provider with the same id would be
        // treated as "the same already-migrated provider".
        let legacy = deterministic_provider_id(SourceKind::Legacy, "one");
        let ccswitch = deterministic_provider_id(SourceKind::CcSwitch, "one");
        assert_ne!(legacy, ccswitch);
    }

    #[test]
    fn deterministic_provider_id_differs_by_source_id() {
        let one = deterministic_provider_id(SourceKind::Legacy, "one");
        let two = deterministic_provider_id(SourceKind::Legacy, "two");
        assert_ne!(one, two);
    }
}
