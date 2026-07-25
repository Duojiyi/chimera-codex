// Chimera++ 2.0 — thin Tauri command shell.
// Business logic lives in chimera-domain / chimera-provider / chimera-runtime / chimera-platform.
// Tauri commands are thin adapters: validate input, call service, return serialisable result.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
