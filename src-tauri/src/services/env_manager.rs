use super::env_checker::EnvConflict;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use uuid::Uuid;

#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub backup_path: String,
    pub timestamp: String,
    pub conflicts: Vec<EnvConflict>,
}

const MAX_ENV_CONFLICTS: usize = 1024;
const MAX_ENV_NAME_LEN: usize = 256;
const MAX_ENV_VALUE_LEN: usize = 64 * 1024;
#[cfg(target_os = "windows")]
const HKCU_ENVIRONMENT: &str = "HKEY_CURRENT_USER\\Environment";
#[cfg(target_os = "windows")]
const HKLM_ENVIRONMENT: &str =
    "HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";

/// Delete environment variables with automatic backup
pub fn delete_env_vars(conflicts: Vec<EnvConflict>) -> Result<BackupInfo, String> {
    if conflicts.len() > MAX_ENV_CONFLICTS {
        return Err(format!("环境变量冲突数量超过限制 ({MAX_ENV_CONFLICTS})"));
    }
    for conflict in &conflicts {
        validate_conflict(conflict)?;
    }

    // Step 1: Create backup
    let backup_info = create_backup(&conflicts)?;

    // Step 2: Delete variables. If a later deletion fails, restore everything
    // that was already deleted so the operation remains best-effort atomic.
    let mut deleted = Vec::new();
    for conflict in &conflicts {
        match delete_single_env(conflict) {
            Ok(_) => deleted.push(conflict.clone()),
            Err(error) => {
                for prior in deleted.iter().rev() {
                    if let Err(rollback_error) = restore_single_env(prior) {
                        log::error!("环境变量删除回滚失败: {rollback_error}");
                    }
                }
                return Err(format!(
                    "删除环境变量失败: {error}。备份已保存到: {}",
                    backup_info.backup_path
                ));
            }
        }
    }

    Ok(backup_info)
}

/// Create backup file before deletion
fn create_backup(conflicts: &[EnvConflict]) -> Result<BackupInfo, String> {
    // Get backup directory
    let backup_dir = get_backup_dir()?;
    fs::create_dir_all(&backup_dir).map_err(|e| format!("创建备份目录失败: {e}"))?;

    // Include a random suffix so two backups created in the same second cannot
    // overwrite one another.
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_file = backup_dir.join(format!("env-backup-{timestamp}-{}.json", Uuid::new_v4()));

    // Create backup data
    let backup_info = BackupInfo {
        backup_path: backup_file.to_string_lossy().to_string(),
        timestamp: timestamp.clone(),
        conflicts: conflicts.to_vec(),
    };

    // Write backup file atomically with restrictive permissions.
    let json = serde_json::to_string_pretty(&backup_info)
        .map_err(|e| format!("序列化备份数据失败: {e}"))?;
    crate::config::atomic_write(&backup_file, json.as_bytes())
        .map_err(|e| format!("写入备份文件失败: {e}"))?;

    Ok(backup_info)
}

/// Get backup directory path
fn get_backup_dir() -> Result<PathBuf, String> {
    Ok(crate::config::get_app_config_dir().join("backups"))
}

/// Delete a single environment variable
#[cfg(target_os = "windows")]
fn delete_single_env(conflict: &EnvConflict) -> Result<(), String> {
    validate_conflict(conflict)?;
    match conflict.source_path.as_str() {
        HKCU_ENVIRONMENT => {
            let hkcu = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags("Environment", KEY_ALL_ACCESS)
                .map_err(|e| format!("打开注册表失败: {e}"))?;
            hkcu.delete_value(&conflict.var_name)
                .map_err(|e| format!("删除注册表项失败: {e}"))?;
            Ok(())
        }
        HKLM_ENVIRONMENT => {
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey_with_flags(
                    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
                    KEY_ALL_ACCESS,
                )
                .map_err(|e| format!("打开系统注册表失败 (需要管理员权限): {e}"))?;
            hklm.delete_value(&conflict.var_name)
                .map_err(|e| format!("删除系统注册表项失败: {e}"))?;
            Ok(())
        }
        _ => Err("不允许的 Windows 环境变量来源".to_string()),
    }
}

#[cfg(not(target_os = "windows"))]
fn delete_single_env(conflict: &EnvConflict) -> Result<(), String> {
    validate_conflict(conflict)?;
    match conflict.source_type.as_str() {
        "file" => {
            let (file_path, _) = parse_unix_source_path(&conflict.source_path)?;
            let content = fs::read_to_string(&file_path)
                .map_err(|e| format!("读取文件失败 {}: {e}", file_path.display()))?;

            let new_content: Vec<String> = content
                .lines()
                .filter(|line| {
                    let trimmed = line.trim();
                    let export_line = trimmed.strip_prefix("export ").unwrap_or(trimmed);
                    export_line
                        .find('=')
                        .map(|eq_pos| export_line[..eq_pos].trim() != conflict.var_name)
                        .unwrap_or(true)
                })
                .map(ToString::to_string)
                .collect();

            crate::config::atomic_write(&file_path, new_content.join("\n").as_bytes())
                .map_err(|e| format!("写入文件失败 {}: {e}", file_path.display()))?;
            Ok(())
        }
        "system" => {
            // Process environment variables are not persistent and cannot be
            // deleted safely from another process.
            Ok(())
        }
        _ => Err(format!("未知的环境变量来源类型: {}", conflict.source_type)),
    }
}

/// Restore environment variables from backup
pub fn restore_from_backup(backup_path: String) -> Result<(), String> {
    let backup_path = validate_backup_path(&backup_path)?;
    let content = fs::read_to_string(&backup_path).map_err(|e| format!("读取备份文件失败: {e}"))?;

    let backup_info: BackupInfo =
        serde_json::from_str(&content).map_err(|e| format!("解析备份文件失败: {e}"))?;
    let declared_path = validate_backup_path(&backup_info.backup_path)?;
    if declared_path != backup_path {
        return Err("备份元数据与实际文件路径不一致".to_string());
    }
    if backup_info.conflicts.len() > MAX_ENV_CONFLICTS {
        return Err(format!(
            "备份中的环境变量数量超过限制 ({MAX_ENV_CONFLICTS})"
        ));
    }
    for conflict in &backup_info.conflicts {
        validate_conflict(conflict)?;
    }

    // Restore in order. The path and every source descriptor were validated
    // before the first write, so a malicious backup cannot redirect a later item.
    for conflict in &backup_info.conflicts {
        restore_single_env(conflict)?;
    }

    Ok(())
}

/// Restore a single environment variable
#[cfg(target_os = "windows")]
fn restore_single_env(conflict: &EnvConflict) -> Result<(), String> {
    validate_conflict(conflict)?;
    match conflict.source_path.as_str() {
        HKCU_ENVIRONMENT => {
            let (hkcu, _) = RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey("Environment")
                .map_err(|e| format!("打开注册表失败: {e}"))?;
            hkcu.set_value(&conflict.var_name, &conflict.var_value)
                .map_err(|e| format!("恢复注册表项失败: {e}"))?;
            Ok(())
        }
        HKLM_ENVIRONMENT => {
            let (hklm, _) = RegKey::predef(HKEY_LOCAL_MACHINE)
                .create_subkey("SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment")
                .map_err(|e| format!("打开系统注册表失败 (需要管理员权限): {e}"))?;
            hklm.set_value(&conflict.var_name, &conflict.var_value)
                .map_err(|e| format!("恢复系统注册表项失败: {e}"))?;
            Ok(())
        }
        _ => Err("不允许的 Windows 环境变量来源".to_string()),
    }
}

#[cfg(not(target_os = "windows"))]
fn restore_single_env(conflict: &EnvConflict) -> Result<(), String> {
    validate_conflict(conflict)?;
    match conflict.source_type.as_str() {
        "file" => {
            let (file_path, _) = parse_unix_source_path(&conflict.source_path)?;
            let mut content = fs::read_to_string(&file_path)
                .map_err(|e| format!("读取文件失败 {}: {e}", file_path.display()))?;
            if !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(&format!(
                "export {}={}\n",
                conflict.var_name, conflict.var_value
            ));
            crate::config::atomic_write(&file_path, content.as_bytes())
                .map_err(|e| format!("写入文件失败 {}: {e}", file_path.display()))?;
            Ok(())
        }
        _ => Err(format!(
            "无法恢复类型为 {} 的环境变量",
            conflict.source_type
        )),
    }
}

fn validate_backup_path(raw: &str) -> Result<PathBuf, String> {
    let backup_dir = get_backup_dir()?;
    let canonical_dir =
        fs::canonicalize(&backup_dir).map_err(|e| format!("备份目录不可用: {e}"))?;
    let candidate = PathBuf::from(raw);
    let metadata = fs::symlink_metadata(&candidate).map_err(|e| format!("备份文件不可用: {e}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("备份文件必须是普通文件，且不得是符号链接".to_string());
    }
    let canonical =
        fs::canonicalize(&candidate).map_err(|e| format!("解析备份文件路径失败: {e}"))?;
    if !canonical.starts_with(&canonical_dir)
        || canonical.extension().and_then(|ext| ext.to_str()) != Some("json")
    {
        return Err("备份文件必须位于应用 backups 目录内".to_string());
    }
    Ok(canonical)
}

fn validate_conflict(conflict: &EnvConflict) -> Result<(), String> {
    validate_env_name(&conflict.var_name)?;
    if conflict.var_value.len() > MAX_ENV_VALUE_LEN || conflict.var_value.contains('\0') {
        return Err("环境变量值过长或包含非法字符".to_string());
    }

    match conflict.source_type.as_str() {
        #[cfg(target_os = "windows")]
        "system"
            if matches!(
                conflict.source_path.as_str(),
                HKCU_ENVIRONMENT | HKLM_ENVIRONMENT
            ) =>
        {
            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        "system" if conflict.source_path == "Process Environment" => Ok(()),
        "file" => {
            #[cfg(target_os = "windows")]
            {
                Err("Windows 系统不允许从文件恢复环境变量".to_string())
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = parse_unix_source_path(&conflict.source_path)?;
                Ok(())
            }
        }
        _ => Err("不允许的环境变量来源".to_string()),
    }
}

fn validate_env_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > MAX_ENV_NAME_LEN {
        return Err("环境变量名为空或过长".to_string());
    }
    let mut chars = name.chars();
    let first = chars.next().ok_or_else(|| "环境变量名为空".to_string())?;
    if !(first == '_' || first.is_ascii_alphabetic())
        || !chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err("环境变量名包含非法字符".to_string());
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn parse_unix_source_path(source_path: &str) -> Result<(PathBuf, u32), String> {
    let (raw_path, raw_line) = source_path
        .rsplit_once(':')
        .ok_or_else(|| "无效的文件路径格式".to_string())?;
    let line = raw_line
        .parse::<u32>()
        .map_err(|_| "无效的环境变量行号".to_string())?;
    if raw_path.is_empty() || line == 0 {
        return Err("无效的文件路径或行号".to_string());
    }

    let path = PathBuf::from(raw_path);
    let metadata =
        fs::symlink_metadata(&path).map_err(|e| format!("环境变量配置文件不可用: {e}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("环境变量配置文件必须是普通文件，且不得是符号链接".to_string());
    }
    let canonical =
        fs::canonicalize(&path).map_err(|e| format!("解析环境变量配置文件失败: {e}"))?;
    let home = crate::config::get_home_dir();
    let mut allowed = vec![
        home.join(".bashrc"),
        home.join(".bash_profile"),
        home.join(".zshrc"),
        home.join(".zprofile"),
        home.join(".profile"),
        PathBuf::from("/etc/profile"),
        PathBuf::from("/etc/bashrc"),
    ];
    allowed.retain(|candidate| {
        fs::canonicalize(candidate)
            .map(|resolved| resolved == canonical)
            .unwrap_or(false)
    });
    if allowed.is_empty() {
        return Err("不允许修改该 shell 配置文件".to_string());
    }
    Ok((path, line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_dir_creation() {
        let backup_dir = get_backup_dir();
        assert!(backup_dir.is_ok());
    }
}
