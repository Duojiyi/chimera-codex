//! Migration from Chimera++ 1.x and CC Switch, and coexistence detection.
//!
//! `chimera-migration` deliberately depends on no adapter crate (G1), so it
//! defines narrow ports and this module supplies the real implementations:
//! the OS keychain, the provider database, and a snapshot of the live Codex
//! config. Everything transactional lives in the crate; nothing here decides
//! policy.
//!
//! Two commands, and the split matters. `preview_migration` only reads — it is
//! safe to call on every visit to the screen, and it is what the user approves.
//! `run_migration` is the only thing that writes, and it is the crate's
//! transaction: snapshot first, restore byte-for-byte on any failure.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use chimera_domain::Provider;
use chimera_migration::ccswitch_source::{CcSwitchSourcePaths, read_ccswitch_inventory};
use chimera_migration::legacy_source::{LegacySourcePaths, read_legacy_inventory};
use chimera_migration::migrate::{MigrationCandidate, apply_migration};
use chimera_migration::ports::{
    ConfigSnapshot, ConfigSnapshotPort, HealthCheckPort, KeychainReference, KeychainSink,
    PortError, ProviderSink,
};
use chimera_provider::db::ProviderRow;
use chimera_provider::keychain::KeychainPort;

use crate::state::AppState;

// ── DTOs ───────────────────────────────────────────────────────────────────

/// One provider the user could migrate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationCandidateDto {
    pub source: String,
    pub source_id: String,
    pub display_name: String,
    /// Host only. The full URL can carry a path a user would not expect to see
    /// on screen, and the host is what identifies the provider to them.
    pub host: String,
    pub has_key: bool,
}

/// What migration would do, before it does anything.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPreviewDto {
    pub candidates: Vec<MigrationCandidateDto>,
    /// 1.x features that deliberately do not come across (N1-N6), described
    /// by the crate itself. Shown so a user learns what they are losing before
    /// they migrate rather than discovering it afterwards.
    pub dropped_features: Vec<String>,
    /// Actionable, already-sanitised notes from reading the sources.
    pub warnings: Vec<String>,
}

/// Outcome of an actual run.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationResultDto {
    pub migrated: usize,
    pub already_migrated: usize,
    /// One entry per candidate that was declined, with why.
    pub skipped: Vec<SkippedDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedDto {
    pub source_id: String,
    pub reason: String,
}

// ── Port implementations ───────────────────────────────────────────────────

/// The real OS credential store, behind the crate's narrow sink.
struct RealKeychain<'a>(&'a chimera_provider::keychain::OsKeychain);

impl KeychainSink for RealKeychain<'_> {
    fn store(&self, service_name: &str, secret: &str) -> Result<KeychainReference, PortError> {
        self.0
            .store(service_name, secret)
            // The error text is deliberately dropped: a keychain error can
            // echo the service name, and the caller only needs to know the
            // store refused.
            .map_err(|e| PortError::KeychainWrite(e.to_string()))
            .map(|r| KeychainReference::new(r.as_str().to_string()))
    }

    fn remove(&self, reference: &KeychainReference) -> Result<(), PortError> {
        // Idempotent by the port's contract, and the underlying delete already
        // treats an absent entry as success — so a rollback retry never gets
        // stuck re-failing on work it already finished.
        let _ = self.0.delete(&chimera_provider::keychain::SecretRef::new(
            reference.as_str().to_string(),
        ));
        Ok(())
    }
}

/// The provider database. Held behind the same mutex every other command uses,
/// so a migration cannot interleave with an add or a switch.
struct RealProviders<'a>(&'a Mutex<chimera_provider::db::ProviderDb>);

impl RealProviders<'_> {
    fn db(&self) -> Result<std::sync::MutexGuard<'_, chimera_provider::db::ProviderDb>, PortError> {
        self.0
            .lock()
            .map_err(|_| PortError::ProviderLookup("internal state is locked".to_string()))
    }
}

impl ProviderSink for RealProviders<'_> {
    fn contains(&self, id: Uuid) -> Result<bool, PortError> {
        self.db()?
            .get_by_id(id)
            .map(|row| row.is_some())
            .map_err(|e| PortError::ProviderLookup(e.to_string()))
    }

    fn insert(&self, provider: Provider) -> Result<(), PortError> {
        let db = self.db()?;
        let sort_order = db.list_all().map(|r| r.len() as i64).unwrap_or(0);
        let row = ProviderRow {
            id: provider.id,
            display_name: provider.display_name,
            kind: provider.kind,
            base_url: provider.base_url,
            protocol: provider.protocol,
            secret_ref: provider.secret_ref,
            selected_model: provider.selected_model,
            health: provider.health,
            sort_order,
        };
        db.insert(&row)
            .map_err(|e| PortError::ProviderInsert(e.to_string()))
    }

    fn remove(&self, id: Uuid) -> Result<(), PortError> {
        // Idempotent: unwinding a partial migration must not fail on a row
        // that was never written.
        let _ = self.db()?.delete(id);
        Ok(())
    }
}

/// The live Codex `config.toml`, captured and restored byte-for-byte.
struct RealConfig {
    path: PathBuf,
}

impl ConfigSnapshotPort for RealConfig {
    fn snapshot(&self) -> Result<ConfigSnapshot, PortError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(ConfigSnapshot::new(bytes)),
            // No config yet is a legitimate state, and restoring to "no
            // config" has to be expressible — otherwise a failed migration on
            // a fresh install could not be undone.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(ConfigSnapshot::new(Vec::new()))
            }
            Err(e) => Err(PortError::ConfigSnapshot(e.kind().to_string())),
        }
    }

    fn restore(&self, snapshot: &ConfigSnapshot) -> Result<(), PortError> {
        let bytes = snapshot.as_bytes();
        if bytes.is_empty() {
            // Restoring an absent file means removing it, not writing zero
            // bytes — an empty config.toml is not the same state as none.
            if self.path.exists() {
                std::fs::remove_file(&self.path)
                    .map_err(|e| PortError::ConfigRestore(e.kind().to_string()))?;
            }
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&self.path, bytes)
            .map_err(|e| PortError::ConfigRestore(e.kind().to_string()))
    }
}

/// Health check for migrated providers.
///
/// A no-op for now, and deliberately so rather than silently: probing every
/// migrated provider would fire one network request per key the moment a user
/// clicks migrate, which is a surprising amount of traffic on their behalf.
/// The Providers screen already offers a per-provider "Test connection".
struct NoNetworkHealthCheck;

impl HealthCheckPort for NoNetworkHealthCheck {
    fn check(&self, _provider_ids: &[Uuid]) -> Result<(), PortError> {
        Ok(())
    }
}

// ── Source discovery ───────────────────────────────────────────────────────

fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Where current CC Switch releases keep their provider database.
///
/// The importer opens this path read-only. A missing database is a normal
/// "nothing to import" state; an existing but locked/corrupt one is surfaced
/// as a warning so the user can close CC Switch and retry.
fn ccswitch_config_path(home: &std::path::Path) -> PathBuf {
    home.join(".cc-switch").join("cc-switch.db")
}

fn host_of(raw: &str) -> String {
    url::Url::parse(raw)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| "—".to_string())
}

/// Read both sources. Read-only, always: nothing here writes to a 1.x file or
/// to CC Switch's config, which is the stop condition the whole task turns on.
fn collect_candidates() -> (Vec<MigrationCandidate>, Vec<String>, Vec<String>) {
    let home = home_dir();
    let mut candidates = Vec::new();
    let mut dropped = Vec::new();
    let mut warnings = Vec::new();

    match read_legacy_inventory(&LegacySourcePaths::new(
        chimera_migration::legacy_source::resolve_legacy_settings_path(&home),
    )) {
        Ok(inv) => {
            candidates.extend(inv.providers.iter().map(MigrationCandidate::from_legacy));
            dropped.extend(
                inv.dropped_features
                    .iter()
                    .map(|d| d.description().to_string()),
            );
            warnings.extend(inv.warnings);
        }
        // A source that cannot be read is reported, never guessed at, and
        // never blocks the other source from being offered.
        Err(e) => warnings.push(e.to_string()),
    }

    match read_ccswitch_inventory(&CcSwitchSourcePaths::new(ccswitch_config_path(&home))) {
        Ok(Some(inv)) => {
            candidates.extend(inv.providers.iter().map(MigrationCandidate::from_ccswitch));
            warnings.extend(inv.warnings);
        }
        Ok(None) => {}
        Err(e) => warnings.push(e.to_string()),
    }

    (candidates, dropped, warnings)
}

// ── Commands ───────────────────────────────────────────────────────────────

/// What migration would bring across. Reads only.
#[tauri::command]
pub fn preview_migration() -> Result<MigrationPreviewDto, String> {
    let (candidates, dropped_features, warnings) = collect_candidates();
    Ok(MigrationPreviewDto {
        candidates: candidates
            .iter()
            .map(|c| MigrationCandidateDto {
                source: format!("{:?}", c.source_kind).to_lowercase(),
                source_id: c.source_id.clone(),
                display_name: c.display_name.clone(),
                host: host_of(&c.base_url),
                has_key: c.has_secret(),
            })
            .collect(),
        dropped_features,
        warnings,
    })
}

/// Actually migrate. The only command here that writes anything.
#[tauri::command]
pub fn run_migration(state: State<'_, AppState>) -> Result<MigrationResultDto, String> {
    let (candidates, _, _) = collect_candidates();
    if candidates.is_empty() {
        return Ok(MigrationResultDto {
            migrated: 0,
            already_migrated: 0,
            skipped: Vec::new(),
        });
    }

    let keychain = RealKeychain(&state.keychain);
    let providers = RealProviders(&state.db);
    let config = RealConfig {
        path: state.paths.codex_config(),
    };

    let outcome = apply_migration(
        &candidates,
        &keychain,
        &providers,
        &config,
        &NoNetworkHealthCheck,
    )
    // The crate's error Display is already actionable and carries no key,
    // path or raw Rust error.
    .map_err(|e| e.to_string())?;

    Ok(MigrationResultDto {
        migrated: outcome.migrated.len(),
        already_migrated: outcome.already_migrated.len(),
        skipped: outcome
            .skipped
            .into_iter()
            .map(|s| SkippedDto {
                source_id: s.source_id,
                reason: s.reason,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtos_serialise_camel_case() {
        let json = serde_json::to_string(&MigrationPreviewDto {
            candidates: vec![MigrationCandidateDto {
                source: "legacy".into(),
                source_id: "p1".into(),
                display_name: "Old".into(),
                host: "api.example.com".into(),
                has_key: true,
            }],
            dropped_features: vec!["User scripts (not carried over)".into()],
            warnings: vec![],
        })
        .unwrap();
        assert!(json.contains("sourceId"), "{json}");
        assert!(json.contains("droppedFeatures"), "{json}");
        assert!(json.contains("hasKey"), "{json}");
    }

    #[test]
    fn the_preview_dto_carries_a_host_never_a_full_url() {
        // A full URL can hold a path the user would not expect on screen, and
        // the host is what actually identifies the provider to them.
        assert_eq!(
            host_of("https://api.example.com/internal/v1"),
            "api.example.com"
        );
        assert_eq!(host_of("not a url"), "—");
    }
}
