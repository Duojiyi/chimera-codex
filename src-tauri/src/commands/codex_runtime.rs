//! Codex desktop runtime management backed by the audited Chimera runtime crate.
//!
//! Provider projection and application installation intentionally remain
//! separate write domains. Every runtime mutation takes a cross-process lock
//! and requires explicit confirmation from the renderer.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chimera_platform::lock::{LockGuard, OperationLock};
use chimera_runtime::manager::{
    detect_portable_codex, detect_windows_codex, diagnose_windows_codex,
    fetch_windows_release_plan, install_windows_release, latest_portable_rollback,
    maintenance_route, rollback_portable_install, uninstall_windows_codex, InstallMode,
    MaintenanceRoute, UpdateSource,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_opener::OpenerExt;

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
    pub supported: bool,
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
pub struct CodexProcessStatus {
    pub supported: bool,
    pub installed: bool,
    pub running: bool,
    pub install_mode: Option<String>,
    pub official_login_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexLaunchResult {
    pub was_running: bool,
    pub running: bool,
    pub action: &'static str,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexModelCatalogStatus {
    pub valid: bool,
    pub default_model: String,
    pub catalog_path: Option<String>,
    pub model_count: usize,
}

fn runtime_root() -> PathBuf {
    crate::config::get_app_config_dir().join("codex-runtime")
}

fn portable_root() -> Result<PathBuf, String> {
    crate::settings::resolve_codex_portable_root()
}

fn process_install_mode(source: &str) -> String {
    if source == "portable" {
        "portable"
    } else {
        "standard"
    }
    .to_string()
}

fn codex_is_running(installed: &codex_win_engine::InstalledWindowsCodex) -> Result<bool, String> {
    codex_win_engine::codex_running_for_root(Path::new(&installed.path))
        .map_err(|error| format!("无法读取 Codex 进程状态: {error}"))
}

fn launch_action(was_running: bool) -> &'static str {
    if was_running {
        "restarted"
    } else {
        "launched"
    }
}

fn official_login_available() -> bool {
    let path = crate::codex_config::get_codex_auth_path();
    let Ok(contents) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(auth) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return false;
    };
    crate::codex_config::codex_auth_has_oauth_login_material(&auth)
}

fn wait_for_codex_running(
    installed: &codex_win_engine::InstalledWindowsCodex,
) -> Result<bool, String> {
    for _ in 0..8 {
        if codex_is_running(installed)? {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(false)
}

fn close_codex_for_restart(
    installed: &codex_win_engine::InstalledWindowsCodex,
    portable_root: &Path,
) -> Result<(), String> {
    if installed.source == "msix" {
        codex_win_engine::close_msix_codex_processes(30)
            .map_err(|error| format!("无法关闭 Codex: {error}"))?;
    } else {
        codex_win_engine::close_codex_gracefully_for_root(30, portable_root)
            .map_err(|error| format!("无法关闭 Codex: {error}"))?;
    }
    if codex_is_running(installed)? {
        return Err("Codex 未能完全退出，已取消重启".to_string());
    }
    Ok(())
}

/// Verify that the live Codex config points at Chimera's catalog and contains
/// the selected default model. This command is read-only.
#[tauri::command]
pub fn verify_codex_model_catalog(
    expected_model: String,
) -> Result<CodexModelCatalogStatus, String> {
    let expected_model = expected_model.trim();
    if expected_model.is_empty() {
        return Err("默认模型不能为空".to_string());
    }
    let config_text = crate::codex_config::read_codex_config_text().map_err(|e| e.to_string())?;
    let config = config_text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("Codex 配置无法解析: {e}"))?;
    let default_model = config
        .get("model")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if default_model != expected_model {
        return Err(format!(
            "Codex 默认模型未正确写入（当前为 {default_model}）"
        ));
    }

    let generated_path = crate::codex_config::get_codex_model_catalog_path();
    let catalog_path =
        crate::codex_config::resolve_cc_switch_catalog_path(&config_text, &generated_path)
            .ok_or_else(|| "Codex 配置未引用 Chimera 模型目录".to_string())?;
    let catalog_text = std::fs::read_to_string(&catalog_path)
        .map_err(|_| "Chimera 模型目录文件不存在或无法读取".to_string())?;
    let catalog: serde_json::Value = serde_json::from_str(&catalog_text)
        .map_err(|e| format!("Chimera 模型目录无法解析: {e}"))?;
    let models = catalog
        .get("models")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Chimera 模型目录缺少 models 列表".to_string())?;
    let contains_default = models
        .iter()
        .any(|entry| entry.get("slug").and_then(serde_json::Value::as_str) == Some(expected_model));
    if !contains_default {
        return Err(format!("模型目录中没有默认模型 {expected_model}"));
    }
    Ok(CodexModelCatalogStatus {
        valid: true,
        default_model,
        catalog_path: Some(catalog_path.to_string_lossy().to_string()),
        model_count: models.len(),
    })
}

/// Restart Codex after an explicit renderer confirmation so it reloads the
/// startup-only model catalog. Reuses Codex App Manager's install detection.
#[tauri::command]
pub async fn restart_codex_for_model_catalog(confirm: bool) -> Result<(), String> {
    require_confirmation(confirm, "重启 Codex 以刷新模型列表")?;
    require_windows()?;
    let portable_root = portable_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = acquire_operation_lock("codex_runtime_restart_catalog")?;
        let installed = codex_win_engine::detect_installed_codex(&portable_root)
            .ok_or_else(|| "未检测到 Codex 安装".to_string())?;
        close_codex_for_restart(&installed, &portable_root)?;
        codex_win_engine::launch_codex(&installed).map_err(|e| format!("无法重新启动 Codex: {e}"))
    })
    .await
    .map_err(|e| format!("Codex 重启任务中断: {e}"))?
}

/// Read whether the exact detected Codex installation currently owns a process.
/// Process discovery is path-pinned so an unrelated ChatGPT installation is not
/// mistaken for, or affected as, the managed Codex instance.
#[tauri::command]
pub async fn get_codex_process_status() -> Result<CodexProcessStatus, String> {
    if !cfg!(target_os = "windows") {
        return Ok(CodexProcessStatus {
            supported: false,
            installed: false,
            running: false,
            install_mode: None,
            official_login_available: official_login_available(),
        });
    }
    let portable_root = portable_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(installed) = codex_win_engine::detect_installed_codex(&portable_root) else {
            return Ok(CodexProcessStatus {
                supported: true,
                installed: false,
                running: false,
                install_mode: None,
                official_login_available: official_login_available(),
            });
        };
        Ok(CodexProcessStatus {
            supported: true,
            installed: true,
            running: codex_is_running(&installed)?,
            install_mode: Some(process_install_mode(&installed.source)),
            official_login_available: official_login_available(),
        })
    })
    .await
    .map_err(|error| format!("读取 Codex 进程状态时任务中断: {error}"))?
}

/// Launch the managed Codex installation, replacing a running managed instance
/// first. The deprecated optional flag is accepted only for IPC compatibility:
/// lifecycle policy is backend-owned and a running target is always restarted.
///
/// The cross-process lock covers discovery, shutdown, launch, and health
/// verification. Process discovery and termination are path-pinned in
/// `codex_win_engine`, so unrelated Electron or ChatGPT processes are never
/// selected.
#[tauri::command]
pub async fn open_codex_runtime(
    restart_if_running: Option<bool>,
) -> Result<CodexLaunchResult, String> {
    // Keep accepting the old renderer argument without allowing it to weaken
    // the safe restart policy. The renderer submits launch intent only.
    let _legacy_restart_preference = restart_if_running;
    require_windows()?;
    let portable_root = portable_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = acquire_operation_lock("codex_runtime_launch")?;
        let installed = codex_win_engine::detect_installed_codex(&portable_root)
            .ok_or_else(|| "未检测到 Codex 安装".to_string())?;
        let was_running = codex_is_running(&installed)?;
        if was_running {
            close_codex_for_restart(&installed, &portable_root)?;
        }
        if let Err(error) = codex_win_engine::launch_codex(&installed) {
            // A second portable Electron process may exit after handing focus
            // to an already-running instance. We intentionally reject that
            // path here: a previous target was either absent or confirmed
            // closed, so accepting it could hide a failed replacement.
            return Err(format!("无法启动 Codex: {error}"));
        }
        let running = wait_for_codex_running(&installed)?;
        if !running {
            return Err("Codex 启动后未保持运行，请在更新页运行诊断".to_string());
        }
        Ok(CodexLaunchResult {
            was_running,
            running,
            action: launch_action(was_running),
        })
    })
    .await
    .map_err(|error| format!("Codex 启动任务中断: {error}"))?
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
    let portable_root = portable_root()?;
    if !cfg!(target_os = "windows") {
        return Ok(CodexRuntimeStatus {
            supported: false,
            installed: false,
            version: None,
            install_mode: None,
            install_path: None,
            portable_root: portable_root.to_string_lossy().to_string(),
            can_repair: false,
            can_rollback: false,
            can_uninstall: false,
            history: Vec::new(),
        });
    }
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
            supported: true,
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

#[tauri::command]
pub async fn open_codex_runtime_directory(handle: AppHandle) -> Result<bool, String> {
    require_windows()?;
    let installed =
        detect_windows_codex(&portable_root()?).ok_or_else(|| "未检测到 Codex 安装".to_string())?;
    let path = PathBuf::from(installed.path);
    let directory = if path.is_file() {
        path.parent().map(PathBuf::from).unwrap_or(path)
    } else {
        path
    };
    handle
        .opener()
        .open_path(directory.to_string_lossy().to_string(), None::<String>)
        .map_err(|error| format!("打开安装目录失败: {error}"))?;
    Ok(true)
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
    let portable_root = portable_root()?;
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
    let portable_root = portable_root()?;
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
    let portable_root = portable_root()?;
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
    let installed = detect_windows_codex(&portable_root()?)
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
    let portable_root = portable_root()?;
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
    let portable_root = portable_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = acquire_operation_lock("codex_runtime_uninstall")?;
        uninstall_windows_codex(&portable_root)
            .map(operation_dto)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|_| "Codex 卸载任务中断，请运行诊断".to_string())?
}

#[cfg(test)]
mod tests {
    use super::{launch_action, process_install_mode, CodexProcessStatus, CodexRuntimeStatus};

    #[test]
    fn launch_action_replaces_a_running_instance() {
        assert_eq!(launch_action(false), "launched");
        assert_eq!(launch_action(true), "restarted");
    }

    #[test]
    fn process_install_mode_keeps_customer_labels_stable() {
        assert_eq!(process_install_mode("portable"), "portable");
        assert_eq!(process_install_mode("msix"), "standard");
    }

    #[test]
    fn process_status_serializes_as_renderer_contract() {
        let status = CodexProcessStatus {
            supported: true,
            installed: true,
            running: false,
            install_mode: Some("portable".to_string()),
            official_login_available: true,
        };
        let json = serde_json::to_value(status).expect("status serializes");
        assert_eq!(json["installed"], true);
        assert_eq!(json["supported"], true);
        assert_eq!(json["running"], false);
        assert_eq!(json["installMode"], "portable");
        assert_eq!(json["officialLoginAvailable"], true);
        assert!(json.get("install_mode").is_none());
    }

    #[test]
    fn unsupported_runtime_status_is_explicit_and_non_actionable() {
        let status = CodexRuntimeStatus {
            supported: false,
            installed: false,
            version: None,
            install_mode: None,
            install_path: None,
            portable_root: String::new(),
            can_repair: false,
            can_rollback: false,
            can_uninstall: false,
            history: Vec::new(),
        };
        let json = serde_json::to_value(status).expect("status serializes");
        assert_eq!(json["supported"], false);
        assert_eq!(json["installed"], false);
        assert_eq!(json["canRepair"], false);
        assert_eq!(json["canRollback"], false);
        assert_eq!(json["canUninstall"], false);
    }
}
