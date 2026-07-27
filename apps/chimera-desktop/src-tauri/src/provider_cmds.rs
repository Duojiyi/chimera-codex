//! Provider lifecycle commands: add, delete, probe.
//!
//! The acceptance criterion these exist to satisfy (Spec 7.x, Task 3):
//! **a provider is only activated after a successful verification**. The
//! frontend cannot uphold that on its own — it can only regex-check a URL. The
//! probe must happen here, over the network, before anything is persisted.
//!
//! Ordering is deliberate and matters:
//!   1. validate the URL shape (cheap, no network, no secret stored)
//!   2. probe the endpoint with the key (network, still nothing persisted)
//!   3. only on success: store the key in the OS keychain, then insert the row
//!
//! A failure at step 1 or 2 leaves zero trace: no keychain entry, no DB row.
//! That is what makes a failed add safe to retry.

use tauri::State;
use uuid::Uuid;

use chimera_domain::{ProviderHealth, ProviderKind, ProviderProtocol};
use chimera_provider::db::ProviderRow;
use chimera_provider::keychain::{KeychainPort, SecretRef};
use chimera_provider::probe::{probe_provider, validate_provider_url};

use crate::dto::ProviderDto;
use crate::state::AppState;

/// ChimeraHub's fixed endpoint. Users supply only a key for this kind, so the
/// URL is ours and never comes from input.
const CHIMERA_HUB_URL: &str = "https://api.chimerahub.org/v1";

const LOCK_MSG: &str = "Internal state is locked. Restart Chimera++.";

/// Keychain service name for a provider.
///
/// Keyed by the provider's UUID, not by its kind. Keying by kind made every
/// custom provider share one credential, so adding a second silently destroyed
/// the first one's key.
fn service_name_for(id: Uuid) -> String {
    format!("provider/{id}")
}

/// Add a provider, but only if it verifies.
///
/// `base_url` is ignored for the ChimeraHub kind.
#[tauri::command]
pub async fn add_provider(
    state: State<'_, AppState>,
    kind: String,
    base_url: String,
    api_key: String,
    dev_mode: bool,
) -> Result<ProviderDto, String> {
    let kind = match kind.as_str() {
        "chimera_hub" => ProviderKind::ChimeraHub,
        "custom" => ProviderKind::Custom,
        other => return Err(format!("Unknown provider type '{other}'.")),
    };

    if api_key.trim().is_empty() {
        return Err("An API key is required.".to_string());
    }

    // ── Step 1: URL shape ────────────────────────────────────────────────────
    let resolved_url = match kind {
        ProviderKind::ChimeraHub => CHIMERA_HUB_URL.to_string(),
        ProviderKind::Custom => {
            let validated = validate_provider_url(&base_url, dev_mode)
                .map_err(|e| crate::commands::url_error_message(&e))?;
            // An origin-only URL exposes a /v1 candidate. Probing the origin
            // would fail for most endpoints, so probe the candidate when the
            // user gave no path — but persist exactly what we probed.
            validated
                .v1_candidate
                .unwrap_or(validated.base_url)
                .to_string()
        }
    };

    // ── Step 2: real network verification, nothing persisted yet ─────────────
    let outcome = probe_provider(&resolved_url, api_key.trim()).await;
    if !outcome.ok {
        // The message is already actionable and carries no raw HTTP error.
        return Err(outcome.message);
    }

    // ── Step 3: verified — now persist ───────────────────────────────────────
    let id = Uuid::new_v4();
    let service = service_name_for(id);

    let secret_ref = state
        .keychain
        .store(&service, api_key.trim())
        .map_err(|_| "Could not save the API key to the system credential store.".to_string())?;

    let display_name = match kind {
        ProviderKind::ChimeraHub => "ChimeraHub".to_string(),
        ProviderKind::Custom => url::Url::parse(&resolved_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_else(|| "Custom".to_string()),
    };

    let parsed_url = url::Url::parse(&resolved_url)
        .map_err(|_| "That endpoint URL could not be parsed.".to_string())?;

    let row = {
        let db = state.db.lock().map_err(|_| LOCK_MSG.to_string())?;
        let sort_order = db.list_all().map(|r| r.len() as i64).unwrap_or(0);
        let row = ProviderRow {
            id,
            display_name,
            kind,
            base_url: parsed_url,
            protocol: ProviderProtocol::Responses,
            secret_ref: Some(secret_ref.as_str().to_string()),
            // The probe just succeeded, so this is measured, not assumed.
            selected_model: outcome.discovered_models.first().cloned(),
            health: ProviderHealth::Healthy,
            sort_order,
        };
        if let Err(e) = db.insert(&row) {
            // Roll back the credential so a failed insert cannot strand a key
            // with no row pointing at it.
            let _ = state.keychain.delete(&secret_ref);
            return Err(format!("Could not save the provider: {e}"));
        }
        row
    };

    Ok(crate::commands::row_to_dto(&row))
}

/// Delete a provider and its stored credential.
///
/// The keychain entry goes first: a leftover credential with no row is invisible
/// to the user and can never be cleaned up through the UI.
#[tauri::command]
pub fn delete_provider(state: State<'_, AppState>, provider_id: String) -> Result<(), String> {
    let id =
        Uuid::parse_str(&provider_id).map_err(|_| "That provider id is not valid.".to_string())?;

    let db = state.db.lock().map_err(|_| LOCK_MSG.to_string())?;

    let row = db
        .get_by_id(id)
        .map_err(|e| format!("Could not read the provider: {e}"))?
        .ok_or_else(|| "That provider no longer exists.".to_string())?;

    let projection_active = std::fs::read_to_string(state.paths.codex_config())
        .ok()
        .and_then(|text| text.parse::<toml_edit::DocumentMut>().ok())
        .and_then(|doc| doc.get("chimera_managed").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    let is_active = projection_active
        && db
            .list_all()
            .ok()
            .and_then(|rows| rows.first().map(|active| active.id == id))
            .unwrap_or(false);
    if is_active {
        return Err("Switch to Official Codex before deleting the active provider.".to_string());
    }

    if let Some(ref r) = row.secret_ref {
        // Idempotent by contract, so an already-absent credential is fine.
        let _ = state.keychain.delete(&SecretRef::new(r.clone()));
    }

    db.delete(id)
        .map_err(|e| format!("Could not delete the provider: {e}"))
}

/// Re-probe an existing provider, resolving its key from the keychain.
#[tauri::command]
pub async fn test_existing_provider(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<crate::dto::ProviderTestDto, String> {
    let id =
        Uuid::parse_str(&provider_id).map_err(|_| "That provider id is not valid.".to_string())?;

    // Scope the lock so it is not held across the await.
    let (base_url, secret_ref) = {
        let db = state.db.lock().map_err(|_| LOCK_MSG.to_string())?;
        let row = db
            .get_by_id(id)
            .map_err(|e| format!("Could not read the provider: {e}"))?
            .ok_or_else(|| "That provider no longer exists.".to_string())?;
        let r = row
            .secret_ref
            .clone()
            .ok_or_else(|| "This provider has no stored API key.".to_string())?;
        (row.base_url.to_string(), SecretRef::new(r))
    };

    let key = state
        .keychain
        .retrieve(&secret_ref)
        .map_err(|_| "Could not read the stored API key.".to_string())?
        .ok_or_else(|| {
            "The stored API key is missing from the system credential store. Re-enter it."
                .to_string()
        })?;

    let outcome = probe_provider(&base_url, &key).await;

    // Record what was measured so the list reflects reality after a test.
    if let Ok(db) = state.db.lock() {
        let _ = db.update_health(id, outcome.health.clone());
    }

    Ok(crate::dto::ProviderTestDto {
        ok: outcome.ok,
        health: format!("{:?}", outcome.health).to_lowercase(),
        message: outcome.message,
        discovered_models: outcome.discovered_models,
    })
}
