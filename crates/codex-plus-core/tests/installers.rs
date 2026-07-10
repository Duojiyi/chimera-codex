use codex_plus_core::branding::{
    DISPLAY_MANAGER_NAME, DISPLAY_SILENT_NAME, MACOS_BUILD_NUMBER, PUBLISHER,
};
use codex_plus_core::install::{
    InstallOptions, LEGACY_MANAGER_NAME, LEGACY_SILENT_NAME, MANAGER_BINARY, SILENT_BINARY,
    SILENT_NAME, MANAGER_NAME, app_bundle_names, build_macos_app_bundle,
    build_windows_entrypoint_plan, companion_binary_path_from_exe, default_install_root_strategy,
    detect_legacy_macos_apps, legacy_app_bundle_names, legacy_shortcut_names, shortcut_names,
    windows_legacy_shortcut_paths,
};

#[test]
fn display_constants_come_from_branding_while_binaries_stay_stable() {
    assert_eq!(SILENT_NAME, DISPLAY_SILENT_NAME);
    assert_eq!(MANAGER_NAME, DISPLAY_MANAGER_NAME);
    assert_eq!(SILENT_NAME, "Chimera Codex");
    assert_eq!(MANAGER_NAME, "Chimera Codex 管理工具");
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

    assert!(plan.silent_shortcut.ends_with("Chimera Codex.lnk"));
    assert!(plan.manager_shortcut.ends_with("Chimera Codex 管理工具.lnk"));
    assert!(!plan.silent_shortcut.contains("Codex++.lnk"));
    assert_eq!(plan.launcher_path, "C:/Tools/codex-plus-plus.exe");
    assert_eq!(plan.manager_path, "C:/Tools/codex-plus-plus-manager.exe");
    assert_eq!(plan.silent_icon_path, "C:/Tools/codex-plus-plus.exe");
    assert_eq!(
        plan.manager_icon_path,
        "C:/Tools/codex-plus-plus-manager.exe"
    );
    assert_eq!(plan.display_name, "Chimera Codex");
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

    assert!(plan.silent_shortcut.ends_with("Chimera Codex.lnk"));
    assert!(plan.manager_shortcut.ends_with("Chimera Codex 管理工具.lnk"));
    assert!(plan.remove_owned_data);
}

#[test]
fn macos_bundle_metadata_uses_chimera_names_and_numeric_versions() {
    let options = InstallOptions {
        install_root: Some("/Applications".into()),
        launcher_path: Some("/opt/ChimeraCodex/codex-plus-plus".into()),
        manager_path: Some("/opt/ChimeraCodex/codex-plus-plus-manager".into()),
        remove_owned_data: false,
    };

    let silent = build_macos_app_bundle(&options, false);
    let manager = build_macos_app_bundle(&options, true);

    assert!(silent.app_path.ends_with("Chimera Codex.app"));
    assert!(manager.app_path.ends_with("Chimera Codex 管理工具.app"));
    assert!(silent.info_plist.contains("<string>Chimera Codex</string>"));
    assert!(
        manager
            .info_plist
            .contains("<string>Chimera Codex 管理工具</string>")
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
        silent
            .info_plist
            .contains(&format!("<key>CFBundleShortVersionString</key>\n  <string>{short_version}</string>"))
            || silent.info_plist.contains(&format!(
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
        ("Chimera Codex.lnk", "Chimera Codex 管理工具.lnk")
    );
    assert_eq!(
        app_bundle_names(),
        ("Chimera Codex.app", "Chimera Codex 管理工具.app")
    );
    assert_eq!(legacy_shortcut_names(), ("Codex++.lnk", "Codex++ 管理工具.lnk"));
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
    assert!(script.contains("ChimeraCodex-${VERSION}-macos-${ARCH}.dmg"));
    assert!(script.contains("\"Chimera Codex\""));
    assert!(script.contains("\"Chimera Codex 管理工具\""));
    assert!(script.contains("CFBundleShortVersionString"));
    assert!(script.contains("SHORT_VERSION"));
    assert!(script.contains("MACOS_BUILD_NUMBER"));
    assert!(script.contains("codesign --force --sign -"));
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
    assert!(detected.paths.iter().any(|path| path.ends_with("Codex++.app")));
    assert!(
        detected
            .paths
            .iter()
            .any(|path| path.ends_with("Codex++ 管理工具.app"))
    );
    assert!(!detected.paths.iter().any(|path| path.ends_with("Chimera Codex.app")));
    assert!(!detected.message.is_empty());
    assert!(legacy_silent.exists());
    assert!(legacy_manager.exists());
}

#[test]
fn companion_binary_path_resolves_macos_silent_app_next_to_manager_app() {
    let manager_exe = std::path::Path::new(
        "/Applications/Chimera Codex 管理工具.app/Contents/MacOS/CodexPlusPlusManager",
    );

    let companion = companion_binary_path_from_exe(manager_exe, SILENT_BINARY);

    assert_eq!(
        companion,
        std::path::PathBuf::from("/Applications/Chimera Codex.app/Contents/MacOS/CodexPlusPlus")
    );
    assert_ne!(
        companion,
        std::path::PathBuf::from(
            "/Applications/Chimera Codex 管理工具.app/Contents/MacOS/codex-plus-plus"
        )
    );
}

#[test]
fn companion_binary_path_resolves_macos_manager_app_next_to_silent_app() {
    let silent_exe =
        std::path::Path::new("/Applications/Chimera Codex.app/Contents/MacOS/CodexPlusPlus");

    let companion =
        companion_binary_path_from_exe(silent_exe, codex_plus_core::install::MANAGER_BINARY);

    assert_eq!(
        companion,
        std::path::PathBuf::from(
            "/Applications/Chimera Codex 管理工具.app/Contents/MacOS/CodexPlusPlusManager"
        )
    );
}

#[test]
fn macos_bundle_does_not_wrap_the_bundle_executable_in_itself() {
    let options = InstallOptions {
        install_root: Some("/Applications".into()),
        launcher_path: Some(
            "/Applications/Chimera Codex.app/Contents/MacOS/CodexPlusPlus".into(),
        ),
        manager_path: Some(
            "/Applications/Chimera Codex 管理工具.app/Contents/MacOS/CodexPlusPlusManager".into(),
        ),
        remove_owned_data: false,
    };

    let silent = build_macos_app_bundle(&options, false);
    let manager = build_macos_app_bundle(&options, true);

    assert_eq!(
        silent.binary_source,
        Some(std::path::PathBuf::from(
            "/Applications/Chimera Codex.app/Contents/MacOS/CodexPlusPlus"
        ))
    );
    assert_eq!(
        manager.binary_source,
        Some(std::path::PathBuf::from(
            "/Applications/Chimera Codex 管理工具.app/Contents/MacOS/CodexPlusPlusManager"
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

    assert!(nsi.contains("Name \"Chimera Codex\""));
    assert!(nsi.contains("ChimeraCodex-${VERSION}-windows-x64-setup.exe"));
    assert!(nsi.contains("InstallDir \"$LOCALAPPDATA\\Programs\\Codex++\""));
    assert!(nsi.contains("Publisher\" \"ChimeraHub\"") || nsi.contains("\"Publisher\" \"ChimeraHub\"") || nsi.contains("Publisher\" \"ChimeraHub"));
    assert!(nsi.contains("DisplayName\" \"Chimera Codex\"") || nsi.contains("\"DisplayName\" \"Chimera Codex\""));
    assert!(nsi.contains("Delete \"$DESKTOP\\Codex++.lnk\""));
    assert!(nsi.contains("Delete \"$DESKTOP\\Codex++ 管理工具.lnk\""));
    assert!(nsi.contains("Delete \"$DESKTOP\\Chimera Codex.lnk\""));
    assert!(nsi.contains("CreateShortcut \"$DESKTOP\\Chimera Codex.lnk\""));
    assert!(nsi.contains("CreateShortcut \"$DESKTOP\\Chimera Codex 管理工具.lnk\""));
    assert!(nsi.contains(".exe.new"));
    assert!(!nsi.contains("Publisher\" \"BigPizzaV3\""));
    assert!(!nsi.contains("CodexPlusPlus-${VERSION}-windows-x64-setup.exe"));
    // 保留乱码清理字面量，供历史坏快捷方式卸载
    assert!(nsi.contains("绠＄悊宸ュ叿"));
}

#[test]
fn release_assets_workflow_verifies_chimera_bundle_paths() {
    let workflow = std::fs::read_to_string("../../.github/workflows/release-assets.yml")
        .expect("read release-assets workflow");

    assert!(workflow.contains("Chimera Codex.app"));
    assert!(workflow.contains("Chimera Codex 管理工具.app"));
    assert!(!workflow.contains("dist/macos/stage/Codex++.app"));
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
