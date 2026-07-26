// Chimera++ 2.0 — thin Tauri command shell.
// Business logic lives in chimera-domain / chimera-provider / chimera-runtime /
// chimera-platform. Tauri commands are thin adapters: validate input, call a
// service, return a serialisable result.

pub mod commands;
pub mod dto;
pub mod provider_cmds;
pub mod runtime_cmds;
pub mod state;

use state::AppState;

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

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
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
