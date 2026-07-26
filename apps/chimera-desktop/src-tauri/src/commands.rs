// Chimera++ 2.0 — Tauri command adapters.
//
// G12: these are THIN. Each command validates input, delegates to a service
// crate, and maps the domain error to an actionable message. No business logic,
// no direct file I/O, no SQL.
//
// Every command returns Result<T, String>: the String is user-facing and must
// never contain a raw Rust error, a stack trace, or a secret.

use tauri::State;

use chimera_provider::db::ProviderRow;
use chimera_provider::probe::{UrlValidationError, validate_provider_url};
use chimera_runtime::health::check_runtime_health;

use crate::dto::{ProviderDto, ProviderTestDto, RuntimeInfoDto, SkinDto, SystemStatusDto};
use crate::state::AppState;

/// Map a ProviderRow to its wire DTO. Deliberately drops `secret_ref` entirely —
/// only whether one exists (G4: keys never cross the IPC boundary).
fn row_to_dto(row: &ProviderRow) -> ProviderDto {
    ProviderDto {
        id: row.id.to_string(),
        display_name: row.display_name.clone(),
        kind: format!("{:?}", row.kind).to_lowercase(),
        base_url: row.base_url.to_string(),
        health: format!("{:?}", row.health).to_lowercase(),
        selected_model: row.selected_model.clone(),
    }
}

/// Home screen aggregate: which provider is live, and is Codex healthy.
#[tauri::command]
pub fn get_system_status(state: State<'_, AppState>) -> Result<SystemStatusDto, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "Internal state is locked. Restart Chimera++.".to_string())?;

    let rows = db
        .list_all()
        .map_err(|_| "Could not read the provider list. Run Diagnose.".to_string())?;

    // Official mode = no provider row is marked active. Task 6 will persist the
    // active id; until then an empty list means official login is in use.
    let active = rows.first();

    let health = check_runtime_health(&state.runtime);

    Ok(SystemStatusDto {
        provider_name: active.map(|r| r.display_name.clone()),
        provider_health: active
            .map(|r| format!("{:?}", r.health).to_lowercase())
            .unwrap_or_else(|| "unknown".to_string()),
        codex_version: health.as_ref().ok().and_then(|h| h.version.clone()),
        codex_running: false, // Task 5.5 wires real process liveness
        official_mode: active.is_none(),
    })
}

/// Providers screen: full list. Keys are never included.
#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderDto>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "Internal state is locked. Restart Chimera++.".to_string())?;

    let rows = db
        .list_all()
        .map_err(|_| "Could not read the provider list. Run Diagnose.".to_string())?;

    Ok(rows.iter().map(row_to_dto).collect())
}

/// Codex screen: managed runtime detail.
#[tauri::command]
pub fn get_runtime_info(state: State<'_, AppState>) -> Result<RuntimeInfoDto, String> {
    match check_runtime_health(&state.runtime) {
        Ok(h) => Ok(RuntimeInfoDto {
            version: h.version,
            install_mode: "managed_portable".to_string(),
            install_path: h.exe_path.map(|p| p.display().to_string()),
            ownership: "chimera_verified".to_string(),
            healthy: h.exe_present,
        }),
        Err(_) => Ok(RuntimeInfoDto {
            version: None,
            install_mode: "not_installed".to_string(),
            install_path: None,
            ownership: "none".to_string(),
            healthy: false,
        }),
    }
}

/// Validate a provider URL without persisting anything.
///
/// This is the only command the Add-Provider form needs before the user
/// confirms: it returns the same taxonomy the frontend validator uses, so the
/// two agree on what "valid" means.
#[tauri::command]
pub fn test_provider(base_url: String, dev_mode: bool) -> Result<ProviderTestDto, String> {
    match validate_provider_url(&base_url, dev_mode) {
        Ok(v) => Ok(ProviderTestDto {
            ok: true,
            health: "unknown".to_string(),
            message: match v.v1_candidate {
                Some(c) => format!("URL accepted. Chimera++ will call {c}"),
                None => "URL accepted.".to_string(),
            },
            discovered_models: Vec::new(),
        }),
        Err(e) => Ok(ProviderTestDto {
            ok: false,
            health: "unreachable".to_string(),
            message: url_error_message(&e),
            discovered_models: Vec::new(),
        }),
    }
}

/// Translate a URL validation error into an actionable Chinese-friendly message.
/// Never leaks the raw Debug form of the error.
fn url_error_message(e: &UrlValidationError) -> String {
    match e {
        UrlValidationError::Empty => "Endpoint URL is required.".to_string(),
        UrlValidationError::ContainsUserinfo => {
            "Remove the username and password from the URL. Enter the API key in the Key field."
                .to_string()
        }
        UrlValidationError::ContainsFragment => "Remove the # fragment from the URL.".to_string(),
        UrlValidationError::InsecureScheme { scheme } => {
            if scheme == "http" {
                "http:// is only allowed for 127.0.0.1 in developer mode. Use https://".to_string()
            } else {
                "Endpoint must start with https://".to_string()
            }
        }
        UrlValidationError::Parse(_) => {
            "That is not a valid URL. Example: https://api.example.com/v1".to_string()
        }
    }
}

/// Switch the live provider, or pass `null` to restore official login.
///
/// Not yet wired to SwitchTransaction: Task 6 connects the CAS transaction so
/// the command is transactional end to end. Returning an explicit error is
/// deliberate — a silent success here would make the UI lie about the live
/// config, which is worse than a visible "not yet available".
#[tauri::command]
pub fn switch_provider(_provider_id: Option<String>) -> Result<(), String> {
    Err(
        "Provider switching is not enabled in this build. Task 6 connects the config transaction."
            .to_string(),
    )
}

/// Launch the managed Codex runtime.
///
/// Deliberately fails loudly rather than pretending: the process-spawn path is
/// Task 5.5 and launching an unverified runtime would violate G5.
#[tauri::command]
pub fn launch_codex(state: State<'_, AppState>) -> Result<(), String> {
    match check_runtime_health(&state.runtime) {
        Ok(h) if h.exe_present => Err(
            "Codex is installed but launching is not enabled in this build. Task 5.5 wires process spawn."
                .to_string(),
        ),
        Ok(_) | Err(_) => Err(
            "Codex is not installed yet. Open the Codex tab to prepare the managed runtime."
                .to_string(),
        ),
    }
}

// ── Appearance: skins ────────────────────────────────────────────────────────
// chimera-theme is a stub until Task 8. These commands return the honest
// default-only state rather than fabricating a skin library.

#[tauri::command]
pub fn list_skins() -> Result<Vec<SkinDto>, String> {
    Ok(vec![SkinDto {
        id: "default".to_string(),
        name: "Default".to_string(),
        description: "No modifications to official app files".to_string(),
        is_default: true,
        applied: true,
    }])
}

#[tauri::command]
pub fn apply_skin(_skin_id: String) -> Result<(), String> {
    Err("Skins are not enabled in this build. Task 8 adds the restricted skin engine.".to_string())
}

#[tauri::command]
pub fn try_skin(_skin_id: String) -> Result<(), String> {
    Err(
        "Skin preview is not enabled in this build. Task 8 adds the restricted skin engine."
            .to_string(),
    )
}

/// Always available: restoring the default is the recovery path, so it must
/// never fail even when the skin engine is absent.
#[tauri::command]
pub fn restore_default_skin() -> Result<(), String> {
    Ok(())
}
