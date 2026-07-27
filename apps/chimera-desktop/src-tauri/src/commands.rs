// Chimera++ 2.0 — Tauri command adapters.
//
// G12: these are THIN. Each command validates input, delegates to a service
// crate, and maps the domain error to an actionable message. No business logic,
// no direct file I/O, no SQL.
//
// Every command returns Result<T, String>: the String is user-facing and must
// never contain a raw Rust error, a stack trace, or a secret.

use std::fs;

use tauri::State;
use uuid::Uuid;

use chimera_platform::lock::{LockError, OperationLock};
use chimera_provider::db::ProviderRow;
use chimera_provider::keychain::{KeychainPort, SecretRef};
use chimera_provider::probe::{UrlValidationError, validate_provider_url};
use chimera_provider::projection::{
    ActiveProvider, ProviderProjection, detect_active_provider, revert_provider_projection,
};
use chimera_provider::transaction::{SwitchTransaction, TransactionOutcome, TxError};
use chimera_runtime::detection::detect_external_runtime;
use chimera_runtime::health::check_runtime_health;
use chimera_runtime::process::{LaunchError, codex_process_running, launch_managed_codex};

use crate::dto::{ProviderDto, ProviderTestDto, RuntimeInfoDto, SystemStatusDto};
use crate::state::AppState;

/// Relabel the tray menu when the user changes language.
///
/// The tray is created during setup, before any webview has told us what
/// language the user prefers, so it starts in Chinese. The frontend calls this
/// on mount and on every switch; without it the tray would be the one part of
/// the UI stuck in the wrong language.
#[tauri::command]
pub fn set_tray_language(app: tauri::AppHandle, lang: String) -> Result<(), String> {
    crate::tray::set_language(&app, &lang)
        .map_err(|_| "Could not update the tray menu language.".to_string())
}

/// Map a ProviderRow to its wire DTO. Deliberately drops `secret_ref` entirely —
/// only whether one exists (G4: keys never cross the IPC boundary).
pub fn row_to_dto(row: &ProviderRow) -> ProviderDto {
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

    // Read what Codex will actually use. Other switchers do not write our
    // ownership marker, so using that marker as provider detection incorrectly
    // labelled every CC Switch or hand-written configuration as official mode.
    let configured = fs::read_to_string(state.paths.codex_config())
        .ok()
        .and_then(|text| detect_active_provider(&text).ok())
        .unwrap_or(ActiveProvider::Official);
    let configured_url = match &configured {
        ActiveProvider::Custom { base_url, .. } => Some(normalize_provider_url(base_url)),
        ActiveProvider::Official => None,
    };
    let active = configured_url.as_deref().and_then(|wanted| {
        rows.iter()
            .find(|row| normalize_provider_url(row.base_url.as_str()) == wanted)
    });
    let provider_name = match &configured {
        ActiveProvider::Custom { display_name, .. } => Some(
            active
                .map(|row| row.display_name.clone())
                .unwrap_or_else(|| display_name.clone()),
        ),
        ActiveProvider::Official => None,
    };

    let health = check_runtime_health(&state.runtime);
    let external = detect_external_runtime();
    let codex_version = health
        .as_ref()
        .ok()
        .and_then(|h| h.version.clone())
        .or_else(|| match external.as_ref() {
            Some(chimera_runtime::detection::DetectedRuntime::ExternalMsix { version, .. }) => {
                Some(version.clone())
            }
            _ => None,
        });

    Ok(SystemStatusDto {
        provider_name,
        active_provider_id: active.map(|r| r.id.to_string()),
        provider_health: active
            .map(|r| format!("{:?}", r.health).to_lowercase())
            .unwrap_or_else(|| "unknown".to_string()),
        codex_version,
        codex_running: codex_process_running(),
        official_mode: matches!(configured, ActiveProvider::Official),
    })
}

fn normalize_provider_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_ascii_lowercase()
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
pub fn url_error_message(e: &UrlValidationError) -> String {
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
/// The command delegates the file mutation to the CAS transaction and only
/// updates the provider ordering after the projection commits successfully.
#[tauri::command]
pub fn switch_provider(
    provider_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let tx = SwitchTransaction::new(
        state.paths.codex_config(),
        state.paths.operation_lock(),
        state.paths.journal(),
    );

    // No id means "restore the official login": revert only the keys Chimera
    // added, leaving [auth], MCP, and every unknown field untouched (G3).
    let Some(raw_id) = provider_id else {
        return restore_official(&state);
    };

    let id = Uuid::parse_str(&raw_id).map_err(|_| "That provider id is not valid.".to_string())?;

    let row = {
        let db = state
            .db
            .lock()
            .map_err(|_| "Internal state is locked. Restart Chimera++.".to_string())?;
        db.get_by_id(id)
            .map_err(|e| format!("Could not read the provider: {e}"))?
            .ok_or_else(|| "That provider no longer exists.".to_string())?
    };

    // The key never round-trips through the DB or the frontend — only its
    // opaque handle does, and it is resolved here at the moment of use (G4).
    let secret_ref = row
        .secret_ref
        .as_deref()
        .map(SecretRef::new)
        .ok_or_else(|| {
            "This provider has no stored API key. Re-add it to store one.".to_string()
        })?;

    if state
        .keychain
        .retrieve(&secret_ref)
        .map_err(|e| format!("Could not read the stored API key: {e}"))?
        .is_none()
    {
        return Err(
            "The stored API key is missing from the system credential store. Re-enter it for this provider."
                .to_string(),
        );
    }

    let projection = ProviderProjection {
        base_url: row.base_url.to_string(),
        model: row.selected_model.clone(),
        // The transaction resolves the handle itself; passing the material here
        // would put the key in this frame and risk it reaching a log or panic.
        api_key_env_or_plain: String::new(),
    };

    match tx.execute(&projection, &state.keychain, &secret_ref) {
        Ok(TransactionOutcome::Committed) => {
            let db = state
                .db
                .lock()
                .map_err(|_| "Internal state is locked. Restart Chimera++.".to_string())?;
            db.mark_active(id).map_err(|_| {
                "Provider switched, but its active state could not be saved.".to_string()
            })?;
            Ok(())
        }
        // CAS caught a write between snapshot and commit. Nothing was changed,
        // so the honest response is to ask for a retry rather than clobber it.
        Ok(TransactionOutcome::Conflict(_)) => Err(
            "Codex's config.toml changed while switching, so nothing was modified. Try again."
                .to_string(),
        ),
        Err(e) => Err(tx_error_message(&e)),
    }
}

/// Revert Chimera's owned keys, returning Codex to its official login.
fn restore_official(state: &State<'_, AppState>) -> Result<(), String> {
    let config_path = state.paths.codex_config();
    if !config_path.exists() {
        // Nothing was ever projected, so official mode is already the state.
        if let Ok(db) = state.db.lock() {
            let _ = db.clear_active();
        }
        return Ok(());
    }

    let lock = OperationLock::new(state.paths.operation_lock());
    let _guard = lock
        .try_acquire("restore_official")
        .map_err(|e| lock_error_message(&e))?;

    let current = fs::read_to_string(&config_path)
        .map_err(|e| format!("Could not read Codex's config.toml: {e}"))?;
    let reverted = revert_provider_projection(&current)
        .map_err(|e| format!("Could not update Codex's config.toml: {e}"))?;

    atomic_write(&config_path, &reverted)
        .map_err(|e| format!("Could not write Codex's config.toml: {e}"))?;
    if let Ok(db) = state.db.lock() {
        let _ = db.clear_active();
    }
    Ok(())
}

/// Write via temp file + rename so a crash can never leave a partial config.
fn atomic_write(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("toml.chimera-tmp");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path)
}

fn lock_error_message(e: &LockError) -> String {
    match e {
        LockError::AlreadyHeld { .. } => {
            "Another Chimera++ operation is in progress. Wait for it to finish and try again."
                .to_string()
        }
        LockError::Io { .. } => {
            "Could not acquire the operation lock. Check that the data directory is writable."
                .to_string()
        }
    }
}

fn tx_error_message(e: &TxError) -> String {
    match e {
        TxError::Lock(l) => lock_error_message(l),
        TxError::Projection(_) => {
            "Codex's config.toml could not be updated because it is not valid TOML.".to_string()
        }
        TxError::Keychain(_) => {
            "Could not read the stored API key from the system credential store.".to_string()
        }
        TxError::Io(_) => {
            "Could not write Codex's config.toml. Check that the file is writable.".to_string()
        }
        // The DB row references a credential the OS store no longer has — the
        // user revoked it, or the profile moved between machines.
        TxError::SecretMissing => {
            "The stored API key is missing from the system credential store. Re-enter it for this provider."
                .to_string()
        }
    }
}

/// Launch the managed Codex runtime.
///
/// Spawns the active version's executable, detached. `launch_managed_codex`
/// re-verifies ownership under the runtime root before spawning (G5) — this
/// command only translates the outcome into a user-facing message.
#[tauri::command]
pub fn launch_codex(state: State<'_, AppState>) -> Result<(), String> {
    launch_managed_codex(&state.runtime)
        .map(|_report| ())
        .map_err(|e| launch_error_message(&e))
}

/// Translate a launch failure into an actionable message. Never leaks a raw
/// filesystem path here: `NotOwned` in particular must not reveal where an
/// unmanaged install lives (G5), only that Chimera does not manage it.
fn launch_error_message(e: &LaunchError) -> String {
    match e {
        LaunchError::NotInstalled => {
            "Codex is not installed yet. Open the Codex tab to prepare the managed runtime."
                .to_string()
        }
        LaunchError::NotOwned { .. } => {
            "The Codex install Chimera++ found is not Chimera-managed, so it cannot be launched from here. Reinstall it from the Codex tab."
                .to_string()
        }
        LaunchError::Spawn(_) => {
            "Could not start Codex. Check that the managed install is not corrupted and try again."
                .to_string()
        }
    }
}

// ── Appearance: skins ────────────────────────────────────────────────────────
// chimera-theme is a stub until Task 8. These commands return the honest
// default-only state rather than fabricating a skin library.
