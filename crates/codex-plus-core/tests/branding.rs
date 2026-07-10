use codex_plus_core::branding::{
    ADS_ENABLED, ARTIFACT_PREFIX, DEFAULT_RELAY_BASE_URL, DEFAULT_RELAY_MODEL,
    DISPLAY_MANAGER_NAME, DISPLAY_SILENT_NAME, LATEST_JSON_URL, PUBLISHER, REPOSITORY,
};

#[test]
fn public_chimera_branding_does_not_point_at_upstream_release() {
    assert!(!ADS_ENABLED);
    assert_eq!(DISPLAY_SILENT_NAME, "Chimera Codex");
    assert_eq!(DISPLAY_MANAGER_NAME, "Chimera Codex 管理工具");
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
    assert_eq!(ARTIFACT_PREFIX, "ChimeraCodex");
}
