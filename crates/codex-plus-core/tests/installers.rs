use codex_plus_core::branding::{
    DISPLAY_MANAGER_NAME, DISPLAY_SILENT_NAME, MACOS_BUILD_NUMBER, PUBLISHER,
};
use codex_plus_core::install::{
    InstallOptions, LEGACY_MANAGER_NAME, LEGACY_SILENT_NAME, MANAGER_BINARY, MANAGER_NAME,
    SILENT_BINARY, SILENT_NAME, app_bundle_names, build_macos_app_bundle,
    build_windows_entrypoint_plan, companion_binary_path_from_exe, default_install_root_strategy,
    detect_legacy_macos_apps, legacy_app_bundle_names, legacy_shortcut_names, shortcut_names,
    windows_legacy_shortcut_paths,
};

fn contains_ascii_token(text: &str, token: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .any(|part| part.eq_ignore_ascii_case(token))
}

fn contains_manual_update_instruction(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let manual = ["手动", "手工", "自行"]
        .iter()
        .any(|word| text.contains(word))
        || ["manual", "manually", "yourself"]
            .iter()
            .any(|word| lower.contains(word));
    let acquire = ["下载", "获取"].iter().any(|word| text.contains(word))
        || ["download", "fetch", "get"]
            .iter()
            .any(|word| lower.contains(word));
    let update_object = ["更新", "最新版", "安装包"]
        .iter()
        .any(|word| text.contains(word))
        || ["update", "latest", "installer", "package"]
            .iter()
            .any(|word| lower.contains(word));
    manual && acquire && update_object
}

fn legacy_codex_display_has_migration_context(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    ["原版", "旧", "覆盖升级", "退出正在运行"]
        .iter()
        .any(|word| line.contains(word))
        || [
            "legacy",
            "existing",
            "overlay",
            "in-place upgrade",
            "quit running",
        ]
        .iter()
        .any(|word| lower.contains(word))
}

#[test]
fn display_constants_come_from_branding_while_binaries_stay_stable() {
    assert_eq!(SILENT_NAME, DISPLAY_SILENT_NAME);
    assert_eq!(MANAGER_NAME, DISPLAY_MANAGER_NAME);
    assert_eq!(SILENT_NAME, "Chimera++");
    assert_eq!(MANAGER_NAME, "Chimera++ 管理工具");
    assert_eq!(SILENT_BINARY, "codex-plus-plus");
    assert_eq!(MANAGER_BINARY, "codex-plus-plus-manager");
    assert_eq!(LEGACY_SILENT_NAME, "Codex++");
    assert_eq!(LEGACY_MANAGER_NAME, "Codex++ 管理工具");
}

#[test]
fn windows_entrypoint_plan_creates_chimera_shortcuts_not_legacy() {
    let options = InstallOptions {
        install_root: Some("C:/Users/A/Desktop".into()),
        launcher_path: Some("C:/Tools/codex-plus-plus.exe".into()),
        manager_path: Some("C:/Tools/codex-plus-plus-manager.exe".into()),
        remove_owned_data: false,
    };

    let plan = build_windows_entrypoint_plan(&options);

    assert!(plan.silent_shortcut.ends_with("Chimera++.lnk"));
    assert!(plan.manager_shortcut.ends_with("Chimera++ 管理工具.lnk"));
    assert!(!plan.silent_shortcut.contains("Codex++.lnk"));
    assert_eq!(plan.launcher_path, "C:/Tools/codex-plus-plus.exe");
    assert_eq!(plan.manager_path, "C:/Tools/codex-plus-plus-manager.exe");
    assert_eq!(plan.silent_icon_path, "C:/Tools/codex-plus-plus.exe");
    assert_eq!(
        plan.manager_icon_path,
        "C:/Tools/codex-plus-plus-manager.exe"
    );
    assert_eq!(plan.display_name, "Chimera++");
    assert_eq!(plan.publisher, PUBLISHER);
    assert_eq!(plan.uninstall_key, "CodexPlusPlus");
    assert_eq!(plan.legacy_uninstall_key, "Codex++");
    assert_eq!(
        plan.uninstaller_path.replace('\\', "/"),
        "C:/Tools/uninstall.exe"
    );
    assert_eq!(
        plan.uninstall_command.replace('\\', "/"),
        "\"C:/Tools/uninstall.exe\""
    );
    assert_eq!(
        plan.quiet_uninstall_command.replace('\\', "/"),
        "\"C:/Tools/uninstall.exe\" /S"
    );
}

#[test]
fn windows_installers_share_current_arp_key_and_remove_only_the_legacy_entry() {
    let plan = build_windows_entrypoint_plan(&InstallOptions::default());
    let current_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        plan.uninstall_key
    );
    let legacy_key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        plan.legacy_uninstall_key
    );
    assert_eq!(
        current_key,
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\CodexPlusPlus"
    );
    assert_eq!(
        legacy_key,
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex++"
    );
    assert_ne!(current_key, legacy_key);

    let nsi = std::fs::read_to_string("../../scripts/installer/windows/CodexPlusPlus.nsi")
        .expect("read NSIS script");
    assert!(nsi.contains(&format!("!define CURRENT_UNINSTALL_KEY \"{current_key}\"")));
    assert!(nsi.contains(&format!("!define LEGACY_UNINSTALL_KEY \"{legacy_key}\"")));

    let install_success = nsi
        .split("Section \"Install\"")
        .nth(1)
        .and_then(|value| value.split("install_metadata_failed:").next())
        .expect("successful install path");
    for value_name in [
        "DisplayName",
        "DisplayVersion",
        "Publisher",
        "DisplayIcon",
        "InstallLocation",
        "UninstallString",
        "QuietUninstallString",
    ] {
        assert!(
            install_success.contains(&format!(
                "WriteRegStr HKCU \"${{CURRENT_UNINSTALL_KEY}}\" \"{value_name}\""
            )),
            "current ARP entry missing {value_name}"
        );
        assert!(
            !install_success.contains(&format!(
                "WriteRegStr HKCU \"${{LEGACY_UNINSTALL_KEY}}\" \"{value_name}\""
            )),
            "legacy ARP entry must not be written"
        );
    }

    let current_write = install_success
        .find("WriteRegStr HKCU \"${CURRENT_UNINSTALL_KEY}\" \"DisplayName\"")
        .expect("current ARP registration");
    let legacy_delete = install_success
        .find("DeleteRegKey HKCU \"${LEGACY_UNINSTALL_KEY}\"")
        .expect("legacy ARP cleanup");
    assert!(
        current_write < legacy_delete,
        "legacy entry must be removed only after current registration succeeds"
    );
    assert!(install_success[legacy_delete..].contains("IfErrors install_legacy_cleanup_failed"));

    let rollback = nsi
        .split("install_metadata_rollback:")
        .nth(1)
        .and_then(|value| value.split("install_rollback:").next())
        .expect("metadata rollback block");
    assert!(rollback.contains("RestoreRegValue \"${CURRENT_UNINSTALL_KEY}\""));
    assert!(!rollback.contains("RestoreRegValue \"${LEGACY_UNINSTALL_KEY}\""));

    let rust_installer = include_str!("../src/install/windows.rs");
    let registration = rust_installer
        .split("fn write_uninstall_registration")
        .nth(1)
        .and_then(|value| value.split("\n#[cfg(").next())
        .expect("Rust uninstall registration");
    let current_write = registration
        .find("set_current_user_string_value(UNINSTALL_SUBKEY")
        .expect("Rust current ARP write");
    let legacy_delete = registration
        .find("delete_current_user_key(LEGACY_UNINSTALL_SUBKEY)?")
        .expect("strict Rust legacy ARP cleanup");
    assert!(
        current_write < legacy_delete,
        "Rust repair must register the current ARP entry before removing legacy"
    );

    let integration = include_str!("../src/windows_integration.rs");
    let delete_helper = integration
        .split("pub fn delete_current_user_key")
        .nth(1)
        .and_then(|value| value.split("\n}").next())
        .expect("registry delete helper");
    assert!(delete_helper.contains("ERROR_FILE_NOT_FOUND"));
    assert!(delete_helper.contains("ERROR_PATH_NOT_FOUND"));
    assert!(!delete_helper.contains(".or_else(|_| Ok(()))"));
}

#[test]
fn windows_legacy_shortcut_cleanup_lists_old_and_mojibake_names() {
    let root = std::path::Path::new("C:/Users/A/Desktop");
    let paths = windows_legacy_shortcut_paths(root);
    let joined = paths
        .iter()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(joined.contains("Codex++.lnk"));
    assert!(joined.contains("Codex++ 管理工具.lnk"));
    assert!(joined.contains("Codex++ 绠＄悊宸ュ叿.lnk"));
    assert!(!joined.contains("Chimera Codex.lnk"));
}

#[test]
fn windows_entrypoint_plan_can_request_owned_data_removal_without_shell_script() {
    let options = InstallOptions {
        install_root: Some("C:/Users/A/Desktop".into()),
        launcher_path: None,
        manager_path: None,
        remove_owned_data: true,
    };

    let plan = build_windows_entrypoint_plan(&options);

    assert!(plan.silent_shortcut.ends_with("Chimera++.lnk"));
    assert!(plan.manager_shortcut.ends_with("Chimera++ 管理工具.lnk"));
    assert!(plan.remove_owned_data);
}

#[test]
fn macos_bundle_metadata_uses_chimera_names_and_numeric_versions() {
    let options = InstallOptions {
        install_root: Some("/Applications".into()),
        launcher_path: Some("/opt/ChimeraPlusPlus/codex-plus-plus".into()),
        manager_path: Some("/opt/ChimeraPlusPlus/codex-plus-plus-manager".into()),
        remove_owned_data: false,
    };

    let silent = build_macos_app_bundle(&options, false);
    let manager = build_macos_app_bundle(&options, true);

    assert!(silent.app_path.ends_with("Chimera++.app"));
    assert!(manager.app_path.ends_with("Chimera++ 管理工具.app"));
    assert!(silent.info_plist.contains("<string>Chimera++</string>"));
    assert!(
        manager
            .info_plist
            .contains("<string>Chimera++ 管理工具</string>")
    );
    assert!(
        silent
            .info_plist
            .contains("com.bigpizzav3.codexplusplus</string>")
    );
    assert!(
        manager
            .info_plist
            .contains("com.bigpizzav3.codexplusplus.manager</string>")
    );
    assert!(silent.info_plist.contains("<string>CodexPlusPlus</string>"));
    assert!(
        manager
            .info_plist
            .contains("<string>CodexPlusPlusManager</string>")
    );

    let short_version = codex_plus_core::version::VERSION
        .split('-')
        .next()
        .expect("marketing version");
    assert!(
        silent.info_plist.contains(&format!(
            "<key>CFBundleShortVersionString</key>\n  <string>{short_version}</string>"
        )) || silent.info_plist.contains(&format!(
            "<key>CFBundleShortVersionString</key>\n  <string>{short_version}</string>"
        ))
    );
    assert!(!silent.info_plist.contains(&format!(
        "<key>CFBundleShortVersionString</key>\n  <string>{}</string>",
        codex_plus_core::version::VERSION
    )));
    assert!(silent.info_plist.contains(&format!(
        "<key>CFBundleVersion</key>\n  <string>{MACOS_BUILD_NUMBER}</string>"
    )));
    assert_eq!(
        silent.binary_target_name.as_deref(),
        Some("codex-plus-plus")
    );
    assert_eq!(
        manager.binary_target_name.as_deref(),
        Some("codex-plus-plus-manager")
    );
    assert!(silent.launch_script.contains("$DIR/codex-plus-plus"));
    assert!(
        manager
            .launch_script
            .contains("$DIR/codex-plus-plus-manager")
    );
}

#[test]
fn installer_exports_chimera_and_legacy_entrypoint_names() {
    assert_eq!(
        shortcut_names(),
        (
            "Chimera++.lnk".to_string(),
            "Chimera++ 管理工具.lnk".to_string()
        )
    );
    assert_eq!(
        app_bundle_names(),
        (
            "Chimera++.app".to_string(),
            "Chimera++ 管理工具.app".to_string()
        )
    );
    assert_eq!(
        legacy_shortcut_names(),
        ("Codex++.lnk", "Codex++ 管理工具.lnk")
    );
    assert_eq!(
        legacy_app_bundle_names(),
        ("Codex++.app", "Codex++ 管理工具.app")
    );
}

#[test]
fn macos_dmg_includes_applications_shortcut_and_chimera_artifact_name() {
    let script = std::fs::read_to_string("../../scripts/installer/macos/package-dmg.sh")
        .expect("read macOS DMG packaging script");

    assert!(script.contains("ln -s /Applications \"$STAGE/Applications\""));
    assert!(script.contains("ChimeraPlusPlus-${VERSION}-macos-${ARCH}.dmg"));
    assert!(script.contains("\"Chimera++\""));
    assert!(script.contains("\"Chimera++ 管理工具\""));
    assert!(script.contains("CFBundleShortVersionString"));
    assert!(script.contains("SHORT_VERSION"));
    assert!(script.contains("MACOS_BUILD_NUMBER"));
    assert!(script.contains("codesign --force --sign -"));
    assert!(script.contains("cp \"$ROOT/LICENSE\" \"$STAGE/LICENSE\""));
    assert!(script.contains("cp \"$ROOT/NOTICE\" \"$STAGE/NOTICE\""));
    assert!(script.contains("cp \"$ROOT/SOURCE_CODE.txt\" \"$STAGE/SOURCE_CODE.txt\""));
    assert!(
        script.contains("ad-hoc")
            || script.contains("ad hoc")
            || script.contains("Developer ID")
            || script.contains("notariz")
    );
    assert!(!script.contains("CodexPlusPlus-${VERSION}-macos-${ARCH}.dmg"));
}

#[test]
fn macos_detects_legacy_apps_without_deleting_them() {
    let root = tempfile::tempdir().expect("tempdir");
    let legacy_silent = root.path().join("Codex++.app");
    let legacy_manager = root.path().join("Codex++ 管理工具.app");
    let chimera_silent = root.path().join("Chimera Codex.app");
    std::fs::create_dir_all(&legacy_silent).expect("legacy silent");
    std::fs::create_dir_all(&legacy_manager).expect("legacy manager");
    std::fs::create_dir_all(&chimera_silent).expect("chimera silent");

    let detected = detect_legacy_macos_apps(&[root.path().to_path_buf()]);

    assert_eq!(detected.paths.len(), 2);
    assert!(
        detected
            .paths
            .iter()
            .any(|path| path.ends_with("Codex++.app"))
    );
    assert!(
        detected
            .paths
            .iter()
            .any(|path| path.ends_with("Codex++ 管理工具.app"))
    );
    assert!(
        !detected
            .paths
            .iter()
            .any(|path| path.ends_with("Chimera Codex.app"))
    );
    assert!(!detected.message.is_empty());
    assert!(legacy_silent.exists());
    assert!(legacy_manager.exists());
}

#[test]
fn companion_binary_path_resolves_macos_silent_app_next_to_manager_app() {
    let manager_exe = std::path::Path::new(
        "/Applications/Chimera++ 管理工具.app/Contents/MacOS/CodexPlusPlusManager",
    );

    let companion = companion_binary_path_from_exe(manager_exe, SILENT_BINARY);

    assert_eq!(
        companion,
        std::path::PathBuf::from("/Applications/Chimera++.app/Contents/MacOS/CodexPlusPlus")
    );
    assert_ne!(
        companion,
        std::path::PathBuf::from(
            "/Applications/Chimera++ 管理工具.app/Contents/MacOS/codex-plus-plus"
        )
    );
}

#[test]
fn companion_binary_path_resolves_macos_manager_app_next_to_silent_app() {
    let silent_exe =
        std::path::Path::new("/Applications/Chimera++.app/Contents/MacOS/CodexPlusPlus");

    let companion =
        companion_binary_path_from_exe(silent_exe, codex_plus_core::install::MANAGER_BINARY);

    assert_eq!(
        companion,
        std::path::PathBuf::from(
            "/Applications/Chimera++ 管理工具.app/Contents/MacOS/CodexPlusPlusManager"
        )
    );
}

#[test]
fn macos_bundle_does_not_wrap_the_bundle_executable_in_itself() {
    let options = InstallOptions {
        install_root: Some("/Applications".into()),
        launcher_path: Some("/Applications/Chimera++.app/Contents/MacOS/CodexPlusPlus".into()),
        manager_path: Some(
            "/Applications/Chimera++ 管理工具.app/Contents/MacOS/CodexPlusPlusManager".into(),
        ),
        remove_owned_data: false,
    };

    let silent = build_macos_app_bundle(&options, false);
    let manager = build_macos_app_bundle(&options, true);

    assert_eq!(
        silent.binary_source,
        Some(std::path::PathBuf::from(
            "/Applications/Chimera++.app/Contents/MacOS/CodexPlusPlus"
        ))
    );
    assert_eq!(
        manager.binary_source,
        Some(std::path::PathBuf::from(
            "/Applications/Chimera++ 管理工具.app/Contents/MacOS/CodexPlusPlusManager"
        ))
    );
    assert!(silent.launch_script.contains("$DIR/codex-plus-plus"));
    assert!(
        manager
            .launch_script
            .contains("$DIR/codex-plus-plus-manager")
    );
}

#[test]
fn windows_nsi_uses_chimera_branding_keeps_install_dir_and_cleans_legacy() {
    let nsi = std::fs::read_to_string("../../scripts/installer/windows/CodexPlusPlus.nsi")
        .expect("read NSIS script");

    assert!(nsi.contains("Name \"Chimera++\""));
    assert!(nsi.contains("ChimeraPlusPlus-${VERSION}-windows-x64-setup.exe"));
    assert!(nsi.contains("InstallDir \"$LOCALAPPDATA\\Programs\\Codex++\""));
    assert!(nsi.contains("RequestExecutionLevel user"));
    assert!(!nsi.contains("RequestExecutionLevel admin"));
    assert!(
        nsi.contains("Publisher\" \"ChimeraHub\"")
            || nsi.contains("\"Publisher\" \"ChimeraHub\"")
            || nsi.contains("Publisher\" \"ChimeraHub")
    );
    assert!(
        nsi.contains("DisplayName\" \"Chimera++\"")
            || nsi.contains("\"DisplayName\" \"Chimera++\"")
    );
    assert!(nsi.contains("!insertmacro DeleteInstallShortcut \"$DESKTOP\\Codex++.lnk\""));
    assert!(nsi.contains("!insertmacro DeleteInstallShortcut \"$DESKTOP\\Codex++ 管理工具.lnk\""));
    assert!(nsi.contains("!insertmacro UninstallShortcut \"$DESKTOP\\Chimera++.lnk\""));
    assert!(nsi.contains("CreateShortcut \"$DESKTOP\\Chimera++.lnk\""));
    assert!(!nsi.contains("CreateShortcut \"$DESKTOP\\Chimera++ 管理工具.lnk\""));
    assert_eq!(
        nsi.lines()
            .filter(|line| line.trim_start().starts_with("CreateShortcut \"$DESKTOP\\"))
            .count(),
        1
    );
    assert!(nsi.contains("CreateShortcut \"$SMPROGRAMS\\Chimera++\\Chimera++ 管理工具.lnk\""));
    assert!(nsi.contains("!insertmacro BackupShortcut \"$DESKTOP\\Chimera++ 管理工具.lnk\""));
    assert!(nsi.contains("!insertmacro RollbackShortcut \"$DESKTOP\\Chimera++ 管理工具.lnk\""));
    assert!(nsi.contains("File \"/oname=LICENSE.new\" \"${ROOT}\\LICENSE\""));
    assert!(nsi.contains("File \"/oname=NOTICE.new\" \"${ROOT}\\NOTICE\""));
    assert!(nsi.contains("File \"/oname=SOURCE_CODE.txt.new\" \"${ROOT}\\SOURCE_CODE.txt\""));
    assert!(nsi.contains("Delete \"$INSTDIR\\LICENSE\""));
    assert!(nsi.contains("Delete \"$INSTDIR\\NOTICE\""));
    assert!(nsi.contains("Delete \"$INSTDIR\\SOURCE_CODE.txt\""));
    assert!(nsi.contains(".exe.new"));
    assert!(nsi.contains(".exe.bak"));
    assert!(nsi.contains("uninstall.exe.new"));
    assert!(nsi.contains("uninstall.exe.bak"));
    assert!(nsi.contains("install_rollback"));
    assert!(nsi.contains("install_metadata_failed:"));
    assert!(nsi.contains("install_metadata_rollback:"));
    assert!(nsi.contains("rollback_failed:"));
    assert!(nsi.contains("IfErrors"));
    assert!(nsi.matches("IfErrors install_metadata_failed").count() >= 8);
    assert!(nsi.contains("\"UninstallString\" '$\\\"$INSTDIR\\uninstall.exe$\\\"'"));

    let metadata_rollback = nsi
        .split("install_metadata_rollback:")
        .nth(1)
        .and_then(|value| value.split("install_rollback:").next())
        .expect("metadata rollback block");
    assert!(metadata_rollback.contains("\"$DESKTOP\\Chimera++.lnk\""));
    assert!(metadata_rollback.contains("!insertmacro RestoreRegValue"));
    assert!(metadata_rollback.contains("ReadINIStr"));
    assert!(metadata_rollback.contains("!insertmacro RollbackShortcut"));
    assert!(metadata_rollback.contains("Goto install_rollback"));
    assert!(nsi.contains("DeleteRegValue HKCU"));
    assert!(nsi.contains("CopyFiles /SILENT"));
    let restore_reg_macro = nsi
        .split("!macro RestoreRegValue")
        .nth(1)
        .and_then(|value| value.split("!macroend").next())
        .expect("registry restore macro");
    assert!(restore_reg_macro.contains("ReadRegStr $9 HKCU \"${KEY}\" \"${VALUE}\""));
    assert!(restore_reg_macro.contains("restore_reg_${SLOT}_failed:"));
    assert!(restore_reg_macro.contains("StrCpy $R9 \"1\""));
    assert!(
        nsi.matches("IfErrors install_backup_cleanup_failed")
            .count()
            >= 3
    );
    assert!(nsi.contains("install_backup_cleanup_failed:"));

    let ordered = [
        "codex-plus-plus.exe.new",
        "codex-plus-plus-manager.exe.new",
        "uninstall.exe.new",
        "Rename \"$INSTDIR\\codex-plus-plus.exe\" \"$INSTDIR\\codex-plus-plus.exe.bak\"",
        "Rename \"$INSTDIR\\codex-plus-plus-manager.exe\" \"$INSTDIR\\codex-plus-plus-manager.exe.bak\"",
        "Rename \"$INSTDIR\\uninstall.exe\" \"$INSTDIR\\uninstall.exe.bak\"",
        "Rename \"$INSTDIR\\codex-plus-plus.exe.new\" \"$INSTDIR\\codex-plus-plus.exe\"",
        "Rename \"$INSTDIR\\codex-plus-plus-manager.exe.new\" \"$INSTDIR\\codex-plus-plus-manager.exe\"",
        "Rename \"$INSTDIR\\uninstall.exe.new\" \"$INSTDIR\\uninstall.exe\"",
    ];
    let mut previous = 0;
    for needle in ordered {
        let position = nsi
            .find(needle)
            .unwrap_or_else(|| panic!("missing {needle}"));
        assert!(
            position >= previous,
            "transaction order regressed at {needle}"
        );
        previous = position;
    }
    assert!(!nsi.contains("Publisher\" \"BigPizzaV3\""));
    assert!(!nsi.contains("CodexPlusPlus-${VERSION}-windows-x64-setup.exe"));
    // 保留乱码清理字面量，供历史坏快捷方式卸载
    assert!(nsi.contains("绠＄悊宸ュ叿"));
}

#[test]
fn windows_install_shortcut_cleanup_is_fail_closed() {
    let nsi = std::fs::read_to_string("../../scripts/installer/windows/CodexPlusPlus.nsi")
        .expect("read NSIS script");
    let windows_rs =
        std::fs::read_to_string("src/install/windows.rs").expect("read Windows runtime installer");
    let macro_body = nsi
        .split("!macro DeleteInstallShortcut PATH SLOT")
        .nth(1)
        .and_then(|value| value.split("!macroend").next())
        .expect("fail-closed install shortcut macro");
    let active: Vec<&str> = macro_body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(';'))
        .collect();
    let position = |needle: &str| active.iter().position(|line| *line == needle);
    assert!(matches!(
        (
            position("IfFileExists \"${PATH}\" 0 install_shortcut_${SLOT}_done"),
            position("ClearErrors"),
            position("Delete \"${PATH}\""),
            position("IfErrors install_metadata_failed"),
        ),
        (Some(exists), Some(clear), Some(delete), Some(fail))
            if exists < clear && clear < delete && delete < fail
    ));
    for (path, destination, slot) in [
        ("$DESKTOP\\Codex++.lnk", "$DESKTOP", "DesktopLegacySilent"),
        (
            "$DESKTOP\\Codex++ 管理工具.lnk",
            "$DESKTOP",
            "DesktopLegacyManager",
        ),
        (
            "$DESKTOP\\Codex++ 绠＄悊宸ュ叿.lnk",
            "$DESKTOP",
            "DesktopMojibakeManager",
        ),
        (
            "$SMPROGRAMS\\Codex++\\Codex++.lnk",
            "$SMPROGRAMS\\Codex++",
            "MenuLegacySilent",
        ),
        (
            "$SMPROGRAMS\\Codex++\\Codex++ 管理工具.lnk",
            "$SMPROGRAMS\\Codex++",
            "MenuLegacyManager",
        ),
        (
            "$SMPROGRAMS\\Codex++\\Codex++ 绠＄悊宸ュ叿.lnk",
            "$SMPROGRAMS\\Codex++",
            "MenuMojibakeManager",
        ),
        (
            "$SMPROGRAMS\\Codex++\\卸载 Codex++.lnk",
            "$SMPROGRAMS\\Codex++",
            "MenuLegacyUninstall",
        ),
        (
            "$SMPROGRAMS\\Codex++\\Chimera++.lnk",
            "$SMPROGRAMS\\Codex++",
            "MenuCompatSilent",
        ),
        (
            "$SMPROGRAMS\\Codex++\\Chimera++ 管理工具.lnk",
            "$SMPROGRAMS\\Codex++",
            "MenuCompatManager",
        ),
        (
            "$SMPROGRAMS\\Codex++\\卸载 Chimera++.lnk",
            "$SMPROGRAMS\\Codex++",
            "MenuCompatUninstall",
        ),
    ] {
        assert!(nsi.contains(&format!(
            "!insertmacro BackupShortcut \"{path}\" \"{slot}\""
        )));
        assert!(nsi.contains(&format!(
            "!insertmacro DeleteInstallShortcut \"{path}\" \"{slot}\""
        )));
        assert!(nsi.contains(&format!(
            "!insertmacro RollbackShortcut \"{path}\" \"{destination}\" \"{slot}\""
        )));
    }

    let install = nsi
        .split("Section \"Install\"")
        .nth(1)
        .and_then(|value| value.split("Section \"Uninstall\"").next())
        .expect("install section");
    let backup_cleanup = install
        .find("cleanup_source_code_backup_done:")
        .expect("last rollback-capable cleanup");
    let legacy_arp_delete = install
        .rfind("DeleteRegKey HKCU \"${LEGACY_UNINSTALL_KEY}\"")
        .expect("final legacy ARP deletion");
    assert!(backup_cleanup < legacy_arp_delete);
    assert!(!install.contains("BackupRegValue \"${LEGACY_UNINSTALL_KEY}"));
    assert!(!install.contains("RestoreRegValue \"${LEGACY_UNINSTALL_KEY}"));

    assert!(windows_rs.contains("run_metadata_transaction"));
    assert!(windows_rs.contains(r#"Local\ChimeraPlusPlus.Setup.Transaction"#));
    assert_eq!(
        windows_rs
            .matches("let _transaction_guard = acquire_setup_transaction_mutex()?")
            .count(),
        2
    );
    assert!(!windows_rs.contains("let _ = std::fs::remove_file"));
    assert!(!windows_rs.contains("let _ = crate::windows_integration::delete_current_user_key"));
}

#[test]
fn windows_nsi_acquires_local_named_mutex_before_using_fixed_staging_files() {
    let nsi = std::fs::read_to_string("../../scripts/installer/windows/CodexPlusPlus.nsi")
        .expect("read NSIS script");

    assert!(nsi.contains("!define SETUP_MUTEX_NAME \"Local\\ChimeraPlusPlus.Setup.Transaction\""));
    assert!(
        !nsi.contains("\"Global\\"),
        "per-user setup must not require a Global mutex"
    );

    let on_init = nsi
        .split("Function .onInit")
        .nth(1)
        .and_then(|value| value.split("FunctionEnd").next())
        .expect("installer .onInit");
    assert!(on_init.contains("CreateMutexW"));
    assert!(on_init.contains("CreateMutexW(p 0, i 1,"));
    assert!(on_init.contains("${SETUP_MUTEX_NAME}"));
    assert!(on_init.contains("?e"));
    assert!(on_init.contains("183"));
    assert!(on_init.contains("Abort"));

    let mutex_position = nsi.find("Function .onInit").expect("mutex initialization");
    let install_position = nsi.find("Section \"Install\"").expect("install section");
    let staging_position = nsi
        .find("codex-plus-plus.exe.new")
        .expect("fixed staging file");
    assert!(mutex_position < install_position);
    assert!(install_position < staging_position);
}

#[test]
fn windows_legal_files_share_the_binary_transaction_and_uninstall_mutex() {
    let nsi = std::fs::read_to_string("../../scripts/installer/windows/CodexPlusPlus.nsi")
        .expect("read NSIS script");

    fn uninstall_file_macro_is_fail_closed(source: &str) -> bool {
        let Some(body) = source
            .split("!macro UninstallFile PATH SLOT")
            .nth(1)
            .and_then(|value| value.split("!macroend").next())
        else {
            return false;
        };
        let active: Vec<&str> = body
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with(';'))
            .collect();
        let position = |needle: &str| active.iter().position(|line| *line == needle);
        matches!(
            (
                position("IfFileExists \"${PATH}\" 0 uninstall_file_${SLOT}_done"),
                position("ClearErrors"),
                position("Delete \"${PATH}\""),
                position("IfErrors uninstall_failed"),
            ),
            (Some(exists), Some(clear), Some(delete), Some(fail))
                if exists < clear && clear < delete && delete < fail
        )
    }

    assert!(uninstall_file_macro_is_fail_closed(&nsi));
    let commented_error_jump = nsi.replacen(
        "  IfErrors uninstall_failed",
        "  ; IfErrors uninstall_failed",
        1,
    );
    assert_ne!(
        commented_error_jump, nsi,
        "mutation fixture must change macro"
    );
    assert!(
        !uninstall_file_macro_is_fail_closed(&commented_error_jump),
        "commenting the error jump must fail the macro contract"
    );

    for legal_file in ["LICENSE", "NOTICE", "SOURCE_CODE.txt"] {
        assert!(nsi.contains(&format!("/oname={legal_file}.new")));
        assert!(nsi.contains(&format!("$INSTDIR\\{legal_file}.bak")));
        assert!(nsi.contains(&format!(
            "Rename \"$INSTDIR\\{legal_file}.new\" \"$INSTDIR\\{legal_file}\""
        )));
        assert!(nsi.contains(&format!(
            "Rename \"$INSTDIR\\{legal_file}.bak\" \"$INSTDIR\\{legal_file}\""
        )));
        assert!(nsi.contains(&format!("Delete \"$INSTDIR\\{legal_file}.new\"")));
        assert!(nsi.contains(&format!(
            "!insertmacro UninstallFile \"$INSTDIR\\{legal_file}.new\""
        )));
    }

    let uninstall_init = nsi
        .split("Function un.onInit")
        .nth(1)
        .and_then(|value| value.split("FunctionEnd").next())
        .expect("uninstaller mutex initialization");
    assert!(uninstall_init.contains("${SETUP_MUTEX_NAME}"));
    assert!(uninstall_init.contains("CreateMutexW"));
    assert!(uninstall_init.contains("CreateMutexW(p 0, i 1,"));
    assert!(uninstall_init.contains("183"));
    assert!(uninstall_init.contains("Abort"));
    let mutex = nsi.find("Function un.onInit").expect("uninstaller mutex");
    let uninstall = nsi
        .find("Section \"Uninstall\"")
        .expect("uninstall section");
    assert!(mutex < uninstall);
}

#[test]
fn release_assets_workflow_verifies_chimera_bundle_paths() {
    let workflow = std::fs::read_to_string("../../.github/workflows/release-assets.yml")
        .expect("read release-assets workflow");

    assert!(workflow.contains("Chimera++.app"));
    assert!(workflow.contains("Chimera++ 管理工具.app"));
    assert!(!workflow.contains("dist/macos/stage/Codex++.app"));
    assert!(workflow.contains("codesign --verify --deep --strict"));
    assert!(workflow.contains("Copy-Item LICENSE,NOTICE,SOURCE_CODE.txt dist/windows/app/"));
    assert!(
        workflow
            .contains("cp LICENSE NOTICE SOURCE_CODE.txt \"dist/macos/app-${{ matrix.arch }}/\"")
    );
    assert!(workflow.contains("test -f \"dist/macos/stage/LICENSE\""));
    assert!(workflow.contains("test -f \"dist/macos/stage/NOTICE\""));
    assert!(workflow.contains("test -f \"dist/macos/stage/SOURCE_CODE.txt\""));
}

#[test]
fn first_release_publish_job_is_build_first_and_environment_gated() {
    let workflow = std::fs::read_to_string("../../.github/workflows/release-assets.yml")
        .expect("read release-assets workflow");
    let publish = workflow
        .split("\n  publish-release:")
        .nth(1)
        .expect("publish-release job");

    let environments = publish
        .lines()
        .filter_map(|line| line.strip_prefix("    environment: "))
        .collect::<Vec<_>>();
    assert_eq!(environments, ["public-release"]);
    let environment_commented = publish.replacen(
        "    environment: public-release",
        "    # environment: public-release",
        1,
    );
    assert!(
        environment_commented
            .lines()
            .all(|line| line.strip_prefix("    environment: ") != Some("public-release"))
    );
    for dependency in ["resolve-version", "gates", "windows-installer", "macos-dmg"] {
        assert!(
            publish.contains(&format!("      - {dependency}")),
            "publish-release must depend on {dependency}"
        );
    }
    assert!(publish.contains("if: needs.resolve-version.outputs.should_publish == 'true'"));
    assert!(publish.contains("permissions:\n      contents: write"));
    assert!(workflow.contains("permissions:\n  contents: read"));
}

#[test]
fn required_check_names_are_stable_and_release_side_effects_are_publish_only() {
    fn job_section<'a>(workflow: &'a str, job_id: &str) -> &'a str {
        let marker = format!("\n  {job_id}:");
        let rest = workflow
            .split_once(&marker)
            .unwrap_or_else(|| panic!("missing workflow job {job_id}"))
            .1;
        let end = rest
            .match_indices("\n  ")
            .find_map(|(index, _)| {
                let line = rest[index + 1..].lines().next()?;
                (line.ends_with(':') && !line.starts_with("    ")).then_some(index)
            })
            .unwrap_or(rest.len());
        &rest[..end]
    }

    fn required_names_are_stable(workflow: &str) -> bool {
        [
            ("gates", "Branding / ads / Rust / frontend"),
            ("windows-artifacts", "Windows artifacts"),
            ("macos-dmg", "macOS DMG (${{ matrix.arch }})"),
        ]
        .iter()
        .all(|(job_id, expected)| {
            let names = job_section(workflow, job_id)
                .lines()
                .filter_map(|line| line.strip_prefix("    name: "))
                .collect::<Vec<_>>();
            names == [*expected]
        })
    }

    let pr_workflow =
        std::fs::read_to_string("../../.github/workflows/pr-build.yml").expect("read PR workflow");
    assert!(required_names_are_stable(&pr_workflow));
    assert!(job_section(&pr_workflow, "macos-dmg").contains("- arch: x64"));
    assert!(job_section(&pr_workflow, "macos-dmg").contains("- arch: arm64"));
    let renamed = pr_workflow.replacen(
        "    name: Windows artifacts",
        "    name: Windows packages\n    # name: Windows artifacts",
        1,
    );
    assert!(
        !required_names_are_stable(&renamed),
        "a comment must not mask a required job rename"
    );

    fn has_no_early_release_side_effect(workflow: &str) -> bool {
        let Some((before_publish, _)) = workflow.split_once("\n  publish-release:") else {
            return false;
        };
        [
            "gh release create",
            "gh release upload",
            "gh api",
            "git push origin \"refs/tags/",
        ]
        .iter()
        .all(|command| !before_publish.contains(command))
    }

    let release_workflow = std::fs::read_to_string("../../.github/workflows/release-assets.yml")
        .expect("read release workflow");
    assert!(has_no_early_release_side_effect(&release_workflow));
    assert_eq!(release_workflow.matches("contents: write").count(), 1);
    assert!(release_workflow.contains("permissions:\n  contents: read"));
    let publish = job_section(&release_workflow, "publish-release");
    assert!(publish.contains("    permissions:\n      contents: write"));
    assert!(
        !release_workflow
            .split_once("\n  publish-release:")
            .expect("publish job boundary")
            .0
            .contains("contents: write")
    );
    let mutated = release_workflow.replacen(
        "    steps:\n      - name: Checkout",
        "    steps:\n      - name: Mutation creates an early Release\n        run: gh release create v0.0.0-test\n      - name: Checkout",
        1,
    );
    assert!(
        !has_no_early_release_side_effect(&mutated),
        "an early Release mutation must fail the build-first contract"
    );
    let api_mutated = release_workflow.replacen(
        "    steps:\n      - name: Checkout",
        "    steps:\n      - name: Mutation calls the Release API early\n        run: gh api --method POST repos/example/releases\n      - name: Checkout",
        1,
    );
    assert!(
        !has_no_early_release_side_effect(&api_mutated),
        "an early GitHub Release API mutation must fail the build-first contract"
    );
}

#[test]
fn macos_packager_refuses_existing_outputs_without_recursive_deletion() {
    let script = std::fs::read_to_string("../../scripts/installer/macos/package-dmg.sh")
        .expect("read macOS packager");

    assert!(!script.contains("rm -rf"));
    assert!(script.contains("refuse_existing_path"));
    assert!(script.contains("refusing to overwrite existing path"));
    assert!(script.contains("refuse_symlink_parent \"$ROOT/dist\""));
    assert!(script.contains("x64|x86_64) ARCH=\"x64\""));
    assert!(script.contains("arm64|aarch64) ARCH=\"arm64\""));
    assert!(script.contains("lipo -archs"));
    assert!(script.contains("binary architecture mismatch"));
    assert!(script.contains("codesign --verify --deep --strict"));
}

#[test]
fn windows_uninstall_keeps_recovery_entries_when_program_files_cannot_be_removed() {
    let nsi = std::fs::read_to_string("../../scripts/installer/windows/CodexPlusPlus.nsi")
        .expect("read NSIS script");
    let uninstall = nsi
        .split("Section \"Uninstall\"")
        .nth(1)
        .expect("uninstall section");
    let file_delete = uninstall
        .find("Delete \"$INSTDIR\\codex-plus-plus.exe\"")
        .expect("program file deletion");
    let shortcut_delete = uninstall
        .find("!insertmacro UninstallShortcut \"$DESKTOP\\Codex++.lnk\"")
        .expect("shortcut deletion");
    let registry_delete = uninstall
        .find("!insertmacro UninstallRegKey")
        .expect("registry deletion");
    let uninstaller_delete = uninstall
        .rfind("Delete \"$INSTDIR\\uninstall.exe\"")
        .expect("uninstaller self deletion");

    assert!(file_delete < shortcut_delete);
    assert!(shortcut_delete < registry_delete);
    assert!(registry_delete < uninstaller_delete);
    assert!(uninstall.contains("IfErrors uninstall_failed"));
    assert!(uninstall.contains("!insertmacro UninstallShortcut"));
    assert!(uninstall.contains("!insertmacro UninstallRegKey"));
    let uninstall_reg_key_macro = nsi
        .split("!macro UninstallRegKey KEY SLOT")
        .nth(1)
        .and_then(|value| value.split("!macroend").next())
        .expect("conservative registry key cleanup macro");
    assert!(uninstall_reg_key_macro.contains("DeleteRegKey /ifempty HKCU \"${KEY}\""));
    assert!(!uninstall_reg_key_macro.contains("DeleteRegKey HKCU \"${KEY}\""));

    let mut previous_protocol_value_delete = shortcut_delete;
    for value in [
        "!insertmacro UninstallBackedRegValue \"${URL_PROTOCOL_SUBKEY}\\shell\\open\\command\" \"\" \"UrlCommand\"",
        "!insertmacro UninstallBackedRegValue \"${URL_PROTOCOL_SUBKEY}\" \"URL Protocol\" \"UrlProtocol\"",
        "!insertmacro UninstallBackedRegValue \"${URL_PROTOCOL_SUBKEY}\" \"\" \"UrlDisplayName\"",
    ] {
        let position = uninstall
            .find(value)
            .unwrap_or_else(|| panic!("missing owned URL protocol value cleanup: {value}"));
        assert!(previous_protocol_value_delete < position);
        assert!(position < registry_delete);
        previous_protocol_value_delete = position;
    }
    let mut previous_protocol_delete = shortcut_delete;
    for key in [
        "${URL_PROTOCOL_SUBKEY}\\shell\\open\\command",
        "${URL_PROTOCOL_SUBKEY}\\shell\\open",
        "${URL_PROTOCOL_SUBKEY}\\shell",
        "${URL_PROTOCOL_SUBKEY}",
    ] {
        let needle = format!("!insertmacro UninstallRegKey \"{key}\"");
        let position = uninstall
            .find(&needle)
            .unwrap_or_else(|| panic!("missing transactional URL protocol cleanup: {key}"));
        assert!(previous_protocol_delete < position);
        assert!(position < uninstaller_delete);
        previous_protocol_delete = position;
    }
    assert!(!nsi.contains("__ChimeraUninstallProbe${SLOT}"));
    assert!(!nsi.contains("EnumRegValue $8 HKCU \"${KEY}\" 0"));
    assert!(uninstall.contains("BackupUninstallShortcut"));
    assert!(uninstall.contains("BackupUninstallRegValue"));
    assert!(uninstall.contains("UninstallBackedRegValue"));
    assert!(uninstall.contains("uninstall_metadata_failed:"));
    assert!(uninstall.contains("uninstall_metadata_restored:"));
    assert!(uninstall.contains("RestoreRegValue"));
    assert!(uninstall.contains(
        "RestoreRegValue \"${URL_PROTOCOL_SUBKEY}\\shell\\open\\command\" \"\" \"UrlCommand\""
    ));
    assert!(
        uninstall.contains(
            "RestoreRegValue \"${URL_PROTOCOL_SUBKEY}\" \"URL Protocol\" \"UrlProtocol\""
        )
    );
    assert!(
        uninstall.contains("RestoreRegValue \"${URL_PROTOCOL_SUBKEY}\" \"\" \"UrlDisplayName\"")
    );
    assert!(uninstall.contains("uninstall_failed:"));
}

#[test]
fn english_readme_uses_actual_chinese_manager_app_bundle_name() {
    let readme = std::fs::read_to_string("../../README_EN.md").expect("read English README");

    assert!(readme.contains("Chimera++ 管理工具.app"));
    assert!(!readme.contains("Chimera++ Manager.app"));
    assert!(readme.contains("Codex++ 管理工具.app"));
    assert!(!readme.contains("Codex++ Manager.app"));
}

#[test]
fn customer_readmes_are_key_first_and_do_not_use_github_or_manual_update_paths() {
    let chinese = std::fs::read_to_string("../../README.md").expect("read Chinese README");
    let english = std::fs::read_to_string("../../README_EN.md").expect("read English README");

    assert_eq!(chinese.matches("## 开发与开源归属").count(), 1);
    assert_eq!(
        english
            .matches("## Development and Open-source Attribution")
            .count(),
        1
    );
    let (chinese_customer, chinese_attribution) = chinese
        .split_once("## 开发与开源归属")
        .expect("Chinese development and attribution section");
    let (english_customer, english_attribution) = english
        .split_once("## Development and Open-source Attribution")
        .expect("English development and attribution section");

    for (label, customer) in [
        ("Chinese customer section", chinese_customer),
        ("English customer section", english_customer),
    ] {
        let normalized = customer.to_ascii_lowercase();
        assert!(
            !normalized.contains("github"),
            "GitHub copy remains in {label}"
        );
        assert!(
            !normalized.contains("about"),
            "About copy remains in {label}"
        );
        assert!(!customer.contains("关于"), "About copy remains in {label}");
        assert!(!normalized.contains("chimera codex"));
        assert!(!normalized.contains("chimeracodex"));
        for line in customer.lines().filter(|line| line.contains("Codex++")) {
            assert!(
                legacy_codex_display_has_migration_context(line),
                "Codex++ lacks migration context in {label}: {line}"
            );
        }
        for line in customer.lines() {
            assert!(
                !contains_manual_update_instruction(line),
                "manual update instruction remains in {label}: {line}"
            );
        }
    }

    for forbidden in [
        "检查更新",
        "下载并运行安装包",
        "手动获取安装包",
        "手动下载新安装包",
    ] {
        assert!(
            !chinese_customer.contains(forbidden),
            "manual update copy remains: {forbidden}"
        );
    }
    for forbidden in [
        "Check for updates",
        "check for updates",
        "Download and run installer",
        "download the installer yourself",
        "download replacement installers",
    ] {
        assert!(
            !english_customer.contains(forbidden),
            "manual update copy remains: {forbidden}"
        );
    }

    assert!(chinese_attribution.contains("BigPizzaV3/CodexPlusPlus"));
    assert!(chinese_attribution.contains("farion1231/cc-switch"));
    assert!(chinese_attribution.contains("Duojiyi/chimera-codex"));
    assert!(english_attribution.contains("BigPizzaV3/CodexPlusPlus"));
    assert!(english_attribution.contains("farion1231/cc-switch"));
    assert!(english_attribution.contains("Duojiyi/chimera-codex"));

    for (label, readme) in [("README.md", &chinese), ("README_EN.md", &english)] {
        assert!(
            !readme.contains("license-MIT"),
            "MIT badge remains in {label}"
        );
        assert!(
            !contains_ascii_token(readme, "MIT"),
            "unverified MIT token remains in {label}"
        );
        assert!(readme.contains("https://api.chimerahub.org/v1"));
        assert!(readme.contains("ChimeraPlusPlus-*-windows-x64-setup.exe"));
        assert!(readme.contains("ChimeraPlusPlus-*-macos-x64.dmg"));
        assert!(readme.contains("ChimeraPlusPlus-*-macos-arm64.dmg"));
    }

    assert!(chinese_customer.contains("只需要填写 API Key"));
    assert!(chinese_customer.contains("自动检查并安装更新"));
    assert!(chinese_customer.contains("桌面只创建一个 `Chimera++` 入口"));
    assert!(chinese_customer.contains(
        "当前开发快照尚未完成单桌面入口和自动更新强制策略验收，不得作为客户正式发行版交付"
    ));

    assert!(english_customer.contains("only need to enter your API Key"));
    assert!(english_customer.contains("checks for and installs updates automatically"));
    assert!(english_customer.contains("creates only one `Chimera++` desktop shortcut"));
    assert!(english_customer.contains("This development snapshot has not completed single-desktop-entry or automatic-update enforcement acceptance and must not be delivered as a customer release"));

    assert!(contains_ascii_token(
        "Released under the mit License",
        "MIT"
    ));
    assert!(contains_ascii_token("采用 MIT 许可证", "MIT"));
    assert!(contains_manual_update_instruction("请自行下载最新版安装包"));
    assert!(contains_manual_update_instruction(
        "Manually fetch the latest package"
    ));
    assert!(!contains_manual_update_instruction(
        "Manually delete the legacy app"
    ));
}

#[test]
fn windows_update_installer_reports_completion_and_relaunches_exactly_once() {
    let nsi = std::fs::read_to_string("../../scripts/installer/windows/CodexPlusPlus.nsi")
        .expect("read NSIS script");

    assert!(nsi.contains("!include \"FileFunc.nsh\""));
    assert!(nsi.contains("/CONTINUATION_TOKEN="));
    assert!(nsi.contains("Function .onInstFailed"));
    assert!(nsi.contains("Function .onInstSuccess"));
    assert!(nsi.contains("Function .onGUIEnd"));
    assert!(nsi.contains("Var UpdateRelaunchHandled"));
    assert!(nsi.contains("--update-continuation-token"));
    assert!(nsi.contains("SetErrorLevel 1"));
    assert!(nsi.contains("SetErrorLevel 0"));
    assert_eq!(
        nsi.matches("Exec '\"$INSTDIR\\codex-plus-plus.exe\"")
            .count(),
        3,
        "success, section failure, and pre-section exit need guarded relaunch paths"
    );
    let gui_end = nsi
        .split("Function .onGUIEnd")
        .nth(1)
        .and_then(|source| source.split("FunctionEnd").next())
        .expect("onGUIEnd callback");
    let handled_guard = gui_end
        .find("StrCmp $UpdateRelaunchHandled \"1\"")
        .expect("handled guard");
    let relaunch = gui_end.find("Exec '").expect("fallback relaunch");
    assert!(handled_guard < relaunch);
}

#[test]
fn branding_check_validates_installer_and_packaging_touchpoints() {
    let script = std::fs::read_to_string("../../scripts/generate-branding.ps1")
        .expect("read branding generator");

    assert!(script.contains("Assert-BrandTouchpoints"));
    for path in [
        "scripts\\installer\\windows\\CodexPlusPlus.nsi",
        "scripts\\installer\\macos\\package-dmg.sh",
        "apps\\codex-plus-manager\\src-tauri\\tauri.conf.json",
        ".github\\workflows\\pr-build.yml",
        ".github\\workflows\\release-assets.yml",
    ] {
        assert!(script.contains(path), "branding check missing {path}");
    }
    assert!(script.contains("windows-x64-setup.exe"));
    assert!(script.contains("macos-${{ matrix.arch }}.dmg"));
    assert!(script.contains("Assert-ActiveTextContains"));
    assert!(script.contains("Assert-ActiveTextNotContains"));
    assert!(script.contains("Assert-TextNotContains"));
    assert!(script.matches("'CodexPlusPlus-$version-'").count() >= 2);
    assert!(script.contains("README.md"));
    assert!(script.contains("README_EN.md"));
}

#[test]
fn pr_and_release_workflows_verify_original_brand_icons() {
    for workflow in [
        "../../.github/workflows/pr-build.yml",
        "../../.github/workflows/release-assets.yml",
    ] {
        let source = std::fs::read_to_string(workflow).expect("read workflow");
        assert!(
            source.contains("pwsh -File scripts/verify-brand-icons.ps1"),
            "workflow missing original icon gate: {workflow}"
        );
    }
}

#[test]
fn brand_icon_gate_self_test_is_fail_closed_and_runs_in_ci() {
    let output = std::process::Command::new("pwsh")
        .args([
            "-NoProfile",
            "-File",
            "../../scripts/verify-brand-icons.ps1",
            "-SelfTest",
        ])
        .output()
        .expect("run brand icon gate self-test");
    assert!(
        output.status.success(),
        "brand icon self-test failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("self-test: PASS"));

    for workflow in [
        "../../.github/workflows/pr-build.yml",
        "../../.github/workflows/release-assets.yml",
    ] {
        let source = std::fs::read_to_string(workflow).expect("read workflow");
        let npm_ci = source
            .find("run: npm ci")
            .expect("workflow must run npm ci");
        let self_test = source
            .find("run: pwsh -File scripts/verify-brand-icons.ps1 -SelfTest")
            .expect("workflow must run icon gate self-test");
        let icon_gate = source
            .rfind("run: pwsh -File scripts/verify-brand-icons.ps1")
            .expect("workflow must run icon gate");
        assert!(
            npm_ci < self_test && self_test < icon_gate,
            "workflow must install the locked Tauri CLI before both icon gates: {workflow}"
        );
    }
}

#[test]
fn windows_default_install_root_uses_known_folder_before_userprofile_desktop() {
    let strategy = default_install_root_strategy();

    if cfg!(windows) {
        assert_eq!(strategy, "windows-known-folder");
    } else if cfg!(target_os = "macos") {
        assert_eq!(strategy, "macos-applications");
    } else {
        assert_eq!(strategy, "user-dirs-desktop");
    }
}
