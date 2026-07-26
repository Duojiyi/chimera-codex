// Step 7.2 RED — CC Switch (farion1231/cc-switch) Codex-provider import.
//
// THIRD_PARTY_SOURCES.md registers cc-switch's "Provider URL and API key data
// model" as adopted-by-reference. The per-provider payload shape below
// (baseUrl/apiKey/apiFormat/config/auth) is copied from the ALREADY-SHIPPED
// 1.x importer at crates/codex-plus-core/src/ccs_import.rs, which parses
// exactly this JSON blob (there, out of a SQLite column; the outer container
// modelled here is a JSON file under the user profile, per this task's design
// notes — see this crate's report for the explicit call-out that the outer
// container shape should be confirmed against a real CC Switch install
// before this reader is wired into the desktop shell).
//
// This importer must never write to CC Switch's own config — every test
// below only ever calls `std::fs::read`/`write` on paths under a tempdir
// that stands in for CC Switch's file, and `read_ccswitch_inventory` itself
// has no write path to that file at all.

use chimera_migration::ccswitch_source::{
    CcSwitchReadError, CcSwitchSourcePaths, read_ccswitch_inventory,
};
use chimera_migration::legacy_source::LegacyProtocol;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn write_config(dir: &tempfile::TempDir, value: &serde_json::Value) -> CcSwitchSourcePaths {
    let path = dir.path().join("cc-switch-config.json");
    fs::write(&path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    CcSwitchSourcePaths::new(path)
}

// ── absent source ────────────────────────────────────────────────────────────

#[test]
fn absent_config_file_yields_none_not_an_error() {
    let dir = tempdir().unwrap();
    let paths = CcSwitchSourcePaths::new(dir.path().join("does-not-exist.json"));

    let result =
        read_ccswitch_inventory(&paths).expect("CC Switch never having run is not an error");

    assert!(result.is_none());
}

#[test]
fn cc_switch_installed_with_no_codex_providers_yields_an_empty_inventory() {
    let dir = tempdir().unwrap();
    let paths = write_config(
        &dir,
        &json!({"apps": {"claude": {"providers": {}, "current": null}}}),
    );

    let inventory = read_ccswitch_inventory(&paths)
        .unwrap()
        .expect("apps key present");

    assert!(inventory.providers.is_empty());
}

// ── golden path: Codex-provider import ──────────────────────────────────────

#[test]
fn imports_codex_providers_and_ignores_other_app_types() {
    let dir = tempdir().unwrap();
    let paths = write_config(
        &dir,
        &json!({
            "apps": {
                "codex": {
                    "current": "one",
                    "providers": {
                        "one": {"name": "One", "settingsConfig": {"baseUrl": "https://one.example/v1/", "apiKey": "key-one"}},
                        "two": {"name": "Two", "settingsConfig": {"baseUrl": "https://two.example/v1", "apiKey": "key-two"}},
                    }
                },
                "claude": {
                    "current": "claude-one",
                    "providers": {
                        "claude-one": {"name": "ClaudeOnly", "settingsConfig": {"baseUrl": "https://claude.example/v1", "apiKey": "claude-key"}}
                    }
                }
            }
        }),
    );

    let inventory = read_ccswitch_inventory(&paths).unwrap().unwrap();

    assert_eq!(
        inventory.providers.len(),
        2,
        "only the codex app-type entries are imported"
    );
    assert!(
        inventory
            .providers
            .iter()
            .all(|p| p.display_name != "ClaudeOnly")
    );
    // Trailing slash is normalised away, mirroring ccs_import.rs.
    let one = inventory
        .providers
        .iter()
        .find(|p| p.source_id == "one")
        .unwrap();
    assert_eq!(one.base_url, "https://one.example/v1");
    assert!(one.is_current);
    let two = inventory
        .providers
        .iter()
        .find(|p| p.source_id == "two")
        .unwrap();
    assert!(!two.is_current);
}

#[test]
fn key_is_read_from_the_flat_api_key_field() {
    let dir = tempdir().unwrap();
    let paths = write_config(
        &dir,
        &json!({"apps": {"codex": {"providers": {
            "one": {"name": "One", "settingsConfig": {"baseUrl": "https://one.example/v1", "apiKey": "sk-flat"}}
        }}}}),
    );

    let inventory = read_ccswitch_inventory(&paths).unwrap().unwrap();

    assert_eq!(inventory.providers[0].reveal_key(), Some("sk-flat"));
}

#[test]
fn key_is_read_from_auth_object_openai_api_key() {
    let dir = tempdir().unwrap();
    let paths = write_config(
        &dir,
        &json!({"apps": {"codex": {"providers": {
            "one": {"name": "One", "settingsConfig": {
                "baseUrl": "https://one.example/v1",
                "auth": {"OPENAI_API_KEY": "sk-from-auth-object"}
            }}
        }}}}),
    );

    let inventory = read_ccswitch_inventory(&paths).unwrap().unwrap();

    assert_eq!(
        inventory.providers[0].reveal_key(),
        Some("sk-from-auth-object")
    );
}

#[test]
fn api_format_chat_completions_is_flagged_as_the_unsupported_protocol() {
    let dir = tempdir().unwrap();
    let paths = write_config(
        &dir,
        &json!({"apps": {"codex": {"providers": {
            "one": {"name": "One", "settingsConfig": {
                "baseUrl": "https://one.example/v1", "apiKey": "k", "apiFormat": "chat_completions"
            }}
        }}}}),
    );

    let inventory = read_ccswitch_inventory(&paths).unwrap().unwrap();

    assert_eq!(
        inventory.providers[0].protocol,
        LegacyProtocol::ChatCompletions
    );
}

// ── the no-key case ──────────────────────────────────────────────────────────

#[test]
fn a_provider_with_no_key_anywhere_is_still_imported_with_has_key_false() {
    let dir = tempdir().unwrap();
    let paths = write_config(
        &dir,
        &json!({"apps": {"codex": {"providers": {
            "one": {"name": "One", "settingsConfig": {"baseUrl": "https://one.example/v1"}}
        }}}}),
    );

    let inventory = read_ccswitch_inventory(&paths).unwrap().unwrap();

    assert_eq!(inventory.providers.len(), 1);
    assert!(!inventory.providers[0].has_key());
    assert_eq!(inventory.providers[0].reveal_key(), None);
}

// ── per-record problems degrade, they don't fail the whole read ────────────

#[test]
fn a_provider_missing_settings_config_is_skipped_with_a_warning() {
    let dir = tempdir().unwrap();
    let paths = write_config(
        &dir,
        &json!({"apps": {"codex": {"providers": {
            "broken": {"name": "Broken"},
            "ok": {"name": "OK", "settingsConfig": {"baseUrl": "https://ok.example/v1", "apiKey": "k"}}
        }}}}),
    );

    let inventory = read_ccswitch_inventory(&paths).unwrap().unwrap();

    assert_eq!(inventory.providers.len(), 1);
    assert_eq!(inventory.providers[0].source_id, "ok");
    assert!(!inventory.warnings.is_empty());
}

#[test]
fn providers_may_also_be_a_list_instead_of_a_map() {
    let dir = tempdir().unwrap();
    let paths = write_config(
        &dir,
        &json!({"apps": {"codex": {"providers": [
            {"id": "one", "name": "One", "settingsConfig": {"baseUrl": "https://one.example/v1", "apiKey": "k"}}
        ]}}}),
    );

    let inventory = read_ccswitch_inventory(&paths).unwrap().unwrap();

    assert_eq!(inventory.providers.len(), 1);
    assert_eq!(inventory.providers[0].source_id, "one");
}

// ── corrupt / unknown schema — fail closed ──────────────────────────────────

#[test]
fn truncated_json_is_refused_not_guessed() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("cc-switch-config.json");
    fs::write(&path, b"{\"apps\": {\"codex\"").unwrap();
    let paths = CcSwitchSourcePaths::new(path);

    let result = read_ccswitch_inventory(&paths);

    assert!(
        matches!(result, Err(CcSwitchReadError::Corrupt(_))),
        "got {result:?}"
    );
}

#[test]
fn a_document_with_no_apps_key_at_all_is_an_unknown_schema() {
    let dir = tempdir().unwrap();
    let paths = write_config(&dir, &json!({"totally": "unrelated document"}));

    let result = read_ccswitch_inventory(&paths);

    assert!(
        matches!(result, Err(CcSwitchReadError::UnknownSchema(_))),
        "got {result:?}"
    );
}

#[test]
fn apps_codex_that_is_not_an_object_is_an_unknown_schema() {
    let dir = tempdir().unwrap();
    let paths = write_config(&dir, &json!({"apps": {"codex": "not-an-object"}}));

    let result = read_ccswitch_inventory(&paths);

    assert!(
        matches!(result, Err(CcSwitchReadError::UnknownSchema(_))),
        "got {result:?}"
    );
}

#[test]
fn providers_of_the_wrong_type_is_an_unknown_schema() {
    let dir = tempdir().unwrap();
    let paths = write_config(&dir, &json!({"apps": {"codex": {"providers": "nope"}}}));

    let result = read_ccswitch_inventory(&paths);

    assert!(
        matches!(result, Err(CcSwitchReadError::UnknownSchema(_))),
        "got {result:?}"
    );
}

// ── source unavailable / locked ─────────────────────────────────────────────

#[test]
fn a_config_path_that_is_a_directory_is_reported_as_source_unavailable() {
    // Portable stand-in for "the file could not be opened right now" without
    // depending on platform-specific sharing-violation semantics.
    let dir = tempdir().unwrap();
    let as_dir = dir.path().join("cc-switch-config.json");
    fs::create_dir(&as_dir).unwrap();
    let paths = CcSwitchSourcePaths::new(as_dir);

    let result = read_ccswitch_inventory(&paths);

    assert!(
        matches!(result, Err(CcSwitchReadError::SourceUnavailable(_))),
        "got {result:?}"
    );
}

#[cfg(windows)]
#[test]
fn a_file_exclusively_locked_by_another_handle_is_reported_as_source_unavailable() {
    use std::os::windows::fs::OpenOptionsExt;

    let dir = tempdir().unwrap();
    let path = dir.path().join("cc-switch-config.json");
    fs::write(
        &path,
        serde_json::to_vec(&json!({"apps": {"codex": {"providers": {}}}})).unwrap(),
    )
    .unwrap();

    // share_mode(0): no other handle, including our own reader, may open this
    // file at all while this one is held open — simulates CC Switch mid-save.
    let _held = fs::OpenOptions::new()
        .write(true)
        .share_mode(0)
        .open(&path)
        .expect("should be able to open exclusively for the test");

    let paths = CcSwitchSourcePaths::new(path);
    let result = read_ccswitch_inventory(&paths);

    assert!(
        matches!(result, Err(CcSwitchReadError::SourceUnavailable(_))),
        "got {result:?}"
    );
}

// ── secrets never leak via Debug ────────────────────────────────────────────

#[test]
fn debug_output_never_contains_a_raw_api_key() {
    let dir = tempdir().unwrap();
    const SECRET: &str = "sk-cc-switch-must-never-print";
    let paths = write_config(
        &dir,
        &json!({"apps": {"codex": {"providers": {
            "one": {"name": "One", "settingsConfig": {"baseUrl": "https://one.example/v1", "apiKey": SECRET}}
        }}}}),
    );

    let inventory = read_ccswitch_inventory(&paths).unwrap().unwrap();
    let rendered = format!("{inventory:?}");

    assert!(!rendered.contains(SECRET));
    assert_eq!(inventory.providers[0].reveal_key(), Some(SECRET));
}
