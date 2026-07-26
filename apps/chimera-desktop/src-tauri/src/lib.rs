// Chimera++ 2.0 — thin Tauri command shell.
// Business logic lives in chimera-domain / chimera-provider / chimera-runtime /
// chimera-platform. Tauri commands are thin adapters: validate input, call a
// service, return a serialisable result.

pub mod bootstrap_cmds;
pub mod commands;
pub mod dto;
pub mod provider_cmds;
pub mod runtime_cmds;
pub mod state;
pub mod tray;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Fail loudly and early: if the data directory or provider DB cannot be
    // opened there is no safe degraded mode — every command needs them.
    let app_state = match AppState::initialise() {
        Ok(state) => state,
        Err(message) => {
            eprintln!("chimera-desktop: failed to initialise application state: {message}");
            std::process::exit(1);
        }
    };

    // Read once, before the builder takes ownership: the tray and the initial
    // window visibility both depend on it.
    let start_minimized = app_state.settings().start_minimized;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(move |app| {
            let handle = app.handle();
            // Chinese is the product default; the frontend calls
            // set_tray_language once it knows the user's choice.
            tray::install(handle, "zh")?;
            if !tray::should_show_window_on_start(start_minimized) {
                if let Some(window) = handle.get_webview_window("main") {
                    window.hide()?;
                }
            }
            Ok(())
        })
        // Closing hides to the tray. Without this the "start minimized" setting
        // would be a one-way trip: hide the window, no way to bring it back.
        .on_window_event(|window, event| {
            tray::handle_window_event(window.app_handle(), event);
        })
        .invoke_handler(tauri::generate_handler![
            commands::set_tray_language,
            // First run: preflight before anything is touched, then fetch.
            bootstrap_cmds::run_preflight,
            bootstrap_cmds::fetch_codex_payload,
            commands::get_system_status,
            commands::list_providers,
            commands::launch_codex,
            commands::switch_provider,
            commands::test_provider,
            provider_cmds::add_provider,
            provider_cmds::delete_provider,
            provider_cmds::test_existing_provider,
            commands::list_skins,
            commands::apply_skin,
            commands::try_skin,
            commands::restore_default_skin,
            // Codex screen — every command the tab calls must be here, or it
            // fails at runtime with "command not found" the moment it opens.
            runtime_cmds::get_runtime_status,
            runtime_cmds::repair_runtime,
            runtime_cmds::run_diagnostics,
            runtime_cmds::rollback_runtime,
            runtime_cmds::apply_codex_update,
            // Settings screen
            runtime_cmds::get_settings,
            runtime_cmds::save_settings,
            runtime_cmds::reset_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
