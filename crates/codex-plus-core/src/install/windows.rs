use std::path::{Path, PathBuf};

use anyhow::Context;

use super::{
    InstallOptions, LEGACY_MANAGER_NAME, LEGACY_MOJIBAKE_MANAGER_LNK, LEGACY_SILENT_NAME,
    MANAGER_BINARY, MANAGER_NAME, SILENT_BINARY, SILENT_NAME, install_root_or_default,
    legacy_shortcut_names, option_or_current_exe, windows_legacy_shortcut_paths,
};

const UNINSTALL_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexPlusPlus";
const LEGACY_UNINSTALL_SUBKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++";
const URL_PROTOCOL_SUBKEY: &str = r"Software\Classes\codexplusplus";
#[cfg(windows)]
const SETUP_TRANSACTION_MUTEX_NAME: &str = r"Local\ChimeraPlusPlus.Setup.Transaction";

#[cfg(windows)]
struct SetupTransactionMutexGuard(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for SetupTransactionMutexGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::System::Threading::ReleaseMutex(self.0);
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn acquire_named_transaction_mutex(name: &str) -> anyhow::Result<SetupTransactionMutexGuard> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{CreateMutexW, INFINITE, WaitForSingleObject};
    use windows::core::PCWSTR;

    let wide_name = std::ffi::OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe { CreateMutexW(None, false, PCWSTR(wide_name.as_ptr())) }
        .with_context(|| format!("创建 Windows 安装事务锁失败：{name}"))?;
    let wait = unsafe { WaitForSingleObject(handle, INFINITE) };
    if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
        Ok(SetupTransactionMutexGuard(handle))
    } else {
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(handle) };
        if wait == WAIT_FAILED {
            Err(windows::core::Error::from_win32())
                .with_context(|| format!("等待 Windows 安装事务锁失败：{name}"))
        } else {
            anyhow::bail!("等待 Windows 安装事务锁返回未知状态：{wait:?}")
        }
    }
}

#[cfg(windows)]
fn acquire_setup_transaction_mutex() -> anyhow::Result<SetupTransactionMutexGuard> {
    acquire_named_transaction_mutex(SETUP_TRANSACTION_MUTEX_NAME)
}

#[cfg(windows)]
#[derive(Debug)]
struct ShortcutSnapshot {
    path: PathBuf,
    contents: Option<Vec<u8>>,
}

#[cfg(windows)]
#[derive(Debug)]
struct RegistrySnapshot {
    subkey: String,
    existed: bool,
    values: Vec<crate::windows_integration::CurrentUserRegistryValueSnapshot>,
}

#[cfg(windows)]
#[derive(Debug)]
struct WindowsMetadataSnapshot {
    shortcuts: Vec<ShortcutSnapshot>,
    registry: Vec<RegistrySnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsEntrypointPlan {
    pub install_root: String,
    pub silent_shortcut: String,
    pub manager_shortcut: String,
    pub launcher_path: String,
    pub manager_path: String,
    pub icon_path: String,
    pub silent_icon_path: String,
    pub manager_icon_path: String,
    pub uninstaller_path: String,
    pub uninstall_command: String,
    pub quiet_uninstall_command: String,
    pub uninstall_key: String,
    pub legacy_uninstall_key: String,
    pub display_name: String,
    pub publisher: String,
    pub remove_owned_data: bool,
}

pub fn build_windows_entrypoint_plan(options: &InstallOptions) -> WindowsEntrypointPlan {
    let install_root = install_root_or_default(options);
    let launcher_path = option_or_current_exe(&options.launcher_path, SILENT_BINARY);
    let manager_path = option_or_current_exe(&options.manager_path, MANAGER_BINARY);
    let icon_path = default_icon_path();
    let install_location = manager_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| install_root.clone());
    let uninstaller_path = install_location.join("uninstall.exe");
    let uninstall_command = format!("\"{}\"", uninstaller_path.to_string_lossy());
    let quiet_uninstall_command = format!("{uninstall_command} /S");
    WindowsEntrypointPlan {
        silent_shortcut: install_root
            .join(format!("{SILENT_NAME}.lnk"))
            .to_string_lossy()
            .to_string(),
        manager_shortcut: install_root
            .join(format!("{MANAGER_NAME}.lnk"))
            .to_string_lossy()
            .to_string(),
        install_root: install_root.to_string_lossy().to_string(),
        launcher_path: launcher_path.to_string_lossy().to_string(),
        manager_path: manager_path.to_string_lossy().to_string(),
        icon_path: icon_path.to_string_lossy().to_string(),
        silent_icon_path: launcher_path.to_string_lossy().to_string(),
        manager_icon_path: manager_path.to_string_lossy().to_string(),
        uninstaller_path: uninstaller_path.to_string_lossy().to_string(),
        uninstall_command,
        quiet_uninstall_command,
        uninstall_key: "CodexPlusPlus".to_string(),
        legacy_uninstall_key: "Codex++".to_string(),
        display_name: SILENT_NAME.to_string(),
        publisher: crate::branding::PUBLISHER.to_string(),
        remove_owned_data: options.remove_owned_data,
    }
}

#[cfg(windows)]
pub fn install_shortcuts(options: &InstallOptions) -> anyhow::Result<()> {
    let _transaction_guard = acquire_setup_transaction_mutex()?;
    let plan = build_windows_entrypoint_plan(options);
    let install_root = PathBuf::from(&plan.install_root);
    let shortcuts = managed_shortcut_paths(&plan, &install_root);
    let registry_keys = managed_registry_keys();
    let snapshot = WindowsMetadataSnapshot::capture(&shortcuts, &registry_keys)?;
    run_metadata_transaction(
        || {
            std::fs::create_dir_all(&install_root)?;
            for path in &shortcuts {
                remove_file_if_exists(path)?;
            }
            create_entrypoint_shortcut(
                PathBuf::from(&plan.silent_shortcut),
                PathBuf::from(&plan.launcher_path),
                &format!("Launch {SILENT_NAME} silently"),
                PathBuf::from(&plan.silent_icon_path),
            )?;
            register_url_protocol(&plan.manager_path)?;
            write_uninstall_registration(&plan)
        },
        || snapshot.restore(),
    )
}

#[cfg(windows)]
pub fn uninstall_shortcuts(options: &InstallOptions) -> anyhow::Result<()> {
    let _transaction_guard = acquire_setup_transaction_mutex()?;
    let plan = build_windows_entrypoint_plan(options);
    let install_root = PathBuf::from(&plan.install_root);
    let shortcuts = managed_shortcut_paths(&plan, &install_root);
    let registry_keys = managed_registry_keys();
    let snapshot = WindowsMetadataSnapshot::capture(&shortcuts, &registry_keys)?;
    run_metadata_transaction(
        || {
            for path in &shortcuts {
                remove_file_if_exists(path)?;
            }
            delete_owned_registry_keys()
        },
        || snapshot.restore(),
    )
}

fn run_metadata_transaction(
    apply: impl FnOnce() -> anyhow::Result<()>,
    rollback: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let Err(apply_error) = apply() else {
        return Ok(());
    };
    match rollback() {
        Ok(()) => Err(apply_error),
        Err(rollback_error) => anyhow::bail!(
            "Windows metadata transaction failed: {apply_error}; rollback failed: {rollback_error}"
        ),
    }
}

#[cfg(windows)]
fn managed_shortcut_paths(plan: &WindowsEntrypointPlan, install_root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from(&plan.silent_shortcut),
        PathBuf::from(&plan.manager_shortcut),
    ];
    paths.extend(windows_legacy_shortcut_paths(install_root));
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(windows)]
fn managed_registry_keys() -> Vec<String> {
    vec![
        URL_PROTOCOL_SUBKEY.to_string(),
        format!(r"{URL_PROTOCOL_SUBKEY}\shell"),
        format!(r"{URL_PROTOCOL_SUBKEY}\shell\open"),
        format!(r"{URL_PROTOCOL_SUBKEY}\shell\open\command"),
        LEGACY_UNINSTALL_SUBKEY.to_string(),
        UNINSTALL_SUBKEY.to_string(),
    ]
}

#[cfg(windows)]
fn remove_file_if_exists(path: &Path) -> anyhow::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("删除快捷方式 {} 失败", path.display())),
    }
}

#[cfg(windows)]
fn delete_owned_registry_keys() -> anyhow::Result<()> {
    let mut keys = managed_registry_keys();
    keys.sort_by_key(|key| std::cmp::Reverse(key.len()));
    for key in keys {
        crate::windows_integration::delete_current_user_key(&key)?;
    }
    Ok(())
}

#[cfg(windows)]
impl WindowsMetadataSnapshot {
    fn capture(shortcut_paths: &[PathBuf], registry_keys: &[String]) -> anyhow::Result<Self> {
        let mut shortcuts = Vec::with_capacity(shortcut_paths.len());
        for path in shortcut_paths {
            let contents = match std::fs::read(path) {
                Ok(contents) => Some(contents),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("读取快捷方式快照 {} 失败", path.display()));
                }
            };
            shortcuts.push(ShortcutSnapshot {
                path: path.clone(),
                contents,
            });
        }

        let mut registry = Vec::new();
        for subkey in registry_keys {
            let raw = crate::windows_integration::snapshot_current_user_key_raw(subkey)?;
            let existed = raw.is_some();
            let values = raw.unwrap_or_default();
            registry.push(RegistrySnapshot {
                subkey: subkey.clone(),
                existed,
                values,
            });
        }
        Ok(Self {
            shortcuts,
            registry,
        })
    }

    fn restore(&self) -> anyhow::Result<()> {
        let mut errors = Vec::new();
        let mut keys = self
            .registry
            .iter()
            .map(|snapshot| snapshot.subkey.clone())
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| std::cmp::Reverse(key.len()));
        for key in keys {
            if let Err(error) = crate::windows_integration::delete_current_user_key(&key) {
                errors.push(error.to_string());
            }
        }
        let mut registry = self.registry.iter().collect::<Vec<_>>();
        registry.sort_by_key(|snapshot| snapshot.subkey.len());
        for snapshot in registry {
            if !snapshot.existed {
                continue;
            }
            if let Err(error) =
                crate::windows_integration::ensure_current_user_key(&snapshot.subkey)
            {
                errors.push(error.to_string());
                continue;
            }
            for value in &snapshot.values {
                if let Err(error) =
                    crate::windows_integration::set_current_user_raw_value(&snapshot.subkey, value)
                {
                    errors.push(error.to_string());
                }
            }
        }
        for snapshot in &self.shortcuts {
            let result = if let Some(contents) = &snapshot.contents {
                snapshot
                    .path
                    .parent()
                    .map(std::fs::create_dir_all)
                    .transpose()
                    .and_then(|_| std::fs::write(&snapshot.path, contents))
            } else {
                match std::fs::remove_file(&snapshot.path) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(error),
                }
            };
            if let Err(error) = result {
                errors.push(format!(
                    "恢复快捷方式 {} 失败：{error}",
                    snapshot.path.display()
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(errors.join("; "))
        }
    }
}

#[cfg(not(windows))]
pub fn install_shortcuts(_options: &InstallOptions) -> anyhow::Result<()> {
    anyhow::bail!("Windows shortcuts are only supported on Windows")
}

#[cfg(not(windows))]
pub fn uninstall_shortcuts(_options: &InstallOptions) -> anyhow::Result<()> {
    anyhow::bail!("Windows shortcuts are only supported on Windows")
}

#[cfg(windows)]
fn create_entrypoint_shortcut(
    path: PathBuf,
    target: PathBuf,
    description: &str,
    icon: PathBuf,
) -> anyhow::Result<()> {
    crate::windows_integration::create_shortcut(&crate::windows_integration::ShortcutSpec {
        working_directory: target.parent().map(Path::to_path_buf),
        path,
        target,
        arguments: String::new(),
        description: description.to_string(),
        icon: Some(icon),
        show_minimized: false,
    })
}

#[cfg(windows)]
fn write_uninstall_registration(plan: &WindowsEntrypointPlan) -> anyhow::Result<()> {
    let install_location = Path::new(&plan.manager_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&plan.install_root))
        .to_string_lossy()
        .to_string();
    for (name, value) in [
        ("DisplayName", plan.display_name.clone()),
        ("DisplayVersion", crate::version::VERSION.to_string()),
        ("Publisher", plan.publisher.clone()),
        ("DisplayIcon", plan.manager_icon_path.clone()),
        ("InstallLocation", install_location),
        ("UninstallString", plan.uninstall_command.clone()),
        ("QuietUninstallString", plan.quiet_uninstall_command.clone()),
    ] {
        crate::windows_integration::set_current_user_string_value(UNINSTALL_SUBKEY, name, &value)?;
    }
    crate::windows_integration::delete_current_user_key(LEGACY_UNINSTALL_SUBKEY)?;
    Ok(())
}

#[cfg(windows)]
fn register_url_protocol(manager_path: &str) -> anyhow::Result<()> {
    crate::windows_integration::set_current_user_string_value(
        URL_PROTOCOL_SUBKEY,
        "",
        &format!("URL:{SILENT_NAME} Import Protocol"),
    )?;
    crate::windows_integration::set_current_user_string_value(
        URL_PROTOCOL_SUBKEY,
        "URL Protocol",
        "",
    )?;
    crate::windows_integration::set_current_user_string_value(
        &format!(r"{URL_PROTOCOL_SUBKEY}\shell\open\command"),
        "",
        &format!("\"{manager_path}\" \"%1\""),
    )?;
    Ok(())
}

fn default_icon_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .map(|path| path.join("codex-plus-plus.ico"))
        .unwrap_or_else(|| PathBuf::from("codex-plus-plus.ico"))
}

#[allow(dead_code)]
fn _entrypoint_names() -> (&'static str, &'static str) {
    (SILENT_NAME, MANAGER_NAME)
}

#[allow(dead_code)]
fn _legacy_entrypoint_names() -> (&'static str, &'static str) {
    let _ = (
        LEGACY_SILENT_NAME,
        LEGACY_MANAGER_NAME,
        LEGACY_MOJIBAKE_MANAGER_LNK,
    );
    legacy_shortcut_names()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    #[test]
    fn metadata_transaction_rolls_back_apply_failures() {
        let rolled_back = Cell::new(false);
        let result = super::run_metadata_transaction(
            || anyhow::bail!("apply failed"),
            || {
                rolled_back.set(true);
                Ok(())
            },
        );

        assert!(result.is_err());
        assert!(rolled_back.get());
    }

    #[test]
    fn metadata_transaction_reports_rollback_failures() {
        let error = super::run_metadata_transaction(
            || anyhow::bail!("apply failed"),
            || anyhow::bail!("rollback failed"),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("apply failed"));
        assert!(error.contains("rollback failed"));
    }

    #[cfg(windows)]
    #[test]
    fn setup_transaction_mutex_serializes_concurrent_metadata_changes() {
        use std::sync::mpsc;
        use std::time::Duration;

        let name = format!(
            r"Local\ChimeraPlusPlus.Setup.Transaction.Test.{}",
            uuid::Uuid::new_v4()
        );
        let first = super::acquire_named_transaction_mutex(&name).unwrap();
        let (sender, receiver) = mpsc::channel();
        let contender_name = name.clone();
        let contender = std::thread::spawn(move || {
            let _guard = super::acquire_named_transaction_mutex(&contender_name).unwrap();
            sender.send(()).unwrap();
        });

        assert!(receiver.recv_timeout(Duration::from_millis(150)).is_err());
        drop(first);
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        contender.join().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn metadata_snapshot_restores_shortcut_bytes_and_registry_value_types() {
        use std::os::windows::ffi::OsStrExt;

        let temp = tempfile::tempdir().unwrap();
        let shortcut = temp.path().join("Chimera++.lnk");
        std::fs::write(&shortcut, b"before").unwrap();
        let subkey = format!(r"Software\ChimeraPlusPlus\Tests\{}", uuid::Uuid::new_v4());
        let original_text = std::ffi::OsStr::new(r"%TEMP%\chimera")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let original_data = unsafe {
            std::slice::from_raw_parts(original_text.as_ptr().cast::<u8>(), original_text.len() * 2)
        }
        .to_vec();
        let original_value = crate::windows_integration::CurrentUserRegistryValueSnapshot {
            name: "ExpandedPath".to_string(),
            value_type: windows::Win32::System::Registry::REG_EXPAND_SZ.0,
            data: original_data.clone(),
        };
        crate::windows_integration::set_current_user_raw_value(&subkey, &original_value).unwrap();
        let snapshot = super::WindowsMetadataSnapshot::capture(
            std::slice::from_ref(&shortcut),
            std::slice::from_ref(&subkey),
        )
        .unwrap();

        std::fs::write(&shortcut, b"after").unwrap();
        crate::windows_integration::set_current_user_string_value(
            &subkey,
            "ExpandedPath",
            "changed",
        )
        .unwrap();
        snapshot.restore().unwrap();

        assert_eq!(std::fs::read(&shortcut).unwrap(), b"before");
        let restored = crate::windows_integration::snapshot_current_user_key_raw(&subkey)
            .unwrap()
            .unwrap();
        assert_eq!(restored, vec![original_value]);
        crate::windows_integration::delete_current_user_key(&subkey).unwrap();
    }
}
