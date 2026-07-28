//! Codex desktop runtime management backed by the audited Chimera runtime crate.
//!
//! Provider projection and application installation intentionally remain
//! separate write domains. Every runtime mutation takes a cross-process lock
//! and requires explicit confirmation from the renderer.

use std::path::PathBuf;

use chimera_platform::lock::{LockGuard, OperationLock};
use chimera_runtime::manager::{
    detect_portable_codex, detect_windows_codex, diagnose_windows_codex,
    fetch_windows_release_plan, install_windows_release, latest_portable_rollback,
    maintenance_route, rollback_portable_install, uninstall_windows_codex, InstallMode,
    MaintenanceRoute, UpdateSource,
};
use serde::Serialize;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRuntimeDiagnostic {
    pub name: String,
    pub result: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRuntimeVersion {
    pub version: String,
    pub state: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRuntimeStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub install_mode: Option<String>,
    pub install_path: Option<String>,
    pub portable_root: String,
    pub can_repair: bool,
    pub can_rollback: bool,
    pub can_uninstall: bool,
    pub history: Vec<CodexRuntimeVersion>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexReleaseStatus {
    pub current_version: Option<String>,
    pub latest_version: String,
    pub package_version: String,
    pub update_available: bool,
    pub source: String,
    pub install_mode: String,
    pub size_bytes: u64,
    pub released_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRuntimeOperation {
    pub version: String,
    pub requested_mode: String,
    pub actual_mode: String,
    pub affected_path: Option<String>,
    pub backup_path: Option<String>,
    pub message: String,
    pub notes: Vec<String>,
}

fn runtime_root() -> PathBuf {
    crate::config::get_app_config_dir().join("codex-runtime")
}

fn portable_root() -> PathBuf {
    if let Some(configured) = crate::settings::get_settings().codex_portable_root {
        return PathBuf::from(configured);
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            let bundled = parent.join("Codex");
            if bundled.exists() {
                return bundled;
            }
        }
    }
    runtime_root().join("portable")
}

fn operation_lock() -> OperationLock {
    OperationLock::new(runtime_root().join("operation.lock"))
}

fn acquire_operation_lock(operation: &str) -> Result<LockGuard, String> {
    let root = runtime_root();
    std::fs::create_dir_all(&root).map_err(|_| "无法创建 Chimera++ 运行时目录".to_string())?;
    operation_lock()
        .try_acquire(operation)
        .map_err(|_| "另一个 Chimera++ 操作正在进行".to_string())
}

fn parse_source(value: Option<String>) -> Result<UpdateSource, String> {
    value
        .unwrap_or_else(|| crate::settings::get_settings().codex_update_source)
        .parse::<UpdateSource>()
        .map_err(|_| "更新源仅支持 auto 或 mirror".to_string())
}

fn parse_install_mode(value: Option<String>) -> Result<InstallMode, String> {
    value
        .unwrap_or_else(|| crate::settings::get_settings().codex_install_mode)
        .parse::<InstallMode>()
        .map_err(|_| "安装方式仅支持 standard 或 portable".to_string())
}

fn source_label(source: UpdateSource) -> String {
    match source {
        UpdateSource::Auto => "auto",
        UpdateSource::Mirror => "mirror",
    }
    .to_string()
}

fn mode_label(mode: InstallMode) -> String {
    match mode {
        InstallMode::Standard => "standard",
        InstallMode::Portable => "portable",
    }
    .to_string()
}

fn operation_dto(value: chimera_runtime::manager::InstallOperationResult) -> CodexRuntimeOperation {
    CodexRuntimeOperation {
        version: value.version,
        requested_mode: value.requested_mode,
        actual_mode: value.actual_mode,
        affected_path: value.affected_path,
        backup_path: value.backup_path,
        message: value.message,
        notes: value.notes,
    }
}

fn require_windows() -> Result<(), String> {
    if cfg!(target_os = "windows") {
        Ok(())
    } else {
        Err("当前运行时管理引擎仅支持 Windows".to_string())
    }
}

fn require_confirmation(confirm: bool, action: &str) -> Result<(), String> {
    if confirm {
        Ok(())
    } else {
        Err(format!("{action}需要用户明确确认"))
    }
}

/// Read installed Codex state without making a network request.
#[tauri::command]
pub async fn get_codex_runtime_status() -> Result<CodexRuntimeStatus, String> {
    require_windows()?;
    let portable_root = portable_root();
    tauri::async_runtime::spawn_blocking(move || {
        let installed = detect_windows_codex(&portable_root);
        let rollback = latest_portable_rollback(&portable_root).ok().flatten();
        let mut history = Vec::new();
        if let Some(current) = installed.as_ref() {
            history.push(CodexRuntimeVersion {
                version: current.version.clone(),
                state: "active",
            });
        }
        if let Some(previous) = rollback.as_deref().and_then(detect_portable_codex) {
            history.push(CodexRuntimeVersion {
                version: previous.version,
                state: "previous",
            });
        }
        let portable = installed
            .as_ref()
            .is_some_and(|value| value.install_mode == "portable");
        Ok(CodexRuntimeStatus {
            installed: installed.is_some(),
            version: installed.as_ref().map(|value| value.version.clone()),
            install_mode: installed.as_ref().map(|value| value.install_mode.clone()),
            install_path: installed.as_ref().map(|value| value.path.clone()),
            portable_root: portable_root.to_string_lossy().to_string(),
            can_repair: installed.is_some(),
            can_rollback: portable && rollback.is_some(),
            can_uninstall: installed.is_some(),
            history,
        })
    })
    .await
    .map_err(|_| "读取 Codex 安装状态时任务中断".to_string())?
}

/// Explicitly query the selected Codex release source.
#[tauri::command]
pub async fn check_codex_runtime_update(
    source: Option<String>,
    install_mode: Option<String>,
) -> Result<CodexReleaseStatus, String> {
    require_windows()?;
    let source = parse_source(source)?;
    let install_mode = parse_install_mode(install_mode)?;
    let portable_root = portable_root();
    tauri::async_runtime::spawn_blocking(move || {
        let installed = detect_windows_codex(&portable_root);
        let current_version = installed.as_ref().map(|value| value.version.clone());
        let plan = fetch_windows_release_plan(source, Some(std::env::consts::ARCH))
            .map_err(|error| error.to_string())?;
        Ok(CodexReleaseStatus {
            update_available: plan.is_update_available(current_version.as_deref()),
            current_version,
            latest_version: plan.version,
            package_version: plan.package_version,
            source: source_label(source),
            install_mode: mode_label(install_mode),
            size_bytes: plan.size_bytes,
            released_at: plan.released_at,
        })
    })
    .await
    .map_err(|_| "检查 Codex 更新时任务中断".to_string())?
}

/// Run installation and launch diagnostics only after a user action.
#[tauri::command]
pub async fn diagnose_codex_runtime() -> Result<Vec<CodexRuntimeDiagnostic>, String> {
    require_windows()?;
    let portable_root = portable_root();
    tauri::async_runtime::spawn_blocking(move || {
        Ok(diagnose_windows_codex(&portable_root)
            .into_iter()
            .map(|entry| CodexRuntimeDiagnostic {
                name: entry.name,
                result: entry.result,
            })
            .collect())
    })
    .await
    .map_err(|_| "Codex 诊断任务中断".to_string())?
}

async fn install_release(
    app: tauri::AppHandle,
    expected_version: Option<String>,
    source: Option<String>,
    install_mode: Option<String>,
) -> Result<CodexRuntimeOperation, String> {
    require_windows()?;
    let source = parse_source(source)?;
    let install_mode = parse_install_mode(install_mode)?;
    let root = runtime_root();
    let portable_root = portable_root();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = acquire_operation_lock("codex_runtime_install")?;
        let plan = fetch_windows_release_plan(source, Some(std::env::consts::ARCH))
            .map_err(|error| error.to_string())?;
        if expected_version
            .as_deref()
            .is_some_and(|expected| expected != plan.version)
        {
            return Err("确认后可用版本发生变化，请重新检查更新".to_string());
        }
        let total = plan.size_bytes;
        let progress_app = app.clone();
        let progress = move |downloaded: u64| {
            let _ = progress_app.emit(
                "codex-runtime-download-progress",
                serde_json::json!({ "downloaded": downloaded, "total": total }),
            );
        };
        install_windows_release(
            &plan,
            install_mode,
            &root.join("downloads"),
            &portable_root,
            &progress,
        )
        .map(operation_dto)
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "Codex 安装任务中断，请先运行诊断".to_string())?
}

/// Install or update Codex after the renderer has shown a confirmation dialog.
#[tauri::command]
pub async fn apply_codex_runtime_update(
    app: tauri::AppHandle,
    expected_version: Option<String>,
    source: Option<String>,
    install_mode: Option<String>,
    confirm: bool,
) -> Result<CodexRuntimeOperation, String> {
    require_confirmation(confirm, "安装或更新 Codex")?;
    install_release(app, expected_version, source, install_mode).await
}

/// Repair Codex by reinstalling a newly verified package in the detected mode.
#[tauri::command]
pub async fn repair_codex_runtime(
    app: tauri::AppHandle,
    source: Option<String>,
    install_mode: Option<String>,
    confirm: bool,
) -> Result<CodexRuntimeOperation, String> {
    require_confirmation(confirm, "修复 Codex")?;
    require_windows()?;
    let installed = detect_windows_codex(&portable_root())
        .ok_or_else(|| "未检测到 Codex，请先执行安装".to_string())?;
    install_release(
        app,
        None,
        source,
        install_mode.or(Some(installed.install_mode)),
    )
    .await
}

/// Restore the latest portable backup. Standard MSIX has no local rollback slot.
#[tauri::command]
pub async fn rollback_codex_runtime(confirm: bool) -> Result<CodexRuntimeOperation, String> {
    require_confirmation(confirm, "回滚 Codex")?;
    require_windows()?;
    let portable_root = portable_root();
    tauri::async_runtime::spawn_blocking(move || {
        let installed =
            detect_windows_codex(&portable_root).ok_or_else(|| "未检测到 Codex".to_string())?;
        if maintenance_route(Some(&installed)) != MaintenanceRoute::Portable {
            return Err("标准安装由 Windows 管理，没有本地回滚副本".to_string());
        }
        let _guard = acquire_operation_lock("codex_runtime_rollback")?;
        rollback_portable_install(&portable_root)
            .map(operation_dto)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "Codex 回滚任务中断，请运行诊断".to_string())?
}

/// Uninstall Codex while preserving the user's `~/.codex` data.
#[tauri::command]
pub async fn uninstall_codex_runtime(confirm: bool) -> Result<CodexRuntimeOperation, String> {
    require_confirmation(confirm, "卸载 Codex")?;
    require_windows()?;
    let portable_root = portable_root();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = acquire_operation_lock("codex_runtime_uninstall")?;
        uninstall_windows_codex(&portable_root)
            .map(operation_dto)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "Codex 卸载任务中断，请运行诊断".to_string())?
}
