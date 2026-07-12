use codex_plus_core::branding::{
    ARTIFACT_PREFIX, DEFAULT_RELAY_BASE_URL, DEFAULT_RELAY_MODEL, DISPLAY_MANAGER_NAME,
    DISPLAY_SILENT_NAME, LATEST_JSON_URL, PUBLISHER, REPOSITORY,
};

#[test]
fn public_chimera_branding_does_not_point_at_upstream_release() {
    assert_eq!(DISPLAY_SILENT_NAME, "Chimera++");
    assert_eq!(DISPLAY_MANAGER_NAME, "Chimera++ 管理工具");
    assert_eq!(PUBLISHER, "ChimeraHub");
    assert!(!REPOSITORY.contains("TBD"));
    assert!(!REPOSITORY.contains("chimera-org"));
    assert!(!REPOSITORY.contains("example"));
    assert_eq!(REPOSITORY, "Duojiyi/chimera-codex");
    assert!(LATEST_JSON_URL.contains(REPOSITORY));
    assert!(!LATEST_JSON_URL.contains("BigPizzaV3/CodexPlusPlus"));
    assert_eq!(
        LATEST_JSON_URL,
        "https://github.com/Duojiyi/chimera-codex/releases/latest/download/latest.json"
    );
    assert_eq!(DEFAULT_RELAY_BASE_URL, "https://api.chimerahub.org/v1");
    assert_eq!(DEFAULT_RELAY_MODEL, "gpt-5.5");
    assert_eq!(ARTIFACT_PREFIX, "ChimeraPlusPlus");
    assert_eq!(
        codex_plus_core::http_client::branded_user_agent(""),
        format!("ChimeraPlusPlus/{}", codex_plus_core::version::VERSION)
    );
    assert_eq!(
        codex_plus_core::http_client::branded_user_agent("RelayTest"),
        "ChimeraPlusPlus/RelayTest"
    );
}

#[test]
fn branding_source_has_no_ads_feature_toggle() {
    for (path, source) in [
        (
            "brand/product.toml",
            include_str!("../../../brand/product.toml"),
        ),
        (
            "scripts/generate-branding.ps1",
            include_str!("../../../scripts/generate-branding.ps1"),
        ),
        ("src/branding.rs", include_str!("../src/branding.rs")),
        (
            "apps/codex-plus-manager/src/branding.generated.ts",
            include_str!("../../../apps/codex-plus-manager/src/branding.generated.ts"),
        ),
    ] {
        assert!(
            !source.contains("ads_enabled"),
            "ads_enabled remains in {path}"
        );
        assert!(
            !source.contains("ADS_ENABLED"),
            "ADS_ENABLED remains in {path}"
        );
    }
}

#[test]
fn production_rust_touchpoints_do_not_expose_the_previous_display_brand() {
    for (path, source) in [
        ("launcher.rs", include_str!("../src/launcher.rs")),
        ("zed_remote.rs", include_str!("../src/zed_remote.rs")),
        ("install/mod.rs", include_str!("../src/install/mod.rs")),
        ("watcher.rs", include_str!("../src/watcher.rs")),
        ("stepwise.rs", include_str!("../src/stepwise.rs")),
        ("update.rs", include_str!("../src/update.rs")),
        ("http_client.rs", include_str!("../src/http_client.rs")),
        ("model_catalog.rs", include_str!("../src/model_catalog.rs")),
        ("relay_config.rs", include_str!("../src/relay_config.rs")),
        (
            "protocol_proxy.rs",
            include_str!("../src/protocol_proxy.rs"),
        ),
        ("ads.rs", include_str!("../src/ads.rs")),
        (
            "plugin_marketplace.rs",
            include_str!("../src/plugin_marketplace.rs"),
        ),
        (
            "apps/codex-plus-mobile-relay/src/main.rs",
            include_str!("../../../apps/codex-plus-mobile-relay/src/main.rs"),
        ),
        (
            "crates/codex-plus-data/src/provider_sync.rs",
            include_str!("../../../crates/codex-plus-data/src/provider_sync.rs"),
        ),
        (
            ".github/ISSUE_TEMPLATE/bug_report.yml",
            include_str!("../../../.github/ISSUE_TEMPLATE/bug_report.yml"),
        ),
        ("CONTRIBUTING.md", include_str!("../../../CONTRIBUTING.md")),
    ] {
        assert!(
            !source.contains("Chimera Codex"),
            "legacy display brand remains in {path}"
        );
        assert!(
            !source.contains("ChimeraCodex"),
            "legacy client brand token remains in {path}"
        );
        assert!(
            !source.contains("CodexPlusPlus/"),
            "legacy client user-agent remains in {path}"
        );
        assert!(
            !source.contains("proxied_client(\"CodexPlusPlus\")"),
            "legacy default client user-agent remains in {path}"
        );
        assert!(
            !source.contains("\"managedBy\": \"Codex++ provider sync\""),
            "legacy provider-sync marker is still written in {path}"
        );
    }
}
