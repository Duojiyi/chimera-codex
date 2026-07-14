#[cfg(windows)]
#[test]
fn manager_binary_uses_windows_gui_subsystem_in_debug_and_release() {
    let main_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
        .expect("read manager main.rs");

    assert!(
        main_rs.contains("#![cfg_attr(windows, windows_subsystem = \"windows\")]"),
        "manager binary should not allocate a console window on Windows"
    );
}

#[test]
fn manager_release_binary_uses_embedded_frontend_assets() {
    let cargo_toml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("read manager Cargo.toml");

    assert!(
        cargo_toml.contains("custom-protocol"),
        "release manager binary should use Tauri custom protocol instead of devUrl localhost"
    );
}

#[test]
fn manager_uses_single_instance_guard_before_starting_tauri() {
    let lib_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read manager lib.rs");

    assert!(lib_rs.contains("acquire_single_instance_guard()"));
    assert!(lib_rs.contains("manager_guard_port"));
    assert!(lib_rs.contains("manager.already_running"));
    assert!(!lib_rs.contains("TcpListener::bind((\"127.0.0.1\", 0))"));
    assert!(lib_rs.contains("VecDeque<String>"));
    assert!(lib_rs.contains("next_pending_or_mark_ready"));
    assert!(lib_rs.contains("manager_frontend_not_ready"));

    let app_tsx = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/App.tsx"))
        .expect("read manager App.tsx");
    assert!(app_tsx.contains("invoke(\"manager_frontend_not_ready\")"));
    assert!(app_tsx.contains(".finally(() => unlisten?.())"));
}

#[test]
fn manager_ui_removes_about_github_and_manual_update_controls() {
    let app_tsx = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/App.tsx"))
        .expect("read manager App.tsx");

    for forbidden in [
        "| \"about\"",
        "{ id: \"about\"",
        "route === \"about\"",
        "setRoute(\"about\")",
        "function AboutScreen",
        "https://github.com/${REPOSITORY}",
        "GitHub",
        "actions.checkUpdate()",
        "actions.performUpdate()",
        "actions.openExternalUrl(script.homepage)",
        "className=\"update-dot\"",
    ] {
        assert!(
            !app_tsx.contains(forbidden),
            "customer UI still exposes forbidden About/update surface: {forbidden}"
        );
    }

    let maintenance = app_tsx
        .split("function MaintenanceScreen")
        .nth(1)
        .and_then(|value| value.split("function SettingsScreen").next())
        .expect("maintenance screen implementation");
    assert!(maintenance.contains("<LogsPanel logs={logs} actions={actions} />"));
    assert!(
        maintenance.contains("<DiagnosticsPanel diagnostics={diagnostics} actions={actions} />")
    );

    let check_update = app_tsx
        .split("const checkUpdate")
        .nth(1)
        .and_then(|value| value.split("const performUpdate").next())
        .expect("checkUpdate implementation");
    assert!(!check_update.contains("result.message"));
    assert!(check_update.contains("版本更新服务暂时不可用，请稍后重试。"));

    let initial_route = app_tsx
        .split("function loadInitialRoute")
        .nth(1)
        .expect("loadInitialRoute implementation");
    assert!(initial_route.contains("params.get(\"showUpdate\") === \"1\""));
    assert!(initial_route.contains("window.location.hash === \"#about\""));
    assert!(initial_route.contains("return \"maintenance\""));
    assert!(!initial_route.contains("return \"about\""));
}

#[test]
fn manager_backend_has_no_recommendation_command_surface() {
    let commands_rs =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands.rs"))
            .expect("read manager commands.rs");
    let lib_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read manager lib.rs");

    for forbidden in [
        "AdsPayload",
        "load_ads",
        "ads_payload",
        "推荐内容",
        "\"homepage\": script.homepage",
    ] {
        assert!(
            !commands_rs.contains(forbidden),
            "manager backend still exposes recommendation command surface: {forbidden}"
        );
    }
    assert!(!lib_rs.contains("commands::load_ads"));

    let app_tsx = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/App.tsx"))
        .expect("read manager App.tsx");
    assert!(!app_tsx.contains("homepage: string"));
    assert!(!app_tsx.contains("homepage?: string"));
}

#[test]
fn manager_repeated_launch_activates_existing_window() {
    let lib_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read manager lib.rs");

    assert!(lib_rs.contains("focus_existing_manager_window();"));
    assert!(lib_rs.contains("windows_activate_process_window"));
}

#[test]
fn manager_main_window_uses_default_window_icon_explicitly() {
    let lib_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read manager lib.rs");

    assert!(lib_rs.contains("main_window_builder"));
    assert!(lib_rs.contains("app.default_window_icon().cloned()"));
    assert!(lib_rs.contains("main_window_builder = main_window_builder.icon(icon)?"));
}

#[test]
fn manager_close_minimizes_to_tray_without_confirmation() {
    let lib_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
        .expect("read manager lib.rs");
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let app_tsx = manifest_dir.parent().unwrap().join("src/App.tsx");
    let app_tsx = std::fs::read_to_string(&app_tsx).expect("read manager App.tsx");

    assert!(!lib_rs.contains("MessageDialogButtons"));
    assert!(!lib_rs.contains(".dialog()"));
    assert!(!lib_rs.contains("manager://close-requested"));
    assert!(lib_rs.contains("let _ = close_event_window.hide();"));
    assert!(!app_tsx.contains("CloseConfirmDialog"));
    assert!(app_tsx.contains("manager_exit_app"));
    assert!(app_tsx.contains("manager_hide_to_tray"));
}

#[test]
fn manager_queues_codexplusplus_provider_urls_for_confirmation_on_startup() {
    let main_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
        .expect("read manager main.rs");
    let commands_rs =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands.rs"))
            .expect("read manager commands.rs");
    let app_tsx = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/App.tsx"))
        .expect("read manager App.tsx");

    assert!(main_rs.contains("codexplusplus://"));
    assert!(main_rs.contains("provider_import::save_pending_provider_import_from_url"));
    assert!(!main_rs.contains("provider_import::import_provider_from_url"));
    assert!(main_rs.contains("manager.provider_import_url.pending"));
    assert!(!main_rs.contains("request.base_url"));
    assert!(!main_rs.contains("error.to_string()"));
    assert!(commands_rs.contains("PendingProviderImportActionRequest"));
    assert!(commands_rs.contains("request.request_id"));
    assert!(app_tsx.contains("requestId: string"));
    assert!(app_tsx.contains("request: { requestId }"));
    assert!(app_tsx.contains("confirmPendingProviderImport(pendingProviderImport.requestId)"));
    assert!(app_tsx.contains("dismissPendingProviderImport(pendingProviderImport.requestId)"));
}

#[test]
fn launcher_binary_embeds_codex_icon_resource() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let launcher_build = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("codex-plus-launcher/build.rs");
    let build_rs = std::fs::read_to_string(&launcher_build).expect("read launcher build.rs");

    assert!(build_rs.contains("WindowsResource"));
    assert!(build_rs.contains("icons/icon.ico"));
}

#[test]
fn windows_apps_and_per_user_installer_run_as_invoker() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manager_build =
        std::fs::read_to_string(manifest_dir.join("build.rs")).expect("read manager build.rs");
    let windows_manifest = std::fs::read_to_string(manifest_dir.join("windows-app-manifest.xml"))
        .expect("read windows app manifest");
    let launcher_build = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("codex-plus-launcher/build.rs");
    let launcher_build = std::fs::read_to_string(&launcher_build).expect("read launcher build.rs");
    let windows_installer = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("scripts/installer/windows/CodexPlusPlus.nsi");
    let windows_installer =
        std::fs::read_to_string(&windows_installer).expect("read windows installer");

    assert!(manager_build.contains("windows-app-manifest.xml"));
    assert!(launcher_build.contains("windows-app-manifest.xml"));
    assert!(windows_manifest.contains("asInvoker"));
    assert!(!windows_manifest.contains("requireAdministrator"));
    assert!(windows_manifest.contains("Microsoft.Windows.Common-Controls"));
    assert!(windows_installer.contains("RequestExecutionLevel user"));
    assert!(!windows_installer.contains("RequestExecutionLevel admin"));
}

#[test]
fn windows_entrypoints_register_codexplusplus_url_protocol() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let windows_install = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("crates/codex-plus-core/src/install/windows.rs");
    let windows_install =
        std::fs::read_to_string(&windows_install).expect("read windows install source");

    assert!(windows_install.contains("Software\\Classes\\codexplusplus"));
    assert!(windows_install.contains("URL Protocol"));
    assert!(windows_install.contains("%1"));
}

#[test]
fn manager_launch_button_spawns_silent_launcher_binary() {
    let commands_rs =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/commands.rs"))
            .expect("read manager commands.rs");

    assert!(commands_rs.contains("SILENT_BINARY"));
    assert!(commands_rs.contains("std::process::Command::new"));
    assert!(!commands_rs.contains("launch_and_inject_with_hooks(options"));
}

#[test]
fn macos_packager_hides_silent_launcher_but_not_manager() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packager = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("scripts/installer/macos/package-dmg.sh");
    let script = std::fs::read_to_string(&packager).expect("read macOS packager");

    assert!(script.contains("<key>LSUIElement</key>"));
    assert!(script.contains("x64|x86_64) ARCH=\"x64\""));
    assert!(script.contains("arm64|aarch64) ARCH=\"arm64\""));
    assert!(script.contains("lipo -archs"));
    assert!(script.contains("BINARY_DIR=\"${BINARY_DIR:-$ROOT/target/release}\""));
    assert!(script.contains("ChimeraPlusPlus-${VERSION}-macos-${ARCH}.dmg"));
    assert!(script.contains(
        "create_app \"$SILENT_APP_NAME\" \"CodexPlusPlus\" \"$BINARY_DIR/codex-plus-plus\" \"com.bigpizzav3.codexplusplus\" \"true\""
    ));
    assert!(script.contains(
        "create_app \"$MANAGER_APP_NAME\" \"CodexPlusPlusManager\" \"$BINARY_DIR/codex-plus-plus-manager\" \"com.bigpizzav3.codexplusplus.manager\" \"false\""
    ));
}

#[test]
fn github_release_workflow_builds_separate_macos_x64_and_arm64_dmgs() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(".github/workflows/release-assets.yml");
    let workflow = std::fs::read_to_string(&workflow).expect("read release assets workflow");

    assert!(workflow.contains("macos-15-intel"));
    assert!(workflow.contains("x86_64-apple-darwin"));
    assert!(workflow.contains("macos-14"));
    assert!(workflow.contains("aarch64-apple-darwin"));
    assert!(workflow.contains("package-dmg.sh \"$VERSION\" \"${{ matrix.arch }}\""));
    assert!(workflow.contains("target/${{ matrix.target }}/release"));
}

#[test]
fn github_release_workflow_uploads_static_latest_json() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap();
    let workflow = root.join(".github/workflows/release-assets.yml");
    let workflow = std::fs::read_to_string(&workflow).expect("read release assets workflow");
    let generator = std::fs::read_to_string(root.join("scripts/release-manifest.mjs"))
        .expect("read release manifest generator");

    assert!(workflow.contains("latest.json"));
    assert!(workflow.contains("sha256"));
    assert!(workflow.contains("gh release upload \"$TAG\" \"${upload_list[@]}\" --repo \"$REPO\""));
    assert!(
        workflow
            .contains("gh release edit \"$TAG\" --repo \"$REPO\" --draft=false --prerelease=false")
    );
    assert!(workflow.contains("generate-branding.ps1 -Check"));
    assert!(workflow.contains("verify-no-upstream-ads.ps1"));
    assert!(workflow.contains("verify-license.ps1"));
    assert!(workflow.contains("verify-license.ps1 -SelfTest"));
    assert!(workflow.contains("cargo fmt --all -- --check"));
    assert!(workflow.contains("cargo test --workspace"));
    assert!(workflow.contains("npm run check"));
    assert!(!workflow.contains("uses: actions/checkout@v4"));
    assert!(!workflow.contains("uses: actions/setup-node@v4"));
    assert!(!workflow.contains("uses: actions/upload-artifact@v4"));
    assert!(!workflow.contains("uses: actions/download-artifact@v4"));
    assert!(!workflow.contains("uses: dtolnay/rust-toolchain@stable"));
    assert!(workflow.contains("permissions:\n  contents: read"));
    assert!(workflow.contains("publish-release:\n    name: Draft"));
    assert!(workflow.contains("      contents: write"));
    assert!(workflow.contains("gh release view \"$TAG\""));
    assert!(workflow.contains("--json isDraft"));
    assert!(
        workflow.contains(
            "gh release upload \"$TAG\" \"${upload_list[@]}\" --repo \"$REPO\" --clobber"
        )
    );
    assert!(workflow.contains("ChimeraPlusPlus-${VERSION}-source.tar.gz"));
    assert!(generator.contains("name !== `ChimeraPlusPlus-${resolved.version}-source.tar.gz`"));
}

#[test]
fn github_release_archives_corresponding_source_from_resolved_target() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap();
    let workflow = root.join(".github/workflows/release-assets.yml");
    let workflow = std::fs::read_to_string(&workflow).expect("read release assets workflow");
    let generator = std::fs::read_to_string(root.join("scripts/release-manifest.mjs"))
        .expect("read release manifest generator");

    assert!(workflow.contains(
        "git archive --format=tar --prefix=\"ChimeraPlusPlus-${VERSION}-source/\" \"$TARGET_SHA\""
    ));
    assert!(workflow.contains("gzip -n"));
    assert!(workflow.contains(
        "git ls-tree -rz --name-only \"$TARGET_SHA\" | LC_ALL=C sort -z > /tmp/source-tree-expected.z"
    ));
    assert!(workflow.contains("tar -xzf \"$source_asset\" -C /tmp/source-tree-root"));
    assert!(workflow.contains("find . -mindepth 1 ! -type d -print0"));
    assert!(workflow.contains("| LC_ALL=C sort -z > /tmp/source-tree-actual.z"));
    assert!(workflow.contains("cmp /tmp/source-tree-expected.z /tmp/source-tree-actual.z"));
    assert!(generator.contains("License: AGPL-3.0-only; third-party notices: NOTICE"));
    assert!(generator.contains(
        "Corresponding source: ${baseUrl}/ChimeraPlusPlus-${resolved.version}-source.tar.gz"
    ));
    for required in [
        "Cargo.lock",
        "package-lock.json",
        "scripts/installer/windows/CodexPlusPlus.nsi",
        "scripts/installer/macos/package-dmg.sh",
        "LICENSE",
        "NOTICE",
    ] {
        assert!(
            workflow.contains(required),
            "missing source check: {required}"
        );
    }
}

#[test]
fn release_packages_embed_exact_corresponding_source_notice() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap();
    let workflow = std::fs::read_to_string(root.join(".github/workflows/release-assets.yml"))
        .expect("read release assets workflow");
    let nsi = std::fs::read_to_string(root.join("scripts/installer/windows/CodexPlusPlus.nsi"))
        .expect("read NSIS script");
    let macos = std::fs::read_to_string(root.join("scripts/installer/macos/package-dmg.sh"))
        .expect("read macOS packager");

    assert!(workflow.matches("SOURCE_CODE.txt").count() >= 5);
    assert!(workflow.contains(
        "https://github.com/Duojiyi/chimera-codex/releases/download/${TAG}/${source_asset}"
    ));
    assert!(workflow.contains("Release commit: ${TARGET_SHA}"));
    assert!(workflow.contains("Copy-Item LICENSE,NOTICE,SOURCE_CODE.txt dist/windows/app/"));
    assert!(
        workflow
            .contains("cp LICENSE NOTICE SOURCE_CODE.txt \"dist/macos/app-${{ matrix.arch }}/\"")
    );
    assert!(workflow.contains("test -f \"dist/macos/stage/SOURCE_CODE.txt\""));
    assert!(nsi.contains("File \"/oname=SOURCE_CODE.txt.new\" \"${ROOT}\\SOURCE_CODE.txt\""));
    assert!(nsi.contains("Delete \"$INSTDIR\\SOURCE_CODE.txt\""));
    assert!(macos.contains("cp \"$ROOT/SOURCE_CODE.txt\" \"$STAGE/SOURCE_CODE.txt\""));
}

#[test]
fn license_self_test_breaks_each_source_archive_integrity_gate() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let verifier = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("scripts/verify-license.ps1");
    let verifier = std::fs::read_to_string(verifier).expect("read license verifier");

    for case_name in [
        "source tree expected NUL sorting integrity",
        "source archive extraction integrity",
        "source tree NUL traversal integrity",
        "source tree NUL normalization integrity",
        "source tree NUL sorting integrity",
        "source tree comparison integrity",
        "source required-file integrity",
        "commented draft asset content gate",
    ] {
        assert!(
            verifier.contains(case_name),
            "missing fail-closed case: {case_name}"
        );
    }
}

#[test]
fn github_release_retries_bind_gates_builds_and_publish_to_original_commit() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(".github/workflows/release-assets.yml");
    let workflow = std::fs::read_to_string(&workflow).expect("read release assets workflow");

    assert!(workflow.contains("target_sha: ${{ steps.resolve.outputs.target_sha }}"));
    assert!(
        workflow
            .matches("ref: ${{ needs.resolve-version.outputs.target_sha }}")
            .count()
            >= 4
    );
    assert!(workflow.contains("needs: [resolve-version, gates]"));
    assert!(workflow.contains("--json isDraft,targetCommitish"));
    assert!(workflow.contains("git rev-parse \"refs/tags/${tag}^{commit}\""));
    assert!(workflow.contains("echo \"target_sha=${target_sha}\" >> \"$GITHUB_OUTPUT\""));
    assert!(workflow.contains("TARGET_SHA: ${{ needs.resolve-version.outputs.target_sha }}"));
    assert!(workflow.contains("checkout_sha=\"$(git rev-parse HEAD)\""));
    assert!(workflow.contains("release target changed before publish"));
    assert!(workflow.contains("GITHUB_REF: ${{ github.ref }}"));
    assert!(workflow.contains("release workflow must run from refs/heads/main"));
    assert!(workflow.contains("git merge-base --is-ancestor \"$target_sha\" origin/main"));
    assert!(workflow.matches("verify_draft_target").count() >= 3);
    let publish = workflow
        .rfind("gh release edit \"$TAG\" --repo \"$REPO\" --draft=false --prerelease=false")
        .expect("publish release");
    let verify_published = workflow
        .rfind("verify_published_target")
        .expect("verify published target after publish");
    assert!(publish < verify_published);
    assert!(workflow.contains("ensure_remote_tag_target"));
    assert!(workflow.contains("verify-published-release:"));
    assert!(workflow.contains("needs.resolve-version.outputs.release_state == 'published'"));
    assert!(workflow.contains("expected_assets"));
    assert!(workflow.matches("validate_latest_manifest").count() >= 4);
    assert!(workflow.contains("(.assets | length) == 6"));
    assert!(workflow.contains("$matches[0].url == $url"));
    assert!(workflow.contains("test(\"^[0-9a-f]{64}$\")"));
    assert!(workflow.contains("$matches[0].size <= 2147483648"));
    assert!(workflow.contains("$release_matches[0].digest == $digest"));
    assert!(workflow.contains("$release_matches[0].size == $manifest_size"));
    assert!(workflow.contains("reconcile_draft_assets"));
    assert!(workflow.contains("verify_draft_assets_exact"));
    assert!(workflow.contains("unexpected draft release asset"));
}

#[test]
fn release_verifies_remote_asset_content_before_publish_and_on_idempotent_exit() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(".github/workflows/release-assets.yml");
    let workflow = std::fs::read_to_string(workflow).expect("read release assets workflow");

    assert!(workflow.contains("verify_draft_assets_content"));
    assert!(workflow.contains("sha256sum \"$local_path\""));
    assert!(workflow.contains("stat -c '%s' \"$local_path\""));
    assert!(workflow.contains("\"$remote_digest\" != \"$local_digest\""));
    assert!(workflow.contains("\"$remote_size\" != \"$local_size\""));
    let content_check = workflow
        .rfind("verify_draft_assets_content")
        .expect("remote content verification call");
    let publish = workflow
        .rfind("gh release edit \"$TAG\" --repo \"$REPO\" --draft=false --prerelease=false")
        .expect("publish release");
    assert!(content_check < publish);
    let upload = workflow
        .rfind("gh release upload \"$TAG\"")
        .expect("asset upload");
    let publish_window = &workflow[upload..publish];
    assert!(
        publish_window
            .lines()
            .any(|line| line.trim() == "verify_draft_assets_content"),
        "draft content gate must be an active command before publish"
    );

    assert!(workflow.contains("published Release asset set is not exact"));
    assert!(workflow.contains("published Release target does not match its tag"));
    assert!(workflow.contains("Verify existing corresponding source"));
    assert!(workflow.contains("cmp /tmp/expected-source.tar.gz /tmp/published-source.tar.gz"));
    assert!(workflow.contains("ref: ${{ needs.resolve-version.outputs.target_sha }}"));
}

#[test]
fn pr_build_enforces_format_and_pins_actions() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(".github/workflows/pr-build.yml");
    let workflow = std::fs::read_to_string(&workflow).expect("read PR workflow");

    assert!(workflow.contains("cargo fmt --all -- --check"));
    assert!(workflow.contains("verify-license.ps1"));
    assert!(workflow.contains("verify-license.ps1 -SelfTest"));
    assert!(workflow.contains("Copy-Item LICENSE,NOTICE,SOURCE_CODE.txt dist/windows/app/"));
    assert!(
        workflow
            .contains("cp LICENSE NOTICE SOURCE_CODE.txt \"dist/macos/app-${{ matrix.arch }}/\"")
    );
    assert!(!workflow.contains("uses: actions/checkout@v4"));
    assert!(!workflow.contains("uses: actions/setup-node@v4"));
    assert!(!workflow.contains("uses: actions/upload-artifact@v4"));
    assert!(!workflow.contains("uses: dtolnay/rust-toolchain@stable"));
}

#[test]
fn pr_build_runs_core_unit_tests_on_each_macos_target() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(".github/workflows/pr-build.yml");
    let workflow = std::fs::read_to_string(&workflow).expect("read PR workflow");
    let macos_job = workflow
        .split("  macos-dmg:")
        .nth(1)
        .expect("macOS matrix job");

    assert!(macos_job.contains("cargo test -p codex-plus-core --lib --locked"));
}

#[test]
fn all_repository_workflows_pin_actions_to_commit_shas() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflows_dir = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(".github/workflows");

    for entry in std::fs::read_dir(workflows_dir).expect("read workflows directory") {
        let entry = entry.expect("read workflow entry");
        let path = entry.path();
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("yml" | "yaml")
        ) {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy();
        let workflow = std::fs::read_to_string(&path).expect("read workflow");
        for line in workflow
            .lines()
            .filter(|line| line.trim_start().starts_with("uses:"))
        {
            let reference = line.split_once("uses:").unwrap().1.trim();
            if reference.starts_with("./") {
                continue;
            }
            let revision = reference
                .rsplit_once('@')
                .unwrap_or_else(|| panic!("{name} action has no revision: {reference}"))
                .1;
            assert!(
                revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{name} action is not pinned to a full commit SHA: {reference}"
            );
        }
    }
}

#[test]
fn upstream_sync_does_not_expose_automation_token_to_merged_code_gates() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join(".github/workflows/sync-upstream.yml");
    let workflow = std::fs::read_to_string(workflow).expect("read sync workflow");

    assert!(!workflow.contains("token: ${{ secrets.CHIMERA_AUTOMATION_TOKEN }}"));
    let before_push = workflow
        .split("- name: Push sync branch and open PR")
        .next()
        .unwrap();
    assert!(!before_push.contains("GH_TOKEN: ${{ secrets.CHIMERA_AUTOMATION_TOKEN }}"));
    assert!(before_push.contains("GH_TOKEN: ${{ github.token }}"));
    assert!(workflow.contains("environment: upstream-sync"));
    assert!(workflow.contains("needs.prepare.outputs.action == 'resume'"));
    assert!(workflow.contains("gated_sha: ${{ steps.sync.outputs.gated_sha }}"));
    assert!(workflow.contains("git bundle verify"));
    assert!(workflow.contains("unsafe gated SHA"));
    assert!(workflow.contains("if: github.ref == 'refs/heads/main'"));
    assert!(workflow.contains("persist-credentials: false"));
    assert!(workflow.contains("$action -eq 'prepared'"));
    assert!(workflow.contains("$action -eq 'resume'"));
    assert!(workflow.contains("remote sync branch changed after gates"));
    assert!(
        workflow
            .matches("$PSNativeCommandUseErrorActionPreference = $true")
            .count()
            >= 2
    );
    assert!(!workflow.contains("Contents / Pull requests / Issues / Actions scope"));
}

#[test]
fn upstream_sync_gates_the_exact_committed_sha_and_can_resume_remote_branches() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let script_path = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("scripts/sync-upstream.ps1");
    let script = std::fs::read_to_string(script_path).expect("read sync script");
    let apply = script
        .split("# --- apply ---")
        .nth(1)
        .expect("apply section");

    let commit = apply
        .find("$commit = Invoke-Git")
        .expect("commit sync branch");
    let gates = apply
        .rfind("Invoke-Gates -Root $worktreePath")
        .expect("gate committed branch");
    let clean = apply
        .rfind("'status', '--porcelain'")
        .expect("check worktree after gates");
    let gated_sha = apply
        .rfind("$script:Result.gated_sha = $gatedSha")
        .expect("record gated SHA");

    assert!(commit < gates, "sync commit must exist before gates run");
    assert!(
        gates < clean,
        "gate output must be checked for tree mutations"
    );
    assert!(
        clean < gated_sha,
        "only a clean committed tree may be exported"
    );
    assert!(script.contains("if ($idemp.Resume)"));
    assert!(script.contains("-Action 'resume'"));
    assert!(script.contains("Test-RemoteSyncBranch"));
    assert!(script.contains("$candidateSha = $candidateHead.Text.Trim()"));
    assert!(script.contains("$afterGateSha -ne $candidateSha"));
    assert!(script.contains("gates changed sync candidate HEAD"));
    assert!(script.contains("gates changed resumed branch HEAD"));
    assert!(script.contains("remote sync branch is not based on trusted origin/main"));
    assert!(script.contains("remote sync branch does not contain formal upstream tag"));
}

#[test]
fn upstream_sync_refreshes_and_validates_lockfiles_before_commit() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let script_path = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("scripts/sync-upstream.ps1");
    let script = std::fs::read_to_string(script_path).expect("read sync script");
    let apply = script
        .split("# --- apply ---")
        .nth(1)
        .expect("apply section");

    let set_version = apply
        .find("Set-WorkspaceChimeraVersion -Root $worktreePath -Version $chimeraVersion")
        .expect("set candidate version");
    let refresh = apply
        .find("Update-AndValidateDependencyLocks -Root $worktreePath -Version $chimeraVersion")
        .expect("refresh dependency locks");
    let stage = apply.find("'add', '-A'").expect("stage sync candidate");

    assert!(
        set_version < refresh && refresh < stage,
        "lockfiles must be refreshed after the version bump and before staging"
    );
    assert!(script.contains("& cargo update --workspace"));
    assert!(!script.contains("& cargo update --workspace --offline"));
    assert!(script.contains("& cargo metadata --locked --format-version 1 --no-deps"));
    assert!(script.contains("& npm ci --ignore-scripts --no-audit --no-fund"));
    assert!(!script.contains("npm install --package-lock-only"));
    assert!(script.contains("Set-PackageLockVersion"));
    assert!(script.contains("$packageLockHashBeforeValidation"));
    assert!(script.contains("package-lock validation changed"));
    assert!(script.contains("& cargo test --workspace --locked"));
}

#[test]
fn release_gates_use_locked_cargo_and_full_tag_history_for_branding_checks() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap();

    for workflow_name in ["pr-build.yml", "release-assets.yml", "sync-upstream.yml"] {
        let workflow = std::fs::read_to_string(root.join(".github/workflows").join(workflow_name))
            .expect("read workflow");
        for line in workflow.lines() {
            let command = line
                .trim()
                .strip_prefix("run: ")
                .unwrap_or_else(|| line.trim());
            if command.starts_with("cargo test")
                || command.starts_with("cargo build")
                || command.starts_with("cargo check")
            {
                assert!(
                    command.contains("--locked"),
                    "{workflow_name} cargo release gate is not locked: {command}"
                );
            }
        }
    }

    for (workflow_name, marker) in [
        ("pr-build.yml", "generate-branding.ps1 -Check"),
        ("release-assets.yml", "generate-branding.ps1 -Check"),
        (
            "sync-upstream.yml",
            "pwsh -NoProfile -File scripts/sync-upstream.ps1",
        ),
    ] {
        let workflow = std::fs::read_to_string(root.join(".github/workflows").join(workflow_name))
            .expect("read workflow");
        let marker_position = workflow.find(marker).expect("find branding gate marker");
        let checkout_position = workflow[..marker_position]
            .rfind("- name: Checkout")
            .expect("find checkout before branding gate");
        let checkout_block = &workflow[checkout_position..marker_position];
        assert!(
            checkout_block.contains("fetch-depth: 0"),
            "{workflow_name} branding/build-number gate needs tags and full history"
        );
    }

    let branding = std::fs::read_to_string(root.join("scripts/generate-branding.ps1"))
        .expect("read branding script");
    assert!(branding.contains("Assert-MacosBuildNumberProgress"));
}

#[cfg(windows)]
#[test]
fn upstream_sync_semver_fixture_contract_passes() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let test_script = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("scripts/test-sync-upstream.ps1");
    let output = std::process::Command::new("pwsh")
        .args(["-NoProfile", "-File"])
        .arg(test_script)
        .output()
        .expect("run sync-upstream contract tests");

    assert!(
        output.status.success(),
        "sync-upstream contract tests failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn upstream_sync_manual_dispatch_can_select_a_formal_release() {
    fn has_manual_tag_contract(workflow: &str) -> bool {
        let dispatch = workflow
            .split("  workflow_dispatch:\n")
            .nth(1)
            .and_then(|rest| rest.split("\nconcurrency:").next())
            .unwrap_or_default();
        let run_sync = workflow
            .split("      - name: Run sync script\n")
            .nth(1)
            .and_then(|rest| {
                rest.split("      - name: Package gated branch and result\n")
                    .next()
            })
            .unwrap_or_default();
        let has_line =
            |block: &str, expected: &str| block.lines().any(|line| line.trim() == expected);

        has_line(dispatch, "upstream_tag:")
            && has_line(dispatch, "required: false")
            && has_line(dispatch, "default: \"\"")
            && has_line(run_sync, "UPSTREAM_TAG: ${{ inputs.upstream_tag }}")
            && has_line(
                run_sync,
                "pwsh -NoProfile -File scripts/sync-upstream.ps1 -ResultPath $resultPath -UpstreamTag \"$env:UPSTREAM_TAG\"",
            )
    }

    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap();
    let workflow = std::fs::read_to_string(root.join(".github/workflows/sync-upstream.yml"))
        .expect("read sync workflow")
        .replace("\r\n", "\n");
    let script = std::fs::read_to_string(root.join("scripts/sync-upstream.ps1"))
        .expect("read sync script")
        .replace("\r\n", "\n");

    assert!(has_manual_tag_contract(&workflow));
    assert!(script.contains("[string]$UpstreamTag = ''"));
    assert!(script.contains("Get-LatestFormalUpstreamRelease -RequestedTag $UpstreamTag"));

    let commented_env = workflow.replace(
        "          UPSTREAM_TAG: ${{ inputs.upstream_tag }}",
        "          # UPSTREAM_TAG: ${{ inputs.upstream_tag }}",
    );
    assert!(!has_manual_tag_contract(&commented_env));
    let commented_command = workflow.replace(
        "          pwsh -NoProfile -File scripts/sync-upstream.ps1 -ResultPath $resultPath -UpstreamTag \"$env:UPSTREAM_TAG\"",
        "          # pwsh -NoProfile -File scripts/sync-upstream.ps1 -ResultPath $resultPath -UpstreamTag \"$env:UPSTREAM_TAG\"",
    );
    assert!(!has_manual_tag_contract(&commented_command));
    let wrong_step_decoy = commented_env.replace(
        "      - name: Package gated branch and result",
        "      - name: Package gated branch and result\n        env:\n          UPSTREAM_TAG: ${{ inputs.upstream_tag }}",
    );
    assert!(!has_manual_tag_contract(&wrong_step_decoy));
}

#[test]
fn ci_pins_rust_and_nsis_toolchains() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap();
    let toolchain = std::fs::read_to_string(root.join("rust-toolchain.toml"))
        .expect("read pinned Rust toolchain");
    assert!(toolchain.contains("channel = \"1.96.1\""));

    for workflow in ["pr-build.yml", "release-assets.yml"] {
        let text = std::fs::read_to_string(root.join(".github/workflows").join(workflow))
            .expect("read build workflow");
        assert!(text.contains("choco install nsis --version=3.11"));
        assert!(!text.contains("choco install nsis -y"));
    }
}

#[test]
fn release_baseline_docs_disclose_post_tag_source_snapshot() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap();
    let spec = std::fs::read_to_string(
        root.join("docs/superpowers/specs/2026-07-10-chimera-private-fork-design.md"),
    )
    .expect("read design spec");
    let report = std::fs::read_to_string(
        root.join("docs/superpowers/reports/2026-07-10-cross-verification.md"),
    )
    .expect("read cross verification report");

    for document in [&spec, &report] {
        assert!(document.contains("`a0506ae` 是 `v1.2.34` 后 1 个未发布提交"));
        assert!(document.contains("`c136029` 仅作为最近正式上游 Release 参照"));
    }
    assert!(!spec.contains("首发 Release 基线：正式 tag"));
}

#[test]
fn upstream_sync_issue_commands_bind_repository_explicitly() {
    fn job<'a>(workflow: &'a str, name: &str, next: Option<&str>) -> Option<&'a str> {
        let (_, rest) = workflow.split_once(&format!("\n  {name}:\n"))?;
        match next {
            Some(next) => rest
                .split_once(&format!("\n  {next}:\n"))
                .map(|(job, _)| job),
            None => Some(rest),
        }
    }

    fn step<'a>(job: &'a str, name: &str) -> Option<&'a str> {
        let (_, rest) = job.split_once(&format!("\n      - name: {name}\n"))?;
        Some(
            rest.split_once("\n      - name: ")
                .map(|(step, _)| step)
                .unwrap_or(rest),
        )
    }

    fn active_issue_commands(step: &str) -> Vec<&str> {
        step.lines()
            .map(str::trim_start)
            .filter(|line| !line.starts_with('#') && line.contains("gh issue "))
            .collect()
    }

    fn has_bound_issue_commands(workflow: &str) -> bool {
        let Some(publish_job) = job(workflow, "publish-sync-pr", Some("report-blocked")) else {
            return false;
        };
        let Some(report_job) = job(workflow, "report-blocked", None) else {
            return false;
        };
        if report_job.contains("uses: actions/checkout") {
            return false;
        }
        let Some(success_step) = step(publish_job, "Push sync branch and open PR") else {
            return false;
        };
        let Some(blocked_step) = step(
            report_job,
            "Upsert blocked Issue (conflict or gate failure)",
        ) else {
            return false;
        };
        let success = active_issue_commands(success_step);
        let blocked = active_issue_commands(blocked_step);
        let repo = "--repo \"$env:GITHUB_REPOSITORY\"";

        success.len() == 2
            && success.iter().all(|line| line.contains(repo))
            && success.iter().any(|line| line.contains("gh issue list "))
            && success.iter().any(|line| line.contains("gh issue close "))
            && blocked.len() == 3
            && blocked.iter().all(|line| line.contains(repo))
            && blocked.iter().any(|line| line.contains("gh issue list "))
            && blocked.iter().any(|line| line.contains("gh issue edit "))
            && blocked.iter().any(|line| line.contains("gh issue create "))
    }

    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap();
    let workflow = std::fs::read_to_string(root.join(".github/workflows/sync-upstream.yml"))
        .expect("read sync workflow")
        .replace("\r\n", "\n");
    assert!(has_bound_issue_commands(&workflow));

    let missing_close =
        workflow.replacen("gh issue close $_.number", "gh issue list --state open", 1);
    assert!(!has_bound_issue_commands(&missing_close));

    let checkout_in_report = workflow.replacen(
        "  report-blocked:\n",
        "  report-blocked:\n    steps:\n      - uses: actions/checkout@decoy\n",
        1,
    );
    assert!(!has_bound_issue_commands(&checkout_in_report));

    let commented_create = workflow.replacen(
        "            gh issue create --repo",
        "            # gh issue create --repo",
        1,
    );
    assert!(!has_bound_issue_commands(&commented_create));

    let wrong_step_decoy = commented_create.replacen(
        "      - name: Fail job on conflict or gate failure",
        "      - name: Decoy Issue command\n        run: gh issue create --repo \"$env:GITHUB_REPOSITORY\" --title decoy --body decoy\n\n      - name: Fail job on conflict or gate failure",
        1,
    );
    assert!(!has_bound_issue_commands(&wrong_step_decoy));
}

#[test]
fn relay_settings_keeps_profile_config_and_auth_files_isolated() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let app_tsx = manifest_dir.parent().unwrap().join("src/App.tsx");
    let app_tsx = std::fs::read_to_string(&app_tsx).expect("read manager App.tsx");
    let commands_rs = manifest_dir.join("src/commands.rs");
    let commands_rs = std::fs::read_to_string(&commands_rs).expect("read manager commands.rs");

    assert!(app_tsx.contains("snapshotActiveRelayFilesBeforeSwitch"));
    assert!(app_tsx.contains("backfill_relay_profile_from_live"));
    assert!(app_tsx.contains("relayProfileSwitchValidation(selectedBeforeSave)"));
    assert!(app_tsx.contains("缺少独立 config.toml"));
    assert!(app_tsx.contains("const command = relayProfileSwitchCommand(selectedAfterSave)"));
    assert!(app_tsx.contains("function relayProfileSwitchCommand"));
    assert!(app_tsx.contains("return \"apply_pure_api_injection\""));
    assert!(app_tsx.contains("return \"apply_relay_injection\""));
    assert!(app_tsx.contains("const createNewAggregateProfile = () =>"));
    assert!(app_tsx.contains("onClick={createNewAggregateProfile}"));
    assert!(app_tsx.contains("已打开聚合供应商详情"));
    assert!(!commands_rs.contains("缺少独立 auth.json"));
    assert!(commands_rs.contains("backfill_relay_profile_from_live"));
    assert!(commands_rs.contains("apply_relay_profile_to_home_with_switch_rules"));
}

#[test]
fn relay_context_management_is_global_not_supplier_scoped() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let app_tsx = manifest_dir.parent().unwrap().join("src/App.tsx");
    let app_tsx = std::fs::read_to_string(&app_tsx).expect("read manager App.tsx");
    let styles = manifest_dir.parent().unwrap().join("src/styles.css");
    let styles = std::fs::read_to_string(&styles).expect("read manager styles.css");

    assert!(app_tsx.contains("作为全局配置独立管理"));
    assert!(
        app_tsx.contains("label: t(\"工具与插件\")") || app_tsx.contains("label: \"工具与插件\"")
    );
    assert!(
        app_tsx.contains("title={t(\"Codex 工具与插件\")}")
            || app_tsx.contains("title=\"Codex 工具与插件\"")
    );
    assert!(!app_tsx.contains("label: \"上下文配置\""));
    assert!(!app_tsx.contains("title=\"上下文配置\""));
    assert!(!app_tsx.contains("<strong>Codex 上下文</strong>"));
    assert!(app_tsx.contains("id: \"context\""));
    assert!(app_tsx.contains("function ContextScreen"));
    assert!(app_tsx.contains("route === \"context\""));
    assert!(app_tsx.contains("if (next === \"context\")"));
    assert!(app_tsx.contains("selectedContextConfigToml(entries)"));
    assert!(app_tsx.contains("toggleContextEntryEnabled"));
    assert!(app_tsx.contains("relayFiles={relayFiles}"));
    assert!(app_tsx.contains("read_live_context_entries"));
    assert!(app_tsx.contains("sync_live_context_entries"));
    assert!(app_tsx.contains("refreshLiveContextEntries"));
    assert!(app_tsx.contains("syncLiveContextEntries(next, true)"));
    assert!(app_tsx.contains("function contextEntriesWithLiveEntries"));
    assert!(app_tsx.contains("liveByKind"));
    assert!(app_tsx.contains("mergeLiveContextEntries"));
    assert!(app_tsx.contains("withLiveEntryState"));
    assert!(app_tsx.contains("contextEnabledSwitch"));
    assert!(!app_tsx.contains("entry.enabled ? \"已启用\" : \"已禁用\""));
    assert!(!app_tsx.contains("空配置体"));
    assert!(app_tsx.contains("relay-context-delete"));
    assert!(!app_tsx.contains("切换供应商时只合并勾选项"));
    assert!(!app_tsx.contains("未勾选的条目不会写入"));
    assert!(!app_tsx.contains("className=\"context-switch\""));
    assert!(!styles.contains(".context-switch {"));
    assert!(styles.contains(".context-enabled-switch"));
    assert!(styles.contains(".context-switch-track"));
    assert!(styles.contains(".context-switch-thumb"));
    assert!(!styles.contains(".relay-context-row code"));
    assert!(styles.contains(".relay-context-delete"));
}

#[test]
fn manager_window_and_relay_detail_header_stay_usable() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let app_tsx = manifest_dir.parent().unwrap().join("src/App.tsx");
    let app_tsx = std::fs::read_to_string(&app_tsx).expect("read manager App.tsx");
    let styles = manifest_dir.parent().unwrap().join("src/styles.css");
    let styles = std::fs::read_to_string(&styles).expect("read manager styles.css");
    let lib_rs =
        std::fs::read_to_string(manifest_dir.join("src/lib.rs")).expect("read manager lib.rs");
    let tauri_conf =
        std::fs::read_to_string(manifest_dir.join("tauri.conf.json")).expect("read tauri config");

    assert!(app_tsx.contains("relay-detail-sticky"));
    assert!(!app_tsx.contains("CardHead title=\"供应商详情\""));
    assert!(styles.contains(".relay-detail-sticky"));
    assert!(styles.contains("position: sticky"));
    assert!(styles.contains("top: 0"));
    assert!(styles.contains("margin: 0"));
    assert!(lib_rs.contains(".inner_size(1180.0, 820.0)"));
    assert!(lib_rs.contains(".min_inner_size(960.0, 720.0)"));
    assert!(tauri_conf.contains("\"width\": 1180"));
    assert!(tauri_conf.contains("\"height\": 820"));
    assert!(tauri_conf.contains("\"minWidth\": 960"));
    assert!(tauri_conf.contains("\"minHeight\": 720"));
    assert!(tauri_conf.contains("\"productName\": \"Chimera++ 管理工具\""));

    let tauri_runtime = include_str!("../src/lib.rs");
    assert!(tauri_runtime.contains(".title(codex_plus_core::branding::DISPLAY_MANAGER_NAME)"));
    assert!(!tauri_runtime.contains(".title(\"Chimera Codex 管理工具\")"));
}

#[test]
fn relay_preview_deduplicates_root_keys_when_merging_common_config() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let app_tsx = manifest_dir.parent().unwrap().join("src/App.tsx");
    let app_tsx = std::fs::read_to_string(&app_tsx).expect("read manager App.tsx");

    assert!(app_tsx.contains("dedupeTomlRootLines"));
    assert!(app_tsx.contains("rootSeen.add(key)"));
    assert!(app_tsx.contains("joinTomlSectionsRootFirst"));
}

#[test]
fn provider_presets_include_chimerahub_and_drop_jojo_promo() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let presets = manifest_dir.parent().unwrap().join("src/presets.ts");
    let presets = std::fs::read_to_string(&presets).expect("read manager presets.ts");

    assert!(presets.contains("id: \"chimerahub\""));
    assert!(presets.contains("name: \"ChimeraHub\""));
    assert!(presets.contains("DEFAULT_RELAY_BASE_URL"));
    assert!(presets.contains("protocol: \"responses\""));
    assert!(!presets.contains("jojocode"));
    assert!(!presets.contains("jojocode.com"));
    assert!(!presets.contains("/i/drGuwc9k"));
    assert!(!presets.contains("ic=RRVJPB5SII"));
}

#[test]
fn manager_ui_uses_chimera_brand_instead_of_legacy_display_name() {
    let manager_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manager directory");
    for relative in [
        "index.html",
        "src/App.tsx",
        "src/i18n.ts",
        "src/i18n-en.ts",
        "src/presets.ts",
    ] {
        let source = std::fs::read_to_string(manager_dir.join(relative)).unwrap();
        assert!(
            !source.contains("Codex++"),
            "legacy Codex++ display name remains in {relative}"
        );
        assert!(
            !source.contains("Chimera Codex"),
            "legacy Chimera Codex display name remains in {relative}"
        );
    }

    let index = std::fs::read_to_string(manager_dir.join("index.html")).unwrap();
    assert!(index.contains("<title>Chimera++ 管理工具</title>"));

    let app = std::fs::read_to_string(manager_dir.join("src/App.tsx")).unwrap();
    assert!(app.contains("windowTitle: DISPLAY_MANAGER_NAME"));
    assert!(!app.contains("windowTitle: \"Chimera++"));

    let i18n_keys = std::fs::read_to_string(
        manager_dir
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repository root")
            .join("tools/i18n-keys.json"),
    )
    .unwrap();
    assert!(!i18n_keys.contains("Chimera Codex"));

    for relative in ["capabilities/default.json", "gen/schemas/capabilities.json"] {
        let source = std::fs::read_to_string(manager_dir.join("src-tauri").join(relative)).unwrap();
        assert!(!source.contains("Chimera Codex Manager"));
        assert!(source.contains("Chimera++ Manager"));
    }
}

#[test]
fn provider_presets_include_runapi() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let presets = manifest_dir.parent().unwrap().join("src/presets.ts");
    let presets = std::fs::read_to_string(&presets).expect("read manager presets.ts");

    assert!(presets.contains("id: \"runapi\""));
    assert!(presets.contains("name: \"RunAPI\""));
    assert!(presets.contains("category: \"aggregator\""));
    assert!(presets.contains("baseUrl: \"https://runapi.co/v1\""));
}

#[test]
fn manager_no_longer_exposes_mobile_control() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let app_tsx = manifest_dir.parent().unwrap().join("src/App.tsx");
    let app_tsx = std::fs::read_to_string(&app_tsx).expect("read manager App.tsx");

    assert!(!app_tsx.contains("mobileControl"));
    assert!(!app_tsx.contains("手机控制"));
    assert!(!app_tsx.contains("mobileRelayServers"));
    assert!(!app_tsx.contains("MobileControlScreen"));
}

#[test]
fn manager_ui_no_longer_exposes_command_wrapper_or_startup_marketplace_prompt() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let app_tsx = manifest_dir.parent().unwrap().join("src/App.tsx");
    let app_tsx = std::fs::read_to_string(&app_tsx).expect("read manager App.tsx");

    assert!(!app_tsx.contains("启用 Codex 命令包装器"));
    assert!(!app_tsx.contains("修复后端"));
    assert!(!app_tsx.contains("repairBackend"));
    assert!(!app_tsx.contains("await checkPluginMarketplacePrompt()"));
}

#[test]
fn manager_update_install_keeps_visible_progress_bar() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let app_tsx = manifest_dir.parent().unwrap().join("src/App.tsx");
    let app_tsx = std::fs::read_to_string(&app_tsx).expect("read manager App.tsx");

    assert!(!app_tsx.contains("下载并运行安装包"));
    assert!(app_tsx.contains("updateInstallProgress"));
    assert!(app_tsx.contains("安装包更新进度"));
    assert!(app_tsx.contains("completedTitle={t(\"上次更新结果\")}"));
    assert!(app_tsx.contains("progress={updateInstallProgress}"));
    assert!(app_tsx.contains("updateInstallProgress={updateInstallProgress}"));
    assert!(app_tsx.contains("call<UpdateResult>(\"perform_update\", { version })"));
    assert!(!app_tsx.contains("call<UpdateResult>(\"perform_update\", { release })"));
    assert!(app_tsx.contains("minimumSupportedVersion?: string | null"));
    assert!(app_tsx.contains("mandatoryUpdate?: boolean"));
    assert!(app_tsx.contains("exitCurrentProcess?: boolean"));
    assert!(app_tsx.contains("requiresUserConfirmation?: boolean"));
    assert!(app_tsx.contains("const updateAutomatically = async"));
    assert!(app_tsx.contains("updateInstallInFlightRef"));
    assert!(app_tsx.contains("if (updateInstallInFlightRef.current) return"));
    let automatic_update = app_tsx
        .split("const updateAutomatically = async")
        .nth(1)
        .and_then(|source| source.split("const saveSettings").next())
        .expect("automatic update function");
    let automatic_guard = automatic_update
        .find("updateInstallInFlightRef.current = true")
        .expect("automatic update guard acquisition");
    let automatic_check = automatic_update
        .find("checkUpdate(silent)")
        .expect("automatic update check");
    let automatic_release = automatic_update
        .rfind("updateInstallInFlightRef.current = false")
        .expect("automatic update guard release");
    assert!(automatic_guard < automatic_check);
    assert!(automatic_check < automatic_release);
    assert!(app_tsx.contains("launch_after_optional_update_failure"));
    assert!(app_tsx.contains("mandatory-update-overlay"));
    assert!(app_tsx.contains("call<null>(\"manager_exit_app\")"));

    let commands_rs = std::fs::read_to_string(manifest_dir.join("src/commands.rs"))
        .expect("read manager commands");
    assert!(commands_rs.contains("version: Option<String>"));
    assert!(commands_rs.contains("fetch_latest_release("));
    assert!(commands_rs.contains("codex_plus_core::update::DEFAULT_LATEST_JSON_URL"));
    assert!(commands_rs.contains("validate_update_request("));
    assert!(commands_rs.contains("\"minimumSupportedVersion\": update.minimum_supported_version"));
    assert!(commands_rs.contains("\"minimumSupportedVersion\": Value::Null"));
    assert!(commands_rs.contains("startup_update_status("));
    assert!(commands_rs.contains("pub fn launch_after_optional_update_failure"));
    assert!(commands_rs.contains("UpdateContinuationStore::default()"));
    assert!(commands_rs.contains("--update-continuation-token"));
    assert!(!commands_rs.contains("CHIMERA_SKIP_UPDATE_ONCE"));
    assert!(commands_rs.contains("UpdateOperationGuard::acquire()"));
    let update_busy = commands_rs
        .split("let Some(_operation_guard) = UpdateOperationGuard::acquire() else")
        .nth(1)
        .and_then(|source| source.split("let update_state_store").next())
        .expect("single-flight busy response");
    assert!(update_busy.contains("\"mandatoryUpdate\": true"));
    assert!(update_busy.contains("\"updateInProgress\": true"));
    let asset_validation = commands_rs
        .find("validate_release_for_install(&release)")
        .expect("installable release validation");
    let floor_record = commands_rs
        .find("update_state_store.record_trusted_floor(floor)")
        .expect("trusted floor record");
    assert!(asset_validation < floor_record);
    assert!(commands_rs.contains("\"mandatoryUpdate\":"));

    let core_update = std::fs::read_to_string(
        manifest_dir
            .parent()
            .and_then(std::path::Path::parent)
            .and_then(std::path::Path::parent)
            .unwrap()
            .join("crates/codex-plus-core/src/update.rs"),
    )
    .expect("read core updater");
    assert!(core_update.contains("arguments: vec![\"/S\".to_string()]"));
    assert!(core_update.contains(".args(&policy.arguments)"));
    assert!(core_update.contains("requires_user_confirmation: true"));
    let core_perform_update = core_update
        .split("pub async fn perform_update(")
        .nth(1)
        .and_then(|source| source.split("pub async fn download_release_asset(").next())
        .expect("core perform_update function");
    let download = core_perform_update
        .find("download_release_asset(release, download_dir).await")
        .expect("verified update download");
    let continuation = core_perform_update
        .find("UpdateContinuationStore::default()")
        .expect("post-download continuation issuance");
    assert!(download < continuation);
    assert!(core_update.contains("std::time::Duration::from_secs(2 * 60 * 60)"));
    let manager_perform_update = commands_rs
        .split("pub async fn perform_update(")
        .nth(1)
        .and_then(|source| source.split("pub fn load_watcher_state").next())
        .expect("manager perform_update function");
    assert!(!manager_perform_update.contains("UpdateContinuationStore::default().issue"));
    let macos_launch = core_update
        .split("fn launch_installer(")
        .nth(1)
        .expect("installer launch function")
        .split("#[cfg(target_os = \"macos\")]")
        .nth(1)
        .and_then(|source| source.split("#[cfg(all(not(windows)").next())
        .expect("macOS installer launch block");
    assert!(macos_launch.contains(".status()"));
    assert!(macos_launch.contains("status.success()"));
    assert!(!macos_launch.contains(".spawn()"));
}

#[test]
fn launcher_checks_update_state_before_settings_and_codex_routing() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let launcher = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("codex-plus-launcher/src/main.rs");
    let launcher = std::fs::read_to_string(launcher).expect("read launcher main");
    let continuation_env = launcher
        .find("std::env::var(\"CHIMERA_UPDATE_CONTINUATION_TOKEN\")")
        .expect("continuation token environment fallback");
    let route_call = launcher
        .find("resolve_single_entry_route(update_continuation_token.as_deref())")
        .expect("single entry route receives continuation token");
    assert!(continuation_env < route_call);
    let route = launcher
        .split("async fn resolve_single_entry_route(")
        .nth(1)
        .and_then(|source| source.split("fn manager_start_argument").next())
        .expect("single entry route source");

    let update = route
        .find("startup_update_status(")
        .expect("startup update");
    let settings = route
        .find("SettingsStore::default()")
        .expect("settings load");
    assert!(update < settings);
    assert!(route.contains("UpdateAction::None"));
    assert!(route.contains("consume_if_supported"));
    assert!(!route.contains("CHIMERA_SKIP_UPDATE_ONCE"));
}

#[test]
fn manager_accepts_single_entry_start_routes_without_exposing_github() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let main_rs =
        std::fs::read_to_string(manifest_dir.join("src/main.rs")).expect("read manager main.rs");
    let lib_rs =
        std::fs::read_to_string(manifest_dir.join("src/lib.rs")).expect("read manager lib.rs");
    let app_tsx =
        std::fs::read_to_string(manifest_dir.join("../src/App.tsx")).expect("read manager App.tsx");

    for argument in [
        "--recover-settings",
        "--configure-relay",
        "--chimera-key-first",
    ] {
        assert!(main_rs.contains(argument));
    }
    assert!(main_rs.contains("CODEX_PLUS_START_ROUTE"));
    assert!(lib_rs.contains("startRoute=maintenance"));
    assert!(lib_rs.contains("startRoute=relay"));
    assert!(app_tsx.contains("params.get(\"startRoute\")"));
    assert!(lib_rs.contains("forward_start_route_to_existing_manager"));
    assert!(lib_rs.contains("write_start_route_request"));
    assert!(lib_rs.contains("START_ROUTE_ACK_TIMEOUT"));
    assert!(lib_rs.contains("manager_frontend_ready"));
    assert!(lib_rs.contains("chimera-start-route"));
    assert!(lib_rs.contains("window.show()"));
    assert!(lib_rs.contains("window.set_focus()"));
    assert!(app_tsx.contains("listen<string>(\"chimera-start-route\""));
    assert!(app_tsx.contains("invoke(\"manager_frontend_ready\")"));
}

#[test]
fn updater_platform_publish_and_launch_paths_keep_verified_object_identity() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let update_rs = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .unwrap()
        .join("crates/codex-plus-core/src/update.rs");
    let update_rs = std::fs::read_to_string(update_rs).expect("read updater implementation");

    assert!(update_rs.contains("renameat2"));
    assert!(update_rs.contains("RENAME_NOREPLACE"));
    assert!(update_rs.contains("O_NOFOLLOW"));
    assert!(update_rs.contains("hdiutil"));
    assert!(update_rs.contains("/dev/fd/{fd}"));
    assert!(update_rs.contains("FD_CLOEXEC"));
    assert!(!update_rs.contains("std::fs::rename(&copy_path, &final_path)"));
}
