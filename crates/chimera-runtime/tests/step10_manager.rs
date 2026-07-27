use chimera_runtime::manager::{
    InstallMode, InstalledCodex, MaintenanceRoute, UpdateSource, WindowsReleasePlan,
    latest_portable_rollback, maintenance_route, mirror_endpoints, parse_windows_release_plan,
};

const MANIFEST: &str = r#"{
  "schemaVersion": 5,
  "codexVersion": "26.721.41059",
  "publishedAt": "2026-07-24T21:33:02Z",
  "sources": {
    "windows": {
      "version": "26.721.4979.0",
      "appVersion": "26.721.41059",
      "packageMoniker": "OpenAI.Codex_26.721.4979.0_x64__2p2nqsd0c76g0",
      "architecture": "x64",
      "contentLength": 744080244,
      "architectures": {
        "x64": {
          "architecture": "x64",
          "downloadable": true,
          "version": "26.721.4979.0",
          "appVersion": "26.721.41059",
          "packageMoniker": "OpenAI.Codex_26.721.4979.0_x64__2p2nqsd0c76g0",
          "contentLength": 744080244
        }
      }
    }
  }
}"#;

const CHECKSUMS: &str = concat!(
    "f0c1d75045952a11a581d34f28f595d1d110fb13f8f7e5c5201802ed2bbd7093  ",
    "OpenAI.Codex_26.721.4979.0_x64__2p2nqsd0c76g0.Msix\n"
);

#[test]
fn public_version_is_not_confused_with_the_msix_package_version() {
    let plan =
        parse_windows_release_plan(MANIFEST, CHECKSUMS, UpdateSource::Mirror, Some("x64")).unwrap();

    assert_eq!(plan.version, "26.721.41059");
    assert_eq!(plan.package_version, "26.721.4979.0");
    assert_eq!(plan.size_bytes, 744080244);
    assert_eq!(
        plan.sha256,
        "f0c1d75045952a11a581d34f28f595d1d110fb13f8f7e5c5201802ed2bbd7093"
    );
}

#[test]
fn mirror_endpoints_are_architecture_specific() {
    let endpoints = mirror_endpoints(UpdateSource::Auto, Some("x64"));
    assert_eq!(
        endpoints.manifest_url,
        "https://codexapp.agentsmirror.com/latest/manifest"
    );
    assert_eq!(
        endpoints.package_url,
        "https://codexapp.agentsmirror.com/latest/win-x64"
    );

    let arm = mirror_endpoints(UpdateSource::Mirror, Some("arm64"));
    assert!(arm.package_url.ends_with("/latest/win-arm64"));
}

#[test]
fn settings_values_parse_strictly() {
    assert_eq!("auto".parse::<UpdateSource>().unwrap(), UpdateSource::Auto);
    assert_eq!(
        "mirror".parse::<UpdateSource>().unwrap(),
        UpdateSource::Mirror
    );
    assert_eq!(
        "standard".parse::<InstallMode>().unwrap(),
        InstallMode::Standard
    );
    assert_eq!(
        "portable".parse::<InstallMode>().unwrap(),
        InstallMode::Portable
    );
    assert!("official".parse::<UpdateSource>().is_err());
    assert!("guess".parse::<InstallMode>().is_err());
}

#[test]
fn update_comparison_accepts_both_app_and_package_versions() {
    let plan = WindowsReleasePlan {
        version: "26.721.41059".into(),
        package_version: "26.721.4979.0".into(),
        package_moniker: "OpenAI.Codex_26.721.4979.0_x64__2p2nqsd0c76g0".into(),
        package_url: "https://codexapp.agentsmirror.com/latest/win-x64".into(),
        sha256: "f".repeat(64),
        size_bytes: 1,
        released_at: None,
    };

    assert!(plan.is_update_available(Some("26.721.31836")));
    assert!(!plan.is_update_available(Some("26.721.41059")));
    assert!(!plan.is_update_available(Some("26.721.4979.0")));
    assert!(plan.is_update_available(None));
}

#[test]
fn remote_package_moniker_cannot_become_a_local_path() {
    let hostile = MANIFEST.replace(
        "OpenAI.Codex_26.721.4979.0_x64__2p2nqsd0c76g0",
        "../../outside",
    );
    let hostile_checksums = format!("{}  ../../outside.Msix\n", "f".repeat(64));

    assert!(
        parse_windows_release_plan(
            &hostile,
            &hostile_checksums,
            UpdateSource::Mirror,
            Some("x64")
        )
        .is_err()
    );
}

#[test]
fn maintenance_routes_follow_the_detected_install_type() {
    let standard = InstalledCodex {
        version: "26.721.41059".into(),
        path: r"C:\Program Files\WindowsApps\OpenAI.Codex".into(),
        install_mode: "standard".into(),
    };
    let portable = InstalledCodex {
        version: "26.721.41059".into(),
        path: r"C:\Users\me\AppData\Local\Chimera\codex-portable".into(),
        install_mode: "portable".into(),
    };

    assert_eq!(
        maintenance_route(Some(&standard)),
        MaintenanceRoute::Standard
    );
    assert_eq!(
        maintenance_route(Some(&portable)),
        MaintenanceRoute::Portable
    );
    assert_eq!(maintenance_route(None), MaintenanceRoute::NotInstalled);
}

#[test]
fn rollback_discovery_ignores_files_and_chooses_the_newest_backup() {
    let root =
        std::env::temp_dir().join(format!("chimera-manager-rollback-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Codex.rollback-old")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::create_dir_all(root.join("Codex.rollback-new")).unwrap();
    std::fs::write(root.join("Codex.rollback-hostile"), b"not a directory").unwrap();

    assert_eq!(
        latest_portable_rollback(&root.join("codex-portable")).unwrap(),
        Some(root.join("Codex.rollback-new"))
    );

    std::fs::remove_dir_all(root).unwrap();
}
