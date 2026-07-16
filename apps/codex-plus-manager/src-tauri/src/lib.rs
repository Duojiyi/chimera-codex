pub mod commands;
pub mod install;

use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};

const TRAY_ID: &str = "codex_plus_tray";

static APP_EXITING: AtomicBool = AtomicBool::new(false);
const TRAY_MENU_SHOW: &str = "tray_show_main";
const TRAY_MENU_QUIT: &str = "tray_quit_app";
const START_ROUTE_EVENT: &str = "chimera-start-route";
const MAX_START_ROUTE_BYTES: u64 = 256;
const START_ROUTE_MAX_AGE_MS: u128 = 30_000;
const START_ROUTE_ACK_TIMEOUT: Duration = Duration::from_secs(2);
const START_ROUTE_ARTIFACT_MAX_AGE: Duration = Duration::from_secs(30);

#[derive(Default)]
struct StartRouteState {
    frontend_ready: bool,
    pending: VecDeque<String>,
}

impl StartRouteState {
    fn queue_or_emit(&mut self, route: &str) -> bool {
        if self.frontend_ready {
            true
        } else {
            self.pending.push_back(route.to_string());
            false
        }
    }

    fn next_pending_or_mark_ready(&mut self) -> Option<String> {
        if let Some(route) = self.pending.pop_front() {
            Some(route)
        } else {
            self.frontend_ready = true;
            None
        }
    }

    fn mark_not_ready(&mut self) {
        self.frontend_ready = false;
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartRouteRequest {
    id: String,
    route: String,
    created_at_ms: u128,
}

struct StartRouteTicket {
    request_path: PathBuf,
    ack_path: PathBuf,
}

static START_ROUTE_STATE: Mutex<StartRouteState> = Mutex::new(StartRouteState {
    frontend_ready: false,
    pending: VecDeque::new(),
});

pub fn run() {
    install_panic_logger();
    let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
        "manager.start",
        serde_json::json!({
            "version": env!("CARGO_PKG_VERSION")
        }),
    );
    let show_update = commands::startup_should_show_update();
    let start_route = std::env::var("CODEX_PLUS_START_ROUTE").ok();
    let route_request = if show_update {
        "update"
    } else {
        start_route
            .as_deref()
            .and_then(normalize_start_route_message)
            .unwrap_or("show")
    };
    let _guard = match acquire_single_instance_guard() {
        Ok(Some(guard)) => guard,
        Ok(None) => {
            if let Err(error) = forward_start_route_to_existing_manager(route_request) {
                let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
                    "manager.route_forward_failed",
                    serde_json::json!({ "failed": true, "kind": format!("{:?}", error.kind()) }),
                );
            }
            return;
        }
        Err(error) => {
            let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
                "manager.guard_failed",
                serde_json::json!({
                    "guard_port": codex_plus_core::ports::manager_guard_port(),
                    "kind": format!("{:?}", error.kind())
                }),
            );
            return;
        }
    };
    let run_result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let url = if show_update {
                "/index.html?showUpdate=1"
            } else if start_route.as_deref() == Some("maintenance") {
                "/index.html?startRoute=maintenance"
            } else if start_route.as_deref() == Some("relay") {
                "/index.html?startRoute=relay"
            } else {
                "/index.html"
            };
            let mut main_window_builder =
                tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App(url.into()))
                    .title(codex_plus_core::branding::DISPLAY_MANAGER_NAME)
                    .inner_size(1180.0, 820.0)
                    .min_inner_size(960.0, 720.0);
            if let Some(icon) = app.default_window_icon().cloned() {
                main_window_builder = main_window_builder.icon(icon)?;
            }
            let main_window = main_window_builder.build()?;
            install_tray(app)?;
            register_main_window_events(main_window);
            start_existing_manager_route_listener(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::backend_version,
            commands::startup_options,
            commands::load_overview,
            commands::launch_codex_plus,
            commands::launch_after_optional_update_failure,
            commands::restart_codex_plus,
            commands::load_settings,
            commands::save_settings,
            commands::load_ccs_providers,
            commands::import_ccs_providers,
            commands::load_pending_provider_import,
            commands::confirm_pending_provider_import,
            commands::dismiss_pending_provider_import,
            commands::list_local_sessions,
            commands::list_zed_remote_projects,
            commands::open_zed_remote,
            commands::forget_zed_remote_project,
            commands::delete_local_session,
            commands::load_provider_sync_targets,
            commands::preview_session_index_cleanup,
            commands::apply_session_index_cleanup,
            commands::sync_providers_now,
            commands::refresh_script_market,
            commands::install_market_script,
            commands::set_user_script_enabled,
            commands::delete_user_script,
            commands::open_external_url,
            commands::open_applications_folder,
            commands::install_entrypoints,
            commands::uninstall_entrypoints,
            commands::repair_shortcuts,
            commands::plugin_marketplace_status,
            commands::repair_plugin_marketplace,
            commands::remote_plugin_marketplace_status,
            commands::repair_remote_plugin_marketplace,
            commands::check_update,
            commands::perform_update,
            commands::load_watcher_state,
            commands::install_watcher,
            commands::uninstall_watcher,
            commands::enable_watcher,
            commands::disable_watcher,
            commands::read_latest_logs,
            commands::copy_diagnostics,
            commands::reset_settings,
            commands::reset_image_overlay_settings,
            commands::relay_status,
            commands::read_relay_files,
            commands::check_env_conflicts,
            commands::check_relay_environment,
            commands::remove_env_conflicts,
            commands::save_relay_file,
            commands::write_diagnostic_event,
            commands::backfill_relay_profile_from_live,
            commands::list_context_entries,
            commands::read_live_context_entries,
            commands::sync_live_context_entries,
            commands::upsert_context_entry,
            commands::delete_context_entry,
            commands::extract_relay_common_config,
            commands::test_relay_profile,
            commands::measure_relay_latency,
            commands::diagnose_relay_profile,
            commands::test_stepwise_settings,
            commands::fetch_relay_profile_models,
            commands::switch_relay_profile,
            commands::save_and_enable_chimera_hub,
            commands::apply_relay_injection,
            commands::apply_pure_api_injection,
            commands::clear_relay_injection,
            manager_exit_app,
            manager_hide_to_tray,
            manager_frontend_ready,
            manager_frontend_not_ready,
            update_tray_labels
        ])
        .run(tauri::generate_context!());
    if let Err(error) = run_result {
        let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
            "manager.run_failed",
            serde_json::json!({
                "error": error.to_string()
            }),
        );
    }
}

fn normalize_start_route_message(route: &str) -> Option<&str> {
    matches!(route, "update" | "maintenance" | "relay" | "show").then_some(route)
}

fn forward_start_route_to_existing_manager(route: &str) -> std::io::Result<()> {
    let route = normalize_start_route_message(route)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid route"))?;
    let request_dir = start_route_request_dir();
    let ticket = write_start_route_request(&request_dir, route)?;
    wait_for_start_route_ack(&ticket, START_ROUTE_ACK_TIMEOUT)
}

fn wait_for_start_route_ack(ticket: &StartRouteTicket, timeout: Duration) -> std::io::Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if ticket.ack_path.exists() {
            let _ = std::fs::remove_file(&ticket.ack_path);
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let request_state = if ticket.request_path.exists() {
        "pending"
    } else {
        "claimed"
    };
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        format!("manager route acknowledgement timed out ({request_state})"),
    ))
}

fn start_route_request_dir() -> PathBuf {
    codex_plus_core::paths::default_app_state_dir().join("manager-route-requests")
}

fn current_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn write_start_route_request(dir: &Path, route: &str) -> std::io::Result<StartRouteTicket> {
    std::fs::create_dir_all(dir)?;
    let id = format!("{}-{}", std::process::id(), current_time_ms());
    let request = StartRouteRequest {
        id: id.clone(),
        route: route.to_string(),
        created_at_ms: current_time_ms(),
    };
    let request_path = dir.join(format!("{id}.json"));
    let temporary_path = dir.join(format!("{id}.tmp"));
    let ack_path = dir.join(format!("{id}.ack"));
    let publish_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        serde_json::to_writer(&mut file, &request).map_err(std::io::Error::other)?;
        file.flush()?;
        file.sync_all()?;
        std::fs::rename(&temporary_path, &request_path)
    })();
    if let Err(error) = publish_result {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(error);
    }
    Ok(StartRouteTicket {
        request_path,
        ack_path,
    })
}

fn take_start_route_requests(dir: &Path) -> std::io::Result<Vec<(StartRouteRequest, PathBuf)>> {
    let mut requests = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(requests),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.len() > MAX_START_ROUTE_BYTES {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let request = match serde_json::from_slice::<StartRouteRequest>(&bytes) {
            Ok(request) => request,
            Err(_) => {
                let _ = std::fs::remove_file(&path);
                continue;
            }
        };
        let stem_matches =
            path.file_stem().and_then(|value| value.to_str()) == Some(request.id.as_str());
        let fresh =
            current_time_ms().saturating_sub(request.created_at_ms) <= START_ROUTE_MAX_AGE_MS;
        if !stem_matches || !fresh || normalize_start_route_message(&request.route).is_none() {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        let claimed_path = path.with_extension("processing");
        match std::fs::rename(&path, &claimed_path) {
            Ok(()) => requests.push((request, claimed_path)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = std::fs::remove_file(&path);
            }
            Err(_) => continue,
        }
    }
    requests.sort_by(|(left, _), (right, _)| {
        left.created_at_ms
            .cmp(&right.created_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(requests)
}

fn cleanup_stale_start_route_artifacts(dir: &Path, max_age: Duration) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        let extension = path.extension().and_then(|value| value.to_str());
        if !matches!(extension, Some("tmp" | "processing" | "ack")) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let age = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .unwrap_or_default();
        if age >= max_age {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

fn dispatch_start_route<R: tauri::Runtime>(app: &tauri::AppHandle<R>, route: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let should_emit = START_ROUTE_STATE
        .lock()
        .map(|mut state| state.queue_or_emit(route))
        .unwrap_or(false);
    if should_emit {
        let _ = app.emit(START_ROUTE_EVENT, route);
    }
}

fn start_existing_manager_route_listener<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    std::thread::spawn(move || {
        let dir = start_route_request_dir();
        let _ = cleanup_stale_start_route_artifacts(&dir, START_ROUTE_ARTIFACT_MAX_AGE);
        let mut last_cleanup = std::time::Instant::now();
        loop {
            if last_cleanup.elapsed() >= START_ROUTE_ARTIFACT_MAX_AGE {
                let _ = cleanup_stale_start_route_artifacts(&dir, START_ROUTE_ARTIFACT_MAX_AGE);
                last_cleanup = std::time::Instant::now();
            }
            if let Ok(requests) = take_start_route_requests(&dir) {
                for (request, path) in requests {
                    dispatch_start_route(&app, &request.route);
                    let _ = std::fs::remove_file(path);
                    let _ = std::fs::write(dir.join(format!("{}.ack", request.id)), b"ok");
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
}

#[tauri::command]
fn manager_frontend_ready(app: tauri::AppHandle) {
    loop {
        let pending = START_ROUTE_STATE
            .lock()
            .ok()
            .and_then(|mut state| state.next_pending_or_mark_ready());
        let Some(route) = pending else {
            break;
        };
        let _ = app.emit(START_ROUTE_EVENT, route);
    }
}

#[tauri::command]
fn manager_frontend_not_ready() {
    if let Ok(mut state) = START_ROUTE_STATE.lock() {
        state.mark_not_ready();
    }
}

fn install_tray<R: tauri::Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, TRAY_MENU_SHOW, "显示主窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, TRAY_MENU_QUIT, "退出程序", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            TRAY_MENU_SHOW => {
                show_main_window(app);
            }
            TRAY_MENU_QUIT => {
                APP_EXITING.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                show_main_window(&tray.app_handle());
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }

    let _ = tray_builder.build(app)?;
    Ok(())
}

fn register_main_window_events<R: tauri::Runtime>(window: tauri::WebviewWindow<R>) {
    let event_window = window.clone();
    let minimized_window = event_window.clone();
    let close_event_window = event_window.clone();

    event_window.on_window_event(move |event| match event {
        WindowEvent::Resized(_) => {
            if matches!(minimized_window.is_minimized(), Ok(true)) {
                let _ = minimized_window.hide();
            }
        }
        WindowEvent::CloseRequested { api, .. } => {
            if APP_EXITING.load(Ordering::SeqCst) {
                return;
            }

            api.prevent_close();
            let _ = close_event_window.hide();
        }
        _ => {}
    });
}

#[tauri::command]
fn manager_exit_app<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    APP_EXITING.store(true, Ordering::SeqCst);
    app.exit(0);
}

#[tauri::command]
fn manager_hide_to_tray<R: tauri::Runtime>(window: tauri::WebviewWindow<R>) {
    let _ = window.hide();
}

#[tauri::command]
fn update_tray_labels<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    show_label: String,
    quit_label: String,
    window_title: String,
) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let show_item = MenuItem::with_id(&app, TRAY_MENU_SHOW, &show_label, true, None::<&str>);
        let quit_item = MenuItem::with_id(&app, TRAY_MENU_QUIT, &quit_label, true, None::<&str>);
        if let (Ok(show), Ok(quit)) = (show_item, quit_item) {
            if let Ok(menu) = Menu::with_items(&app, &[&show, &quit]) {
                let _ = tray.set_menu(Some(menu));
            }
        }
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title(&window_title);
    }
}

fn show_main_window<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Restores and focuses an existing manager window on Windows.
///
/// This is a no-op on other platforms.
pub fn focus_existing_manager_window() {
    #[cfg(windows)]
    {
        let current_process_id = std::process::id();
        for process in codex_plus_core::windows_enumerate_processes() {
            if process.process_id == current_process_id {
                continue;
            }
            if process
                .exe_file
                .eq_ignore_ascii_case("codex-plus-plus-manager.exe")
            {
                let _ = codex_plus_core::windows_activate_process_window(process.process_id);
                break;
            }
        }
    }
}

fn install_panic_logger() {
    std::panic::set_hook(Box::new(|panic_info| {
        let payload = panic_info
            .payload()
            .downcast_ref::<&str>()
            .map(|message| (*message).to_string())
            .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "非字符串 panic payload".to_string());
        let location = panic_info.location().map(|location| {
            serde_json::json!({
                "file": location.file(),
                "line": location.line(),
                "column": location.column()
            })
        });
        let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
            "manager.panic",
            serde_json::json!({
                "payload": payload,
                "location": location
            }),
        );
    }));
}

fn classify_single_instance_guard_result<T>(
    result: std::io::Result<T>,
) -> std::io::Result<Option<T>> {
    match result {
        Ok(guard) => Ok(Some(guard)),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::AddrInUse | std::io::ErrorKind::WouldBlock
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn acquire_single_instance_guard()
-> std::io::Result<Option<codex_plus_core::ports::LoopbackPortGuard>> {
    let result = classify_single_instance_guard_result(
        codex_plus_core::ports::acquire_resilient_loopback_port_guard(
            codex_plus_core::ports::manager_guard_port(),
        ),
    );
    match &result {
        Ok(Some(guard)) => {
            if let Some(fallback_lock_path) = guard.fallback_path() {
                let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
                    "manager.guard_fallback",
                    serde_json::json!({
                        "requested_guard_port": codex_plus_core::ports::manager_guard_port(),
                        "fallback_lock_path": fallback_lock_path
                    }),
                );
            }
        }
        Ok(None) => {
            let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
                "manager.already_running",
                serde_json::json!({
                    "guard_port": codex_plus_core::ports::manager_guard_port()
                }),
            );
            focus_existing_manager_window();
        }
        Err(_) => {}
    }
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn single_instance_guard_only_treats_lock_conflicts_as_existing_instance() {
        assert_eq!(
            super::classify_single_instance_guard_result::<u8>(Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "held",
            )))
            .unwrap(),
            None
        );
        assert_eq!(
            super::classify_single_instance_guard_result(Ok(7_u8)).unwrap(),
            Some(7)
        );
        let fatal = super::classify_single_instance_guard_result::<u8>(Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        )))
        .unwrap_err();
        assert_eq!(fatal.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn start_route_state_preserves_all_pending_routes_until_frontend_ready() {
        let mut state = super::StartRouteState::default();

        assert!(!state.queue_or_emit("relay"));
        assert!(!state.queue_or_emit("maintenance"));
        assert_eq!(
            state.next_pending_or_mark_ready(),
            Some("relay".to_string())
        );
        assert!(!state.frontend_ready);
        assert!(!state.queue_or_emit("update"));
        assert_eq!(
            state.next_pending_or_mark_ready(),
            Some("maintenance".to_string())
        );
        assert_eq!(
            state.next_pending_or_mark_ready(),
            Some("update".to_string())
        );
        assert_eq!(state.next_pending_or_mark_ready(), None);
        assert!(state.frontend_ready);
        assert!(state.queue_or_emit("update"));
        state.mark_not_ready();
        assert!(!state.frontend_ready);
        assert!(!state.queue_or_emit("show"));
        assert_eq!(state.next_pending_or_mark_ready(), Some("show".to_string()));
    }

    #[test]
    fn start_route_request_round_trip_is_atomic_and_validated() {
        let temp = tempfile::tempdir().unwrap();
        let ticket = super::write_start_route_request(temp.path(), "relay").unwrap();

        assert!(ticket.request_path.exists());
        assert!(!temp.path().read_dir().unwrap().any(|entry| {
            entry
                .unwrap()
                .path()
                .extension()
                .and_then(|value| value.to_str())
                == Some("tmp")
        }));
        let requests = super::take_start_route_requests(temp.path()).unwrap();

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0.route, "relay");
        assert_eq!(
            requests[0].1.extension().and_then(|value| value.to_str()),
            Some("processing")
        );
        assert!(!ticket.request_path.exists());
    }

    #[test]
    fn start_route_ack_timeout_keeps_request_for_slow_primary_startup() {
        let temp = tempfile::tempdir().unwrap();
        let ticket = super::write_start_route_request(temp.path(), "relay").unwrap();

        let error =
            super::wait_for_start_route_ack(&ticket, std::time::Duration::ZERO).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(ticket.request_path.exists());
    }

    #[test]
    fn start_route_request_rejects_stale_and_untrusted_payloads() {
        let temp = tempfile::tempdir().unwrap();
        let stale = super::StartRouteRequest {
            id: "stale".to_string(),
            route: "relay".to_string(),
            created_at_ms: 0,
        };
        std::fs::write(
            temp.path().join("stale.json"),
            serde_json::to_vec(&stale).unwrap(),
        )
        .unwrap();
        let invalid = super::StartRouteRequest {
            id: "invalid".to_string(),
            route: "https://example.invalid".to_string(),
            created_at_ms: super::current_time_ms(),
        };
        std::fs::write(
            temp.path().join("invalid.json"),
            serde_json::to_vec(&invalid).unwrap(),
        )
        .unwrap();

        assert!(
            super::take_start_route_requests(temp.path())
                .unwrap()
                .is_empty()
        );
        assert!(!temp.path().join("stale.json").exists());
        assert!(!temp.path().join("invalid.json").exists());
    }

    #[test]
    fn start_route_requests_are_consumed_in_stable_creation_order() {
        let temp = tempfile::tempdir().unwrap();
        let now = super::current_time_ms();
        let newer = super::StartRouteRequest {
            id: "a-newer".to_string(),
            route: "maintenance".to_string(),
            created_at_ms: now,
        };
        let older = super::StartRouteRequest {
            id: "z-older".to_string(),
            route: "relay".to_string(),
            created_at_ms: now.saturating_sub(1),
        };
        std::fs::write(
            temp.path().join("a-newer.json"),
            serde_json::to_vec(&newer).unwrap(),
        )
        .unwrap();
        std::fs::write(
            temp.path().join("z-older.json"),
            serde_json::to_vec(&older).unwrap(),
        )
        .unwrap();

        let requests = super::take_start_route_requests(temp.path()).unwrap();
        assert_eq!(
            requests
                .iter()
                .map(|(request, _)| request.route.as_str())
                .collect::<Vec<_>>(),
            vec!["relay", "maintenance"]
        );
    }

    #[test]
    fn broken_request_entry_does_not_discard_an_already_claimed_request() {
        let temp = tempfile::tempdir().unwrap();
        let valid = super::StartRouteRequest {
            id: "a-valid".to_string(),
            route: "relay".to_string(),
            created_at_ms: super::current_time_ms(),
        };
        std::fs::write(
            temp.path().join("a-valid.json"),
            serde_json::to_vec(&valid).unwrap(),
        )
        .unwrap();
        std::fs::create_dir(temp.path().join("z-broken.json")).unwrap();

        let requests = super::take_start_route_requests(temp.path()).unwrap();

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0.route, "relay");
        assert_eq!(
            requests[0].1.extension().and_then(|value| value.to_str()),
            Some("processing")
        );
    }

    #[test]
    fn stale_route_artifact_cleanup_keeps_pending_json_requests() {
        let temp = tempfile::tempdir().unwrap();
        for name in ["orphan.tmp", "orphan.processing", "orphan.ack"] {
            std::fs::write(temp.path().join(name), b"orphan").unwrap();
        }
        std::fs::write(temp.path().join("pending.json"), b"pending").unwrap();

        super::cleanup_stale_start_route_artifacts(temp.path(), std::time::Duration::ZERO).unwrap();

        assert!(temp.path().join("pending.json").exists());
        assert!(!temp.path().join("orphan.tmp").exists());
        assert!(!temp.path().join("orphan.processing").exists());
        assert!(!temp.path().join("orphan.ack").exists());
    }
}
