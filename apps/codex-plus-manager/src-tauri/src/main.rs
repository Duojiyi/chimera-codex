#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    for arg in &args {
        if arg.starts_with("codexplusplus://") {
            match codex_plus_core::provider_import::save_pending_provider_import_from_url(&arg) {
                Ok(_request) => {
                    let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
                        "manager.provider_import_url.pending",
                        serde_json::json!({
                            "queued": true
                        }),
                    );
                    codex_plus_manager_lib::focus_existing_manager_window();
                }
                Err(_error) => {
                    let _ = codex_plus_core::diagnostic_log::append_diagnostic_log(
                        "manager.provider_import_url.failed",
                        serde_json::json!({
                            "failed": true
                        }),
                    );
                }
            }
        }
    }
    if args.iter().any(|arg| arg == "--show-update") {
        unsafe {
            std::env::set_var("CODEX_PLUS_SHOW_UPDATE", "1");
        }
    }
    let start_route = if args.iter().any(|arg| arg == "--recover-settings") {
        Some("maintenance")
    } else if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--configure-relay" | "--chimera-key-first"))
    {
        Some("relay")
    } else {
        None
    };
    if let Some(start_route) = start_route {
        unsafe {
            std::env::set_var("CODEX_PLUS_START_ROUTE", start_route);
        }
    }
    codex_plus_manager_lib::run();
}
