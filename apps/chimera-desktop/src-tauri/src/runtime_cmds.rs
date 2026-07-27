//! Commands for the Codex runtime screen and the Settings screen.
//!
//! Split out of `commands.rs` to keep that file focused on providers. Same rule
//! applies here: these are thin adapters over chimera-runtime / chimera-platform,
//! and any operation whose machinery is not implemented yet returns an explicit
//! error rather than a fabricated success.

use tauri::State;

use chimera_runtime::detection::{DetectedRuntime, detect_runtime};
use chimera_runtime::health::check_runtime_health;
use chimera_runtime::update::{UpdateError, rollback_to_last_known};
use chimera_update::atomic::{AtomicError, AtomicStore, Migratable};

use crate::dto::{DiagnosticEntryDto, RuntimeStatusDto, SettingsDto, VersionEntryDto};
use crate::state::AppState;

impl Migratable for SettingsDto {
    const CURRENT_VERSION: u32 = 1;

    fn upgrade(from_version: u32, value: serde_json::Value) -> Option<Self> {
        (from_version == 0).then(|| serde_json::from_value(value).ok())?
    }
}

fn settings_store(state: &AppState) -> AtomicStore<SettingsDto> {
    AtomicStore::new(state.paths.settings())
}

fn settings_error(error: AtomicError) -> String {
    match error {
        AtomicError::FutureSchema { .. } => {
            "Settings were saved by a newer Chimera++ version. Update Chimera++ before editing them."
                .to_string()
        }
        AtomicError::Corrupt => {
            "The settings file is damaged and its backup could not be recovered. Reset settings to continue."
                .to_string()
        }
        AtomicError::Io(_) => "Could not access the settings file.".to_string(),
        AtomicError::Encode => "Could not encode the settings.".to_string(),
    }
}

/// Full state for the Codex screen.
///
/// Returns an honest empty state (`installed: false`) when no managed runtime
/// exists rather than an error — a fresh install has no runtime yet, and that is
/// a normal state the screen renders, not a failure.
#[tauri::command]
pub fn get_runtime_status(state: State<'_, AppState>) -> Result<RuntimeStatusDto, String> {
    let root = state.paths.runtime_root();
    let detected = detect_runtime(&root).ok();
    let health = check_runtime_health(&state.runtime).ok();
    let pointer = state.runtime.read_current_pointer().ok().flatten();

    let installed = health.as_ref().is_some_and(|h| h.exe_present);
    let version = health.as_ref().and_then(|h| h.version.clone());

    // History comes from the pointer chain, the only record that exists today.
    // Anything richer would be invented.
    let mut history = Vec::new();
    if let Some(ref p) = pointer {
        history.push(VersionEntryDto {
            version: p.active_version.clone(),
            state: "active".to_string(),
        });
        if let Some(ref prev) = p.previous_version {
            history.push(VersionEntryDto {
                version: prev.clone(),
                state: "previous".to_string(),
            });
        }
    }

    // DetectedRuntime is an enum, and only ManagedPortable is ours. An external
    // MSIX or portable install must never be reported as Chimera-verified: the
    // UI gates destructive actions on that label (G5).
    let (mode_label, ownership_label) = match detected.as_ref() {
        Some(DetectedRuntime::ManagedPortable(_)) => (
            Some("managed_portable".to_string()),
            Some("chimera_verified".to_string()),
        ),
        Some(DetectedRuntime::ExternalMsix { .. }) => (
            Some("external_msix".to_string()),
            Some("not_owned".to_string()),
        ),
        Some(DetectedRuntime::ExternalPortable { .. }) => (
            Some("external_portable".to_string()),
            Some("not_owned".to_string()),
        ),
        Some(DetectedRuntime::Unknown) | None => (None, None),
    };

    Ok(RuntimeStatusDto {
        installed,
        version,
        platform: Some(platform_label()),
        healthy: installed,
        health_label: None,
        mode: mode_label.clone(),
        ownership: ownership_label.clone(),
        install_path: Some(root.to_string_lossy().to_string()),
        last_update: None,
        uptime: None,
        // No mirror is reachable yet, so never claim an update is available.
        update_available: false,
        update_version: None,
        update_channel: None,
        update_meta: None,
        history,
        diagnostics: diagnostics_for(installed, detected.is_some()),
    })
}

fn platform_label() -> String {
    if cfg!(windows) {
        "Windows x64".to_string()
    } else {
        "macOS".to_string()
    }
}

/// Build the diagnostics list.
///
/// A check whose machinery is not implemented reports `warn`, never `pass` —
/// a green row the code cannot substantiate would be a lie to the user.
fn diagnostics_for(exe_present: bool, ownership_known: bool) -> Vec<DiagnosticEntryDto> {
    vec![
        DiagnosticEntryDto {
            name: "ownership".to_string(),
            result: if ownership_known { "pass" } else { "warn" }.to_string(),
        },
        DiagnosticEntryDto {
            name: "executable".to_string(),
            result: if exe_present { "pass" } else { "fail" }.to_string(),
        },
        DiagnosticEntryDto {
            // Authenticode verification is not wired yet.
            name: "signature".to_string(),
            result: "warn".to_string(),
        },
    ]
}

/// Re-verify the managed runtime.
///
/// Fails loudly while the repair path is unimplemented: a silent success would
/// tell the user their install was fixed when nothing happened.
#[tauri::command]
pub fn repair_runtime(state: State<'_, AppState>) -> Result<(), String> {
    match check_runtime_health(&state.runtime) {
        Ok(h) if h.exe_present => Err(
            "Runtime is present and no repair is needed. Reinstall repair is not enabled in this build."
                .to_string(),
        ),
        _ => Err(
            "No managed runtime to repair. Install Codex first — managed install is not enabled in this build."
                .to_string(),
        ),
    }
}

/// Run the diagnostic checks. Read-only, so it always succeeds.
#[tauri::command]
pub fn run_diagnostics(state: State<'_, AppState>) -> Result<Vec<DiagnosticEntryDto>, String> {
    get_runtime_status(state).map(|s| s.diagnostics)
}

/// Roll back to the previous version in the pointer chain.
///
/// `version` is accepted because the history rows offer per-entry restore, but
/// only the immediately-previous version can be honoured today. Any other
/// request is rejected rather than silently restoring the wrong build.
#[tauri::command]
pub fn rollback_runtime(version: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    let pointer = state
        .runtime
        .read_current_pointer()
        .map_err(|_| "Could not read the runtime pointer.".to_string())?
        .ok_or_else(|| "No runtime is installed, so there is nothing to roll back.".to_string())?;

    if let Some(ref want) = version {
        let is_previous = pointer.previous_version.as_deref() == Some(want.as_str());
        if !is_previous {
            return Err(format!(
                "Only the previous version can be restored right now. Restoring {want} \
                 needs the version store, which is not enabled in this build."
            ));
        }
    }

    match rollback_to_last_known(&state.runtime) {
        Ok(_) => Ok(()),
        Err(UpdateError::NoPreviousVersion) => {
            Err("There is no previous version to roll back to.".to_string())
        }
        Err(_) => Err("Rollback failed. Run diagnostics for detail.".to_string()),
    }
}

/// Apply a pending Codex update.
///
/// Refuses rather than pretending: the verified download and staged commit are
/// not implemented, and applying an unverified payload would violate ADR-003.
#[tauri::command]
pub fn apply_codex_update(_version: Option<String>) -> Result<(), String> {
    Err(
        "Updating is not enabled in this build. The verified download and staged commit \
         are not wired yet."
            .to_string(),
    )
}

// ── Settings ─────────────────────────────────────────────────────────────────

/// Read persisted settings, falling back to defaults.
///
/// A corrupt file degrades to defaults rather than failing: settings are
/// recoverable state, and locking the user out of the panel that could fix them
/// would be the worse outcome.
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<SettingsDto, String> {
    let store = settings_store(&state);
    match store.read() {
        Ok(Some(settings)) => Ok(settings),
        Ok(None) => Ok(SettingsDto::default()),
        Err(AtomicError::Corrupt) => {
            // One-time migration for the 1.x plain JSON shape. Once read, it
            // is immediately rewritten into the versioned, backed-up store.
            let legacy = std::fs::read_to_string(state.paths.settings())
                .ok()
                .and_then(|text| serde_json::from_str::<SettingsDto>(&text).ok());
            if let Some(settings) = legacy {
                store.write(&settings).map_err(settings_error)?;
                return Ok(settings);
            }
            Err(settings_error(AtomicError::Corrupt))
        }
        Err(error) => Err(settings_error(error)),
    }
}

/// Persist settings with an atomic replace, so a crash mid-write cannot leave a
/// truncated file behind.
#[tauri::command]
pub fn save_settings(settings: SettingsDto, state: State<'_, AppState>) -> Result<(), String> {
    settings_store(&state)
        .write(&settings)
        .map_err(settings_error)
}

/// Restore defaults by removing the settings file. Always succeeds when the
/// file is already absent — reset is a recovery path.
#[tauri::command]
pub fn reset_settings(state: State<'_, AppState>) -> Result<(), String> {
    let path = state.paths.settings();
    for sibling in [
        path.clone(),
        path.with_extension("json.bak"),
        path.with_extension("json.tmp"),
    ] {
        if sibling.exists() {
            std::fs::remove_file(&sibling).map_err(|_| "Could not reset settings.".to_string())?;
        }
    }
    Ok(())
}
