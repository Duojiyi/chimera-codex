//! Chimera++ product policy for the CC Switch capability base.
//!
//! Upstream capabilities stay compiled and callable, but only Codex-owned
//! integrations may touch live application state during startup. Features
//! hidden by the product must never become background side effects.

use crate::app_config::AppType;
use serde::Serialize;

const STARTUP_MANAGED_APPS: &[AppType] = &[AppType::Codex];
const DEFAULT_VISIBLE_APPS: &[AppType] = &[AppType::Codex];

pub const PRODUCT_NAME: &str = "Chimera++";
pub const PRODUCT_DATA_DIR: &str = ".chimera-plus-plus";
pub const PRODUCT_DATABASE_FILE: &str = "chimera.db";
pub const PRODUCT_LOG_FILE: &str = "chimera-plus-plus";
pub const PRODUCT_TRAY_ID: &str = "chimera-plus-plus";
pub const PRODUCT_DEEP_LINK_SCHEME: &str = "chimera";
pub const LEGACY_DEEP_LINK_SCHEME: &str = "ccswitch";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackendCapability {
    pub id: &'static str,
    pub available: bool,
    pub enabled_by_default: bool,
    pub starts_automatically: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductCapabilities {
    pub policy_version: u32,
    pub available_apps: Vec<String>,
    pub default_visible_apps: Vec<&'static str>,
    pub startup_managed_apps: Vec<&'static str>,
    pub commercial_presets_enabled: bool,
    pub sponsor_content_enabled: bool,
    pub app_update_channel_configured: bool,
    pub capabilities: Vec<BackendCapability>,
}

/// Returns the complete backend capability inventory exposed to the renderer.
#[tauri::command]
pub fn get_product_capabilities() -> ProductCapabilities {
    ProductCapabilities {
        policy_version: 2,
        // `AppType::all()` yields values, so retain owned names in the
        // capability payload rather than borrowing from each temporary value.
        available_apps: AppType::all().map(|app| app.as_str().to_owned()).collect(),
        default_visible_apps: DEFAULT_VISIBLE_APPS.iter().map(AppType::as_str).collect(),
        startup_managed_apps: STARTUP_MANAGED_APPS.iter().map(AppType::as_str).collect(),
        commercial_presets_enabled: false,
        sponsor_content_enabled: false,
        app_update_channel_configured: app_update_channel_configured(),
        capabilities: vec![
            capability("providers", true, true),
            capability("model_discovery", true, false),
            capability("local_proxy", false, false),
            capability("failover", false, false),
            capability("usage", false, false),
            capability("managed_accounts", false, false),
            capability("mcp", false, false),
            capability("skills", false, false),
            capability("prompts", false, false),
            capability("sessions", false, false),
            capability("webdav_sync", false, false),
            capability("s3_sync", false, false),
            capability("codex_runtime_manager", true, false),
            capability("codex_themes", true, false),
        ],
    }
}

fn capability(
    id: &'static str,
    enabled_by_default: bool,
    starts_automatically: bool,
) -> BackendCapability {
    BackendCapability {
        id,
        available: true,
        enabled_by_default,
        starts_automatically,
    }
}

pub fn startup_managed_apps() -> impl Iterator<Item = AppType> {
    STARTUP_MANAGED_APPS.iter().cloned()
}

pub fn is_startup_managed_app_name(app: &str) -> bool {
    STARTUP_MANAGED_APPS
        .iter()
        .any(|candidate| candidate.as_str() == app)
}

pub fn is_app_visible_by_product(app: &AppType) -> bool {
    DEFAULT_VISIBLE_APPS.contains(app)
}

pub fn accepts_deep_link(url: &str) -> bool {
    let scheme = url.split_once("://").map(|(scheme, _)| scheme);
    matches!(
        scheme,
        Some(PRODUCT_DEEP_LINK_SCHEME | LEGACY_DEEP_LINK_SCHEME)
    )
}

pub const fn import_extended_apps_on_startup() -> bool {
    false
}

pub const fn import_content_on_startup() -> bool {
    false
}

pub const fn initialize_skills_on_startup() -> bool {
    false
}

pub const fn sync_session_usage_on_startup() -> bool {
    true
}

pub const fn start_cloud_sync_workers() -> bool {
    false
}

pub const fn refresh_usage_from_tray() -> bool {
    false
}

pub const fn app_update_channel_configured() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_upstream_app_adapters_remain_available() {
        let policy = get_product_capabilities();
        assert_eq!(
            policy.available_apps,
            vec![
                "claude",
                "claude-desktop",
                "codex",
                "gemini",
                "grokbuild",
                "opencode",
                "openclaw",
                "hermes",
            ]
        );
        assert_eq!(policy.default_visible_apps, vec!["codex"]);
    }

    #[test]
    fn only_codex_may_touch_live_config_during_startup() {
        let apps: Vec<_> = startup_managed_apps().collect();
        assert_eq!(apps, vec![AppType::Codex]);
        assert!(is_startup_managed_app_name("codex"));
        assert!(!is_startup_managed_app_name("claude"));
        assert!(!is_startup_managed_app_name("gemini"));
    }

    #[test]
    fn commercial_content_and_upstream_networks_are_disabled() {
        let policy = get_product_capabilities();
        assert!(!policy.commercial_presets_enabled);
        assert!(!policy.sponsor_content_enabled);
        assert!(!import_extended_apps_on_startup());
        assert!(!import_content_on_startup());
        assert!(!initialize_skills_on_startup());
        assert!(sync_session_usage_on_startup());
        assert!(!start_cloud_sync_workers());
        assert!(!refresh_usage_from_tray());
        assert!(app_update_channel_configured());
    }

    #[test]
    fn chimera_and_legacy_ccswitch_deep_links_are_accepted() {
        assert!(accepts_deep_link("chimera://providers/import"));
        assert!(accepts_deep_link("ccswitch://providers/import"));
        assert!(!accepts_deep_link("https://example.com"));
    }
}
