use codex_plus_core::branding::{ARTIFACT_PREFIX, LATEST_JSON_URL, REPOSITORY};
use codex_plus_core::update::{
    DEFAULT_LATEST_JSON_URL, DEFAULT_REPOSITORY, Release, download_asset_to, is_newer_version,
    parse_version_tag, release_from_latest_json_payload, safe_asset_name, select_update_asset,
    verify_downloaded_bytes,
};
use serde_json::json;
use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn chimera_assets(version: &str) -> serde_json::Value {
    let win = format!("{ARTIFACT_PREFIX}-{version}-windows-x64-setup.exe");
    let mac_x64 = format!("{ARTIFACT_PREFIX}-{version}-macos-x64.dmg");
    let mac_arm = format!("{ARTIFACT_PREFIX}-{version}-macos-arm64.dmg");
    json!([
        {
            "name": "source.zip",
            "url": "https://example.test/source.zip",
            "sha256": "aa",
            "size": 1
        },
        {
            "name": win,
            "url": "https://example.test/setup.exe",
            "sha256": "11".repeat(32),
            "size": 100
        },
        {
            "name": mac_x64,
            "url": "https://example.test/app-x64.dmg",
            "sha256": "22".repeat(32),
            "size": 200
        },
        {
            "name": mac_arm,
            "url": "https://example.test/app-arm64.dmg",
            "sha256": "33".repeat(32),
            "size": 300
        }
    ])
}

#[test]
fn updater_constants_use_public_chimera_branding_not_upstream() {
    assert_eq!(DEFAULT_REPOSITORY, REPOSITORY);
    assert_eq!(DEFAULT_LATEST_JSON_URL, LATEST_JSON_URL);
    assert!(!DEFAULT_LATEST_JSON_URL.contains("BigPizzaV3/CodexPlusPlus"));
    assert!(DEFAULT_LATEST_JSON_URL.contains(REPOSITORY));
}

#[test]
fn chimera_semver_comparison_orders_revision_and_upstream() {
    assert!(is_newer_version("1.2.34-chimera.2", "1.2.34-chimera.1").unwrap());
    assert!(is_newer_version("1.2.35-chimera.1", "1.2.34-chimera.9").unwrap());
    assert!(!is_newer_version("1.2.34-chimera.1", "1.2.34-chimera.1").unwrap());
    assert!(!is_newer_version("1.2.34-chimera.1", "1.2.34-chimera.2").unwrap());
}

#[test]
fn chimera_semver_rejects_illegal_and_foreign_channels() {
    assert!(is_newer_version("1.2.34", "1.2.34-chimera.1").is_err());
    assert!(is_newer_version("1.2.34-beta.1", "1.2.34-chimera.1").is_err());
    assert!(is_newer_version("1.2.34-chimera", "1.2.34-chimera.1").is_err());
    assert!(parse_version_tag("v1.2.3-beta.1").is_err());
    assert!(parse_version_tag("1.2.34").is_err());
    assert_eq!(
        parse_version_tag("v1.2.34-chimera.1").unwrap().to_string(),
        "1.2.34-chimera.1"
    );
}

#[test]
fn latest_json_rejects_missing_checksum_fields() {
    let err = release_from_latest_json_payload(&json!({
        "version": "1.2.34-chimera.1",
        "url": "https://github.com/Duojiyi/chimera-codex/releases/tag/v1.2.34-chimera.1",
        "body": "notes",
        "assets": [{
            "name": format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-windows-x64-setup.exe"),
            "url": "https://example.test/setup.exe"
        }]
    }))
    .unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("sha256") || message.contains("size"),
        "missing checksum fields must fail: {message}"
    );
}

#[test]
fn latest_json_selects_strict_chimera_platform_assets() {
    let release = release_from_latest_json_payload(&json!({
        "version": "1.2.34-chimera.1",
        "url": "https://github.com/Duojiyi/chimera-codex/releases/tag/v1.2.34-chimera.1",
        "body": "静态更新描述",
        "assets": chimera_assets("1.2.34-chimera.1")
    }))
    .unwrap();

    assert_eq!(release.version, "1.2.34-chimera.1");
    assert_eq!(release.body, "静态更新描述");
    if cfg!(windows) {
        let expected_name =
            format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-windows-x64-setup.exe");
        let expected_hash = "11".repeat(32);
        assert_eq!(release.asset_name.as_deref(), Some(expected_name.as_str()));
        assert_eq!(release.asset_size, Some(100));
        assert_eq!(release.asset_sha256.as_deref(), Some(expected_hash.as_str()));
    } else if cfg!(target_os = "macos") {
        let expected = match std::env::consts::ARCH {
            "x86_64" => format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-x64.dmg"),
            "aarch64" => format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-arm64.dmg"),
            other => panic!("unexpected target arch in test: {other}"),
        };
        assert_eq!(release.asset_name.as_deref(), Some(expected.as_str()));
    } else {
        assert_eq!(release.asset_name.as_deref(), None);
    }
}

#[test]
fn asset_selection_rejects_zip_source_and_near_prefix() {
    let assets = vec![
        codex_plus_core::update::ReleaseAsset {
            name: "source.zip".to_string(),
            browser_download_url: "https://example.test/source.zip".to_string(),
            sha256: "aa".repeat(32),
            size: 1,
        },
        codex_plus_core::update::ReleaseAsset {
            name: format!("{ARTIFACT_PREFIX}Extra-1.2.34-chimera.1-windows-x64-setup.exe"),
            browser_download_url: "https://example.test/near.exe".to_string(),
            sha256: "bb".repeat(32),
            size: 2,
        },
        codex_plus_core::update::ReleaseAsset {
            name: "CodexPlusPlus-1.2.34-windows-x64-setup.exe".to_string(),
            browser_download_url: "https://example.test/upstream.exe".to_string(),
            sha256: "cc".repeat(32),
            size: 3,
        },
        codex_plus_core::update::ReleaseAsset {
            name: format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-windows-arm64-setup.exe"),
            browser_download_url: "https://example.test/arm.exe".to_string(),
            sha256: "dd".repeat(32),
            size: 4,
        },
        codex_plus_core::update::ReleaseAsset {
            name: format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-windows-x64-setup.exe"),
            browser_download_url: "https://example.test/setup.exe".to_string(),
            sha256: "ee".repeat(32),
            size: 5,
        },
        codex_plus_core::update::ReleaseAsset {
            name: format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-x64.dmg"),
            browser_download_url: "https://example.test/app-x64.dmg".to_string(),
            sha256: "ff".repeat(32),
            size: 6,
        },
        codex_plus_core::update::ReleaseAsset {
            name: format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-arm64.dmg"),
            browser_download_url: "https://example.test/app-arm64.dmg".to_string(),
            sha256: "01".repeat(32),
            size: 7,
        },
    ];

    if cfg!(windows) {
        let selected = select_update_asset(&assets).unwrap();
        assert_eq!(
            selected.name,
            format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-windows-x64-setup.exe")
        );
    } else if cfg!(target_os = "macos") {
        let selected = select_update_asset(&assets).unwrap();
        let expected = match std::env::consts::ARCH {
            "x86_64" => format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-x64.dmg"),
            "aarch64" => format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-arm64.dmg"),
            other => panic!("unexpected target arch in test: {other}"),
        };
        assert_eq!(selected.name, expected);
    } else {
        assert!(select_update_asset(&assets).is_none());
    }
}

#[test]
fn asset_selection_prefers_macos_native_arch() {
    let assets = vec![
        codex_plus_core::update::ReleaseAsset {
            name: format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-arm64.dmg"),
            browser_download_url: "https://example.test/app-arm64.dmg".to_string(),
            sha256: "33".repeat(32),
            size: 300,
        },
        codex_plus_core::update::ReleaseAsset {
            name: format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-x64.dmg"),
            browser_download_url: "https://example.test/app-x64.dmg".to_string(),
            sha256: "22".repeat(32),
            size: 200,
        },
    ];

    if cfg!(target_os = "macos") {
        let selected = select_update_asset(&assets).expect("macOS DMG should be selected");
        let expected = match std::env::consts::ARCH {
            "x86_64" => format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-x64.dmg"),
            "aarch64" => format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-arm64.dmg"),
            other => panic!("unexpected target arch in test: {other}"),
        };
        assert_eq!(selected.name, expected);
    } else {
        assert!(select_update_asset(&assets).is_none());
    }
}

#[test]
fn safe_asset_name_rejects_path_traversal() {
    assert_eq!(safe_asset_name("pkg.zip").unwrap(), "pkg.zip");
    assert!(safe_asset_name("../pkg.zip").is_err());
    assert!(safe_asset_name("").is_err());
}

#[test]
fn download_asset_to_verifies_sha256_and_size() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = b"abcdef";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };

    let path = download_asset_to(&release, bytes, dir.path()).unwrap();
    assert_eq!(path, dir.path().join("pkg.bin"));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn download_asset_to_rejects_wrong_hash_and_cleans_temp() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = b"abcdef";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some("00".repeat(32)),
        asset_size: Some(bytes.len() as u64),
    };

    let err = download_asset_to(&release, bytes, dir.path()).unwrap_err();
    assert!(err.to_string().to_ascii_lowercase().contains("sha256"));
    assert!(!dir.path().join("pkg.bin").exists());
    assert!(!dir.path().join("pkg.bin.part").exists());
}

#[test]
fn download_asset_to_rejects_wrong_size_and_cleans_temp() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = b"abcdef";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(999),
    };

    let err = download_asset_to(&release, bytes, dir.path()).unwrap_err();
    assert!(err.to_string().to_ascii_lowercase().contains("size"));
    assert!(!dir.path().join("pkg.bin").exists());
    assert!(!dir.path().join("pkg.bin.part").exists());
}

#[test]
fn verify_downloaded_bytes_accepts_matching_digest() {
    let bytes = b"hello-chimera";
    verify_downloaded_bytes(bytes, &sha256_hex(bytes), bytes.len() as u64).unwrap();
}
