use chimera_desktop_lib::skin_catalog::{catalog_asset_url, parse_catalog};

#[test]
fn catalog_assets_cannot_escape_the_fixed_skin_mirror() {
    assert_eq!(
        catalog_asset_url("previews/guts-terminal.webp").unwrap(),
        "https://skins.agentsmirror.com/previews/guts-terminal.webp"
    );
    assert!(catalog_asset_url("https://evil.example/skin").is_err());
    assert!(catalog_asset_url("../outside").is_err());
    assert!(catalog_asset_url("/absolute").is_err());
}

#[test]
fn catalog_parser_rejects_unverifiable_entries() {
    let json = format!(
        r#"{{"skins":[
          {{"id":"good","name":"Good","version":"1.0.0","bytes":42,"sha256":"{}","pack":"packs/good.codexskin","preview":"previews/good.webp"}},
          {{"id":"bad","name":"Bad","version":"1.0.0","bytes":42,"sha256":"short","pack":"packs/bad.codexskin","preview":"previews/bad.webp"}}
        ]}}"#,
        "a".repeat(64)
    );

    let parsed = parse_catalog(&json).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, "good");
}
