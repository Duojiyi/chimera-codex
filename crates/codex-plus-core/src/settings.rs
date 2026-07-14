use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;
use serde_json::{Map, Value};
use toml_edit::{DocumentMut, Item};

use crate::zed_remote::ZedOpenStrategy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LaunchMode {
    #[default]
    Patch,
    Relay,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayContextSelection {
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub plugins: Vec<String>,
}

impl Default for RelayContextSelection {
    fn default() -> Self {
        Self {
            mcp_servers: Vec::new(),
            skills: Vec::new(),
            plugins: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayProfile {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing)]
    pub model: String,
    #[serde(default = "default_relay_base_url", skip_serializing)]
    pub base_url: String,
    #[serde(rename = "upstreamBaseUrl", default)]
    pub upstream_base_url: String,
    #[serde(
        default,
        skip_serializing,
        deserialize_with = "deserialize_profile_api_key"
    )]
    pub api_key: String,
    #[serde(default)]
    pub protocol: RelayProtocol,
    #[serde(rename = "relayMode", default)]
    pub relay_mode: RelayMode,
    #[serde(rename = "officialMixApiKey", default)]
    pub official_mix_api_key: bool,
    #[serde(rename = "testModel", default)]
    pub test_model: String,
    #[serde(rename = "configContents", default)]
    pub config_contents: String,
    #[serde(rename = "authContents", default)]
    pub auth_contents: String,
    #[serde(rename = "useCommonConfig", default = "default_true")]
    pub use_common_config: bool,
    #[serde(rename = "contextSelection", default)]
    pub context_selection: RelayContextSelection,
    #[serde(rename = "contextSelectionInitialized", default)]
    pub context_selection_initialized: bool,
    #[serde(rename = "contextWindow", default)]
    pub context_window: String,
    #[serde(rename = "autoCompactLimit", default)]
    pub auto_compact_limit: String,
    #[serde(rename = "modelInsertMode", default)]
    pub model_insert_mode: RelayModelInsertMode,
    #[serde(rename = "modelList", default)]
    pub model_list: String,
    #[serde(
        rename = "modelWindows",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub model_windows: String,
    #[serde(
        rename = "userAgent",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub user_agent: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AggregateRelayStrategy {
    #[default]
    Failover,
    ConversationRoundRobin,
    RequestRoundRobin,
    WeightedRoundRobin,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateRelayMember {
    #[serde(rename = "relayId")]
    pub relay_id: String,
    #[serde(default = "default_aggregate_member_weight")]
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateRelayProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub strategy: AggregateRelayStrategy,
    #[serde(default)]
    pub members: Vec<AggregateRelayMember>,
}

impl Default for RelayProfile {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            name: "默认中转".to_string(),
            model: String::new(),
            base_url: default_relay_base_url(),
            upstream_base_url: String::new(),
            api_key: String::new(),
            protocol: RelayProtocol::Responses,
            relay_mode: RelayMode::Official,
            official_mix_api_key: false,
            test_model: String::new(),
            config_contents: String::new(),
            auth_contents: String::new(),
            use_common_config: true,
            context_selection: RelayContextSelection::default(),
            context_selection_initialized: false,
            context_window: String::new(),
            auto_compact_limit: String::new(),
            model_insert_mode: RelayModelInsertMode::Patch,
            model_list: String::new(),
            model_windows: String::new(),
            user_agent: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RelayModelInsertMode {
    ModelCatalog,
    #[default]
    Patch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RelayProtocol {
    #[default]
    Responses,
    ChatCompletions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RelayMode {
    Official,
    #[default]
    MixedApi,
    PureApi,
    Aggregate,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackendSettings {
    #[serde(rename = "codexAppPath", default)]
    pub codex_app_path: String,
    #[serde(rename = "codexExtraArgs", default)]
    pub codex_extra_args: Vec<String>,
    #[serde(rename = "providerSyncEnabled", default)]
    pub provider_sync_enabled: bool,
    #[serde(rename = "providerSyncSavedProviders", default)]
    pub provider_sync_saved_providers: Vec<String>,
    #[serde(rename = "providerSyncManualProviders", default)]
    pub provider_sync_manual_providers: Vec<String>,
    #[serde(rename = "providerSyncLastSelectedProvider", default)]
    pub provider_sync_last_selected_provider: String,
    #[serde(rename = "relayProfilesEnabled", default = "default_true")]
    pub relay_profiles_enabled: bool,
    #[serde(rename = "enhancementsEnabled", default = "default_true")]
    pub enhancements_enabled: bool,
    #[serde(rename = "computerUseGuardEnabled", default)]
    pub computer_use_guard_enabled: bool,
    #[serde(rename = "codexAppPluginMarketplaceUnlock", default = "default_true")]
    pub codex_app_plugin_marketplace_unlock: bool,
    #[serde(rename = "codexAppPluginAutoExpand", default = "default_true")]
    pub codex_app_plugin_auto_expand: bool,
    #[serde(rename = "codexAppModelWhitelistUnlock", default = "default_true")]
    pub codex_app_model_whitelist_unlock: bool,
    #[serde(rename = "codexAppSessionDelete", default = "default_true")]
    pub codex_app_session_delete: bool,
    #[serde(rename = "codexAppMarkdownExport", default = "default_true")]
    pub codex_app_markdown_export: bool,
    #[serde(rename = "codexAppPasteFix", default)]
    pub codex_app_paste_fix: bool,
    #[serde(rename = "codexAppForceChineseLocale", default = "default_true")]
    pub codex_app_force_chinese_locale: bool,
    #[serde(rename = "codexAppFastStartup", default)]
    pub codex_app_fast_startup: bool,
    #[serde(rename = "codexAppProjectMove", default = "default_true")]
    pub codex_app_project_move: bool,
    #[serde(rename = "codexAppThreadIdBadge", default)]
    pub codex_app_thread_id_badge: bool,
    #[serde(rename = "codexAppConversationView", default)]
    pub codex_app_conversation_view: bool,
    #[serde(rename = "codexAppThreadScrollRestore", default = "default_true")]
    pub codex_app_thread_scroll_restore: bool,
    #[serde(rename = "codexAppZedRemoteOpen", default = "default_true")]
    pub codex_app_zed_remote_open: bool,
    #[serde(rename = "zedRemoteOpenStrategy", default)]
    pub zed_remote_open_strategy: ZedOpenStrategy,
    #[serde(rename = "zedRemoteProjectRegistryEnabled", default = "default_true")]
    pub zed_remote_project_registry_enabled: bool,
    #[serde(rename = "zedRemoteSyncToZedSettings", default)]
    pub zed_remote_sync_to_zed_settings: bool,
    #[serde(rename = "codexAppUpstreamWorktreeCreate", default = "default_true")]
    pub codex_app_upstream_worktree_create: bool,
    #[serde(rename = "codexAppNativeMenuPlacement", default = "default_true")]
    pub codex_app_native_menu_placement: bool,
    #[serde(rename = "codexAppNativeMenuLocalization", default = "default_true")]
    pub codex_app_native_menu_localization: bool,
    #[serde(rename = "codexAppServiceTierControls", default)]
    pub codex_app_service_tier_controls: bool,
    #[serde(rename = "codexAppPetRealMouseLook", default)]
    pub codex_app_pet_real_mouse_look: bool,
    #[serde(rename = "codexAppStepwiseEnabled", default)]
    pub codex_app_stepwise_enabled: bool,
    #[serde(rename = "codexAppStepwiseDirectSend", default)]
    pub codex_app_stepwise_direct_send: bool,
    #[serde(rename = "codexAppStepwiseBaseUrl", default)]
    pub codex_app_stepwise_base_url: String,
    #[serde(rename = "codexAppStepwiseApiKey", default)]
    pub codex_app_stepwise_api_key: String,
    #[serde(
        rename = "codexAppStepwiseApiKeyEnv",
        default = "default_stepwise_api_key_env",
        deserialize_with = "empty_as_default_stepwise_api_key_env"
    )]
    pub codex_app_stepwise_api_key_env: String,
    #[serde(rename = "codexAppStepwiseModel", default)]
    pub codex_app_stepwise_model: String,
    #[serde(
        rename = "codexAppStepwiseMaxItems",
        default = "default_stepwise_max_items",
        deserialize_with = "deserialize_stepwise_max_items"
    )]
    pub codex_app_stepwise_max_items: u8,
    #[serde(
        rename = "codexAppStepwiseMaxInputChars",
        default = "default_stepwise_max_input_chars",
        deserialize_with = "deserialize_stepwise_max_input_chars"
    )]
    pub codex_app_stepwise_max_input_chars: u32,
    #[serde(
        rename = "codexAppStepwiseMaxOutputTokens",
        default = "default_stepwise_max_output_tokens",
        deserialize_with = "deserialize_stepwise_max_output_tokens"
    )]
    pub codex_app_stepwise_max_output_tokens: u32,
    #[serde(
        rename = "codexAppStepwiseTimeoutMs",
        default = "default_stepwise_timeout_ms",
        deserialize_with = "deserialize_stepwise_timeout_ms"
    )]
    pub codex_app_stepwise_timeout_ms: u64,
    #[serde(rename = "codexAppImageOverlayEnabled", default)]
    pub codex_app_image_overlay_enabled: bool,
    #[serde(rename = "codexAppImageOverlayPath", default)]
    pub codex_app_image_overlay_path: String,
    #[serde(
        rename = "codexAppImageOverlayOpacity",
        default = "default_image_overlay_opacity",
        deserialize_with = "deserialize_image_overlay_opacity"
    )]
    pub codex_app_image_overlay_opacity: u8,
    #[serde(
        rename = "codexAppImageOverlayFitMode",
        default = "default_image_overlay_fit_mode",
        deserialize_with = "deserialize_image_overlay_fit_mode"
    )]
    pub codex_app_image_overlay_fit_mode: String,
    #[serde(rename = "codexGoalsEnabled", default)]
    pub codex_goals_enabled: bool,
    #[serde(rename = "launchMode", default)]
    pub launch_mode: LaunchMode,
    #[serde(rename = "relayBaseUrl", default = "default_relay_base_url")]
    pub relay_base_url: String,
    #[serde(rename = "relayApiKey", default)]
    pub relay_api_key: String,
    #[serde(rename = "relayProfiles", default = "default_relay_profiles")]
    pub relay_profiles: Vec<RelayProfile>,
    #[serde(rename = "relayCommonConfigContents", default)]
    pub relay_common_config_contents: String,
    #[serde(rename = "relayContextConfigContents", default)]
    pub relay_context_config_contents: String,
    #[serde(rename = "activeRelayId", default = "default_active_relay_id")]
    pub active_relay_id: String,
    #[serde(rename = "aggregateRelayProfiles", default)]
    pub aggregate_relay_profiles: Vec<AggregateRelayProfile>,
    #[serde(rename = "activeAggregateRelayId", default)]
    pub active_aggregate_relay_id: String,
    #[serde(rename = "relayTestModel", default = "default_relay_test_model")]
    pub relay_test_model: String,
}

impl Default for BackendSettings {
    fn default() -> Self {
        Self {
            codex_app_path: String::new(),
            codex_extra_args: Vec::new(),
            provider_sync_enabled: false,
            provider_sync_saved_providers: Vec::new(),
            provider_sync_manual_providers: Vec::new(),
            provider_sync_last_selected_provider: String::new(),
            relay_profiles_enabled: true,
            enhancements_enabled: true,
            computer_use_guard_enabled: false,
            codex_app_plugin_marketplace_unlock: true,
            codex_app_plugin_auto_expand: true,
            codex_app_model_whitelist_unlock: true,
            codex_app_session_delete: true,
            codex_app_markdown_export: true,
            codex_app_paste_fix: false,
            codex_app_force_chinese_locale: true,
            codex_app_fast_startup: false,
            codex_app_project_move: true,
            codex_app_thread_id_badge: false,
            codex_app_conversation_view: false,
            codex_app_thread_scroll_restore: true,
            codex_app_zed_remote_open: true,
            zed_remote_open_strategy: ZedOpenStrategy::AddToFocusedWorkspace,
            zed_remote_project_registry_enabled: true,
            zed_remote_sync_to_zed_settings: false,
            codex_app_upstream_worktree_create: true,
            codex_app_native_menu_placement: true,
            codex_app_native_menu_localization: true,
            codex_app_service_tier_controls: false,
            codex_app_pet_real_mouse_look: false,
            codex_app_stepwise_enabled: false,
            codex_app_stepwise_direct_send: false,
            codex_app_stepwise_base_url: String::new(),
            codex_app_stepwise_api_key: String::new(),
            codex_app_stepwise_api_key_env: default_stepwise_api_key_env(),
            codex_app_stepwise_model: String::new(),
            codex_app_stepwise_max_items: default_stepwise_max_items(),
            codex_app_stepwise_max_input_chars: default_stepwise_max_input_chars(),
            codex_app_stepwise_max_output_tokens: default_stepwise_max_output_tokens(),
            codex_app_stepwise_timeout_ms: default_stepwise_timeout_ms(),
            codex_app_image_overlay_enabled: false,
            codex_app_image_overlay_path: String::new(),
            codex_app_image_overlay_opacity: default_image_overlay_opacity(),
            codex_app_image_overlay_fit_mode: default_image_overlay_fit_mode(),
            codex_goals_enabled: false,
            launch_mode: LaunchMode::Patch,
            relay_base_url: default_relay_base_url(),
            relay_api_key: String::new(),
            relay_profiles: default_relay_profiles(),
            relay_common_config_contents: String::new(),
            relay_context_config_contents: String::new(),
            active_relay_id: default_active_relay_id(),
            aggregate_relay_profiles: Vec::new(),
            active_aggregate_relay_id: String::new(),
            relay_test_model: default_relay_test_model(),
        }
    }
}

impl BackendSettings {
    pub fn active_relay_profile(&self) -> RelayProfile {
        if self.active_relay_id == default_active_relay_id()
            && self.relay_profiles.len() == 1
            && self.relay_profiles[0] == RelayProfile::default()
            && (!self.relay_api_key.is_empty() || self.relay_base_url != default_relay_base_url())
        {
            return RelayProfile {
                id: default_active_relay_id(),
                name: "默认中转".to_string(),
                model: String::new(),
                base_url: if self.relay_base_url.is_empty() {
                    default_relay_base_url()
                } else {
                    self.relay_base_url.clone()
                },
                upstream_base_url: if self.relay_base_url.is_empty() {
                    default_relay_base_url()
                } else {
                    self.relay_base_url.clone()
                },
                api_key: self.relay_api_key.clone(),
                protocol: RelayProtocol::Responses,
                relay_mode: RelayMode::MixedApi,
                official_mix_api_key: true,
                test_model: String::new(),
                config_contents: String::new(),
                auth_contents: String::new(),
                use_common_config: true,
                context_selection: RelayContextSelection::default(),
                context_selection_initialized: false,
                context_window: String::new(),
                auto_compact_limit: String::new(),
                model_insert_mode: RelayModelInsertMode::Patch,
                model_list: String::new(),
                model_windows: String::new(),
                user_agent: String::new(),
            };
        }

        if let Some(profile) = self
            .relay_profiles
            .iter()
            .find(|profile| profile.id == self.active_relay_id)
        {
            return profile.clone();
        }

        RelayProfile {
            id: if self.active_relay_id.is_empty() {
                default_active_relay_id()
            } else {
                self.active_relay_id.clone()
            },
            name: "默认中转".to_string(),
            model: String::new(),
            base_url: if self.relay_base_url.is_empty() {
                default_relay_base_url()
            } else {
                self.relay_base_url.clone()
            },
            upstream_base_url: if self.relay_base_url.is_empty() {
                default_relay_base_url()
            } else {
                self.relay_base_url.clone()
            },
            api_key: self.relay_api_key.clone(),
            protocol: RelayProtocol::Responses,
            relay_mode: RelayMode::Official,
            official_mix_api_key: false,
            test_model: String::new(),
            config_contents: String::new(),
            auth_contents: String::new(),
            use_common_config: true,
            context_selection: RelayContextSelection::default(),
            context_selection_initialized: false,
            context_window: String::new(),
            auto_compact_limit: String::new(),
            model_insert_mode: RelayModelInsertMode::Patch,
            model_list: String::new(),
            model_windows: String::new(),
            user_agent: String::new(),
        }
    }

    pub fn active_aggregate_relay_profile(&self) -> Option<AggregateRelayProfile> {
        let active_relay = self
            .relay_profiles
            .iter()
            .find(|profile| profile.id == self.active_relay_id)?;
        if active_relay.relay_mode != RelayMode::Aggregate {
            return None;
        }

        let active_aggregate_id = if self.active_aggregate_relay_id.trim().is_empty() {
            active_relay.id.as_str()
        } else {
            self.active_aggregate_relay_id.trim()
        };

        if active_aggregate_id != active_relay.id {
            return None;
        }

        self.aggregate_relay_profiles
            .iter()
            .find(|profile| profile.id == active_aggregate_id)
            .cloned()
    }

    pub fn active_relay_uses_protocol_proxy(&self) -> bool {
        self.active_aggregate_relay_profile().is_some()
            || self.active_relay_profile().protocol == RelayProtocol::ChatCompletions
    }
}

pub fn default_stepwise_api_key_env() -> String {
    "CODEX_STEPWISE_API_KEY".to_string()
}

pub fn default_stepwise_max_items() -> u8 {
    6
}

pub fn default_stepwise_max_input_chars() -> u32 {
    6000
}

pub fn default_stepwise_max_output_tokens() -> u32 {
    500
}

pub fn default_stepwise_timeout_ms() -> u64 {
    8000
}

fn default_image_overlay_opacity() -> u8 {
    35
}

fn clamp_image_overlay_opacity(value: u8) -> u8 {
    value.clamp(1, 100)
}

pub fn default_image_overlay_fit_mode() -> String {
    "fit".to_string()
}

fn normalize_image_overlay_fit_mode(value: &str) -> String {
    match value {
        "fill" | "fit" | "stretch" | "tile" | "center" => value.to_string(),
        _ => default_image_overlay_fit_mode(),
    }
}

pub fn clamp_stepwise_max_items(value: u8) -> u8 {
    value.min(default_stepwise_max_items())
}

pub fn clamp_stepwise_max_input_chars(value: u32) -> u32 {
    value.clamp(1000, 24000)
}

pub fn clamp_stepwise_max_output_tokens(value: u32) -> u32 {
    value.clamp(100, 4000)
}

pub fn clamp_stepwise_timeout_ms(value: u64) -> u64 {
    value.clamp(1000, 60000)
}

pub fn default_true() -> bool {
    true
}

pub fn default_relay_base_url() -> String {
    String::new()
}

pub fn default_active_relay_id() -> String {
    "default".to_string()
}

pub fn default_relay_test_model() -> String {
    "gpt-5.4-mini".to_string()
}

pub fn default_relay_profiles() -> Vec<RelayProfile> {
    vec![RelayProfile::default()]
}

/// 仅在 `settings.json` 不存在时使用的全新安装默认值。
/// 不改变 `BackendSettings::default()` / serde 缺省，避免覆盖升级用户。
pub fn chimera_first_run_settings() -> BackendSettings {
    let base_url = crate::branding::DEFAULT_RELAY_BASE_URL.to_string();
    let model = crate::branding::DEFAULT_RELAY_MODEL.to_string();
    BackendSettings {
        relay_profiles_enabled: true,
        relay_base_url: base_url.clone(),
        active_relay_id: "chimerahub".to_string(),
        relay_profiles: vec![RelayProfile {
            id: "chimerahub".to_string(),
            name: "ChimeraHub".to_string(),
            model: model.clone(),
            base_url,
            upstream_base_url: crate::branding::DEFAULT_RELAY_BASE_URL.to_string(),
            api_key: String::new(),
            protocol: RelayProtocol::Responses,
            relay_mode: RelayMode::PureApi,
            model_list: model,
            ..RelayProfile::default()
        }],
        ..BackendSettings::default()
    }
}

pub fn default_aggregate_member_weight() -> u32 {
    1
}

pub fn empty_as_default_stepwise_api_key_env<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_stepwise_api_key_env))
}

fn deserialize_image_overlay_opacity<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<u8>::deserialize(deserializer)?
        .map(clamp_image_overlay_opacity)
        .unwrap_or_else(default_image_overlay_opacity))
}

fn deserialize_image_overlay_fit_mode<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?
        .map(|value| normalize_image_overlay_fit_mode(&value))
        .unwrap_or_else(default_image_overlay_fit_mode))
}

fn deserialize_stepwise_max_items<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<u8>::deserialize(deserializer)?
        .map(clamp_stepwise_max_items)
        .unwrap_or_else(default_stepwise_max_items))
}

fn deserialize_stepwise_max_input_chars<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<u32>::deserialize(deserializer)?
        .map(clamp_stepwise_max_input_chars)
        .unwrap_or_else(default_stepwise_max_input_chars))
}

fn deserialize_stepwise_max_output_tokens<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<u32>::deserialize(deserializer)?
        .map(clamp_stepwise_max_output_tokens)
        .unwrap_or_else(default_stepwise_max_output_tokens))
}

fn deserialize_stepwise_timeout_ms<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<u64>::deserialize(deserializer)?
        .map(clamp_stepwise_timeout_ms)
        .unwrap_or_else(default_stepwise_timeout_ms))
}

fn deserialize_profile_api_key<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

pub fn normalize_codex_extra_args(args: &[String]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.trim())
        .filter(|arg| !arg.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn relay_profile_has_usable_key(profile: &RelayProfile) -> bool {
    if !profile.api_key.trim().is_empty()
        || experimental_bearer_token_from_config_text(&profile.config_contents).is_some()
    {
        return true;
    }
    serde_json::from_str::<Value>(&profile.auth_contents)
        .ok()
        .and_then(|value| {
            value
                .get("OPENAI_API_KEY")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(ToString::to_string)
        })
        .is_some()
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
    codex_home: Option<PathBuf>,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new_with_codex_home(
            crate::paths::default_settings_path(),
            crate::codex_home::default_codex_home_dir(),
        )
    }
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            codex_home: None,
        }
    }

    pub fn new_with_codex_home(path: PathBuf, codex_home: PathBuf) -> Self {
        Self {
            path,
            codex_home: Some(codex_home),
        }
    }

    pub fn load(&self) -> anyhow::Result<BackendSettings> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(self.missing_settings_defaults());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read settings {}", self.path.display()));
            }
        };

        Ok(normalize_settings_config_sections(
            serde_json::from_str(&contents).unwrap_or_default(),
        ))
    }

    pub fn load_strict(&self) -> anyhow::Result<BackendSettings> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(self.missing_settings_defaults());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read settings {}", self.path.display()));
            }
        };
        let settings = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse settings {}", self.path.display()))?;
        Ok(normalize_settings_config_sections(settings))
    }

    pub fn save(&self, settings: &BackendSettings) -> anyhow::Result<()> {
        self.save_normalized(settings).map(|_| ())
    }

    pub(crate) fn save_normalized(
        &self,
        settings: &BackendSettings,
    ) -> anyhow::Result<BackendSettings> {
        let mut settings = normalize_settings_config_sections(settings.clone());
        settings.codex_extra_args = normalize_codex_extra_args(&settings.codex_extra_args);
        let bytes = serde_json::to_vec_pretty(&settings)?;
        atomic_write(&self.path, &bytes)?;
        Ok(settings)
    }

    pub(crate) fn snapshot_bytes(&self) -> anyhow::Result<Option<Vec<u8>>> {
        match fs::read(&self.path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error)
                .with_context(|| format!("failed to snapshot settings {}", self.path.display())),
        }
    }

    pub(crate) fn restore_snapshot(&self, snapshot: Option<&[u8]>) -> anyhow::Result<()> {
        match snapshot {
            Some(bytes) => atomic_write(&self.path, bytes),
            None => match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error).with_context(|| {
                    format!("failed to remove restored settings {}", self.path.display())
                }),
            },
        }
    }

    pub fn update(&self, payload: Value) -> anyhow::Result<BackendSettings> {
        let Value::Object(payload) = payload else {
            return self.load();
        };

        let mut raw = self.load_raw_object()?;
        merge_known_setting_fields(&mut raw, &payload);
        let settings = normalize_settings_config_sections(
            serde_json::from_value(Value::Object(raw.clone())).unwrap_or_default(),
        );
        raw.insert(
            "relayCommonConfigContents".to_string(),
            Value::String(settings.relay_common_config_contents.clone()),
        );
        raw.insert(
            "relayContextConfigContents".to_string(),
            Value::String(settings.relay_context_config_contents.clone()),
        );
        let bytes = serde_json::to_vec_pretty(&Value::Object(raw))?;
        atomic_write(&self.path, &bytes)?;
        Ok(settings)
    }

    fn load_raw_object(&self) -> anyhow::Result<Map<String, Value>> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(settings_to_object(&self.missing_settings_defaults()));
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read settings {}", self.path.display()));
            }
        };

        match serde_json::from_str::<Value>(&contents) {
            Ok(Value::Object(map)) => Ok(map),
            Ok(_) | Err(_) => Ok(settings_to_object(&BackendSettings::default())),
        }
    }

    fn missing_settings_defaults(&self) -> BackendSettings {
        if self
            .codex_home
            .as_deref()
            .is_some_and(codex_home_has_nonempty_live_files)
        {
            BackendSettings::default()
        } else {
            chimera_first_run_settings()
        }
    }
}

fn codex_home_has_nonempty_live_files(home: &Path) -> bool {
    ["config.toml", "auth.json"].into_iter().any(|file_name| {
        fs::metadata(home.join(file_name))
            .map(|metadata| metadata.is_file() && metadata.len() > 0)
            .unwrap_or(false)
    })
}

fn merge_known_setting_fields(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    if let Some(value) = source.get("codexAppPath").and_then(Value::as_str) {
        target.insert("codexAppPath".to_string(), Value::String(value.to_string()));
    }
    if let Some(value) = source.get("codexExtraArgs").and_then(Value::as_array) {
        let args = value
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        target.insert(
            "codexExtraArgs".to_string(),
            Value::Array(
                normalize_codex_extra_args(&args)
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    if let Some(value) = source.get("providerSyncEnabled").and_then(Value::as_bool) {
        target.insert("providerSyncEnabled".to_string(), Value::Bool(value));
    }
    if let Some(value) = source.get("relayProfilesEnabled").and_then(Value::as_bool) {
        target.insert("relayProfilesEnabled".to_string(), Value::Bool(value));
    }
    if let Some(value) = source.get("enhancementsEnabled").and_then(Value::as_bool) {
        target.insert("enhancementsEnabled".to_string(), Value::Bool(value));
    }
    if let Some(value) = source
        .get("computerUseGuardEnabled")
        .and_then(Value::as_bool)
    {
        target.insert("computerUseGuardEnabled".to_string(), Value::Bool(value));
    }
    merge_bool_setting(target, source, "codexAppPluginMarketplaceUnlock");
    merge_bool_setting(target, source, "codexAppPluginAutoExpand");
    merge_bool_setting(target, source, "codexAppModelWhitelistUnlock");
    merge_bool_setting(target, source, "codexAppSessionDelete");
    merge_bool_setting(target, source, "codexAppMarkdownExport");
    merge_bool_setting(target, source, "codexAppPasteFix");
    merge_bool_setting(target, source, "codexAppForceChineseLocale");
    merge_bool_setting(target, source, "codexAppFastStartup");
    merge_bool_setting(target, source, "codexAppProjectMove");
    merge_bool_setting(target, source, "codexAppThreadIdBadge");
    merge_bool_setting(target, source, "codexAppConversationView");
    merge_bool_setting(target, source, "codexAppThreadScrollRestore");
    merge_bool_setting(target, source, "codexAppZedRemoteOpen");
    if let Some(value) = source.get("zedRemoteOpenStrategy") {
        if serde_json::from_value::<ZedOpenStrategy>(value.clone()).is_ok() {
            target.insert("zedRemoteOpenStrategy".to_string(), value.clone());
        }
    }
    merge_bool_setting(target, source, "zedRemoteProjectRegistryEnabled");
    merge_bool_setting(target, source, "zedRemoteSyncToZedSettings");
    merge_bool_setting(target, source, "codexAppUpstreamWorktreeCreate");
    merge_bool_setting(target, source, "codexAppNativeMenuPlacement");
    merge_bool_setting(target, source, "codexAppNativeMenuLocalization");
    merge_bool_setting(target, source, "codexAppServiceTierControls");
    merge_bool_setting(target, source, "codexAppPetRealMouseLook");
    merge_bool_setting(target, source, "codexAppStepwiseEnabled");
    merge_bool_setting(target, source, "codexAppStepwiseDirectSend");
    if let Some(value) = source
        .get("codexAppStepwiseBaseUrl")
        .and_then(Value::as_str)
    {
        target.insert(
            "codexAppStepwiseBaseUrl".to_string(),
            Value::String(value.trim().trim_end_matches('/').to_string()),
        );
    }
    if let Some(value) = source.get("codexAppStepwiseApiKey").and_then(Value::as_str) {
        target.insert(
            "codexAppStepwiseApiKey".to_string(),
            Value::String(value.trim().to_string()),
        );
    }
    if let Some(value) = source
        .get("codexAppStepwiseApiKeyEnv")
        .and_then(Value::as_str)
    {
        target.insert(
            "codexAppStepwiseApiKeyEnv".to_string(),
            Value::String(if value.trim().is_empty() {
                default_stepwise_api_key_env()
            } else {
                value.trim().to_string()
            }),
        );
    }
    if let Some(value) = source.get("codexAppStepwiseModel").and_then(Value::as_str) {
        target.insert(
            "codexAppStepwiseModel".to_string(),
            Value::String(value.trim().to_string()),
        );
    }
    if let Some(value) = source
        .get("codexAppStepwiseMaxItems")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
    {
        target.insert(
            "codexAppStepwiseMaxItems".to_string(),
            Value::Number(serde_json::Number::from(clamp_stepwise_max_items(value))),
        );
    }
    if let Some(value) = source
        .get("codexAppStepwiseMaxInputChars")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
    {
        target.insert(
            "codexAppStepwiseMaxInputChars".to_string(),
            Value::Number(serde_json::Number::from(clamp_stepwise_max_input_chars(
                value,
            ))),
        );
    }
    if let Some(value) = source
        .get("codexAppStepwiseMaxOutputTokens")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
    {
        target.insert(
            "codexAppStepwiseMaxOutputTokens".to_string(),
            Value::Number(serde_json::Number::from(clamp_stepwise_max_output_tokens(
                value,
            ))),
        );
    }
    if let Some(value) = source
        .get("codexAppStepwiseTimeoutMs")
        .and_then(Value::as_u64)
    {
        target.insert(
            "codexAppStepwiseTimeoutMs".to_string(),
            Value::Number(serde_json::Number::from(clamp_stepwise_timeout_ms(value))),
        );
    }
    merge_bool_setting(target, source, "codexAppImageOverlayEnabled");
    if let Some(value) = source
        .get("codexAppImageOverlayPath")
        .and_then(Value::as_str)
    {
        target.insert(
            "codexAppImageOverlayPath".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = source
        .get("codexAppImageOverlayOpacity")
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
    {
        target.insert(
            "codexAppImageOverlayOpacity".to_string(),
            Value::Number(serde_json::Number::from(clamp_image_overlay_opacity(value))),
        );
    }
    if let Some(value) = source
        .get("codexAppImageOverlayFitMode")
        .and_then(Value::as_str)
    {
        target.insert(
            "codexAppImageOverlayFitMode".to_string(),
            Value::String(normalize_image_overlay_fit_mode(value)),
        );
    }
    if let Some(value) = source.get("codexGoalsEnabled").and_then(Value::as_bool) {
        target.insert("codexGoalsEnabled".to_string(), Value::Bool(value));
    }
    if let Some(value) = source.get("launchMode").and_then(Value::as_str) {
        if matches!(value, "patch" | "relay") {
            target.insert("launchMode".to_string(), Value::String(value.to_string()));
        }
    }
    if let Some(value) = source.get("relayBaseUrl").and_then(Value::as_str) {
        target.insert("relayBaseUrl".to_string(), Value::String(value.to_string()));
    }
    if let Some(value) = source.get("relayApiKey").and_then(Value::as_str) {
        target.insert("relayApiKey".to_string(), Value::String(value.to_string()));
    }
    if let Some(value) = source.get("relayProfiles").and_then(Value::as_array) {
        let mut profiles = serde_json::from_value::<Vec<RelayProfile>>(Value::Array(value.clone()))
            .unwrap_or_default();
        preserve_official_mix_bearer_tokens(&mut profiles, target);
        target.insert(
            "relayProfiles".to_string(),
            serde_json::to_value(profiles).unwrap_or_else(|_| Value::Array(Vec::new())),
        );
    }
    if let Some(value) = source
        .get("relayCommonConfigContents")
        .and_then(Value::as_str)
    {
        target.insert(
            "relayCommonConfigContents".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = source
        .get("relayContextConfigContents")
        .and_then(Value::as_str)
    {
        target.insert(
            "relayContextConfigContents".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = source.get("activeRelayId").and_then(Value::as_str) {
        target.insert(
            "activeRelayId".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = source
        .get("aggregateRelayProfiles")
        .and_then(Value::as_array)
    {
        target.insert(
            "aggregateRelayProfiles".to_string(),
            Value::Array(value.clone()),
        );
    }
    if let Some(value) = source.get("activeAggregateRelayId").and_then(Value::as_str) {
        target.insert(
            "activeAggregateRelayId".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = source.get("relayTestModel").and_then(Value::as_str) {
        target.insert(
            "relayTestModel".to_string(),
            Value::String(if value.trim().is_empty() {
                default_relay_test_model()
            } else {
                value.trim().to_string()
            }),
        );
    }
}

fn merge_bool_setting(target: &mut Map<String, Value>, source: &Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).and_then(Value::as_bool) {
        target.insert(key.to_string(), Value::Bool(value));
    }
}

fn preserve_official_mix_bearer_tokens(
    profiles: &mut [RelayProfile],
    previous: &Map<String, Value>,
) {
    let previous_tokens = previous
        .get("relayProfiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| serde_json::from_value::<RelayProfile>(value.clone()).ok())
        .filter_map(|profile| {
            if profile.relay_mode != RelayMode::Official || !profile.official_mix_api_key {
                return None;
            }
            let token = experimental_bearer_token_from_config_text(&profile.config_contents)?;
            Some((profile.id, token))
        })
        .collect::<HashMap<_, _>>();

    for profile in profiles {
        if profile.relay_mode != RelayMode::Official || !profile.official_mix_api_key {
            continue;
        }
        if experimental_bearer_token_from_config_text(&profile.config_contents).is_some() {
            continue;
        }
        let token = if profile.api_key.trim().is_empty() {
            previous_tokens.get(&profile.id).cloned()
        } else {
            Some(profile.api_key.trim().to_string())
        };
        let Some(token) = token else {
            continue;
        };
        profile.config_contents =
            set_or_replace_experimental_bearer_token(&profile.config_contents, &token);
    }
}

fn set_or_replace_experimental_bearer_token(contents: &str, token: &str) -> String {
    let mut doc = parse_toml_document(contents).unwrap_or_else(|_| DocumentMut::new());
    let provider_id = active_provider_id(&doc).unwrap_or_else(|| "codex-plus-relay".to_string());
    doc["model_provider"] = toml_edit::value(provider_id.as_str());
    doc["model_providers"][provider_id.as_str()]["experimental_bearer_token"] =
        toml_edit::value(token.trim());
    ensure_text_newline(doc.to_string())
}

fn ensure_text_newline(mut value: String) -> String {
    if !value.is_empty() && !value.ends_with('\n') {
        value.push('\n');
    }
    value
}

fn experimental_bearer_token_from_config_text(contents: &str) -> Option<String> {
    let doc = parse_toml_document(contents).ok()?;
    let provider_id = active_provider_id(&doc)?;
    doc.get("model_providers")
        .and_then(Item::as_table)
        .and_then(|providers| providers.get(&provider_id))
        .and_then(Item::as_table)
        .and_then(|provider| provider.get("experimental_bearer_token"))
        .and_then(Item::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn active_provider_id(doc: &DocumentMut) -> Option<String> {
    doc.get("model_provider")
        .and_then(Item::as_str)
        .map(str::trim)
        .filter(|provider| !provider.is_empty())
        .map(ToString::to_string)
}

fn parse_toml_document(contents: &str) -> anyhow::Result<DocumentMut> {
    let contents = contents.trim_start_matches('\u{feff}');
    if contents.trim().is_empty() {
        Ok(DocumentMut::new())
    } else {
        contents
            .parse::<DocumentMut>()
            .map_err(|error| anyhow::anyhow!("config.toml TOML 解析失败：{error}"))
    }
}

fn settings_to_object(settings: &BackendSettings) -> Map<String, Value> {
    match serde_json::to_value(settings).unwrap_or_else(|_| Value::Object(Map::new())) {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn normalize_settings_config_sections(mut settings: BackendSettings) -> BackendSettings {
    let (common, extracted_context) =
        split_context_config_sections(&settings.relay_common_config_contents);
    let context = join_config_sections(&[
        settings.relay_context_config_contents.as_str(),
        extracted_context.as_str(),
    ]);
    settings.relay_common_config_contents = crate::relay_config::normalize_config_text(&common);
    settings.relay_context_config_contents = crate::relay_config::normalize_config_text(&context);
    for profile in &mut settings.relay_profiles {
        let _ = crate::relay_config::normalize_relay_profile_for_storage(profile);
    }
    settings.codex_app_image_overlay_opacity =
        clamp_image_overlay_opacity(settings.codex_app_image_overlay_opacity);
    settings.codex_app_image_overlay_fit_mode =
        normalize_image_overlay_fit_mode(&settings.codex_app_image_overlay_fit_mode);
    settings.codex_app_stepwise_base_url = settings
        .codex_app_stepwise_base_url
        .trim()
        .trim_end_matches('/')
        .to_string();
    settings.codex_app_stepwise_api_key = settings.codex_app_stepwise_api_key.trim().to_string();
    settings.codex_app_stepwise_api_key_env =
        if settings.codex_app_stepwise_api_key_env.trim().is_empty() {
            default_stepwise_api_key_env()
        } else {
            settings.codex_app_stepwise_api_key_env.trim().to_string()
        };
    settings.codex_app_stepwise_model = settings.codex_app_stepwise_model.trim().to_string();
    settings.codex_app_stepwise_max_items =
        clamp_stepwise_max_items(settings.codex_app_stepwise_max_items);
    settings.codex_app_stepwise_max_input_chars =
        clamp_stepwise_max_input_chars(settings.codex_app_stepwise_max_input_chars);
    settings.codex_app_stepwise_max_output_tokens =
        clamp_stepwise_max_output_tokens(settings.codex_app_stepwise_max_output_tokens);
    settings.codex_app_stepwise_timeout_ms =
        clamp_stepwise_timeout_ms(settings.codex_app_stepwise_timeout_ms);
    settings
}

fn split_context_config_sections(config: &str) -> (String, String) {
    let mut common = Vec::new();
    let mut context = Vec::new();
    let mut in_context_table = false;

    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_context_table = is_context_table_header(trimmed);
        }
        if in_context_table {
            context.push(line);
        } else {
            common.push(line);
        }
    }

    (
        normalize_text_config(common.join("\n")),
        normalize_text_config(context.join("\n")),
    )
}

fn is_context_table_header(header: &str) -> bool {
    header.starts_with("[mcp_servers.")
        || header.starts_with("[skills.")
        || header.starts_with("[plugins.")
}

fn join_config_sections(sections: &[&str]) -> String {
    let joined = sections
        .iter()
        .map(|section| section.trim())
        .filter(|section| !section.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    normalize_text_config(joined)
}

fn normalize_text_config(contents: String) -> String {
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    atomic_write_inner(
        path,
        bytes,
        |_| {},
        |_| {},
        path_file_identity_nofollow,
        |_| {},
        |_| {},
    )
}

fn atomic_write_inner(
    path: &Path,
    bytes: &[u8],
    before_publish: impl FnOnce(&Path),
    before_rename: impl FnOnce(&Path),
    initially_published_identity: impl FnOnce(&Path) -> std::io::Result<OpenFileIdentity>,
    after_publish: impl FnOnce(&Path),
    before_published_cleanup: impl Fn(&Path),
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let (temp_path, mut temp_file) = create_unique_atomic_temp(path)?;
    if let Err(error) = std::io::Write::write_all(&mut temp_file, bytes) {
        let error = anyhow::Error::new(error)
            .context(format!("failed to write temp file {}", temp_path.display()));
        let error = scrub_open_atomic_file_after_error(&mut temp_file, &temp_path, error);
        drop(temp_file);
        return Err(cleanup_atomic_temp_after_error(&temp_path, error));
    }
    if let Err(error) = temp_file.sync_all() {
        let error = anyhow::Error::new(error)
            .context(format!("failed to sync temp file {}", temp_path.display()));
        let error = scrub_open_atomic_file_after_error(&mut temp_file, &temp_path, error);
        drop(temp_file);
        return Err(cleanup_atomic_temp_after_error(&temp_path, error));
    }
    before_publish(&temp_path);
    let expected_identity = match open_file_identity(&temp_file) {
        Ok(identity) => identity,
        Err(error) => {
            let error = anyhow::Error::new(error).context(format!(
                "failed to inspect temp file {}",
                temp_path.display()
            ));
            let error = scrub_open_atomic_file_after_error(&mut temp_file, &temp_path, error);
            drop(temp_file);
            return Err(cleanup_atomic_temp_after_error(&temp_path, error));
        }
    };
    if expected_identity.links != 1 {
        let error = anyhow::anyhow!(
            "temp file has unexpected link count before replacing {}",
            path.display()
        );
        let error = scrub_open_atomic_file_after_error(&mut temp_file, &temp_path, error);
        drop(temp_file);
        return Err(cleanup_atomic_temp_after_error(&temp_path, error));
    }
    let source_identity = match path_file_identity_nofollow(&temp_path) {
        Ok(identity) => identity,
        Err(error) => {
            let error = anyhow::Error::new(error).context(format!(
                "failed to verify temp file identity before replacing {}",
                path.display()
            ));
            let error = scrub_open_atomic_file_after_error(&mut temp_file, &temp_path, error);
            drop(temp_file);
            return Err(cleanup_atomic_temp_after_error(&temp_path, error));
        }
    };
    if source_identity.links != 1 || !source_identity.same_file(expected_identity) {
        let error = anyhow::anyhow!(
            "temp file identity changed before replacing {}",
            path.display()
        );
        let error = scrub_open_atomic_file_after_error(&mut temp_file, &temp_path, error);
        drop(temp_file);
        return Err(cleanup_atomic_temp_after_error(&temp_path, error));
    }
    before_rename(&temp_path);
    if let Err(error) = fs::rename(&temp_path, path) {
        let error = anyhow::Error::new(error).context(format!(
            "failed to replace {} with {}",
            path.display(),
            temp_path.display()
        ));
        let error = scrub_open_atomic_file_after_error(&mut temp_file, &temp_path, error);
        drop(temp_file);
        return Err(cleanup_atomic_temp_after_error(&temp_path, error));
    }
    let initially_published_identity = match initially_published_identity(path) {
        Ok(identity) => identity,
        Err(error) => {
            let error = anyhow::Error::new(error).context(format!(
                "failed to verify initially published file identity: {}",
                path.display()
            ));
            let error = scrub_open_atomic_file_after_error(&mut temp_file, path, error);
            drop(temp_file);
            before_published_cleanup(path);
            return Err(quarantine_unverified_atomic_published_path(path, error));
        }
    };
    if initially_published_identity.links != 1
        || !initially_published_identity.same_file(expected_identity)
    {
        let error = anyhow::anyhow!(
            "initially published file identity changed: {}",
            path.display()
        );
        let error = scrub_open_atomic_file_after_error(&mut temp_file, path, error);
        drop(temp_file);
        before_published_cleanup(path);
        return Err(cleanup_atomic_published_after_error(
            path,
            Some(initially_published_identity),
            error,
        ));
    }
    after_publish(path);
    let published_identity = match path_file_identity_nofollow(path) {
        Ok(identity) => identity,
        Err(error) => {
            let unsafe_path = error.kind() == std::io::ErrorKind::InvalidData;
            let error = anyhow::Error::new(error).context(format!(
                "failed to verify published file identity: {}",
                path.display()
            ));
            let error = scrub_open_atomic_file_after_error(&mut temp_file, path, error);
            drop(temp_file);
            return if unsafe_path {
                before_published_cleanup(path);
                Err(cleanup_atomic_published_after_error(path, None, error))
            } else {
                Err(error)
            };
        }
    };
    if published_identity.links != 1 {
        let error = anyhow::anyhow!("published file identity changed: {}", path.display());
        let error = scrub_open_atomic_file_after_error(&mut temp_file, path, error);
        drop(temp_file);
        before_published_cleanup(path);
        return Err(cleanup_atomic_published_after_error(
            path,
            Some(published_identity),
            error,
        ));
    }
    if !published_identity.same_file(expected_identity) {
        let error = anyhow::anyhow!("published file identity changed: {}", path.display());
        let error = scrub_open_atomic_file_after_error(&mut temp_file, path, error);
        drop(temp_file);
        return Err(error);
    }
    drop(temp_file);
    Ok(())
}

#[cfg(test)]
fn atomic_write_with_before_publish_hook(
    path: &Path,
    bytes: &[u8],
    before_publish: impl FnOnce(&Path),
) -> anyhow::Result<()> {
    atomic_write_inner(
        path,
        bytes,
        before_publish,
        |_| {},
        path_file_identity_nofollow,
        |_| {},
        |_| {},
    )
}

#[cfg(test)]
fn atomic_write_with_before_rename_hook(
    path: &Path,
    bytes: &[u8],
    before_rename: impl FnOnce(&Path),
) -> anyhow::Result<()> {
    atomic_write_inner(
        path,
        bytes,
        |_| {},
        before_rename,
        path_file_identity_nofollow,
        |_| {},
        |_| {},
    )
}

#[cfg(test)]
fn atomic_write_with_before_rename_and_initial_identity_hook(
    path: &Path,
    bytes: &[u8],
    before_rename: impl FnOnce(&Path),
    initially_published_identity: impl FnOnce(&Path) -> std::io::Result<OpenFileIdentity>,
) -> anyhow::Result<()> {
    atomic_write_inner(
        path,
        bytes,
        |_| {},
        before_rename,
        initially_published_identity,
        |_| {},
        |_| {},
    )
}

#[cfg(test)]
fn atomic_write_with_after_publish_hook(
    path: &Path,
    bytes: &[u8],
    after_publish: impl FnOnce(&Path),
) -> anyhow::Result<()> {
    atomic_write_inner(
        path,
        bytes,
        |_| {},
        |_| {},
        path_file_identity_nofollow,
        after_publish,
        |_| {},
    )
}

#[cfg(test)]
fn atomic_write_with_publish_cleanup_hook(
    path: &Path,
    bytes: &[u8],
    after_publish: impl FnOnce(&Path),
    before_published_cleanup: impl Fn(&Path),
) -> anyhow::Result<()> {
    atomic_write_inner(
        path,
        bytes,
        |_| {},
        |_| {},
        path_file_identity_nofollow,
        after_publish,
        before_published_cleanup,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenFileIdentity {
    volume: u64,
    index: u64,
    links: u64,
}

impl OpenFileIdentity {
    fn same_file(self, other: Self) -> bool {
        self.volume == other.volume && self.index == other.index
    }
}

#[cfg(unix)]
fn open_file_identity(file: &fs::File) -> std::io::Result<OpenFileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata()?;
    Ok(OpenFileIdentity {
        volume: metadata.dev(),
        index: metadata.ino(),
        links: metadata.nlink(),
    })
}

#[cfg(windows)]
fn open_file_identity(file: &fs::File) -> std::io::Result<OpenFileIdentity> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut info = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let handle = HANDLE(file.as_raw_handle());
    unsafe { GetFileInformationByHandle(handle, info.as_mut_ptr()) }
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let info = unsafe { info.assume_init() };
    Ok(OpenFileIdentity {
        volume: u64::from(info.dwVolumeSerialNumber),
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        links: u64::from(info.nNumberOfLinks),
    })
}

#[cfg(unix)]
fn path_file_identity_nofollow(path: &Path) -> std::io::Result<OpenFileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "atomic temp path is a symlink",
        ));
    }
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "atomic temp path is not a regular file",
        ));
    }
    Ok(OpenFileIdentity {
        volume: metadata.dev(),
        index: metadata.ino(),
        links: metadata.nlink(),
    })
}

#[cfg(windows)]
fn path_file_identity_nofollow(path: &Path) -> std::io::Result<OpenFileIdentity> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};

    const FILE_READ_ATTRIBUTES: u32 = 0x0000_0080;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    let mut options = fs::OpenOptions::new();
    options
        .access_mode(FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 || !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "atomic temp path is not a regular non-reparse file",
        ));
    }
    open_file_identity(&file)
}

static NEXT_ATOMIC_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn create_unique_atomic_temp(path: &Path) -> anyhow::Result<(PathBuf, fs::File)> {
    const MAX_ATTEMPTS: usize = 128;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("atomic-write"));

    for _ in 0..MAX_ATTEMPTS {
        let id = NEXT_ATOMIC_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut temp_name = std::ffi::OsString::from(".");
        temp_name.push(file_name);
        temp_name.push(format!(".tmp-{}-{timestamp}-{id}", std::process::id()));
        let temp_path = parent.join(temp_name);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        match options.open(&temp_path) {
            Ok(file) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Err(error) = file.set_permissions(std::fs::Permissions::from_mode(0o600))
                    {
                        drop(file);
                        let error = anyhow::Error::new(error).context(format!(
                            "failed to secure temp file {}",
                            temp_path.display()
                        ));
                        return Err(cleanup_atomic_temp_after_error(&temp_path, error));
                    }
                }
                return Ok((temp_path, file));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(anyhow::Error::new(error).context(format!(
                    "failed to create temp file {}",
                    temp_path.display()
                )));
            }
        }
    }

    anyhow::bail!(
        "failed to create unique temp file for {} after {MAX_ATTEMPTS} attempts",
        path.display()
    )
}

fn cleanup_atomic_temp_after_error(temp_path: &Path, error: anyhow::Error) -> anyhow::Error {
    match fs::remove_file(temp_path) {
        Ok(()) => error,
        Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => error,
        Err(cleanup_error) => error.context(format!(
            "failed to remove temp file {} after write failure: {cleanup_error}",
            temp_path.display()
        )),
    }
}

fn scrub_open_atomic_file_after_error(
    file: &mut fs::File,
    path: &Path,
    error: anyhow::Error,
) -> anyhow::Error {
    if let Err(scrub_error) = file.set_len(0).and_then(|()| file.sync_all()) {
        error.context(format!(
            "failed to scrub open atomic file {} after validation failure: {scrub_error}",
            path.display()
        ))
    } else {
        error
    }
}

fn cleanup_atomic_published_after_error(
    path: &Path,
    unsafe_identity: Option<OpenFileIdentity>,
    error: anyhow::Error,
) -> anyhow::Error {
    cleanup_atomic_published_after_error_inner(path, unsafe_identity, error, |_| {})
}

fn cleanup_atomic_published_after_error_inner(
    path: &Path,
    unsafe_identity: Option<OpenFileIdentity>,
    error: anyhow::Error,
    after_identity_verification: impl FnOnce(&Path),
) -> anyhow::Error {
    let quarantine_path = match quarantine_atomic_published_path(path) {
        Ok(Some(path)) => path,
        Ok(None) => return error,
        Err(cleanup_error) => {
            return error.context(format!(
                "failed to quarantine unsafe published file {}: {cleanup_error}",
                path.display()
            ));
        }
    };

    let quarantined_is_unsafe = match path_file_identity_nofollow(&quarantine_path) {
        Ok(identity) => unsafe_identity
            .map(|expected| identity.same_file(expected))
            .unwrap_or(identity.links != 1),
        Err(check_error) if check_error.kind() == std::io::ErrorKind::InvalidData => true,
        Err(check_error) => {
            return restore_quarantined_atomic_path(
                path,
                &quarantine_path,
                error.context(format!(
                    "failed to verify quarantined published file {}: {check_error}",
                    quarantine_path.display()
                )),
            );
        }
    };

    if !quarantined_is_unsafe {
        return restore_quarantined_atomic_path(path, &quarantine_path, error);
    }

    after_identity_verification(&quarantine_path);
    error.context(format!(
        "unsafe published file was retained outside the destination at {}",
        quarantine_path.display()
    ))
}

fn quarantine_unverified_atomic_published_path(path: &Path, error: anyhow::Error) -> anyhow::Error {
    match quarantine_atomic_published_path(path) {
        Ok(Some(quarantine_path)) => error.context(format!(
            "unverified published file was retained outside the destination at {}",
            quarantine_path.display()
        )),
        Ok(None) => error,
        Err(quarantine_error) => error.context(format!(
            "failed to quarantine unverified published file {}: {quarantine_error}",
            path.display()
        )),
    }
}

fn quarantine_atomic_published_path(path: &Path) -> std::io::Result<Option<PathBuf>> {
    const MAX_ATTEMPTS: usize = 128;
    for _ in 0..MAX_ATTEMPTS {
        let quarantine_path = unique_atomic_sidecar_path(path, "quarantine");
        match rename_noreplace(path, &quarantine_path) {
            Ok(()) => return Ok(Some(quarantine_path)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "failed to allocate atomic quarantine path",
    ))
}

pub(crate) fn quarantine_corrupt_state_file(path: &Path) -> anyhow::Result<Option<PathBuf>> {
    quarantine_atomic_published_path(path)
        .with_context(|| format!("failed to quarantine corrupt state file {}", path.display()))
}

fn restore_quarantined_atomic_path(
    path: &Path,
    quarantine_path: &Path,
    error: anyhow::Error,
) -> anyhow::Error {
    match rename_noreplace(quarantine_path, path) {
        Ok(()) => error,
        Err(restore_error) => error.context(format!(
            "preserved concurrently replaced file at {} because restoring {} failed: {restore_error}",
            quarantine_path.display(),
            path.display()
        )),
    }
}

#[cfg(windows)]
fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::MoveFileW;
    use windows::core::PCWSTR;

    let from_wide = from
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let to_wide = to
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe { MoveFileW(PCWSTR(from_wide.as_ptr()), PCWSTR(to_wide.as_ptr())) }
        .map_err(|_| std::io::Error::last_os_error())
}

#[cfg(target_os = "linux")]
fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    const AT_FDCWD: i32 = -100;
    const RENAME_NOREPLACE: u32 = 1;
    unsafe extern "C" {
        fn renameat2(
            olddirfd: i32,
            oldpath: *const std::ffi::c_char,
            newdirfd: i32,
            newpath: *const std::ffi::c_char,
            flags: u32,
        ) -> i32;
    }

    let from = CString::new(from.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in source path"))?;
    let to = CString::new(to.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in destination path")
    })?;
    let result = unsafe {
        renameat2(
            AT_FDCWD,
            from.as_ptr(),
            AT_FDCWD,
            to.as_ptr(),
            RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    const RENAME_EXCL: u32 = 0x0000_0004;
    unsafe extern "C" {
        fn renamex_np(
            from: *const std::ffi::c_char,
            to: *const std::ffi::c_char,
            flags: u32,
        ) -> i32;
    }

    let from = CString::new(from.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in source path"))?;
    let to = CString::new(to.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "NUL in destination path")
    })?;
    let result = unsafe { renamex_np(from.as_ptr(), to.as_ptr(), RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::hard_link(from, to)?;
    fs::remove_file(from)
}

fn unique_atomic_sidecar_path(path: &Path, kind: &str) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("atomic-write"));
    let id = NEXT_ATOMIC_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut sidecar_name = std::ffi::OsString::from(".");
    sidecar_name.push(file_name);
    sidecar_name.push(format!(".{kind}-{}-{timestamp}-{id}", std::process::id()));
    parent.join(sidecar_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct TestTempDir(tempfile::TempDir);

    impl std::ops::Deref for TestTempDir {
        type Target = Path;

        fn deref(&self) -> &Self::Target {
            self.0.path()
        }
    }

    impl AsRef<Path> for TestTempDir {
        fn as_ref(&self) -> &Path {
            self.0.path()
        }
    }

    fn temp_dir() -> TestTempDir {
        TestTempDir(tempfile::tempdir().unwrap())
    }

    #[test]
    fn settings_default_matches_expected_behavior() {
        let settings = BackendSettings::default();
        assert!(!settings.provider_sync_enabled);
        assert!(settings.relay_profiles_enabled);
        assert!(settings.enhancements_enabled);
        assert!(!settings.computer_use_guard_enabled);
        assert!(settings.codex_app_plugin_marketplace_unlock);
        assert!(settings.codex_app_plugin_auto_expand);
        assert!(!settings.codex_app_thread_id_badge);
        assert!(settings.codex_app_force_chinese_locale);
        assert!(!settings.codex_goals_enabled);
        assert!(settings.codex_app_path.is_empty());
        assert!(settings.codex_extra_args.is_empty());
        assert_eq!(
            settings.zed_remote_open_strategy,
            ZedOpenStrategy::AddToFocusedWorkspace
        );
        assert!(settings.zed_remote_project_registry_enabled);
        assert!(!settings.zed_remote_sync_to_zed_settings);
        assert!(settings.codex_app_native_menu_localization);
        assert_eq!(settings.launch_mode, LaunchMode::Patch);
        assert_eq!(settings.relay_base_url, default_relay_base_url());
        assert!(settings.relay_api_key.is_empty());
        assert_eq!(settings.relay_profiles[0].relay_mode, RelayMode::Official);
        assert!(settings.relay_common_config_contents.is_empty());
        assert_eq!(settings.relay_test_model, default_relay_test_model());
        assert!(!settings.codex_app_stepwise_enabled);
        assert!(!settings.codex_app_stepwise_direct_send);
        assert!(settings.codex_app_stepwise_base_url.is_empty());
        assert!(settings.codex_app_stepwise_api_key.is_empty());
        assert_eq!(
            settings.codex_app_stepwise_api_key_env,
            "CODEX_STEPWISE_API_KEY"
        );
        assert!(settings.codex_app_stepwise_model.is_empty());
        assert_eq!(settings.codex_app_stepwise_max_items, 6);
        assert_eq!(settings.codex_app_stepwise_max_input_chars, 6000);
        assert_eq!(settings.codex_app_stepwise_max_output_tokens, 500);
        assert_eq!(settings.codex_app_stepwise_timeout_ms, 8000);
    }

    #[test]
    fn atomic_write_removes_secret_temp_file_when_replace_fails() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        std::fs::create_dir(&path).unwrap();
        let secret = b"sk-must-not-remain-in-temp";

        atomic_write(&path, secret).expect_err("replacing a directory should fail");

        assert!(path.is_dir());
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            if entry.path().is_file() {
                assert!(
                    !std::fs::read(entry.path())
                        .unwrap()
                        .windows(secret.len())
                        .any(|v| v == secret)
                );
            }
        }
    }

    #[test]
    fn atomic_write_ignores_legacy_fixed_temp_path_collision() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let legacy_temp = dir.join("settings.json.tmp");
        std::fs::create_dir(&legacy_temp).unwrap();

        atomic_write(&path, b"{\"secret\":\"new\"}").unwrap();

        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"{\"secret\":\"new\"}".as_slice()
        );
        assert!(legacy_temp.is_dir(), "不得复用可预测的旧临时路径");
    }

    #[test]
    fn atomic_write_opens_temp_with_create_new() {
        let source = include_str!("settings.rs");
        let atomic_write_source = source
            .split("pub(crate) fn atomic_write")
            .nth(1)
            .and_then(|source| source.split("fn cleanup_atomic_temp_after_error").next())
            .expect("atomic_write source");

        assert!(
            atomic_write_source.contains(".create_new(true)"),
            "临时文件必须以 create_new 打开，禁止覆盖或跟随已有路径"
        );
    }

    #[test]
    fn atomic_rename_noreplace_preserves_an_existing_destination() {
        let dir = temp_dir();
        let source = dir.join("source");
        let destination = dir.join("destination");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&destination, b"destination").unwrap();

        assert!(rename_noreplace(&source, &destination).is_err());
        assert_eq!(std::fs::read(source).unwrap(), b"source");
        assert_eq!(std::fs::read(destination).unwrap(), b"destination");
    }

    #[test]
    fn atomic_write_rejects_a_swapped_temp_path_before_publish() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        std::fs::write(&path, b"original").unwrap();

        let result = atomic_write_with_before_publish_hook(&path, b"trusted", |temp_path| {
            let stolen = dir.join("stolen-temp");
            std::fs::rename(temp_path, &stolen).unwrap();
            std::fs::write(temp_path, b"attacker").unwrap();
        });

        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
    }

    #[test]
    fn atomic_write_never_leaves_swapped_bytes_published_after_source_verification() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let stolen = dir.join("stolen-temp");
        let attacker = b"attacker-after-source-verification";
        std::fs::write(&path, b"original").unwrap();

        let result = atomic_write_with_before_rename_hook(&path, b"trusted", |temp_path| {
            std::fs::rename(temp_path, &stolen).unwrap();
            std::fs::write(temp_path, attacker).unwrap();
        });

        assert!(result.is_err());
        assert!(
            std::fs::read(&path).map_or(true, |bytes| bytes != attacker),
            "source-path replacement bytes must never remain at the destination"
        );
        assert_eq!(std::fs::read(stolen).unwrap(), b"");
    }

    #[test]
    fn atomic_write_quarantines_an_unverifiable_initially_published_path() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let stolen = dir.join("stolen-temp");
        let attacker = b"attacker-with-unverifiable-identity";
        std::fs::write(&path, b"original").unwrap();

        let result = atomic_write_with_before_rename_and_initial_identity_hook(
            &path,
            b"trusted",
            |temp_path| {
                std::fs::rename(temp_path, &stolen).unwrap();
                std::fs::write(temp_path, attacker).unwrap();
            },
            |_| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "injected identity denial",
                ))
            },
        );

        assert!(result.is_err());
        assert!(
            std::fs::read(&path).map_or(true, |bytes| bytes != attacker),
            "unverifiable published bytes must not remain at the destination"
        );
        assert_eq!(std::fs::read(stolen).unwrap(), b"");
    }

    #[test]
    fn atomic_write_rejects_temp_files_with_attacker_hardlinks() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let attacker_link = dir.join("attacker-link");
        std::fs::write(&path, b"original").unwrap();

        let result = atomic_write_with_before_publish_hook(&path, b"trusted", |temp_path| {
            std::fs::hard_link(temp_path, &attacker_link).unwrap();
        });

        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert_eq!(std::fs::read(attacker_link).unwrap(), b"");
    }

    #[test]
    fn atomic_write_scrubs_a_hardlink_added_after_publish() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let attacker_link = dir.join("attacker-link");
        std::fs::write(&path, b"original").unwrap();

        let result = atomic_write_with_after_publish_hook(&path, b"trusted", |published_path| {
            std::fs::hard_link(published_path, &attacker_link).unwrap();
        });

        assert!(result.is_err());
        assert!(!path.exists());
        assert_eq!(std::fs::read(attacker_link).unwrap(), b"");
    }

    #[test]
    fn atomic_write_does_not_delete_a_concurrent_safe_replacement() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let displaced = dir.join("displaced-first-writer");
        std::fs::write(&path, b"original").unwrap();

        let result = atomic_write_with_after_publish_hook(&path, b"trusted", |published_path| {
            std::fs::rename(published_path, &displaced).unwrap();
            std::fs::write(published_path, b"concurrent").unwrap();
        });

        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"concurrent");
        assert_eq!(std::fs::read(displaced).unwrap(), b"");
    }

    #[test]
    fn atomic_write_cleanup_does_not_delete_a_late_safe_replacement() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let attacker_link = dir.join("attacker-link");
        let displaced_unsafe = dir.join("displaced-unsafe");
        std::fs::write(&path, b"original").unwrap();

        let result = atomic_write_with_publish_cleanup_hook(
            &path,
            b"trusted",
            |published_path| {
                std::fs::hard_link(published_path, &attacker_link).unwrap();
            },
            |published_path| {
                std::fs::rename(published_path, &displaced_unsafe).unwrap();
                std::fs::write(published_path, b"late-concurrent").unwrap();
            },
        );

        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"late-concurrent");
        assert_eq!(std::fs::read(attacker_link).unwrap(), b"");
        assert_eq!(std::fs::read(displaced_unsafe).unwrap(), b"");
    }

    #[test]
    fn atomic_quarantine_never_deletes_a_replacement_after_identity_verification() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let displaced = dir.join("displaced-quarantine");
        let replacement_path = std::cell::RefCell::new(None::<PathBuf>);
        std::fs::write(&path, b"unsafe-published").unwrap();
        let unsafe_identity = path_file_identity_nofollow(&path).unwrap();

        let error = cleanup_atomic_published_after_error_inner(
            &path,
            Some(unsafe_identity),
            anyhow::anyhow!("injected validation failure"),
            |quarantine_path| {
                std::fs::rename(quarantine_path, &displaced).unwrap();
                std::fs::write(quarantine_path, b"safe-replacement").unwrap();
                *replacement_path.borrow_mut() = Some(quarantine_path.to_path_buf());
            },
        );

        assert!(format!("{error:#}").contains("injected validation failure"));
        assert_eq!(std::fs::read(displaced).unwrap(), b"unsafe-published");
        assert_eq!(
            std::fs::read(replacement_path.into_inner().unwrap()).unwrap(),
            b"safe-replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_path_identity_ignores_content_read_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir();
        let path = dir.join("mode-000");
        let file = std::fs::File::create(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();

        let open_identity = open_file_identity(&file).unwrap();
        let path_identity = path_file_identity_nofollow(&path).unwrap();

        assert!(path_identity.same_file(open_identity));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn atomic_path_identity_rejects_fifo_without_blocking() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        unsafe extern "C" {
            fn mkfifo(pathname: *const std::ffi::c_char, mode: u32) -> i32;
        }

        let dir = temp_dir();
        let path = dir.join("fifo");
        let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { mkfifo(c_path.as_ptr(), 0o600) }, 0);

        let started = std::time::Instant::now();
        let error = path_file_identity_nofollow(&path).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_rejects_a_symlink_to_the_original_temp_inode() {
        use std::os::unix::fs::symlink;

        let dir = temp_dir();
        let path = dir.join("settings.json");
        std::fs::write(&path, b"original").unwrap();

        let result = atomic_write_with_before_publish_hook(&path, b"trusted", |temp_path| {
            let stolen = dir.join("stolen-temp");
            std::fs::rename(temp_path, &stolen).unwrap();
            symlink(&stolen, temp_path).unwrap();
        });

        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_creates_secret_file_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir();
        let path = dir.join("auth.json");

        atomic_write(&path, b"{\"OPENAI_API_KEY\":\"sk-test\"}").unwrap();

        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn settings_deserialize_ignores_removed_cli_wrapper_keys() {
        let settings: BackendSettings = serde_json::from_str(
            r#"{"codexAppPath":"C:\\Portable\\Codex\\app","providerSyncEnabled":true,"codexGoalsEnabled":true,"cliWrapperEnabled":true,"cliWrapperBaseUrl":"https://example.test","cliWrapperApiKey":"sk-test","cliWrapperApiKeyEnv":""}"#,
        )
        .unwrap();
        assert_eq!(settings.codex_app_path, r"C:\Portable\Codex\app");
        assert!(settings.provider_sync_enabled);
        assert!(settings.codex_goals_enabled);
        assert_eq!(settings.relay_base_url, default_relay_base_url());
        assert!(settings.codex_extra_args.is_empty());
        let saved = serde_json::to_value(&settings).unwrap();
        assert!(saved.get("cliWrapperEnabled").is_none());
        assert!(saved.get("cliWrapperBaseUrl").is_none());
        assert!(saved.get("cliWrapperApiKey").is_none());
        assert!(saved.get("cliWrapperApiKeyEnv").is_none());
    }

    #[test]
    fn settings_deserialize_keeps_plugin_marketplace_unlock_switch() {
        let settings: BackendSettings = serde_json::from_str(
            r#"{
                "codexAppPluginMarketplaceUnlock": true,
                "codexAppPluginAutoExpand": false
            }"#,
        )
        .unwrap();

        assert!(settings.codex_app_plugin_marketplace_unlock);
        assert!(!settings.codex_app_plugin_auto_expand);

        let legacy_settings: BackendSettings = serde_json::from_str(
            r#"{
                "codexAppForcePluginInstall": false
            }"#,
        )
        .unwrap();

        assert!(legacy_settings.codex_app_plugin_marketplace_unlock);
        assert!(legacy_settings.codex_app_plugin_auto_expand);
    }

    #[test]
    fn settings_deserialize_reads_codex_extra_args() {
        let settings: BackendSettings = serde_json::from_str(
            r#"{"codexExtraArgs":["--force_high_performance_gpu"," --ignored-trimmed-by-ui "]}"#,
        )
        .unwrap();

        assert_eq!(
            settings.codex_extra_args,
            vec![
                "--force_high_performance_gpu".to_string(),
                " --ignored-trimmed-by-ui ".to_string(),
            ]
        );
    }

    #[test]
    fn relay_profile_official_mix_api_key_defaults_to_false() {
        let profile: RelayProfile =
            serde_json::from_str(r#"{"id":"official","name":"官方","relayMode":"official"}"#)
                .unwrap();

        assert_eq!(profile.relay_mode, RelayMode::Official);
        assert!(!profile.official_mix_api_key);
        assert!(profile.test_model.is_empty());
    }

    #[test]
    fn relay_profile_context_fields_default_to_empty() {
        let profile = RelayProfile::default();

        assert!(profile.context_selection.mcp_servers.is_empty());
        assert!(profile.context_selection.skills.is_empty());
        assert!(profile.context_selection.plugins.is_empty());
        assert!(profile.use_common_config);
        assert!(!profile.context_selection_initialized);
        assert!(profile.context_window.is_empty());
        assert!(profile.auto_compact_limit.is_empty());
        assert_eq!(profile.model_insert_mode, RelayModelInsertMode::Patch);
        assert!(profile.model_list.is_empty());
    }

    #[test]
    fn relay_profile_context_fields_deserialize_from_camel_case() {
        let profile: RelayProfile = serde_json::from_str(
            r#"{
                "id":"relay-a",
                "name":"供应商 A",
                "contextSelection":{
                    "mcpServers":["context7"],
                    "skills":["writer"],
                    "plugins":["local"]
                },
                "contextSelectionInitialized":true,
                "useCommonConfig":false,
                "contextWindow":"200000",
                "autoCompactLimit":"160000",
                "modelInsertMode":"patch",
                "modelList":"qwen3-coder\ndeepseek-coder"
            }"#,
        )
        .unwrap();

        assert_eq!(profile.context_selection.mcp_servers, vec!["context7"]);
        assert_eq!(profile.context_selection.skills, vec!["writer"]);
        assert_eq!(profile.context_selection.plugins, vec!["local"]);
        assert!(!profile.use_common_config);
        assert!(profile.context_selection_initialized);
        assert_eq!(profile.context_window, "200000");
        assert_eq!(profile.auto_compact_limit, "160000");
        assert_eq!(profile.model_insert_mode, RelayModelInsertMode::Patch);
        assert_eq!(profile.model_list, "qwen3-coder\ndeepseek-coder");
    }

    #[test]
    fn relay_profile_derived_fields_are_read_but_not_serialized() {
        let profile: RelayProfile = serde_json::from_str(
            r#"{
                "id":"relay-a",
                "name":"供应商 A",
                "model":"gpt-5.4",
                "baseUrl":"https://relay.example/v1",
                "apiKey":"sk-test",
                "configContents":"model = \"gpt-5.4\"\n",
                "authContents":"{\"OPENAI_API_KEY\":\"sk-test\"}"
            }"#,
        )
        .unwrap();

        assert_eq!(profile.model, "gpt-5.4");
        assert_eq!(profile.base_url, "https://relay.example/v1");
        assert_eq!(profile.api_key, "sk-test");

        let saved = serde_json::to_value(&profile).unwrap();
        assert!(saved.get("model").is_none());
        assert!(saved.get("baseUrl").is_none());
        assert!(saved.get("apiKey").is_none());
        assert_eq!(saved["configContents"], "model = \"gpt-5.4\"\n");
        assert_eq!(saved["authContents"], "{\"OPENAI_API_KEY\":\"sk-test\"}");
    }

    #[test]
    fn chat_protocol_profile_roundtrip_migrates_upstream_base_url_out_of_config() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));
        let settings = BackendSettings {
            relay_profiles: vec![RelayProfile {
                id: "relay-chat".to_string(),
                name: "DeepSeek".to_string(),
                protocol: RelayProtocol::ChatCompletions,
                relay_mode: RelayMode::PureApi,
                config_contents: r#"model = "deepseek-chat"
codex_plus_chat_base_url = "https://api.deepseek.com"
model_provider = "custom"

[model_providers.custom]
name = "custom"
wire_api = "responses"
requires_openai_auth = true
base_url = "http://127.0.0.1:57321/v1"
"#
                .to_string(),
                auth_contents: r#"{"OPENAI_API_KEY":"sk-test"}"#.to_string(),
                ..RelayProfile::default()
            }],
            active_relay_id: "relay-chat".to_string(),
            ..BackendSettings::default()
        };

        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        let active = loaded.active_relay_profile();

        assert_eq!(active.protocol, RelayProtocol::ChatCompletions);
        assert_eq!(active.base_url, "https://api.deepseek.com");
        assert_eq!(active.upstream_base_url, "https://api.deepseek.com");
        assert_eq!(active.api_key, "sk-test");
        assert!(!active.config_contents.contains("codex_plus_chat_base_url"));

        let saved: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap())
                .unwrap();
        let profile = &saved["relayProfiles"][0];
        assert!(profile.get("baseUrl").is_none());
        assert_eq!(profile["upstreamBaseUrl"], "https://api.deepseek.com");
        assert!(profile.get("apiKey").is_none());
        assert!(
            !profile["configContents"]
                .as_str()
                .unwrap()
                .contains("codex_plus_chat_base_url")
        );
    }

    #[test]
    fn official_profile_without_mix_does_not_persist_api_config() {
        let settings = BackendSettings {
            relay_profiles: vec![RelayProfile {
                id: "official".to_string(),
                name: "官方".to_string(),
                relay_mode: RelayMode::Official,
                official_mix_api_key: false,
                model: "gpt-5.5".to_string(),
                base_url: "https://relay.example/v1".to_string(),
                api_key: "sk-test".to_string(),
                config_contents: r#"model = "gpt-5.5"
model_provider = "custom"

[model_providers.custom]
requires_openai_auth = true
"#
                .to_string(),
                auth_contents: r#"{"OPENAI_API_KEY":"sk-test"}"#.to_string(),
                ..RelayProfile::default()
            }],
            active_relay_id: "official".to_string(),
            ..BackendSettings::default()
        };

        let value = settings_to_object(&normalize_settings_config_sections(settings));
        let profile = &value["relayProfiles"][0];
        assert_eq!(profile["relayMode"], "official");
        assert_eq!(profile["officialMixApiKey"], false);
        assert_eq!(profile["configContents"], "");
        assert_eq!(profile["authContents"], "");
        assert!(profile.get("model").is_none());
        assert!(profile.get("baseUrl").is_none());
        assert!(profile.get("apiKey").is_none());
    }

    #[test]
    fn official_mix_profile_keeps_key_in_config_not_auth() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));
        let settings = BackendSettings {
            relay_profiles: vec![RelayProfile {
                id: "official-mix".to_string(),
                name: "官方混入".to_string(),
                relay_mode: RelayMode::Official,
                official_mix_api_key: true,
                model: "gpt-5.5".to_string(),
                base_url: "https://relay.example/v1".to_string(),
                api_key: "sk-mix".to_string(),
                config_contents: r#"model = "gpt-5.5"
model_provider = "custom"

[model_providers.custom]
requires_openai_auth = true
base_url = "https://relay.example/v1"
experimental_bearer_token = "sk-mix"
"#
                .to_string(),
                auth_contents: r#"{"OPENAI_API_KEY":"sk-mix","auth_mode":"chatgpt"}"#.to_string(),
                ..RelayProfile::default()
            }],
            active_relay_id: "official-mix".to_string(),
            ..BackendSettings::default()
        };

        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        let profile = &loaded.relay_profiles[0];

        assert_eq!(profile.relay_mode, RelayMode::Official);
        assert!(profile.official_mix_api_key);
        assert_eq!(profile.api_key, "sk-mix");
        assert!(!profile.auth_contents.contains("OPENAI_API_KEY"));
        assert!(
            profile
                .config_contents
                .contains(r#"experimental_bearer_token = "sk-mix""#)
        );

        let saved: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap())
                .unwrap();
        assert!(saved["relayProfiles"][0].get("apiKey").is_none());
        assert!(
            !saved["relayProfiles"][0]["authContents"]
                .as_str()
                .unwrap()
                .contains("OPENAI_API_KEY")
        );
        assert!(
            saved["relayProfiles"][0]["configContents"]
                .as_str()
                .unwrap()
                .contains(r#"experimental_bearer_token = "sk-mix""#)
        );
    }

    #[test]
    fn settings_update_preserves_official_mix_key_when_payload_loses_it() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));
        store
            .save(&BackendSettings {
                relay_profiles: vec![RelayProfile {
                    id: "official-mix".to_string(),
                    name: "官方混入".to_string(),
                    relay_mode: RelayMode::Official,
                    official_mix_api_key: true,
                    config_contents: r#"model_provider = "custom"

[model_providers.other]
base_url = "https://other.example/v1"
experimental_bearer_token = "sk-other"

[model_providers.custom]
base_url = "https://relay.example/v1"
experimental_bearer_token = "sk-existing"
"#
                    .to_string(),
                    ..RelayProfile::default()
                }],
                active_relay_id: "official-mix".to_string(),
                ..BackendSettings::default()
            })
            .unwrap();

        let updated = store
            .update(json!({
                "relayProfiles": [{
                    "id": "official-mix",
                    "name": "官方混入",
                    "relayMode": "official",
                    "officialMixApiKey": true,
                    "configContents": "model_provider = \"custom\"\n\n[model_providers.other]\nbase_url = \"https://other.example/v1\"\nexperimental_bearer_token = \"sk-other\"\n\n[model_providers.custom]\nbase_url = \"https://relay.example/v1\"\nexperimental_bearer_token = \"\"\n",
                    "authContents": ""
                }],
                "activeRelayId": "official-mix"
            }))
            .unwrap();

        let profile = &updated.relay_profiles[0];
        assert_eq!(profile.api_key, "sk-existing");
        assert!(!profile.config_contents.contains("sk-other"));
        assert!(profile.config_contents.contains(
            r#"[model_providers.custom]
base_url = "https://relay.example/v1"
experimental_bearer_token = "sk-existing""#
        ));
    }

    #[test]
    fn official_mix_update_uses_api_key_when_config_token_missing() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "relayProfiles": [{
                    "id": "official-mix",
                    "name": "官方混入",
                    "relayMode": "official",
                    "officialMixApiKey": true,
                    "baseUrl": "https://relay.example/v1",
                    "apiKey": "sk-new",
                    "configContents": "model_provider = \"custom\"\n\n[model_providers.custom]\nbase_url = \"https://relay.example/v1\"\n",
                    "authContents": ""
                }],
                "activeRelayId": "official-mix"
            }))
            .unwrap();

        let profile = &updated.relay_profiles[0];
        assert_eq!(profile.api_key, "sk-new");
        assert!(
            profile
                .config_contents
                .contains(r#"experimental_bearer_token = "sk-new""#)
        );
        assert!(!profile.auth_contents.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn settings_update_preserves_manual_official_mix_config_token() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "relayProfiles": [{
                    "id": "official-mix",
                    "name": "官方混入",
                    "relayMode": "official",
                    "officialMixApiKey": true,
                    "configContents": "model_provider = \"custom\"\n\n[model_providers.custom]\nbase_url = \"https://relay.example/v1\"\nexperimental_bearer_token = \"22222222222222222222222222222222222\"\n",
                    "authContents": ""
                }],
                "activeRelayId": "official-mix"
            }))
            .unwrap();

        let profile = &updated.relay_profiles[0];
        assert_eq!(profile.relay_mode, RelayMode::Official);
        assert!(profile.official_mix_api_key);
        assert_eq!(profile.api_key, "22222222222222222222222222222222222");
        assert!(
            profile
                .config_contents
                .contains(r#"experimental_bearer_token = "22222222222222222222222222222222222""#)
        );
        assert!(!profile.auth_contents.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn settings_store_load_missing_file_returns_chimera_first_run_without_writing() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let codex_home = dir.join("codex-home");
        std::fs::create_dir_all(&codex_home).unwrap();
        let store = SettingsStore::new_with_codex_home(path.clone(), codex_home);

        let loaded = store.load().unwrap();

        assert!(
            !path.exists(),
            "missing settings must not be created on load"
        );
        assert_eq!(loaded, chimera_first_run_settings());
        assert_eq!(loaded.active_relay_id, "chimerahub");
        assert!(loaded.relay_profiles_enabled);
        assert_eq!(loaded.relay_profiles.len(), 1);
        let profile = &loaded.relay_profiles[0];
        assert_eq!(profile.id, "chimerahub");
        assert_eq!(profile.name, "ChimeraHub");
        assert_eq!(profile.relay_mode, RelayMode::PureApi);
        assert_eq!(profile.protocol, RelayProtocol::Responses);
        assert_eq!(profile.base_url, crate::branding::DEFAULT_RELAY_BASE_URL);
        assert_eq!(profile.model, crate::branding::DEFAULT_RELAY_MODEL);
        assert!(profile.api_key.is_empty());
        assert!(profile.config_contents.is_empty());
        assert!(profile.auth_contents.is_empty());
    }

    #[test]
    fn settings_store_new_does_not_probe_the_real_codex_home() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        assert!(
            store.codex_home.is_none(),
            "an explicitly injected settings path must not inspect the process default CODEX_HOME"
        );
        assert_eq!(
            store.missing_settings_defaults(),
            chimera_first_run_settings()
        );
    }

    #[test]
    fn settings_store_missing_file_preserves_nonempty_codex_home_without_writing() {
        for (file_name, contents) in [
            ("config.toml", "model_provider = \"existing\"\n"),
            ("auth.json", "{\"auth_mode\":\"chatgpt\"}\n"),
        ] {
            let dir = temp_dir();
            let path = dir.join("settings.json");
            let codex_home = dir.join("codex-home");
            std::fs::create_dir_all(&codex_home).unwrap();
            std::fs::write(codex_home.join(file_name), contents).unwrap();
            let store = SettingsStore::new_with_codex_home(path.clone(), codex_home);

            let loaded = store.load().unwrap();

            assert_eq!(loaded, BackendSettings::default(), "{file_name}");
            assert_eq!(loaded.active_relay_id, "default", "{file_name}");
            assert!(
                !loaded
                    .relay_profiles
                    .iter()
                    .any(|profile| profile.id == "chimerahub"),
                "{file_name}"
            );
            assert!(!path.exists(), "{file_name}");
        }
    }

    #[test]
    fn settings_store_empty_codex_files_still_use_chimera_first_run() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let codex_home = dir.join("codex-home");
        std::fs::create_dir_all(&codex_home).unwrap();
        std::fs::write(codex_home.join("config.toml"), []).unwrap();
        std::fs::write(codex_home.join("auth.json"), []).unwrap();
        let store = SettingsStore::new_with_codex_home(path.clone(), codex_home);

        let loaded = store.load().unwrap();

        assert_eq!(loaded, chimera_first_run_settings());
        assert!(!path.exists());
    }

    #[test]
    fn settings_store_load_existing_file_preserves_active_profile_without_chimera_injection() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let store = SettingsStore::new(path);
        let existing = BackendSettings {
            active_relay_id: "my-relay".to_string(),
            relay_base_url: "https://example.test/v1".to_string(),
            relay_api_key: "sk-existing".to_string(),
            relay_profiles: vec![RelayProfile {
                id: "my-relay".to_string(),
                name: "Existing".to_string(),
                base_url: "https://example.test/v1".to_string(),
                relay_mode: RelayMode::PureApi,
                ..RelayProfile::default()
            }],
            relay_profiles_enabled: false,
            ..BackendSettings::default()
        };
        store.save(&existing).unwrap();

        let loaded = store.load().unwrap();

        assert_eq!(loaded.active_relay_id, "my-relay");
        assert!(!loaded.relay_profiles_enabled);
        assert_eq!(loaded.relay_base_url, "https://example.test/v1");
        assert_eq!(loaded.relay_profiles.len(), 1);
        assert_eq!(loaded.relay_profiles[0].id, "my-relay");
        assert!(!loaded.relay_profiles.iter().any(|p| p.id == "chimerahub"));
    }

    #[test]
    fn chimera_first_run_settings_does_not_change_serde_default() {
        let defaults = BackendSettings::default();
        assert_eq!(defaults.active_relay_id, "default");
        assert_eq!(defaults.relay_profiles[0].id, "default");
        assert_eq!(defaults.relay_profiles[0].relay_mode, RelayMode::Official);

        let first_run = chimera_first_run_settings();
        assert_ne!(first_run.active_relay_id, defaults.active_relay_id);
        assert_eq!(first_run.active_relay_id, "chimerahub");
    }

    #[test]
    fn settings_store_load_bad_json_returns_default() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        std::fs::write(&path, "{bad json").unwrap();
        let store = SettingsStore::new(path);

        assert_eq!(store.load().unwrap(), BackendSettings::default());
    }

    #[test]
    fn settings_store_save_load_roundtrip_uses_custom_path() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("nested").join("settings.json"));
        let settings = BackendSettings {
            provider_sync_enabled: true,
            codex_extra_args: vec!["--force_high_performance_gpu".to_string()],
            ..BackendSettings::default()
        };

        store.save(&settings).unwrap();

        assert_eq!(store.load().unwrap(), settings);
    }

    #[test]
    fn settings_store_save_load_roundtrip_preserves_aggregate_relay_settings() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));
        let settings = BackendSettings {
            relay_profiles: vec![
                RelayProfile {
                    id: "relay-a".to_string(),
                    name: "中转 A".to_string(),
                    ..RelayProfile::default()
                },
                RelayProfile {
                    id: "relay-b".to_string(),
                    name: "中转 B".to_string(),
                    ..RelayProfile::default()
                },
                RelayProfile {
                    id: "agg".to_string(),
                    name: "聚合".to_string(),
                    relay_mode: RelayMode::Aggregate,
                    ..RelayProfile::default()
                },
            ],
            active_relay_id: "agg".to_string(),
            aggregate_relay_profiles: vec![AggregateRelayProfile {
                id: "agg".to_string(),
                name: "聚合".to_string(),
                strategy: AggregateRelayStrategy::WeightedRoundRobin,
                members: vec![
                    AggregateRelayMember {
                        relay_id: "relay-a".to_string(),
                        weight: 1,
                    },
                    AggregateRelayMember {
                        relay_id: "relay-b".to_string(),
                        weight: 3,
                    },
                ],
            }],
            active_aggregate_relay_id: "agg".to_string(),
            ..BackendSettings::default()
        };

        store.save(&settings).unwrap();

        let loaded = store.load().unwrap();
        let expected = normalize_settings_config_sections(settings);
        let active_aggregate = loaded.active_aggregate_relay_profile().unwrap();
        assert_eq!(loaded, expected);
        assert_eq!(
            active_aggregate.strategy,
            AggregateRelayStrategy::WeightedRoundRobin
        );
        assert_eq!(active_aggregate.members[1].relay_id, "relay-b");
        assert_eq!(active_aggregate.members[1].weight, 3);
        assert!(loaded.active_relay_uses_protocol_proxy());
    }

    #[test]
    fn settings_store_update_only_mutates_present_known_fields() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));
        let initial = BackendSettings {
            provider_sync_enabled: false,
            ..BackendSettings::default()
        };
        store.save(&initial).unwrap();

        let updated = store
            .update(json!({
            "providerSyncEnabled": true,
            "codexAppPath": "C:\\Portable\\Codex\\Codex.exe",
            "enhancementsEnabled": false,
            "codexAppSessionDelete": false,
            "codexAppConversationView": true,
            "codexAppThreadIdBadge": true,
            "codexAppNativeMenuLocalization": false,
            "codexAppServiceTierControls": true,
            "codexAppPetRealMouseLook": true,
            "codexGoalsEnabled": true,
            "relayBaseUrl": "https://relay.example.test/v1",
            "relayApiKey": "sk-relay",
            "codexExtraArgs": ["--force_high_performance_gpu", "", "  ", " --enable-gpu "],
            "unknownKey": "ignored"
            }))
            .unwrap();

        assert!(updated.provider_sync_enabled);
        assert_eq!(updated.codex_app_path, r"C:\Portable\Codex\Codex.exe");
        assert!(!updated.enhancements_enabled);
        assert!(!updated.codex_app_session_delete);
        assert!(updated.codex_app_conversation_view);
        assert!(updated.codex_app_thread_id_badge);
        assert!(!updated.codex_app_native_menu_localization);
        assert!(updated.codex_app_service_tier_controls);
        assert!(updated.codex_app_pet_real_mouse_look);
        assert!(updated.codex_goals_enabled);
        assert_eq!(updated.relay_base_url, "https://relay.example.test/v1");
        assert_eq!(updated.relay_api_key, "sk-relay");
        assert_eq!(
            updated.codex_extra_args,
            vec![
                "--force_high_performance_gpu".to_string(),
                "--enable-gpu".to_string(),
            ]
        );
        assert_eq!(store.load().unwrap(), updated);
    }

    #[test]
    fn settings_store_update_persists_image_overlay_settings() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "codexAppImageOverlayEnabled": true,
                "codexAppImageOverlayPath": "C:\\Users\\me\\Pictures\\overlay.png",
                "codexAppImageOverlayOpacity": 42,
                "codexAppImageOverlayFitMode": "fill"
            }))
            .unwrap();

        assert!(updated.codex_app_image_overlay_enabled);
        assert_eq!(
            updated.codex_app_image_overlay_path,
            r"C:\Users\me\Pictures\overlay.png"
        );
        assert_eq!(updated.codex_app_image_overlay_opacity, 42);
        assert_eq!(updated.codex_app_image_overlay_fit_mode, "fill");
        assert_eq!(store.load().unwrap(), updated);
    }

    #[test]
    fn settings_store_defaults_invalid_image_overlay_fit_mode_to_fit() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "codexAppImageOverlayFitMode": "unknown"
            }))
            .unwrap();

        assert_eq!(updated.codex_app_image_overlay_fit_mode, "fit");
    }

    #[test]
    fn settings_store_update_persists_stepwise_settings() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "codexAppStepwiseEnabled": true,
                "codexAppStepwiseDirectSend": true,
                "codexAppStepwiseBaseUrl": "https://api.example.test/v1/",
                "codexAppStepwiseApiKey": " sk-stepwise ",
                "codexAppStepwiseApiKeyEnv": "",
                "codexAppStepwiseModel": " stepwise-mini ",
                "codexAppStepwiseMaxItems": 12,
                "codexAppStepwiseMaxInputChars": 25000,
                "codexAppStepwiseMaxOutputTokens": 50,
                "codexAppStepwiseTimeoutMs": 70000
            }))
            .unwrap();

        assert!(updated.codex_app_stepwise_enabled);
        assert!(updated.codex_app_stepwise_direct_send);
        assert_eq!(
            updated.codex_app_stepwise_base_url,
            "https://api.example.test/v1"
        );
        assert_eq!(updated.codex_app_stepwise_api_key, "sk-stepwise");
        assert_eq!(
            updated.codex_app_stepwise_api_key_env,
            default_stepwise_api_key_env()
        );
        assert_eq!(updated.codex_app_stepwise_model, "stepwise-mini");
        assert_eq!(updated.codex_app_stepwise_max_items, 6);
        assert_eq!(updated.codex_app_stepwise_max_input_chars, 24000);
        assert_eq!(updated.codex_app_stepwise_max_output_tokens, 100);
        assert_eq!(updated.codex_app_stepwise_timeout_ms, 60000);
        assert_eq!(store.load().unwrap(), updated);
    }

    #[test]
    fn settings_store_update_persists_launch_mode() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store.update(json!({"launchMode": "relay"})).unwrap();
        let saved: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap())
                .unwrap();

        assert_eq!(updated.launch_mode, LaunchMode::Relay);
        assert_eq!(saved["launchMode"], json!("relay"));
    }

    #[test]
    fn settings_store_update_persists_relay_profiles_and_active_profile() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "relayProfiles": [
                    {
                        "id": "relay-a",
                        "name": "中转 A",
                        "baseUrl": "https://relay-a.example/v1",
                        "apiKey": "sk-a"
                    },
                    {
                        "id": "relay-b",
                        "name": "中转 B",
                        "baseUrl": "https://relay-b.example/v1",
                        "apiKey": "sk-b"
                    }
                ],
                "activeRelayId": "relay-b",
                "relayTestModel": "claude-sonnet-4"
            }))
            .unwrap();

        let active = updated.active_relay_profile();
        assert_eq!(updated.relay_profiles.len(), 2);
        assert_eq!(active.id, "relay-b");
        assert_eq!(active.name, "中转 B");
        assert_eq!(updated.relay_test_model, "claude-sonnet-4");

        let saved: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap())
                .unwrap();
        assert!(saved["relayProfiles"][1].get("baseUrl").is_none());
        assert!(saved["relayProfiles"][1].get("apiKey").is_none());
    }

    #[test]
    fn settings_store_update_does_not_persist_relay_profile_derived_fields() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "relayProfiles": [
                    {
                        "id": "relay-a",
                        "name": "供应商 A",
                        "model": "gpt-5.4",
                        "baseUrl": "https://relay.example/v1",
                        "apiKey": "sk-a",
                        "configContents": "model = \"gpt-5.4\"\n",
                        "authContents": "{\"OPENAI_API_KEY\":\"sk-a\"}"
                    }
                ],
                "activeRelayId": "relay-a"
            }))
            .unwrap();

        assert_eq!(updated.relay_profiles[0].id, "relay-a");
        assert_eq!(updated.relay_profiles[0].name, "供应商 A");

        let saved: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("settings.json")).unwrap())
                .unwrap();
        let saved_profile = &saved["relayProfiles"][0];
        assert!(saved_profile.get("model").is_none());
        assert!(saved_profile.get("baseUrl").is_none());
        assert!(saved_profile.get("apiKey").is_none());
        assert_eq!(saved_profile["configContents"], "model = \"gpt-5.4\"\n");
        assert_eq!(
            saved_profile["authContents"],
            "{\"OPENAI_API_KEY\":\"sk-a\"}"
        );
    }

    #[test]
    fn settings_store_update_moves_context_tables_out_of_common_config() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "relayCommonConfigContents": "[mcp_servers.context7]\ncommand = \"npx\"\n"
            }))
            .unwrap();

        assert!(updated.relay_common_config_contents.is_empty());
        assert_eq!(
            updated.relay_context_config_contents,
            "[mcp_servers.context7]\ncommand = \"npx\"\n"
        );
        assert_eq!(store.load().unwrap(), updated);
    }

    #[test]
    fn settings_store_update_extracts_context_config_from_common_config() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "relayCommonConfigContents": "model_reasoning_effort = \"high\"\n\n[mcp_servers.context7]\ncommand = \"npx\"\n\n[plugins.\"superpowers@openai-curated\"]\nenabled = true\n"
            }))
            .unwrap();

        assert_eq!(
            updated.relay_common_config_contents,
            "model_reasoning_effort = \"high\"\n"
        );
        assert!(
            updated
                .relay_context_config_contents
                .contains("[mcp_servers.context7]")
        );
        assert!(
            updated
                .relay_context_config_contents
                .contains("[plugins.\"superpowers@openai-curated\"]")
        );
        assert_eq!(store.load().unwrap(), updated);
    }

    #[test]
    fn settings_store_update_persists_aggregate_relay_profiles_and_active_id() {
        let dir = temp_dir();
        let store = SettingsStore::new(dir.join("settings.json"));

        let updated = store
            .update(json!({
                "relayProfiles": [
                    { "id": "relay-a", "name": "中转 A" },
                    { "id": "relay-b", "name": "中转 B" },
                    { "id": "agg", "name": "聚合", "relayMode": "aggregate" }
                ],
                "activeRelayId": "agg",
                "aggregateRelayProfiles": [
                    {
                        "id": "agg",
                        "name": "聚合",
                        "strategy": "weightedRoundRobin",
                        "members": [
                            { "relayId": "relay-a", "weight": 1 },
                            { "relayId": "relay-b", "weight": 4 }
                        ]
                    }
                ],
                "activeAggregateRelayId": "agg"
            }))
            .unwrap();

        let active_aggregate = updated.active_aggregate_relay_profile().unwrap();
        assert_eq!(updated.active_relay_id, "agg");
        assert_eq!(updated.active_aggregate_relay_id, "agg");
        assert_eq!(
            active_aggregate.strategy,
            AggregateRelayStrategy::WeightedRoundRobin
        );
        assert_eq!(active_aggregate.members.len(), 2);
        assert_eq!(active_aggregate.members[1].relay_id, "relay-b");
        assert_eq!(active_aggregate.members[1].weight, 4);
        assert!(updated.active_relay_uses_protocol_proxy());
    }

    #[test]
    fn active_relay_profile_uses_legacy_single_relay_when_profiles_are_default() {
        let settings = BackendSettings {
            relay_base_url: "https://legacy.example/v1".to_string(),
            relay_api_key: "sk-legacy".to_string(),
            ..BackendSettings::default()
        };

        let active = settings.active_relay_profile();

        assert_eq!(active.id, "default");
        assert_eq!(active.name, "默认中转");
        assert_eq!(active.base_url, "https://legacy.example/v1");
        assert_eq!(active.api_key, "sk-legacy");
        assert_eq!(active.relay_mode, RelayMode::MixedApi);
        assert!(active.official_mix_api_key);
    }

    #[test]
    fn settings_store_update_preserves_existing_unknown_fields() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let store = SettingsStore::new(path.clone());
        std::fs::write(
            &path,
            r#"{"providerSyncEnabled":false,"customField":{"nested":true}}"#,
        )
        .unwrap();

        let updated = store
            .update(json!({
                "providerSyncEnabled": true
            }))
            .unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        assert!(updated.provider_sync_enabled);
        assert_eq!(saved["providerSyncEnabled"], json!(true));
        assert_eq!(saved["codexExtraArgs"], Value::Null);
        assert_eq!(saved["customField"], json!({"nested": true}));
    }

    #[test]
    fn settings_store_update_persists_codex_extra_args_and_preserves_unknown_fields() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let store = SettingsStore::new(path.clone());
        std::fs::write(
            &path,
            r#"{"providerSyncEnabled":false,"customField":{"nested":true}}"#,
        )
        .unwrap();

        let updated = store
            .update(json!({
                "codexExtraArgs": ["--force_high_performance_gpu", "--enable-features=UseOzonePlatform"]
            }))
            .unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        assert_eq!(
            updated.codex_extra_args,
            vec![
                "--force_high_performance_gpu".to_string(),
                "--enable-features=UseOzonePlatform".to_string(),
            ]
        );
        assert_eq!(
            saved["codexExtraArgs"],
            json!([
                "--force_high_performance_gpu",
                "--enable-features=UseOzonePlatform"
            ])
        );
        assert_eq!(saved["customField"], json!({"nested": true}));
    }

    #[test]
    fn settings_store_update_with_non_object_payload_does_not_write_file() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        let store = SettingsStore::new(path.clone());
        let original = r#"{"providerSyncEnabled":false,"customField":"keep me"}"#;
        std::fs::write(&path, original).unwrap();

        let updated = store.update(json!(null)).unwrap();

        assert!(!updated.provider_sync_enabled);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
}
