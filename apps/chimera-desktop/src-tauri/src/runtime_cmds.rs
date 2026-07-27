//! Commands for the Codex runtime screen and the Settings screen.
//!
//! Split out of `commands.rs` to keep that file focused on providers. Same rule
//! applies here: these are thin adapters over chimera-runtime / chimera-platform,
//! and any operation whose machinery is not implemented yet returns an explicit
//! error rather than a fabricated success.

use tauri::{Emitter, State};

use chimera_platform::lock::OperationLock;
use chimera_runtime::detection::{DetectedRuntime, detect_external_runtime, detect_runtime};
use chimera_runtime::health::check_runtime_health;
use chimera_runtime::manager::{
    InstallMode, MaintenanceRoute, UpdateSource, detect_portable_codex, detect_windows_codex,
    diagnose_windows_codex, fetch_windows_release_plan, install_windows_release,
    latest_portable_rollback, maintenance_route, rollback_portable_install,
    uninstall_windows_codex,
};
use chimera_runtime::update::{UpdateError, rollback_to_last_known};
use chimera_update::atomic::{AtomicError, AtomicStore, Migratable};

use crate::dto::{
    CodexOperationDto, CodexUpdateDto, DiagnosticEntryDto, RuntimeStatusDto, SettingsDto,
    VersionEntryDto,
};
use crate::state::AppState;

impl Migratable for SettingsDto {
    const CURRENT_VERSION: u32 = 2;

    fn upgrade(from_version: u32, value: serde_json::Value) -> Option<Self> {
        (from_version <= 1).then(|| serde_json::from_value(value).ok())?
    }
}

/// Query the selected source and return the real public Codex version.
/// Network and Windows package detection run in the blocking pool so opening
/// the screen cannot freeze the webview.
#[tauri::command]
pub async fn check_codex_update(
    source: Option<String>,
    install_mode: Option<String>,
    state: State<'_, AppState>,
) -> Result<CodexUpdateDto, String> {
    let settings = state.settings();
    let source_raw = source.unwrap_or(settings.codex_update_source);
    let mode_raw = install_mode.unwrap_or(settings.codex_install_mode);
    let source = source_raw
        .parse::<UpdateSource>()
        .map_err(|_| "Choose Automatic or Mirror as the Codex update source.".to_string())?;
    let install_mode = mode_raw
        .parse::<InstallMode>()
        .map_err(|_| "Choose Standard or Portable as the Codex install mode.".to_string())?;
    let portable_root = state.paths.data_root.join("codex-portable");

    tauri::async_runtime::spawn_blocking(move || {
        let installed = detect_windows_codex(&portable_root);
        let current_version = installed.as_ref().map(|value| value.version.clone());
        let plan = fetch_windows_release_plan(source, Some(std::env::consts::ARCH))
            .map_err(|error| error.to_string())?;
        Ok(CodexUpdateDto {
            update_available: plan.is_update_available(current_version.as_deref()),
            current_version,
            latest_version: plan.version,
            package_version: plan.package_version,
            source: match source {
                UpdateSource::Auto => "auto".to_string(),
                UpdateSource::Mirror => "mirror".to_string(),
            },
            install_mode: match install_mode {
                InstallMode::Standard => "standard".to_string(),
                InstallMode::Portable => "portable".to_string(),
            },
            size_bytes: plan.size_bytes,
            released_at: plan.released_at,
        })
    })
    .await
    .map_err(|_| "The Codex update check was interrupted. Try again.".to_string())?
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
    let detected = detect_runtime(&root)
        .ok()
        .filter(|runtime| !matches!(runtime, DetectedRuntime::Unknown));
    let detected = detected.or_else(detect_external_runtime);
    let health = check_runtime_health(&state.runtime).ok();
    let pointer = state.runtime.read_current_pointer().ok().flatten();
    let manager_install = detect_windows_codex(&state.paths.data_root.join("codex-portable"));

    let installed = manager_install.is_some()
        || health.as_ref().is_some_and(|h| h.exe_present)
        || detected.is_some();
    let version = manager_install
        .as_ref()
        .map(|install| install.version.clone())
        .or_else(|| health.as_ref().and_then(|h| h.version.clone()));

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
    if history.is_empty() {
        if let Some(install) = manager_install.as_ref() {
            history.push(VersionEntryDto {
                version: install.version.clone(),
                state: "active".to_string(),
            });
            if install.install_mode == "portable" {
                if let Ok(Some(backup)) =
                    latest_portable_rollback(&state.paths.data_root.join("codex-portable"))
                {
                    if let Some(previous) = detect_portable_codex(&backup) {
                        history.push(VersionEntryDto {
                            version: previous.version,
                            state: "previous".to_string(),
                        });
                    }
                }
            }
        }
    }

    // DetectedRuntime is an enum, and only ManagedPortable is ours. An external
    // MSIX or portable install must never be reported as Chimera-verified: the
    // UI gates destructive actions on that label (G5).
    let (mode_label, ownership_label) = if let Some(install) = manager_install.as_ref() {
        if install.install_mode == "portable" {
            (
                Some("managed_portable".to_string()),
                Some("chimera_verified".to_string()),
            )
        } else {
            (
                Some("external_msix".to_string()),
                Some("not_owned".to_string()),
            )
        }
    } else {
        match detected.as_ref() {
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
        }
    };

    Ok(RuntimeStatusDto {
        installed,
        version: version.or_else(|| match detected.as_ref() {
            Some(DetectedRuntime::ExternalMsix { version, .. }) => Some(version.clone()),
            _ => None,
        }),
        platform: Some(platform_label()),
        healthy: manager_install.is_some()
            || health.as_ref().is_some_and(|value| value.exe_present),
        health_label: None,
        mode: mode_label.clone(),
        ownership: ownership_label.clone(),
        install_path: manager_install
            .as_ref()
            .map(|install| install.path.clone())
            .or_else(|| match detected.as_ref() {
                Some(DetectedRuntime::ExternalMsix { path, .. })
                | Some(DetectedRuntime::ExternalPortable { path }) => {
                    Some(path.to_string_lossy().to_string())
                }
                _ => Some(root.to_string_lossy().to_string()),
            }),
        last_update: None,
        uptime: None,
        // No mirror is reachable yet, so never claim an update is available.
        update_available: false,
        update_version: None,
        update_channel: None,
        update_meta: None,
        history,
        diagnostics: diagnostics_for(
            installed,
            manager_install
                .as_ref()
                .is_some_and(|install| install.install_mode == "portable")
                || matches!(detected, Some(DetectedRuntime::ManagedPortable(_))),
        ),
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

/// Repair the detected Codex install by reinstalling a freshly verified copy.
#[tauri::command]
pub async fn repair_runtime(
    app: tauri::AppHandle,
    source: Option<String>,
    install_mode: Option<String>,
    state: State<'_, AppState>,
) -> Result<CodexOperationDto, String> {
    let portable_root = state.paths.data_root.join("codex-portable");
    let installed = detect_windows_codex(&portable_root)
        .ok_or_else(|| "Codex is not installed. Use Install before running Repair.".to_string())?;
    let mode = install_mode.or_else(|| Some(installed.install_mode));
    apply_codex_update(app, None, source, mode, state).await
}

fn operation_dto(result: chimera_runtime::manager::InstallOperationResult) -> CodexOperationDto {
    CodexOperationDto {
        version: result.version,
        requested_mode: result.requested_mode,
        actual_mode: result.actual_mode,
        affected_path: result.affected_path,
        backup_path: result.backup_path,
        message: result.message,
        notes: result.notes,
    }
}

/// Run signature, package registration, dependency, and launch diagnostics.
#[tauri::command]
pub async fn run_diagnostics(
    state: State<'_, AppState>,
) -> Result<Vec<DiagnosticEntryDto>, String> {
    let portable_root = state.paths.data_root.join("codex-portable");
    tauri::async_runtime::spawn_blocking(move || {
        Ok(diagnose_windows_codex(&portable_root)
            .into_iter()
            .map(|entry| DiagnosticEntryDto {
                name: entry.name,
                result: entry.result,
            })
            .collect())
    })
    .await
    .map_err(|_| "Codex diagnostics were interrupted. Try again.".to_string())?
}

/// Roll back to the previous version in the pointer chain.
///
/// `version` is accepted because the history rows offer per-entry restore, but
/// only the immediately-previous version can be honoured today. Any other
/// request is rejected rather than silently restoring the wrong build.
#[tauri::command]
pub async fn rollback_runtime(
    version: Option<String>,
    state: State<'_, AppState>,
) -> Result<CodexOperationDto, String> {
    let portable_root = state.paths.data_root.join("codex-portable");
    if let Some(installed) = detect_windows_codex(&portable_root) {
        return match maintenance_route(Some(&installed)) {
            MaintenanceRoute::Portable => {
                let lock_path = state.paths.operation_lock();
                tauri::async_runtime::spawn_blocking(move || {
                    let lock = OperationLock::new(lock_path);
                    let _guard = lock
                        .try_acquire("rollback_runtime")
                        .map_err(|_| "Another Chimera++ operation is already running.".to_string())?;
                    rollback_portable_install(&portable_root)
                        .map(operation_dto)
                        .map_err(|error| error.to_string())
                })
                .await
                .map_err(|_| "Codex rollback was interrupted. Run Diagnose before retrying.".to_string())?
            }
            MaintenanceRoute::Standard => Err(
                "Standard MSIX installs are maintained by Windows and do not expose an app rollback backup. Choose Portable to keep local rollback copies."
                    .to_string(),
            ),
            MaintenanceRoute::NotInstalled => Err("Codex is not installed.".to_string()),
        };
    }

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
        Ok(pointer) => Ok(CodexOperationDto {
            version: pointer.active_version,
            requested_mode: "portable".to_string(),
            actual_mode: "portable".to_string(),
            affected_path: Some(state.paths.runtime_root().to_string_lossy().to_string()),
            backup_path: None,
            message: "Codex was restored to the previous managed version.".to_string(),
            notes: Vec::new(),
        }),
        Err(UpdateError::NoPreviousVersion) => {
            Err("There is no previous version to roll back to.".to_string())
        }
        Err(_) => Err("Rollback failed. Run diagnostics for detail.".to_string()),
    }
}

/// Uninstall the detected standard or portable Codex installation.
/// User configuration and provider credentials are deliberately preserved.
#[tauri::command]
pub async fn uninstall_codex(state: State<'_, AppState>) -> Result<CodexOperationDto, String> {
    let portable_root = state.paths.data_root.join("codex-portable");
    let lock_path = state.paths.operation_lock();
    tauri::async_runtime::spawn_blocking(move || {
        let lock = OperationLock::new(lock_path);
        let _guard = lock
            .try_acquire("uninstall_codex")
            .map_err(|_| "Another Chimera++ operation is already running.".to_string())?;
        uninstall_windows_codex(&portable_root)
            .map(operation_dto)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "Codex uninstall was interrupted. Run Diagnose before retrying.".to_string())?
}

/// Download, verify and install the selected Codex release.
#[tauri::command]
pub async fn apply_codex_update(
    app: tauri::AppHandle,
    version: Option<String>,
    source: Option<String>,
    install_mode: Option<String>,
    state: State<'_, AppState>,
) -> Result<CodexOperationDto, String> {
    let settings = state.settings();
    let source = source
        .unwrap_or(settings.codex_update_source)
        .parse::<UpdateSource>()
        .map_err(|_| "Choose Automatic or Mirror as the Codex update source.".to_string())?;
    let install_mode = install_mode
        .unwrap_or(settings.codex_install_mode)
        .parse::<InstallMode>()
        .map_err(|_| "Choose Standard or Portable as the Codex install mode.".to_string())?;
    let data_root = state.paths.data_root.clone();
    let lock_path = state.paths.operation_lock();

    tauri::async_runtime::spawn_blocking(move || {
        let lock = OperationLock::new(lock_path);
        let _guard = lock
            .try_acquire("apply_codex_update")
            .map_err(|_| "Another Chimera++ operation is already running.".to_string())?;
        let plan = fetch_windows_release_plan(source, Some(std::env::consts::ARCH))
            .map_err(|error| error.to_string())?;
        if version
            .as_deref()
            .is_some_and(|expected| expected != plan.version)
        {
            return Err(
                "The available Codex version changed after confirmation. Review it and try again."
                    .to_string(),
            );
        }
        let total = plan.size_bytes;
        let progress_app = app.clone();
        let progress = move |downloaded: u64| {
            let _ = progress_app.emit(
                "codex://download-progress",
                serde_json::json!({ "downloaded": downloaded, "total": total }),
            );
        };
        let result = install_windows_release(
            &plan,
            install_mode,
            &data_root.join("downloads"),
            &data_root.join("codex-portable"),
            &progress,
        )
        .map_err(|error| error.to_string())?;
        Ok(operation_dto(result))
    })
    .await
    .map_err(|_| {
        "The Codex installation was interrupted. Run Diagnose before retrying.".to_string()
    })?
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
