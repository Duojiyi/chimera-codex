// Task 1.x RED — Tauri command DTO contract.
//
// The frontend calls invoke() and destructures camelCase fields. If the Rust
// DTO serialises snake_case, every field reads `undefined` and the UI silently
// shows empty state — the exact failure mode that made the app look "wired up"
// while doing nothing. These tests pin the wire contract.

use chimera_desktop_lib::dto::{ProviderDto, ProviderTestDto, SkinDto, SystemStatusDto};

#[test]
fn system_status_serialises_camel_case_for_frontend() {
    let dto = SystemStatusDto {
        provider_name: Some("ChimeraHub".into()),
        active_provider_id: Some("provider-1".into()),
        provider_health: "healthy".into(),
        codex_version: Some("26.721".into()),
        codex_running: true,
        official_mode: false,
    };
    let json = serde_json::to_string(&dto).unwrap();

    // Frontend reads status.providerName / status.codexVersion / status.officialMode
    assert!(json.contains("\"providerName\""), "got {json}");
    assert!(json.contains("\"providerHealth\""), "got {json}");
    assert!(json.contains("\"codexVersion\""), "got {json}");
    assert!(json.contains("\"codexRunning\""), "got {json}");
    assert!(json.contains("\"officialMode\""), "got {json}");
    // and must NOT emit snake_case
    assert!(!json.contains("provider_name"), "snake_case leaked: {json}");
}

#[test]
fn system_status_official_mode_has_null_provider_name() {
    let dto = SystemStatusDto {
        provider_name: None,
        active_provider_id: None,
        provider_health: "unknown".into(),
        codex_version: None,
        codex_running: false,
        official_mode: true,
    };
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("\"providerName\":null"), "got {json}");
    assert!(json.contains("\"officialMode\":true"), "got {json}");
}

#[test]
fn provider_dto_never_serialises_a_secret() {
    // G4: neither the API key nor its keychain handle may cross the IPC boundary.
    // and it is an opaque keychain handle, not the key.
    let dto = ProviderDto {
        id: "11111111-1111-4111-8111-111111111111".into(),
        display_name: "Work API".into(),
        kind: "custom".into(),
        base_url: "https://api.example.com/v1".into(),
        health: "healthy".into(),
        selected_model: Some("gpt-4o".into()),
    };
    let json = serde_json::to_string(&dto).unwrap();

    assert!(json.contains("\"displayName\""), "got {json}");
    assert!(json.contains("\"baseUrl\""), "got {json}");
    // No secret-bearing field may exist on the wire at all.
    assert!(!json.contains("apiKey"), "api key on the wire: {json}");
    assert!(!json.contains("api_key"), "api key on the wire: {json}");
    assert!(
        !json.contains("secretRef"),
        "secret ref on the wire: {json}"
    );
    assert!(!json.contains("secret"), "secret on the wire: {json}");
}

#[test]
fn test_result_dto_carries_actionable_message_not_raw_error() {
    let dto = ProviderTestDto {
        ok: false,
        health: "auth_failed".into(),
        message: "Authentication failed. Check that the API key is correct and active.".into(),
        discovered_models: vec![],
    };
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("\"discoveredModels\""), "got {json}");
    assert!(
        !json.contains("discovered_models"),
        "snake_case leaked: {json}"
    );
    // Message must be human-actionable, not a Rust Debug string.
    assert!(!dto.message.contains("Error {"), "raw Debug leaked");
    assert!(!dto.message.starts_with("reqwest::"), "raw error leaked");
}

#[test]
fn skin_dto_serialises_camel_case() {
    let dto = SkinDto {
        id: "default".into(),
        name: "Default".into(),
        description: "No modifications to official app files".into(),
        applied: true,
        is_default: true,
    };
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("\"isDefault\""), "got {json}");
    assert!(!json.contains("is_default"), "snake_case leaked: {json}");
}
