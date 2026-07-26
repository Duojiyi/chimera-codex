// Step 7.1 RED — read-only inventory of a Chimera++ 1.x install.
//
// 1.x settings.json shapes are NOT invented here: the field names below are
// copied from crates/codex-plus-core/src/settings.rs (`BackendSettings`,
// `RelayProfile`) and crates/codex-plus-core/src/paths.rs (`APP_STATE_DIR`).
// Two historical profile shapes exist in the wild and both must keep working:
//   - old: flat `baseUrl` / `apiKey` fields (kept only via `deserialize_with`,
//     `skip_serializing` — 1.x stopped writing them but must still read them)
//   - new: `upstreamBaseUrl` + the key folded into `authContents` (JSON) or
//     `configContents` (TOML `experimental_bearer_token`, the "official mix"
//     path — see `preserve_official_mix_bearer_tokens` in settings.rs)
//
// G2/N1-N6: nothing here may ever open the source for writing, and none of
// the advanced 1.x sections (MCP/skills/plugin context sync, protocol
// conversion, plugin marketplace unlock, user scripts, session DB, watcher)
// may reach the inventoried provider/settings fields — only their *presence*
// is reported, for transparency, never their content.

use chimera_migration::legacy_source::{
    DroppedFeature, LegacyAuxiliaryMarkers, LegacyProtocol, LegacyReadError, LegacySourcePaths,
    read_legacy_inventory, resolve_legacy_settings_path,
};
use serde_json::json;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn write_settings(dir: &Path, value: &serde_json::Value) -> LegacySourcePaths {
    let path = dir.join("settings.json");
    fs::write(&path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    LegacySourcePaths::new(path)
}

// ── path resolution (pure) ───────────────────────────────────────────────────

#[test]
fn resolve_legacy_settings_path_matches_the_1x_app_state_directory_layout() {
    let home = Path::new("/home/example-user");
    let resolved = resolve_legacy_settings_path(home);
    assert_eq!(
        resolved,
        Path::new("/home/example-user/.codex-session-delete/settings.json")
    );
}

// ── absent source ────────────────────────────────────────────────────────────

#[test]
fn absent_settings_file_yields_empty_inventory_not_an_error() {
    let dir = tempdir().unwrap();
    let paths = LegacySourcePaths::new(dir.path().join("does-not-exist.json"));

    let inventory = read_legacy_inventory(&paths).expect("an absent 1.x install is not an error");

    assert!(inventory.providers.is_empty());
    assert!(inventory.active_source_id.is_none());
}

// ── settings / profile / active / key (golden paths) ────────────────────────

#[test]
fn reads_multiple_relay_profiles_with_display_name_and_base_url() {
    let dir = tempdir().unwrap();
    let paths = write_settings(
        dir.path(),
        &json!({
            "activeRelayId": "one",
            "relayProfiles": [
                {"id": "one", "name": "One", "upstreamBaseUrl": "https://one.example/v1", "apiKey": "key-one"},
                {"id": "two", "name": "Two", "upstreamBaseUrl": "https://two.example/v1", "apiKey": "key-two"},
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();

    assert_eq!(inventory.providers.len(), 2);
    assert_eq!(inventory.providers[0].display_name, "One");
    assert_eq!(inventory.providers[0].base_url, "https://one.example/v1");
    assert_eq!(inventory.providers[1].display_name, "Two");
}

#[test]
fn marks_the_profile_named_by_active_relay_id_as_active() {
    let dir = tempdir().unwrap();
    let paths = write_settings(
        dir.path(),
        &json!({
            "activeRelayId": "two",
            "relayProfiles": [
                {"id": "one", "name": "One", "upstreamBaseUrl": "https://one.example/v1"},
                {"id": "two", "name": "Two", "upstreamBaseUrl": "https://two.example/v1"},
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();

    assert!(!inventory.providers[0].is_active);
    assert!(inventory.providers[1].is_active);
    assert_eq!(inventory.active_source_id.as_deref(), Some("two"));
}

#[test]
fn unmatched_active_relay_id_leaves_nothing_marked_active() {
    let dir = tempdir().unwrap();
    let paths = write_settings(
        dir.path(),
        &json!({
            "activeRelayId": "ghost",
            "relayProfiles": [
                {"id": "one", "name": "One", "upstreamBaseUrl": "https://one.example/v1"},
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();

    assert!(!inventory.providers[0].is_active);
}

#[test]
fn key_is_read_from_the_old_flat_api_key_field() {
    // Old historical shape: `baseUrl` + `apiKey` directly on the profile.
    let dir = tempdir().unwrap();
    let paths = write_settings(
        dir.path(),
        &json!({
            "relayProfiles": [
                {"id": "old", "name": "Old Shape", "baseUrl": "https://old.example/v1", "apiKey": "sk-old-flat-key"},
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();

    assert_eq!(inventory.providers[0].base_url, "https://old.example/v1");
    assert!(inventory.providers[0].has_key());
    assert_eq!(inventory.providers[0].reveal_key(), Some("sk-old-flat-key"));
}

#[test]
fn key_is_read_from_experimental_bearer_token_when_api_key_field_is_empty() {
    // New historical shape: official-mix profiles fold the bearer token into
    // `configContents` TOML instead of a flat `apiKey` (settings.rs
    // `set_or_replace_experimental_bearer_token`).
    let dir = tempdir().unwrap();
    let config_contents = concat!(
        "model_provider = \"codex-plus-relay\"\n\n",
        "[model_providers.codex-plus-relay]\n",
        "name = \"codex-plus-relay\"\n",
        "experimental_bearer_token = \"sk-from-toml\"\n",
    );
    let paths = write_settings(
        dir.path(),
        &json!({
            "relayProfiles": [
                {
                    "id": "mix",
                    "name": "Official Mix",
                    "upstreamBaseUrl": "https://mix.example/v1",
                    "apiKey": "",
                    "configContents": config_contents,
                },
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();

    assert_eq!(inventory.providers[0].reveal_key(), Some("sk-from-toml"));
}

#[test]
fn key_is_read_from_auth_contents_openai_api_key_when_others_are_absent() {
    let dir = tempdir().unwrap();
    let paths = write_settings(
        dir.path(),
        &json!({
            "relayProfiles": [
                {
                    "id": "authed",
                    "name": "Authed",
                    "upstreamBaseUrl": "https://authed.example/v1",
                    "authContents": "{\"OPENAI_API_KEY\": \"sk-from-auth-json\"}",
                },
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();

    assert_eq!(
        inventory.providers[0].reveal_key(),
        Some("sk-from-auth-json")
    );
}

#[test]
fn profile_with_no_key_anywhere_is_still_inventoried_with_has_key_false() {
    let dir = tempdir().unwrap();
    let paths = write_settings(
        dir.path(),
        &json!({
            "relayProfiles": [
                {"id": "nokey", "name": "No Key", "upstreamBaseUrl": "https://nokey.example/v1"},
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();

    assert_eq!(
        inventory.providers.len(),
        1,
        "a keyless profile is not dropped"
    );
    assert!(!inventory.providers[0].has_key());
    assert_eq!(inventory.providers[0].reveal_key(), None);
}

// ── protocol: only Responses migrates automatically (chimera-provider only
// commits to OpenAI Responses — probe.rs "v2.0.0 only promises Responses") ──

#[test]
fn chat_completions_protocol_profile_is_flagged_not_silently_treated_as_responses() {
    let dir = tempdir().unwrap();
    let paths = write_settings(
        dir.path(),
        &json!({
            "relayProfiles": [
                {"id": "cc", "name": "Chat", "upstreamBaseUrl": "https://chat.example/v1", "protocol": "chatCompletions"},
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();

    assert_eq!(
        inventory.providers[0].protocol,
        LegacyProtocol::ChatCompletions
    );
    assert!(
        inventory
            .dropped_features
            .contains(&DroppedFeature::ProtocolConversion),
        "a chat-completions profile must surface as a dropped-capability warning, \
         not be silently imported as if it spoke Responses"
    );
}

// ── corrupt / wrong-shape samples — fail closed ─────────────────────────────

#[test]
fn truncated_json_settings_file_is_refused_not_guessed() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, b"{\"relayProfiles\": [ { \"id\": \"a\"").unwrap();
    let paths = LegacySourcePaths::new(path);

    let result = read_legacy_inventory(&paths);

    assert!(
        matches!(result, Err(LegacyReadError::Corrupt(_))),
        "expected Corrupt, got {result:?}"
    );
}

#[test]
fn settings_file_that_is_a_json_array_is_refused_as_unknown_shape() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, b"[1, 2, 3]").unwrap();
    let paths = LegacySourcePaths::new(path);

    let result = read_legacy_inventory(&paths);

    assert!(matches!(result, Err(LegacyReadError::Corrupt(_))));
}

#[test]
fn relay_profiles_field_with_wrong_type_is_skipped_with_a_warning_not_a_crash() {
    let dir = tempdir().unwrap();
    let paths = write_settings(dir.path(), &json!({"relayProfiles": "not-a-list"}));

    let inventory = read_legacy_inventory(&paths).expect("a wrong-typed field must not be fatal");

    assert!(inventory.providers.is_empty());
    assert!(!inventory.warnings.is_empty());
}

#[test]
fn a_profile_missing_any_base_url_is_skipped_with_a_warning() {
    let dir = tempdir().unwrap();
    let paths = write_settings(
        dir.path(),
        &json!({
            "relayProfiles": [
                {"id": "no-url", "name": "No URL"},
                {"id": "has-url", "name": "Has URL", "upstreamBaseUrl": "https://ok.example/v1"},
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).expect("per-profile problems degrade, not fail");

    assert_eq!(inventory.providers.len(), 1);
    assert_eq!(inventory.providers[0].source_id, "has-url");
    assert!(
        inventory
            .warnings
            .iter()
            .any(|w| w.contains("no-url") || w.to_lowercase().contains("base url")),
        "expected a warning about the skipped profile, got {:?}",
        inventory.warnings
    );
}

#[test]
fn a_profile_entry_that_is_not_an_object_is_skipped_with_a_warning() {
    let dir = tempdir().unwrap();
    let paths = write_settings(
        dir.path(),
        &json!({
            "relayProfiles": [
                "just-a-string",
                {"id": "ok", "name": "OK", "upstreamBaseUrl": "https://ok.example/v1"},
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();

    assert_eq!(inventory.providers.len(), 1);
    assert_eq!(inventory.providers[0].source_id, "ok");
}

// ── N1-N6: detected for transparency, never carried into the output ────────

#[test]
fn dropped_feature_sections_are_detected_but_their_payloads_never_reach_output() {
    let dir = tempdir().unwrap();
    const SENTINEL: &str = "SENTINEL_MUST_NOT_MIGRATE";
    let paths = write_settings(
        dir.path(),
        &json!({
            "activeRelayId": "one",
            "codexAppPluginMarketplaceUnlock": true,
            "aggregateRelayProfiles": [
                {"id": "agg", "name": "Aggregate", "strategy": "failover", "members": []}
            ],
            "relayContextConfigContents": SENTINEL,
            "relayProfiles": [
                {
                    "id": "one",
                    "name": "One",
                    "upstreamBaseUrl": "https://one.example/v1",
                    "apiKey": "key-one",
                    "contextSelection": {
                        "mcpServers": [SENTINEL],
                        "skills": [SENTINEL],
                        "plugins": [SENTINEL]
                    }
                },
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();

    assert!(
        inventory
            .dropped_features
            .contains(&DroppedFeature::McpSkillsAndPluginContextSync)
    );
    assert!(
        inventory
            .dropped_features
            .contains(&DroppedFeature::ProtocolConversion)
    );
    assert!(
        inventory
            .dropped_features
            .contains(&DroppedFeature::PluginMarketplaceUnlock)
    );

    // The provider itself must carry only display_name/base_url/key/active —
    // never the sentinel that lived in the dropped sections.
    let provider = &inventory.providers[0];
    assert_eq!(provider.display_name, "One");
    assert_eq!(provider.base_url, "https://one.example/v1");
    assert_ne!(provider.display_name, SENTINEL);
    assert_ne!(provider.base_url, SENTINEL);

    let rendered = format!("{inventory:?}");
    assert!(
        !rendered.contains(SENTINEL),
        "a dropped 1.x section's payload leaked into the inventory: {rendered}"
    );
}

#[test]
fn auxiliary_markers_are_detected_by_presence_only_never_by_content() {
    let dir = tempdir().unwrap();
    let user_scripts = dir.path().join("user-scripts.json");
    let watcher_flag = dir.path().join("watcher.disabled");
    let session_db = dir.path().join("codex-sessions.sqlite");
    // Deliberately unparseable — proves detection never reads the content.
    fs::write(&user_scripts, b"\x00not json at all").unwrap();
    fs::write(&watcher_flag, b"").unwrap();
    fs::write(&session_db, b"\x00\x01sqlite-garbage").unwrap();

    let paths = write_settings(dir.path(), &json!({})).with_auxiliary(LegacyAuxiliaryMarkers {
        user_scripts_config_path: Some(user_scripts),
        watcher_disabled_flag_path: Some(watcher_flag),
        session_database_paths: vec![session_db],
    });

    let inventory =
        read_legacy_inventory(&paths).expect("garbage auxiliary content must never be fatal");

    assert!(
        inventory
            .dropped_features
            .contains(&DroppedFeature::UserScripts)
    );
    assert!(
        inventory
            .dropped_features
            .contains(&DroppedFeature::Watcher)
    );
    assert!(
        inventory
            .dropped_features
            .contains(&DroppedFeature::SessionDatabase)
    );
}

// ── secrets never leak via Debug ────────────────────────────────────────────

#[test]
fn debug_output_of_the_inventory_never_contains_a_raw_api_key() {
    let dir = tempdir().unwrap();
    const SECRET: &str = "sk-must-never-be-printed-1234567890";
    let paths = write_settings(
        dir.path(),
        &json!({
            "relayProfiles": [
                {"id": "one", "name": "One", "upstreamBaseUrl": "https://one.example/v1", "apiKey": SECRET},
            ]
        }),
    );

    let inventory = read_legacy_inventory(&paths).unwrap();
    let rendered = format!("{inventory:?}");

    assert!(
        !rendered.contains(SECRET),
        "Debug output of LegacyInventory must never contain the raw API key"
    );
    // The accessor still exists for the one caller allowed to read it (the
    // keychain-write step in migrate.rs) — redaction is about Debug, not access.
    assert_eq!(inventory.providers[0].reveal_key(), Some(SECRET));
}
