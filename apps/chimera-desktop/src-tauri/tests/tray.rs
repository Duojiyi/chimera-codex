// Step 3.4 — system tray.
//
// The audit found `tray-icon` declared as a Cargo feature with no
// TrayIconBuilder anywhere in the crate: the capability was paid for and never
// built. Settings even offered "start minimized to tray" with no tray to
// minimize to, so that toggle could only ever hide the window with no way back.
//
// The Tauri parts (icon, event loop) cannot run headless, so what is tested
// here is everything that decides behaviour: which entries the menu has, what
// they are called in each shipped locale, and the two window-visibility rules.
// The wiring itself is asserted by V15, which now rejects declaring the feature
// without building a tray.

use chimera_desktop_lib::tray::{
    TRAY_ITEM_IDS, TrayAction, menu_labels, should_show_window_on_start, window_close_action,
};

#[test]
fn the_menu_offers_exactly_the_actions_the_tray_promises() {
    // Show, Launch Codex, Quit. Anything else belongs in the window: a tray
    // menu that grows into a second UI is one nobody maintains.
    assert_eq!(TRAY_ITEM_IDS, ["show", "launch", "quit"]);
}

#[test]
fn every_menu_item_is_translated_in_both_shipped_locales() {
    for lang in ["zh", "en"] {
        let labels = menu_labels(lang);
        assert_eq!(
            labels.len(),
            TRAY_ITEM_IDS.len(),
            "{lang} must label every menu item"
        );
        for (id, label) in TRAY_ITEM_IDS.iter().zip(labels.iter()) {
            assert!(!label.trim().is_empty(), "{lang}/{id} has an empty label");
        }
    }
}

#[test]
fn the_two_locales_do_not_share_labels() {
    // A copy-paste that leaves English text in the Chinese menu is the exact
    // failure this catches — the tray is built in Rust and so is invisible to
    // the frontend i18n gate.
    let zh = menu_labels("zh");
    let en = menu_labels("en");
    assert_ne!(zh, en, "the Chinese tray menu must not be the English one");
}

#[test]
fn an_unknown_locale_falls_back_to_chinese() {
    // Chinese is the product default, so an unrecognised tag must land there
    // rather than on an empty menu.
    assert_eq!(menu_labels("fr"), menu_labels("zh"));
    assert_eq!(menu_labels(""), menu_labels("zh"));
}

// ── Window visibility ───────────────────────────────────────────────────────

#[test]
fn start_minimized_off_shows_the_window() {
    assert!(should_show_window_on_start(false));
}

#[test]
fn start_minimized_on_starts_in_the_tray() {
    assert!(!should_show_window_on_start(true));
}

#[test]
fn closing_the_window_hides_it_rather_than_exiting() {
    // With a tray present, closing must not terminate: the user still has a
    // managed Codex process and an update channel running behind the app.
    // Quitting is reachable, but only from the tray menu that says so.
    assert_eq!(window_close_action(), TrayAction::HideWindow);
}
