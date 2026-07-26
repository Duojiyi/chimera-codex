//! Step 3.4 — system tray.
//!
//! Chimera keeps a managed Codex runtime and an update channel alive behind the
//! window, so closing the window must not end the process. The tray is what
//! makes that honest: it gives the app a visible place to live while hidden and
//! an unambiguous way to actually quit.
//!
//! The tray menu is built in Rust, which puts it outside the frontend's i18n
//! dictionary and therefore outside V17's reach. Its strings live here with
//! their own bilingual table and their own test, rather than being quietly
//! left in English.

use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, WindowEvent};

/// Menu entry ids, in display order. Kept deliberately short: a tray menu that
/// grows into a second UI is one nobody keeps in sync with the real one.
pub const TRAY_ITEM_IDS: [&str; 3] = ["show", "launch", "quit"];

/// What a UI gesture should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ShowWindow,
    HideWindow,
    LaunchCodex,
    Quit,
}

/// Labels for [`TRAY_ITEM_IDS`], in the same order.
///
/// Chinese is the product default, so an unrecognised tag falls back to it —
/// an empty menu would be worse than one in the wrong language.
pub fn menu_labels(lang: &str) -> [&'static str; 3] {
    match lang {
        "en" => ["Show Chimera++", "Launch Codex", "Quit"],
        _ => ["显示 Chimera++", "启动 Codex", "退出"],
    }
}

/// Whether the main window is visible at startup.
pub fn should_show_window_on_start(start_minimized: bool) -> bool {
    !start_minimized
}

/// What the window's close button does.
///
/// Hide, never exit. The user's Codex session and any in-flight update outlive
/// the window; quitting is available from the tray menu, which says "Quit".
pub fn window_close_action() -> TrayAction {
    TrayAction::HideWindow
}

/// Map a menu event id to the action it performs.
pub fn action_for_menu_id(id: &str) -> Option<TrayAction> {
    match id {
        "show" => Some(TrayAction::ShowWindow),
        "launch" => Some(TrayAction::LaunchCodex),
        "quit" => Some(TrayAction::Quit),
        _ => None,
    }
}

/// Bring the main window to the front, restoring it if it was hidden.
fn focus_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        // Unminimize before show: a window minimised to the taskbar and then
        // shown stays minimised on Windows, which reads as the tray doing
        // nothing at all.
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn perform<R: Runtime>(app: &AppHandle<R>, action: TrayAction) {
    match action {
        TrayAction::ShowWindow => focus_main_window(app),
        TrayAction::HideWindow => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        TrayAction::LaunchCodex => {
            // Best effort by design: the tray has nowhere to report an error,
            // so a failed launch surfaces on the Codex screen instead, which
            // ShowWindow makes reachable.
            if let Some(state) = app.try_state::<crate::state::AppState>() {
                let _ = chimera_runtime::process::launch_managed_codex(&state.runtime);
            }
            focus_main_window(app);
        }
        TrayAction::Quit => app.exit(0),
    }
}

/// Id the tray registers under, so the language switch can find it again.
const TRAY_ID: &str = "chimera-tray";

fn build_menu<R: Runtime>(app: &AppHandle<R>, lang: &str) -> tauri::Result<Menu<R>> {
    let labels = menu_labels(lang);
    let items: Vec<MenuItem<R>> = TRAY_ITEM_IDS
        .iter()
        .zip(labels.iter())
        .map(|(id, label)| MenuItem::with_id(app, *id, *label, true, None::<&str>))
        .collect::<tauri::Result<Vec<_>>>()?;

    let refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<R>)
        .collect();
    Menu::with_items(app, &refs)
}

/// Relabel the tray after the user switches language.
///
/// The tray is built before the webview reports a language, so it starts in
/// Chinese and is corrected here rather than being left permanently out of step
/// with the rest of the window.
pub fn set_language<R: Runtime>(app: &AppHandle<R>, lang: &str) -> tauri::Result<()> {
    let menu = build_menu(app, lang)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}

/// Build the tray icon and attach its handlers.
///
/// Called from `run()`'s setup hook. Errors propagate: a declared tray that
/// silently failed to appear is how "start minimized" becomes a window the user
/// cannot get back.
pub fn install<R: Runtime>(app: &AppHandle<R>, lang: &str) -> tauri::Result<()> {
    let menu = build_menu(app, lang)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().ok_or_else(|| {
            tauri::Error::AssetNotFound("no default window icon for the tray".into())
        })?)
        .tooltip("Chimera++")
        .menu(&menu)
        // The menu is the only way to reach Quit, so it must not appear on the
        // left click that people use to reopen the window.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event: MenuEvent| {
            if let Some(action) = action_for_menu_id(event.id.as_ref()) {
                perform(app, action);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                perform(tray.app_handle(), TrayAction::ShowWindow);
            }
        })
        .build(app)?;

    Ok(())
}

/// Intercept the window close button so it hides instead of exiting.
pub fn handle_window_event<R: Runtime>(app: &AppHandle<R>, event: &WindowEvent) -> bool {
    if matches!(event, WindowEvent::CloseRequested { .. }) {
        perform(app, window_close_action());
        return true;
    }
    false
}
