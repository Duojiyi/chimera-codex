use codex_plus_core::branding::{ARTIFACT_PREFIX, LATEST_JSON_URL, REPOSITORY};
use codex_plus_core::update::{
    DEFAULT_LATEST_JSON_URL, DEFAULT_REPOSITORY, MAX_LATEST_JSON_BYTES, Release,
    TrustedFloorSnapshot, UpdateAction, UpdateCacheStatus, UpdateContinuationStore, UpdatePlatform,
    UpdateStateStore, decide_update, download_asset_to, download_release_asset_with_client,
    download_release_asset_with_client_and_timeouts, fetch_latest_release_with_client,
    fetch_latest_release_with_client_and_timeout, installer_launch_policy, is_newer_version,
    parse_version_tag, release_from_latest_json_payload, safe_asset_name, select_update_asset,
    select_update_asset_for_target, startup_update_status_from_parts, update_check_from_release,
    validate_installer_for_launch, validate_release_for_install, validate_update_request,
    verify_downloaded_bytes,
};
use fs2::FileExt as _;
use serde_json::json;
use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn loopback_http_client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

fn assert_no_part_files(dir: &std::path::Path) {
    let parts = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".part"))
        .collect::<Vec<_>>();
    assert!(parts.is_empty(), "temporary downloads remain: {parts:?}");
}

fn serve_http_once(response: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{Read as _, Write as _};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        stream.write_all(&response).unwrap();
    });
    (format!("http://{address}/fixture"), handle)
}

fn serve_stalled_http_once(delay: std::time::Duration) -> (String, std::thread::JoinHandle<()>) {
    use std::io::Read as _;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = [0_u8; 4096];
        let _ = stream.read(&mut request);
        std::thread::sleep(delay);
    });
    (format!("http://{address}/fixture"), handle)
}

fn branded_release_asset_url(version: &str, name: &str) -> String {
    format!("https://github.com/{REPOSITORY}/releases/download/v{version}/{name}")
}

fn chimera_assets(version: &str) -> serde_json::Value {
    let win = format!("{ARTIFACT_PREFIX}-{version}-windows-x64-setup.exe");
    let mac_x64 = format!("{ARTIFACT_PREFIX}-{version}-macos-x64.dmg");
    let mac_arm = format!("{ARTIFACT_PREFIX}-{version}-macos-arm64.dmg");
    let source_url = branded_release_asset_url(version, "source.zip");
    let win_url = branded_release_asset_url(version, &win);
    let mac_x64_url = branded_release_asset_url(version, &mac_x64);
    let mac_arm_url = branded_release_asset_url(version, &mac_arm);
    json!([
        {
            "name": "source.zip",
            "url": source_url,
            "sha256": "aa".repeat(32),
            "size": 1
        },
        {
            "name": win,
            "url": win_url,
            "sha256": "11".repeat(32),
            "size": 100
        },
        {
            "name": mac_x64,
            "url": mac_x64_url,
            "sha256": "22".repeat(32),
            "size": 200
        },
        {
            "name": mac_arm,
            "url": mac_arm_url,
            "sha256": "33".repeat(32),
            "size": 300
        }
    ])
}

fn chimera_manifest(version: &str, minimum_supported_version: Option<&str>) -> serde_json::Value {
    let mut payload = json!({
        "version": version,
        "url": format!("https://github.com/{REPOSITORY}/releases/tag/v{version}"),
        "body": "notes",
        "assets": chimera_assets(version)
    });
    if let Some(minimum_supported_version) = minimum_supported_version {
        payload["minimum_supported_version"] = json!(minimum_supported_version);
    }
    payload
}

fn decision_release(
    version: &str,
    minimum_supported_version: Option<&str>,
    has_asset: bool,
) -> Release {
    Release {
        version: version.to_string(),
        minimum_supported_version: minimum_supported_version.map(str::to_string),
        url: "https://example.test/release".to_string(),
        body: "notes".to_string(),
        asset_name: has_asset.then(|| "setup.exe".to_string()),
        asset_url: has_asset.then(|| "https://example.test/setup.exe".to_string()),
        asset_sha256: has_asset.then(|| "ab".repeat(32)),
        asset_size: has_asset.then_some(42),
    }
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
    assert!(parse_version_tag("vv1.2.34-chimera.1").is_err());
    assert!(parse_version_tag(" 1.2.34-chimera.1").is_err());
    assert!(parse_version_tag("1.2.34-chimera.1 ").is_err());
    assert!(parse_version_tag("1.2.34-chimera.18446744073709551616").is_err());
    assert_eq!(
        parse_version_tag("V1.2.34-chimera.18446744073709551615")
            .unwrap()
            .to_string(),
        "1.2.34-chimera.18446744073709551615"
    );
}

#[test]
fn latest_json_defaults_missing_minimum_supported_version() {
    let release =
        release_from_latest_json_payload(&chimera_manifest("1.2.35-chimera.1", None)).unwrap();
    let serialized_release = serde_json::to_value(&release).unwrap();
    assert_eq!(
        serialized_release.get("minimum_supported_version"),
        Some(&serde_json::Value::Null)
    );

    let check = update_check_from_release("1.2.34-chimera.9", release).unwrap();
    let serialized_check = serde_json::to_value(check).unwrap();
    assert_eq!(
        serialized_check.get("minimum_supported_version"),
        Some(&serde_json::Value::Null)
    );
}

#[test]
fn latest_json_accepts_minimum_supported_version_boundaries() {
    for (latest, minimum) in [
        ("1.2.34-chimera.3", "1.2.34-chimera.3"),
        ("1.2.35-chimera.1", "1.2.34-chimera.3"),
    ] {
        let release =
            release_from_latest_json_payload(&chimera_manifest(latest, Some(minimum))).unwrap();
        let serialized_release = serde_json::to_value(&release).unwrap();
        assert_eq!(
            serialized_release["minimum_supported_version"],
            json!(minimum)
        );

        let check = update_check_from_release(minimum, release).unwrap();
        let serialized_check = serde_json::to_value(check).unwrap();
        assert_eq!(
            serialized_check["minimum_supported_version"],
            json!(minimum)
        );
    }
}

#[test]
fn latest_json_rejects_foreign_or_invalid_minimum_supported_version() {
    for invalid in ["1.2.34", "1.2.34-beta.1", "1.2.34-chimera", "not-a-version"] {
        let error =
            release_from_latest_json_payload(&chimera_manifest("1.2.35-chimera.1", Some(invalid)))
                .unwrap_err();
        assert!(
            error.to_string().contains("minimum_supported_version"),
            "unexpected error for {invalid}: {error:#}"
        );
    }

    for invalid in [serde_json::Value::Null, json!(42), json!({})] {
        let mut payload = chimera_manifest("1.2.35-chimera.1", None);
        payload["minimum_supported_version"] = invalid;
        let error = release_from_latest_json_payload(&payload).unwrap_err();
        assert!(
            error.to_string().contains("minimum_supported_version"),
            "unexpected error: {error:#}"
        );
    }
}

#[test]
fn latest_json_rejects_minimum_supported_version_above_latest() {
    let error = release_from_latest_json_payload(&chimera_manifest(
        "1.2.35-chimera.1",
        Some("1.2.35-chimera.2"),
    ))
    .unwrap_err();
    assert!(
        error.to_string().contains("minimum_supported_version"),
        "unexpected error: {error:#}"
    );
}

#[test]
fn release_workflow_emits_minimum_supported_version() {
    let workflow = std::fs::read_to_string("../../.github/workflows/release-assets.yml")
        .expect("read release-assets workflow");

    fn active_release_manifest_contract(workflow: &str) -> bool {
        let Some(publish_job) = workflow.split("\n  publish-release:").nth(1) else {
            return false;
        };
        let Some(create_step) = publish_job
            .split("      - name: Create draft, upload assets, publish")
            .nth(1)
            .and_then(|source| source.split("\n      - name:").next())
        else {
            return false;
        };
        let create_active = create_step
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();
        let mut in_env = false;
        let mut direct_floor_env = false;
        for line in create_step.lines() {
            if line == "        env:" {
                in_env = true;
                continue;
            }
            if !in_env || line.trim().is_empty() {
                continue;
            }
            let indentation = line.len() - line.trim_start().len();
            if indentation <= 8 {
                in_env = false;
                continue;
            }
            if indentation == 10
                && line
                    == "          MINIMUM_SUPPORTED_VERSION: ${{ vars.MINIMUM_SUPPORTED_VERSION }}"
            {
                direct_floor_env = true;
            }
        }
        let all_active = workflow
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect::<Vec<_>>();
        create_active.contains(&"node scripts/release-manifest.mjs --self-test")
            && create_active
                .contains(&"node scripts/release-manifest.mjs --generate release-assets")
            && direct_floor_env
            && all_active
                .iter()
                .filter(|line| {
                    **line == "node scripts/release-manifest.mjs --validate-floor \"$manifest\""
                })
                .count()
                == 2
    }

    assert!(active_release_manifest_contract(&workflow));
    let commented_generate = workflow.replacen(
        "          node scripts/release-manifest.mjs --generate release-assets",
        "          # node scripts/release-manifest.mjs --generate release-assets",
        1,
    );
    assert!(
        !active_release_manifest_contract(&commented_generate),
        "commenting out manifest generation must fail the workflow contract"
    );
    let commented_floor_env = workflow.replacen(
        "          MINIMUM_SUPPORTED_VERSION: ${{ vars.MINIMUM_SUPPORTED_VERSION }}",
        "          # MINIMUM_SUPPORTED_VERSION: ${{ vars.MINIMUM_SUPPORTED_VERSION }}",
        1,
    );
    assert!(
        !active_release_manifest_contract(&commented_floor_env),
        "commenting out the publish-step floor env must fail the workflow contract"
    );
    let scalar_floor_env = workflow.replacen(
        "          MINIMUM_SUPPORTED_VERSION: ${{ vars.MINIMUM_SUPPORTED_VERSION }}",
        "          FLOOR_CONTRACT: |\n            MINIMUM_SUPPORTED_VERSION: ${{ vars.MINIMUM_SUPPORTED_VERSION }}",
        1,
    );
    assert!(
        !active_release_manifest_contract(&scalar_floor_env),
        "floor text inside a block scalar must not satisfy the direct env binding contract"
    );
    assert!(!workflow.contains("const minimumSupportedVersion"));
}

#[test]
fn release_manifest_self_test_runs_in_pr_checks() {
    let workflow = std::fs::read_to_string("../../.github/workflows/pr-build.yml")
        .expect("read pr-build workflow");

    assert!(workflow.contains("node scripts/release-manifest.mjs --self-test"));
}

#[test]
fn trusted_floor_cache_is_atomic_monotonic_and_separate_from_user_settings() {
    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("update-state.json");
    let settings_path = dir.path().join("settings.json");
    std::fs::write(&settings_path, b"user-settings-sentinel").unwrap();
    let store = UpdateStateStore::new(state_path.clone());

    let missing = store.load_trusted_floor().unwrap();
    assert_eq!(missing.status, UpdateCacheStatus::Missing);
    assert!(missing.minimum_supported_version.is_none());

    let first = store.record_trusted_floor("1.2.34-chimera.2").unwrap();
    assert_eq!(first.status, UpdateCacheStatus::Valid);
    assert_eq!(
        first.minimum_supported_version.as_deref(),
        Some("1.2.34-chimera.2")
    );
    let first_bytes = std::fs::read(&state_path).unwrap();

    let rollback = store.record_trusted_floor("1.2.34-chimera.1").unwrap();
    assert_eq!(
        rollback.minimum_supported_version.as_deref(),
        Some("1.2.34-chimera.2")
    );
    assert_eq!(std::fs::read(&state_path).unwrap(), first_bytes);

    let raised = store.record_trusted_floor("1.2.35-chimera.1").unwrap();
    assert_eq!(
        raised.minimum_supported_version.as_deref(),
        Some("1.2.35-chimera.1")
    );
    assert_eq!(
        std::fs::read(&settings_path).unwrap(),
        b"user-settings-sentinel"
    );
    let sidecars = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.contains(".tmp"))
        .collect::<Vec<_>>();
    assert!(
        sidecars.is_empty(),
        "atomic temp files remain: {sidecars:?}"
    );
}

#[test]
fn concurrent_trusted_floor_writers_cannot_roll_back_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let store = std::sync::Arc::new(UpdateStateStore::new(dir.path().join("update-state.json")));
    let revisions = [8_u64, 2, 7, 1, 6, 3, 5, 4];
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(revisions.len() + 1));
    let workers = revisions
        .into_iter()
        .map(|revision| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .record_trusted_floor(&format!("1.2.34-chimera.{revision}"))
                    .unwrap();
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    for worker in workers {
        worker.join().unwrap();
    }

    let final_state = store.load_trusted_floor().unwrap();
    assert_eq!(
        final_state.minimum_supported_version.as_deref(),
        Some("1.2.34-chimera.8")
    );
}

#[test]
fn optional_update_continuation_is_bound_single_use_and_rechecks_the_floor() {
    let dir = tempfile::tempdir().unwrap();
    let state_store = UpdateStateStore::new(dir.path().join("update-state.json"));
    let continuation_store =
        UpdateContinuationStore::new(dir.path().join("update-continuation.json"));
    let current = "1.2.34-chimera.2";

    let token = continuation_store
        .issue(current, None)
        .expect("issue continuation");
    assert!(
        !continuation_store
            .consume_if_supported("forged-token", current, &state_store)
            .unwrap()
    );
    assert!(
        continuation_store
            .consume_if_supported(&token, current, &state_store)
            .unwrap()
    );
    assert!(
        !continuation_store
            .consume_if_supported(&token, current, &state_store)
            .unwrap()
    );

    let raced = continuation_store
        .issue(current, None)
        .expect("issue raced continuation");
    state_store
        .record_trusted_floor("1.2.34-chimera.3")
        .unwrap();
    assert!(
        !continuation_store
            .consume_if_supported(&raced, current, &state_store)
            .unwrap()
    );

    let still_supported = "1.2.34-chimera.4";
    let raised_but_supported = continuation_store
        .issue(still_supported, Some("1.2.34-chimera.3"))
        .expect("issue supported continuation");
    state_store
        .record_trusted_floor("1.2.34-chimera.4")
        .unwrap();
    assert!(
        continuation_store
            .consume_if_supported(&raised_but_supported, still_supported, &state_store)
            .unwrap()
    );

    let boundary_state = UpdateStateStore::new(dir.path().join("boundary-update-state.json"));
    let boundary_continuation =
        UpdateContinuationStore::new(dir.path().join("boundary-update-continuation.json"));
    let boundary_token = boundary_continuation
        .issue(still_supported, None)
        .expect("issue no-floor continuation");
    boundary_state
        .record_trusted_floor(still_supported)
        .unwrap();
    assert!(
        boundary_continuation
            .consume_if_supported(&boundary_token, still_supported, &boundary_state)
            .unwrap()
    );
}

#[test]
fn install_authorization_rechecks_latest_floor_at_launch_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let store = UpdateStateStore::new(dir.path().join("update-state.json"));
    let release = decision_release("1.2.35-chimera.1", None, true);
    store.record_trusted_floor("1.2.35-chimera.2").unwrap();
    let launched = std::sync::atomic::AtomicBool::new(false);

    let error = store
        .authorize_release_install(&release, || {
            launched.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .unwrap_err();

    assert!(error.to_string().contains("below the trusted"));
    assert!(!launched.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn incomplete_native_release_is_rejected_before_floor_can_be_recorded() {
    let mut release = decision_release("1.2.35-chimera.1", Some("1.2.35-chimera.1"), false);
    assert!(validate_release_for_install(&release).is_err());

    release.asset_name = Some("setup.exe".to_string());
    release.asset_url = Some("https://example.test/setup.exe".to_string());
    release.asset_sha256 = Some("ab".repeat(32));
    release.asset_size = Some(42);
    assert!(validate_release_for_install(&release).is_ok());
}

#[test]
fn corrupt_trusted_floor_cache_is_quarantined_without_blocking_startup() {
    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("update-state.json");
    std::fs::write(&state_path, b"{broken-json").unwrap();
    let store = UpdateStateStore::new(state_path.clone());

    let snapshot = store.load_trusted_floor().unwrap();

    assert_eq!(snapshot.status, UpdateCacheStatus::CorruptQuarantined);
    assert!(snapshot.minimum_supported_version.is_none());
    assert!(!state_path.exists());
    let quarantined = snapshot.quarantined_path.expect("quarantine path");
    assert_eq!(std::fs::read(quarantined).unwrap(), b"{broken-json");

    let recovered = store.record_trusted_floor("1.2.34-chimera.3").unwrap();
    assert_eq!(recovered.status, UpdateCacheStatus::Valid);
    assert_eq!(
        recovered.minimum_supported_version.as_deref(),
        Some("1.2.34-chimera.3")
    );
}

#[test]
fn corrupt_cache_quarantine_cannot_displace_a_concurrent_recovery_write() {
    let dir = tempfile::tempdir().unwrap();
    let state_path = dir.path().join("update-state.json");
    std::fs::write(&state_path, b"{broken-json").unwrap();
    let store = std::sync::Arc::new(UpdateStateStore::new(state_path));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));

    let reader = {
        let store = store.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            store.load_trusted_floor().unwrap();
        })
    };
    let writer = {
        let store = store.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            store.record_trusted_floor("1.2.35-chimera.1").unwrap();
        })
    };
    barrier.wait();
    reader.join().unwrap();
    writer.join().unwrap();

    let recovered = store.load_trusted_floor().unwrap();
    assert_eq!(recovered.status, UpdateCacheStatus::Valid);
    assert_eq!(
        recovered.minimum_supported_version.as_deref(),
        Some("1.2.35-chimera.1")
    );
}

#[test]
fn update_decision_covers_none_automatic_mandatory_and_offline_paths() {
    let current = "1.2.34-chimera.2";
    let automatic_release = decision_release("1.2.35-chimera.1", None, true);
    assert_eq!(
        decide_update(current, Some(&automatic_release), None)
            .unwrap()
            .action,
        UpdateAction::Automatic
    );

    let no_asset = decision_release("1.2.35-chimera.1", None, false);
    assert_eq!(
        decide_update(current, Some(&no_asset), None)
            .unwrap()
            .action,
        UpdateAction::None
    );
    assert_eq!(
        decide_update(current, None, None).unwrap().action,
        UpdateAction::None
    );

    assert_eq!(
        decide_update(current, None, Some("1.2.34-chimera.3"))
            .unwrap()
            .action,
        UpdateAction::Mandatory
    );
    assert_eq!(
        decide_update("1.2.34-chimera.3", None, Some("1.2.34-chimera.3"))
            .unwrap()
            .action,
        UpdateAction::None
    );

    let mandatory_no_asset = decision_release("1.2.35-chimera.1", Some("1.2.34-chimera.3"), false);
    assert_eq!(
        decide_update(
            current,
            Some(&mandatory_no_asset),
            mandatory_no_asset.minimum_supported_version.as_deref(),
        )
        .unwrap()
        .action,
        UpdateAction::Mandatory
    );
    assert!(decide_update(current, None, Some("foreign-version")).is_err());
}

#[test]
fn manifest_rollback_below_cached_floor_never_exposes_an_installer() {
    let trusted = TrustedFloorSnapshot {
        minimum_supported_version: Some("1.2.35-chimera.2".to_string()),
        status: UpdateCacheStatus::Valid,
        quarantined_path: None,
    };
    let rollback_release = decision_release("1.2.35-chimera.1", Some("1.2.34-chimera.3"), true);

    let status =
        startup_update_status_from_parts("1.2.34-chimera.2", Some(rollback_release), trusted)
            .unwrap();

    assert_eq!(status.decision.action, UpdateAction::Mandatory);
    assert!(!status.manifest_available);
    assert!(status.check.is_none());
}

#[test]
fn update_check_marks_only_versions_below_floor_as_mandatory() {
    let below = update_check_from_release(
        "1.2.34-chimera.2",
        decision_release("1.2.35-chimera.1", Some("1.2.34-chimera.3"), true),
    )
    .unwrap();
    assert!(below.update_available);
    assert!(below.mandatory_update);

    let boundary = update_check_from_release(
        "1.2.34-chimera.3",
        decision_release("1.2.35-chimera.1", Some("1.2.34-chimera.3"), true),
    )
    .unwrap();
    assert!(boundary.update_available);
    assert!(!boundary.mandatory_update);
}

#[test]
fn platform_install_policies_match_windows_and_unsigned_macos_requirements() {
    let windows = installer_launch_policy(UpdatePlatform::Windows);
    assert_eq!(windows.arguments, ["/S"]);
    assert!(windows.exit_current_process);
    assert!(!windows.requires_user_confirmation);

    let macos = installer_launch_policy(UpdatePlatform::Macos);
    assert_eq!(macos.arguments, ["attach", "-autoopen"]);
    assert!(!macos.exit_current_process);
    assert!(macos.requires_user_confirmation);
}

#[test]
fn latest_json_rejects_missing_checksum_fields() {
    let name = format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-windows-x64-setup.exe");
    let err = release_from_latest_json_payload(&json!({
        "version": "1.2.34-chimera.1",
        "url": "https://github.com/Duojiyi/chimera-codex/releases/tag/v1.2.34-chimera.1",
        "body": "notes",
        "assets": [{
            "name": name,
            "url": branded_release_asset_url("1.2.34-chimera.1", &name)
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
        let expected_name = format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-windows-x64-setup.exe");
        let expected_hash = "11".repeat(32);
        assert_eq!(release.asset_name.as_deref(), Some(expected_name.as_str()));
        assert_eq!(release.asset_size, Some(100));
        assert_eq!(
            release.asset_sha256.as_deref(),
            Some(expected_hash.as_str())
        );
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
fn latest_json_rejects_asset_urls_outside_branded_github_release() {
    let version = "1.2.34-chimera.1";
    let name = format!("{ARTIFACT_PREFIX}-{version}-windows-x64-setup.exe");
    let suffix = format!("/releases/download/v{version}/{name}");
    let invalid_urls = [
        ("http", format!("http://github.com/{REPOSITORY}{suffix}")),
        (
            "userinfo",
            format!("https://attacker@github.com/{REPOSITORY}{suffix}"),
        ),
        (
            "foreign host",
            format!("https://downloads.example.test/{REPOSITORY}{suffix}"),
        ),
        (
            "upstream repository",
            format!("https://github.com/BigPizzaV3/CodexPlusPlus{suffix}"),
        ),
        (
            "latest alias instead of versioned release",
            format!("https://github.com/{REPOSITORY}/releases/latest/download/{name}"),
        ),
    ];

    for (case, url) in invalid_urls {
        let error = release_from_latest_json_payload(&json!({
            "version": version,
            "assets": [{
                "name": name,
                "url": url,
                "sha256": "11".repeat(32),
                "size": 100
            }]
        }))
        .unwrap_err();
        assert!(
            error.to_string().contains("asset url"),
            "{case} produced unexpected error: {error}"
        );
    }
}

#[test]
fn latest_json_rejects_assets_from_a_different_chimera_version() {
    let mut assets = chimera_assets("1.2.34-chimera.1");
    assets[0]["url"] = json!(branded_release_asset_url("1.2.34-chimera.2", "source.zip"));
    let error = release_from_latest_json_payload(&json!({
        "version": "1.2.34-chimera.2",
        "url": "https://github.com/Duojiyi/chimera-codex/releases/tag/v1.2.34-chimera.2",
        "body": "notes",
        "assets": assets
    }))
    .unwrap_err();

    assert!(error.to_string().contains("version"));
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
fn asset_selection_rejects_only_opposite_macos_architecture() {
    let x64 = codex_plus_core::update::ReleaseAsset {
        name: format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-x64.dmg"),
        browser_download_url: "https://example.test/app-x64.dmg".to_string(),
        sha256: "22".repeat(32),
        size: 200,
    };
    let arm64 = codex_plus_core::update::ReleaseAsset {
        name: format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-arm64.dmg"),
        browser_download_url: "https://example.test/app-arm64.dmg".to_string(),
        sha256: "33".repeat(32),
        size: 300,
    };

    assert!(select_update_asset_for_target(&[arm64], "macos", "x86_64").is_none());
    assert!(select_update_asset_for_target(&[x64], "macos", "aarch64").is_none());
}

#[test]
fn asset_selection_rejects_unknown_macos_architecture() {
    let assets = vec![
        codex_plus_core::update::ReleaseAsset {
            name: format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-x64.dmg"),
            browser_download_url: "https://example.test/app-x64.dmg".to_string(),
            sha256: "22".repeat(32),
            size: 200,
        },
        codex_plus_core::update::ReleaseAsset {
            name: format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-macos-arm64.dmg"),
            browser_download_url: "https://example.test/app-arm64.dmg".to_string(),
            sha256: "33".repeat(32),
            size: 300,
        },
    ];

    assert!(select_update_asset_for_target(&assets, "macos", "riscv64").is_none());
}

#[test]
fn latest_json_rejects_malformed_sha256_and_invalid_asset_sizes() {
    let mut malformed_hash = json!({
        "version": "v1.2.34-chimera.1",
        "assets": chimera_assets("1.2.34-chimera.1")
    });
    malformed_hash["assets"][0]["sha256"] = json!("zz");
    assert!(release_from_latest_json_payload(&malformed_hash).is_err());

    let mut zero_size = json!({
        "version": "v1.2.34-chimera.1",
        "assets": chimera_assets("1.2.34-chimera.1")
    });
    zero_size["assets"][0]["size"] = json!(0);
    assert!(release_from_latest_json_payload(&zero_size).is_err());

    let mut excessive_size = json!({
        "version": "v1.2.34-chimera.1",
        "assets": chimera_assets("1.2.34-chimera.1")
    });
    excessive_size["assets"][0]["size"] = json!(2_u64 * 1024 * 1024 * 1024 + 1);
    assert!(release_from_latest_json_payload(&excessive_size).is_err());
}

#[test]
fn safe_asset_name_rejects_path_traversal() {
    assert_eq!(safe_asset_name("pkg.zip").unwrap(), "pkg.zip");
    assert!(safe_asset_name("../pkg.zip").is_err());
    assert!(safe_asset_name("").is_err());
    assert!(safe_asset_name("CON").is_err());
    for reserved in [
        "COM¹.exe",
        "COM²",
        "COM³.bin",
        "LPT¹.dmg",
        "LPT²",
        "LPT³.txt",
        "CONIN$",
        "CONOUT$.txt",
    ] {
        assert!(safe_asset_name(reserved).is_err(), "accepted {reserved}");
    }
    assert!(safe_asset_name("installer.exe:payload").is_err());
    assert!(safe_asset_name("installer.exe. ").is_err());
    assert!(safe_asset_name("installer\n.exe").is_err());
}

#[test]
fn newer_release_without_a_native_asset_is_not_reported_as_available() {
    let release = Release {
        version: "1.2.35-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test/release".to_string(),
        body: "notes".to_string(),
        asset_name: None,
        asset_url: None,
        asset_sha256: None,
        asset_size: None,
    };

    let check = update_check_from_release("1.2.34-chimera.1", release).unwrap();

    assert!(!check.update_available);
    assert!(check.asset_name.is_none());
}

#[cfg(any(unix, windows))]
#[test]
fn dangling_symlink_is_treated_as_an_occupied_asset_name() {
    let dir = tempfile::tempdir().unwrap();
    let dangling = dir.path().join("pkg.bin");
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path().join("missing-target"), &dangling).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(dir.path().join("missing-target"), &dangling).unwrap();
    let bytes = b"verified-installer";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };

    let downloaded = download_asset_to(&release, bytes, dir.path()).unwrap();

    assert_eq!(downloaded, dir.path().join("pkg (1).bin"));
    assert!(dangling.symlink_metadata().is_ok());
}

#[cfg(any(unix, windows))]
#[test]
fn live_symlink_with_matching_bytes_is_never_reused_as_an_installer() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = b"verified-installer";
    let target = dir.path().join("external-target.bin");
    std::fs::write(&target, bytes).unwrap();
    let linked = dir.path().join("pkg.bin");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &linked).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&target, &linked).unwrap();
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };

    let downloaded = download_asset_to(&release, bytes, dir.path()).unwrap();

    assert_eq!(downloaded, dir.path().join("pkg (1).bin"));
    assert!(linked.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read(target).unwrap(), bytes);
}

#[cfg(any(unix, windows))]
#[test]
fn launch_validation_rejects_symlinks_mutation_and_unrelated_paths() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = b"verified-installer";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };
    let installer = dir.path().join("pkg (1).bin");
    std::fs::write(&installer, bytes).unwrap();
    validate_installer_for_launch(&release, &installer).unwrap();

    std::fs::write(&installer, b"mutated-installer").unwrap();
    assert!(validate_installer_for_launch(&release, &installer).is_err());

    let target = dir.path().join("target.bin");
    std::fs::write(&target, bytes).unwrap();
    let linked = dir.path().join("pkg.bin");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &linked).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&target, &linked).unwrap();
    assert!(validate_installer_for_launch(&release, &linked).is_err());
    assert!(validate_installer_for_launch(&release, &target).is_err());
}

#[test]
fn latest_json_rejects_extra_tokens_in_installer_name() {
    let error = release_from_latest_json_payload(&json!({
        "version": "1.2.34-chimera.1",
        "assets": [{
            "name": format!("{ARTIFACT_PREFIX}-1.2.34-chimera.1-untrusted-windows-x64-setup.exe"),
            "url": "https://example.test/setup.exe",
            "sha256": "11".repeat(32),
            "size": 100
        }]
    }))
    .unwrap_err();

    assert!(error.to_string().contains("filename"));
}

#[test]
fn download_asset_to_verifies_sha256_and_size() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = b"abcdef";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
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
    assert_no_part_files(dir.path());
}

#[test]
fn download_asset_to_rejects_wrong_hash_and_cleans_temp() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = b"abcdef";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
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
    assert_no_part_files(dir.path());
}

#[test]
fn download_asset_to_rejects_wrong_size_and_cleans_temp() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = b"abcdef";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
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
    assert_no_part_files(dir.path());
}

#[test]
fn failed_download_does_not_delete_an_existing_complete_asset() {
    let dir = tempfile::tempdir().unwrap();
    let final_path = dir.path().join("pkg.bin");
    std::fs::write(&final_path, b"previous-good-installer").unwrap();
    let bytes = b"corrupt-new-download";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some("00".repeat(32)),
        asset_size: Some(bytes.len() as u64),
    };

    assert!(download_asset_to(&release, bytes, dir.path()).is_err());
    assert_eq!(
        std::fs::read(&final_path).unwrap(),
        b"previous-good-installer"
    );
    assert!(!dir.path().join("pkg.bin.part").exists());
    assert_no_part_files(dir.path());
}

#[test]
fn valid_download_uses_a_unique_name_when_complete_asset_exists() {
    let dir = tempfile::tempdir().unwrap();
    let final_path = dir.path().join("pkg.bin");
    std::fs::write(&final_path, b"previous-good-installer").unwrap();
    let bytes = b"verified-new-installer";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };

    let downloaded = download_asset_to(&release, bytes, dir.path()).unwrap();

    assert_ne!(downloaded, final_path);
    assert_eq!(
        downloaded.extension().and_then(|value| value.to_str()),
        Some("bin")
    );
    assert_eq!(
        std::fs::read(&final_path).unwrap(),
        b"previous-good-installer"
    );
    assert_eq!(std::fs::read(downloaded).unwrap(), bytes);
    assert_no_part_files(dir.path());
}

#[test]
fn matching_existing_asset_is_reused_even_when_new_response_is_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let final_path = dir.path().join("pkg.bin");
    let existing = b"verified-existing-installer";
    std::fs::write(&final_path, existing).unwrap();
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(existing)),
        asset_size: Some(existing.len() as u64),
    };

    let downloaded = download_asset_to(&release, b"truncated", dir.path()).unwrap();

    assert_eq!(downloaded, final_path);
    assert_eq!(std::fs::read(downloaded).unwrap(), existing);
    assert_no_part_files(dir.path());
}

#[test]
fn matching_numbered_asset_is_reused_before_downloading_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("pkg.bin"), b"older-installer").unwrap();
    let verified_path = dir.path().join("pkg (1).bin");
    let verified = b"verified-numbered-installer";
    std::fs::write(&verified_path, verified).unwrap();
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(verified)),
        asset_size: Some(verified.len() as u64),
    };

    let downloaded = download_asset_to(&release, b"corrupt", dir.path()).unwrap();

    assert_eq!(downloaded, verified_path);
    assert_no_part_files(dir.path());
}

#[test]
fn stale_owned_part_file_is_removed_while_holding_the_download_lock() {
    let dir = tempfile::tempdir().unwrap();
    let stale = dir.path().join(".chimera-update.pkg.bin.999999.1.part");
    let stale_other_version = dir
        .path()
        .join(".chimera-update.old-version.exe.999999.2.part");
    let foreign = dir.path().join(".notes.txt.999999.3.part");
    std::fs::write(&stale, b"interrupted").unwrap();
    std::fs::write(&stale_other_version, b"interrupted-old-version").unwrap();
    std::fs::write(&foreign, b"not-owned-by-chimera").unwrap();
    let bytes = b"verified-installer";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };

    download_asset_to(&release, bytes, dir.path()).unwrap();

    assert!(!stale.exists());
    assert!(!stale_other_version.exists());
    assert_eq!(std::fs::read(foreign).unwrap(), b"not-owned-by-chimera");
}

#[cfg(unix)]
#[test]
fn stale_cleanup_preserves_non_utf8_lookalike_files() {
    use std::os::unix::ffi::OsStringExt as _;

    let dir = tempfile::tempdir().unwrap();
    let mut name = b".chimera-update.pkg.bin.999999.".to_vec();
    name.push(0xff);
    name.extend_from_slice(b".part");
    let foreign = dir.path().join(std::ffi::OsString::from_vec(name));
    std::fs::write(&foreign, b"foreign-non-utf8").unwrap();
    let bytes = b"verified-installer";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };

    download_asset_to(&release, bytes, dir.path()).unwrap();

    assert_eq!(std::fs::read(foreign).unwrap(), b"foreign-non-utf8");
}

#[test]
fn update_directory_retains_at_most_three_owned_installers() {
    let dir = tempfile::tempdir().unwrap();
    for revision in 1..=4 {
        let name = format!("{ARTIFACT_PREFIX}-1.2.34-chimera.{revision}-windows-x64-setup.exe");
        std::fs::write(dir.path().join(name), format!("old-{revision}")).unwrap();
    }
    let bytes = b"current-verified-installer";
    let current_name = format!("{ARTIFACT_PREFIX}-1.2.35-chimera.1-windows-x64-setup.exe");
    let release = Release {
        version: "1.2.35-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some(current_name),
        asset_url: Some("https://example.test/current.exe".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };

    let current = download_asset_to(&release, bytes, dir.path()).unwrap();
    let retained = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_file())
                && entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(ARTIFACT_PREFIX)
        })
        .count();

    assert!(current.is_file());
    assert!(retained <= 3, "retained {retained} owned installers");
}

#[tokio::test]
async fn streamed_asset_download_rejects_declared_and_chunked_overflow_and_cleans_temp() {
    let dir = tempfile::tempdir().unwrap();
    let expected = b"abcd";
    let release_for = |url: String| Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some(url),
        asset_sha256: Some(sha256_hex(expected)),
        asset_size: Some(expected.len() as u64),
    };

    let (url, server) = serve_http_once(
        b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nabcdefgh".to_vec(),
    );
    let error =
        download_release_asset_with_client(&release_for(url), dir.path(), &loopback_http_client())
            .await
            .unwrap_err();
    server.join().unwrap();
    assert!(error.to_string().contains("size"));
    assert_no_part_files(dir.path());

    let (url, server) = serve_http_once(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\nabcd\r\n4\r\nefgh\r\n0\r\n\r\n".to_vec(),
    );
    let error =
        download_release_asset_with_client(&release_for(url), dir.path(), &loopback_http_client())
            .await
            .unwrap_err();
    server.join().unwrap();
    assert!(error.to_string().contains("size"));
    assert_no_part_files(dir.path());
}

#[tokio::test]
async fn streamed_asset_download_accepts_an_exact_bounded_response() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = b"verified-stream";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )
    .into_bytes()
    .into_iter()
    .chain(bytes.iter().copied())
    .collect();
    let (url, server) = serve_http_once(response);
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some(url),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };

    let path = download_release_asset_with_client(&release, dir.path(), &loopback_http_client())
        .await
        .unwrap();
    server.join().unwrap();

    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert_no_part_files(dir.path());
}

#[tokio::test]
async fn latest_json_response_is_bounded_before_json_decode() {
    let declared = MAX_LATEST_JSON_BYTES + 1;
    let response =
        format!("HTTP/1.1 200 OK\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n{{}}")
            .into_bytes();
    let (url, server) = serve_http_once(response);

    let error = fetch_latest_release_with_client(&url, &loopback_http_client())
        .await
        .unwrap_err();
    server.join().unwrap();

    assert!(error.to_string().contains("latest.json"));
    assert!(error.to_string().contains("size"));
}

#[tokio::test]
async fn stalled_manifest_and_asset_requests_respect_timeouts() {
    let timeout = std::time::Duration::from_millis(50);
    let (url, server) = serve_stalled_http_once(std::time::Duration::from_millis(200));
    let manifest_error =
        fetch_latest_release_with_client_and_timeout(&url, &loopback_http_client(), timeout)
            .await
            .unwrap_err();
    server.join().unwrap();
    assert!(
        manifest_error.to_string().contains("timed out")
            || manifest_error.to_string().contains("timeout")
    );

    let dir = tempfile::tempdir().unwrap();
    let bytes = b"expected";
    let (url, server) = serve_stalled_http_once(std::time::Duration::from_millis(200));
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some(url),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };
    let asset_error = download_release_asset_with_client_and_timeouts(
        &release,
        dir.path(),
        &loopback_http_client(),
        timeout,
        std::time::Duration::from_secs(1),
    )
    .await
    .unwrap_err();
    server.join().unwrap();
    assert!(
        asset_error.to_string().contains("timed out")
            || asset_error.to_string().contains("timeout")
    );
    assert_no_part_files(dir.path());
}

#[tokio::test]
async fn async_download_lock_wait_is_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let lock_path = dir.path().join(".chimera-update.lock");
    let held_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(lock_path)
        .unwrap();
    held_lock.lock_exclusive().unwrap();
    let bytes = b"expected";
    let release = Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("http://127.0.0.1:9/not-requested".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };

    let error = download_release_asset_with_client_and_timeouts(
        &release,
        dir.path(),
        &loopback_http_client(),
        std::time::Duration::from_secs(1),
        std::time::Duration::from_millis(50),
    )
    .await
    .unwrap_err();
    drop(held_lock);

    let message = error.to_string();
    assert!(message.contains("lock"), "{message}");
    assert!(message.contains("timeout"), "{message}");
}

#[test]
fn concurrent_downloads_never_publish_bytes_from_another_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let download_dir = dir.path().to_path_buf();
    let bytes_a = vec![b'a'; 2 * 1024 * 1024];
    let bytes_b = vec![b'b'; 2 * 1024 * 1024];
    let release = |bytes: &[u8]| Release {
        version: "1.2.34-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://example.test".to_string(),
        body: "fixes".to_string(),
        asset_name: Some("pkg.bin".to_string()),
        asset_url: Some("https://example.test/pkg.bin".to_string()),
        asset_sha256: Some(sha256_hex(bytes)),
        asset_size: Some(bytes.len() as u64),
    };
    let release_a = release(&bytes_a);
    let release_b = release(&bytes_b);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));

    let worker = |release: Release, bytes: Vec<u8>| {
        let barrier = barrier.clone();
        let download_dir = download_dir.clone();
        std::thread::spawn(move || {
            barrier.wait();
            let path = download_asset_to(&release, &bytes, &download_dir).unwrap();
            (path, bytes)
        })
    };
    let first = worker(release_a, bytes_a);
    let second = worker(release_b, bytes_b);
    barrier.wait();
    let (path_a, expected_a) = first.join().unwrap();
    let (path_b, expected_b) = second.join().unwrap();

    assert_ne!(path_a, path_b);
    assert_eq!(std::fs::read(path_a).unwrap(), expected_a);
    assert_eq!(std::fs::read(path_b).unwrap(), expected_b);
    assert_no_part_files(dir.path());
}

#[test]
fn update_request_binding_returns_only_the_trusted_newer_release() {
    let trusted = Release {
        version: "1.2.35-chimera.1".to_string(),
        minimum_supported_version: None,
        url: "https://trusted.example/release".to_string(),
        body: "trusted".to_string(),
        asset_name: Some("trusted.exe".to_string()),
        asset_url: Some("https://trusted.example/trusted.exe".to_string()),
        asset_sha256: Some("ab".repeat(32)),
        asset_size: Some(42),
    };

    let bound =
        validate_update_request("v1.2.35-chimera.1", trusted.clone(), "1.2.34-chimera.9").unwrap();

    assert_eq!(bound, trusted);
}

#[test]
fn update_request_binding_rejects_version_drift_same_version_and_downgrade() {
    let release = |version: &str| Release {
        version: version.to_string(),
        minimum_supported_version: None,
        url: "https://trusted.example/release".to_string(),
        body: "trusted".to_string(),
        asset_name: Some("trusted.exe".to_string()),
        asset_url: Some("https://trusted.example/trusted.exe".to_string()),
        asset_sha256: Some("ab".repeat(32)),
        asset_size: Some(42),
    };

    assert!(
        validate_update_request(
            "1.2.36-chimera.1",
            release("1.2.35-chimera.1"),
            "1.2.34-chimera.1"
        )
        .is_err()
    );
    assert!(
        validate_update_request(
            "1.2.35-chimera.1",
            release("1.2.35-chimera.1"),
            "1.2.35-chimera.1"
        )
        .is_err()
    );
    assert!(
        validate_update_request(
            "1.2.34-chimera.9",
            release("1.2.34-chimera.9"),
            "1.2.35-chimera.1"
        )
        .is_err()
    );
    assert!(
        validate_update_request(
            "not-a-version",
            release("1.2.35-chimera.1"),
            "1.2.34-chimera.1"
        )
        .is_err()
    );
}

#[test]
fn verify_downloaded_bytes_accepts_matching_digest() {
    let bytes = b"hello-chimera";
    verify_downloaded_bytes(bytes, &sha256_hex(bytes), bytes.len() as u64).unwrap();
}
