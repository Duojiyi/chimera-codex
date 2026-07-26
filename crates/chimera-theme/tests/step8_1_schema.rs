// Step 8.1 RED — `.codexskin` manifest (`theme.json`) schema.
//
// ADR-005: a `.codexskin` package declares CSS + image/font assets only. The
// manifest is the first thing parsed, before a single byte of the archive is
// trusted, so its own validation must fail closed: an unknown schema version,
// a missing field, or an `entry_css` that tries to escape the package must
// all be refused rather than best-effort accepted.

use chimera_theme::schema::{ManifestError, SUPPORTED_SCHEMA_VERSION, SkinManifest};

fn valid_json() -> String {
    r#"{
        "schema_version": 1,
        "name": "Midnight",
        "version": "1.0.0",
        "entry_css": "theme.css",
        "description": "A dark theme"
    }"#
    .to_string()
}

#[test]
fn a_well_formed_manifest_parses() {
    let manifest = SkinManifest::parse(valid_json().as_bytes()).expect("must parse");
    assert_eq!(manifest.name, "Midnight");
    assert_eq!(manifest.entry_css, "theme.css");
    assert_eq!(manifest.schema_version, SUPPORTED_SCHEMA_VERSION);
}

#[test]
fn description_is_optional() {
    let json = r#"{"schema_version":1,"name":"X","version":"1.0.0","entry_css":"a.css"}"#;
    let manifest = SkinManifest::parse(json.as_bytes()).expect("must parse without description");
    assert_eq!(manifest.description, None);
}

#[test]
fn not_json_at_all_is_refused() {
    let result = SkinManifest::parse(b"this is not json");
    assert!(matches!(result, Err(ManifestError::Malformed(_))));
}

#[test]
fn non_utf8_bytes_are_refused() {
    let result = SkinManifest::parse(&[0xff, 0xfe, 0x00, 0x01]);
    assert!(matches!(result, Err(ManifestError::Malformed(_))));
}

#[test]
fn unsupported_schema_version_is_refused() {
    let json = r#"{"schema_version":99,"name":"X","version":"1.0.0","entry_css":"a.css"}"#;
    let result = SkinManifest::parse(json.as_bytes());
    assert!(matches!(
        result,
        Err(ManifestError::UnsupportedSchemaVersion { found: 99, .. })
    ));
}

#[test]
fn schema_version_zero_is_refused() {
    // Zero is not "unversioned"; it is simply not version 1. Treating it as a
    // wildcard would let an old/malformed manifest slip past validation.
    let json = r#"{"schema_version":0,"name":"X","version":"1.0.0","entry_css":"a.css"}"#;
    let result = SkinManifest::parse(json.as_bytes());
    assert!(matches!(
        result,
        Err(ManifestError::UnsupportedSchemaVersion { found: 0, .. })
    ));
}

#[test]
fn missing_required_field_is_refused() {
    let json = r#"{"schema_version":1,"version":"1.0.0","entry_css":"a.css"}"#;
    let result = SkinManifest::parse(json.as_bytes());
    assert!(matches!(result, Err(ManifestError::Malformed(_))));
}

#[test]
fn empty_name_is_refused() {
    let json = r#"{"schema_version":1,"name":"   ","version":"1.0.0","entry_css":"a.css"}"#;
    let result = SkinManifest::parse(json.as_bytes());
    assert!(matches!(result, Err(ManifestError::EmptyName)));
}

// ── entry_css must name a bundled relative CSS file, nothing else ──────────

#[test]
fn entry_css_must_end_in_dot_css() {
    let json = r#"{"schema_version":1,"name":"X","version":"1.0.0","entry_css":"theme.txt"}"#;
    let result = SkinManifest::parse(json.as_bytes());
    assert!(matches!(result, Err(ManifestError::InvalidEntryCss(_))));
}

#[test]
fn entry_css_rejects_absolute_paths() {
    let json = r#"{"schema_version":1,"name":"X","version":"1.0.0","entry_css":"/etc/theme.css"}"#;
    let result = SkinManifest::parse(json.as_bytes());
    assert!(matches!(result, Err(ManifestError::InvalidEntryCss(_))));
}

#[test]
fn entry_css_rejects_windows_absolute_paths() {
    let json = r#"{"schema_version":1,"name":"X","version":"1.0.0","entry_css":"C:\\theme.css"}"#;
    let result = SkinManifest::parse(json.as_bytes());
    assert!(matches!(result, Err(ManifestError::InvalidEntryCss(_))));
}

#[test]
fn entry_css_rejects_traversal() {
    let json = r#"{"schema_version":1,"name":"X","version":"1.0.0","entry_css":"../outside.css"}"#;
    let result = SkinManifest::parse(json.as_bytes());
    assert!(matches!(result, Err(ManifestError::InvalidEntryCss(_))));
}

#[test]
fn entry_css_rejects_backslash_separators() {
    let json = r#"{"schema_version":1,"name":"X","version":"1.0.0","entry_css":"sub\\theme.css"}"#;
    let result = SkinManifest::parse(json.as_bytes());
    assert!(matches!(result, Err(ManifestError::InvalidEntryCss(_))));
}

#[test]
fn entry_css_rejects_a_remote_url() {
    let json = r#"{"schema_version":1,"name":"X","version":"1.0.0","entry_css":"https://evil.example/theme.css"}"#;
    let result = SkinManifest::parse(json.as_bytes());
    assert!(matches!(result, Err(ManifestError::InvalidEntryCss(_))));
}
