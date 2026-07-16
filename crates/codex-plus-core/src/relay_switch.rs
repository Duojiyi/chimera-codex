use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::relay_config::{
    backfill_relay_profile_from_home_with_common, relay_config_status_from_home,
};
use crate::settings::{BackendSettings, RelayMode, SettingsStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelaySwitchResult {
    pub settings: BackendSettings,
    pub configured: bool,
    pub backup_path: Option<String>,
}

pub fn switch_relay_profile_in_home(
    store: &SettingsStore,
    home: &Path,
    next_settings: BackendSettings,
    previous_active_relay_id: &str,
) -> anyhow::Result<RelaySwitchResult> {
    let mut selected_settings = next_settings;
    if !selected_settings.relay_profiles_enabled {
        anyhow::bail!("供应商配置总开关已关闭，未写入 config.toml / auth.json。");
    }
    crate::codex_app_state::capture_app_state_snapshot_nonfatal(home, "relay_switch.before");

    let original_settings_snapshot = store.snapshot_bytes()?;
    let selected_profile = selected_settings.active_relay_profile();
    let live_snapshot = LiveFilesSnapshot::capture(home, &selected_profile)?;
    if !previous_active_relay_id.trim().is_empty()
        && previous_active_relay_id != selected_settings.active_relay_id
    {
        backfill_profile_before_switch(home, &mut selected_settings, previous_active_relay_id)?;
    }

    let switch_result = (|| {
        let selected_settings = store
            .save_normalized(&selected_settings)
            .context("保存供应商设置失败")?;
        apply_selected_relay_profile(home, &selected_settings)
    })();

    match switch_result {
        Ok(result) => {
            crate::codex_app_state::sync_app_state_after_provider_switch_nonfatal(
                home,
                "relay_switch.after",
            );
            Ok(result)
        }
        Err(error) => {
            let settings_rollback = store.restore_snapshot(original_settings_snapshot.as_deref());
            let live_rollback = live_snapshot.restore();
            match (settings_rollback, live_rollback) {
                (Ok(()), Ok(())) => Err(error),
                (settings_result, live_result) => {
                    let mut failures = Vec::new();
                    if let Err(rollback_error) = settings_result {
                        failures.push(format!("settings.json: {rollback_error}"));
                    }
                    if let Err(rollback_error) = live_result {
                        failures.push(format!("live files: {rollback_error}"));
                    }
                    Err(error.context(format!(
                        "供应商切换失败，且回滚未完整完成：{}",
                        failures.join("; ")
                    )))
                }
            }
        }
    }
}

#[derive(Debug)]
struct FileSnapshot {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
}

impl FileSnapshot {
    fn capture(path: PathBuf) -> anyhow::Result<Self> {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(error).with_context(|| format!("读取事务快照失败：{}", path.display()));
            }
        };
        Ok(Self { path, bytes })
    }

    fn restore(&self) -> anyhow::Result<()> {
        match self.bytes.as_deref() {
            Some(bytes) => crate::settings::atomic_write(&self.path, bytes),
            None => match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error)
                    .with_context(|| format!("删除事务中新建文件失败：{}", self.path.display())),
            },
        }
    }
}

#[derive(Debug)]
struct LiveFilesSnapshot {
    config: FileSnapshot,
    auth: FileSnapshot,
    catalog: FileSnapshot,
    catalog_parent_existed: bool,
}

impl LiveFilesSnapshot {
    fn capture(home: &Path, profile: &crate::settings::RelayProfile) -> anyhow::Result<Self> {
        let catalog_path = crate::relay_config::generated_model_catalog_path(home, profile);
        let catalog_parent_existed = catalog_path.parent().is_some_and(Path::exists);
        Ok(Self {
            config: FileSnapshot::capture(home.join("config.toml"))?,
            auth: FileSnapshot::capture(home.join("auth.json"))?,
            catalog: FileSnapshot::capture(catalog_path)?,
            catalog_parent_existed,
        })
    }

    fn restore(&self) -> anyhow::Result<()> {
        let config_result = self.config.restore();
        let auth_result = self.auth.restore();
        let catalog_result = self.catalog.restore();
        let catalog_parent_result = self.remove_created_catalog_parent_if_empty();
        let mut failures = Vec::new();
        for (name, result) in [
            ("config.toml", config_result),
            ("auth.json", auth_result),
            ("model catalog", catalog_result),
            ("model catalog directory", catalog_parent_result),
        ] {
            if let Err(error) = result {
                failures.push(format!("{name}: {error}"));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(failures.join("; "))
        }
    }

    fn remove_created_catalog_parent_if_empty(&self) -> anyhow::Result<()> {
        if self.catalog_parent_existed {
            return Ok(());
        }
        let Some(parent) = self.catalog.path.parent() else {
            return Ok(());
        };
        let mut entries = match std::fs::read_dir(parent) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).context("读取新建 model-catalogs 目录失败"),
        };
        if entries.next().is_none() {
            std::fs::remove_dir(parent).context("删除事务中新建的空 model-catalogs 目录")?;
        }
        Ok(())
    }
}

fn backfill_profile_before_switch(
    home: &Path,
    settings: &mut BackendSettings,
    previous_active_relay_id: &str,
) -> anyhow::Result<()> {
    let profile = settings
        .relay_profiles
        .iter_mut()
        .find(|profile| profile.id == previous_active_relay_id)
        .with_context(|| "当前供应商已不在配置列表中，已停止切换以避免覆盖用户改动。")?;
    backfill_relay_profile_from_home_with_common(
        home,
        profile,
        &mut settings.relay_context_config_contents,
    )
    .with_context(|| "回填当前供应商配置失败")
}

fn apply_selected_relay_profile(
    home: &Path,
    settings: &BackendSettings,
) -> anyhow::Result<RelaySwitchResult> {
    let relay = settings.active_relay_profile();
    let common_config = relay_combined_common_config(settings);
    let result = if relay.relay_mode == RelayMode::Official && !relay.official_mix_api_key {
        let auth_contents =
            (!relay.auth_contents.trim().is_empty()).then_some(relay.auth_contents.as_str());
        crate::relay_config::clear_relay_config_to_home_with_auth_and_computer_use_guard(
            home,
            auth_contents,
            settings.computer_use_guard_enabled,
        )?
    } else {
        validate_switch_profile_files(&relay)?;
        crate::relay_config::apply_relay_profile_to_home_with_switch_rules_and_computer_use_guard(
            home,
            &relay,
            &common_config,
            settings.computer_use_guard_enabled,
        )?
    };
    let status = relay_config_status_from_home(home);
    if relay.relay_mode == RelayMode::PureApi && !status.configured {
        anyhow::bail!(
            "纯 API 配置写入后未检测到完整 custom provider，请检查 config.toml 和供应商 API Key。"
        );
    }
    Ok(RelaySwitchResult {
        settings: settings.clone(),
        configured: status.configured,
        backup_path: result.backup_path,
    })
}

fn validate_switch_profile_files(profile: &crate::settings::RelayProfile) -> anyhow::Result<()> {
    if profile.relay_mode != RelayMode::Aggregate && profile.config_contents.trim().is_empty() {
        anyhow::bail!(
            "供应商「{}」缺少独立 config.toml，已停止切换，避免继续显示上一套配置文件。",
            if profile.name.trim().is_empty() {
                profile.id.as_str()
            } else {
                profile.name.as_str()
            }
        );
    }
    if profile.relay_mode == RelayMode::Official
        && serde_json::from_str::<serde_json::Value>(&profile.auth_contents)
            .ok()
            .and_then(|value| {
                value
                    .get("OPENAI_API_KEY")
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .map(str::is_empty)
            })
            == Some(false)
    {
        anyhow::bail!(
            "官方混合 API 不应在 auth.json 中保存 OPENAI_API_KEY。请清理此供应商的 auth.json 后再切换。"
        );
    }
    Ok(())
}

fn relay_combined_common_config(settings: &BackendSettings) -> String {
    let sections = [
        settings.relay_common_config_contents.trim(),
        settings.relay_context_config_contents.trim(),
    ]
    .into_iter()
    .filter(|section| !section.is_empty())
    .collect::<Vec<_>>();
    if sections.is_empty() {
        String::new()
    } else {
        crate::relay_config::normalize_config_text(&format!("{}\n", sections.join("\n\n")))
    }
}
