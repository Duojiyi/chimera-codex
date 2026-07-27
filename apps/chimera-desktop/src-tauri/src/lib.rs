// Chimera++ 2.0 — thin Tauri command shell.
// Business logic lives in chimera-domain / chimera-provider / chimera-runtime /
// chimera-platform. Tauri commands are thin adapters: validate input, call a
// service, return a serialisable result.

pub mod bootstrap_cmds;
pub mod commands;
pub mod dto;
pub mod migration_cmds;
pub mod portable_cmds;
pub mod provider_cmds;
pub mod runtime_cmds;
pub mod skin_catalog;
pub mod skin_cmds;
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
            bootstrap_cmds::install_codex_payload,
            // Portable install: what it cannot do, and how to remove it. There
            // is no Apps & Features entry, so this is the whole uninstall path.
            // 1.x / CC Switch migration: preview reads only, run writes.
            migration_cmds::preview_migration,
            migration_cmds::run_migration,
            portable_cmds::get_portable_limitations,
            portable_cmds::get_cleanup_plan,
            portable_cmds::execute_cleanup,
            commands::get_system_status,
            commands::list_providers,
            commands::launch_codex,
            commands::switch_provider,
            commands::test_provider,
            provider_cmds::add_provider,
            provider_cmds::delete_provider,
            provider_cmds::test_existing_provider,
            // Skins, driving a real CDP session (Step 8.2/8.3).
            skin_cmds::import_skin,
            skin_cmds::list_skins,
            skin_cmds::apply_skin,
            skin_cmds::try_skin,
            skin_cmds::cancel_try_skin,
            skin_cmds::restore_default_skin,
            skin_catalog::list_skin_catalog,
            skin_catalog::install_catalog_skin,
            skin_catalog::import_skin_package,
            skin_catalog::apply_skin_package,
            skin_catalog::try_skin_package,
            skin_catalog::restore_skin_package,
            // Codex screen — every command the tab calls must be here, or it
            // fails at runtime with "command not found" the moment it opens.
            runtime_cmds::get_runtime_status,
            runtime_cmds::check_codex_update,
            runtime_cmds::repair_runtime,
            runtime_cmds::run_diagnostics,
            runtime_cmds::rollback_runtime,
            runtime_cmds::uninstall_codex,
            runtime_cmds::apply_codex_update,
            // Settings screen
            runtime_cmds::get_settings,
            runtime_cmds::save_settings,
            runtime_cmds::reset_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
