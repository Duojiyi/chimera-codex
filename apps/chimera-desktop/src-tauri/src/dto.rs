//! Serialisable DTOs crossing the Tauri IPC boundary.
//!
//! These mirror the TypeScript interfaces in `src/features/*/lib/*.ts` exactly.
//! Domain types are NOT exposed directly: the boundary owns its own shape so a
//! domain refactor cannot silently break the frontend contract.
//!
//! Field naming is camelCase to match TypeScript convention.

use serde::{Deserialize, Serialize};

/// Home screen system status. Mirrors `SystemStatus` in features/home/index.tsx.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatusDto {
    /// Display name of the active provider; `None` in official mode.
    pub provider_name: Option<String>,
    /// One of: unknown | healthy | auth_failed | unreachable.
    pub provider_health: String,
    /// Managed Codex version string, `None` when not installed.
    pub codex_version: Option<String>,
    /// Whether a Chimera-owned Codex process is currently running.
    pub codex_running: bool,
    /// True when Codex uses the official login rather than a custom provider.
    pub official_mode: bool,
}

impl Default for SystemStatusDto {
    fn default() -> Self {
        Self {
            provider_name: None,
            provider_health: "unknown".to_string(),
            codex_version: None,
            codex_running: false,
            official_mode: true,
        }
    }
}

/// One provider row for the Providers list. Mirrors `ProviderEntry`.
///
/// G4: neither the API key nor its keychain handle crosses the IPC boundary.
/// `secret_ref` stays backend-side — the UI only needs to know a key exists,
/// which it infers from `kind` (custom/chimera_hub always have one).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDto {
    pub id: String,
    pub display_name: String,
    /// One of: chimera_hub | custom.
    pub kind: String,
    pub base_url: String,
    /// One of: unknown | healthy | auth_failed | unreachable | incompatible.
    pub health: String,
    pub selected_model: Option<String>,
}

/// Result of a provider connectivity test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTestDto {
    pub ok: bool,
    /// Health verdict written back to the row on success.
    pub health: String,
    /// Actionable, already-localised message. Never a raw Rust/HTTP error.
    pub message: String,
    /// Models discovered during the probe, when the endpoint reports them.
    #[serde(default)]
    pub discovered_models: Vec<String>,
}

/// One installed skin for the Appearance screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinDto {
    pub id: String,
    pub name: String,
    pub description: String,
    /// True for the built-in "Default" entry, which cannot be removed.
    pub is_default: bool,
    /// True when this skin is the one currently applied.
    pub applied: bool,
}

/// Managed-runtime detail for the Codex screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfoDto {
    pub version: Option<String>,
    /// One of: managed_portable | external_msix | external_portable | none.
    pub install_mode: String,
    pub install_path: Option<String>,
    /// Ownership verification verdict, shown verbatim in the spec sheet.
    pub ownership: String,
    pub healthy: bool,
}

impl Default for RuntimeInfoDto {
    fn default() -> Self {
        Self {
            version: None,
            install_mode: "none".to_string(),
            install_path: None,
            ownership: "not managed".to_string(),
            healthy: false,
        }
    }
}
