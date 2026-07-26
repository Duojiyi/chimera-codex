// Step 2.4 RED — Codex config.toml projection golden fixtures.
// Spec 7.3-7.4: 结构化投影、保留未知字段、MCP、官方登录材料不被覆盖。
use chimera_provider::projection::{
    ProviderProjection, apply_provider_projection, revert_provider_projection,
};

// ── Golden fixture helpers ─────────────────────────────────────────────────

const CONFIG_WITH_UNKNOWN_FIELDS: &str = r#"
model = "gpt-4o"
model_provider = "custom"
model_base_url = "https://old.example.com/v1"

[some_user_custom_section]
my_setting = true
another_value = 42

[mcp_servers]
my_server = { command = "node", args = ["server.js"] }
"#;

const CONFIG_WITH_OFFICIAL_LOGIN: &str = r#"
model = "gpt-4o"
model_provider = "openai"

[auth]
api_key = "sk-official-key"
type = "openai"
"#;

const CONFIG_EMPTY: &str = "";

// ── 未知字段和 MCP 必须保留 ─────────────────────────────────────────────────

#[test]
fn unknown_fields_are_preserved_after_projection() {
    let result = apply_provider_projection(
        CONFIG_WITH_UNKNOWN_FIELDS,
        &ProviderProjection {
            base_url: "https://api.chimerahub.org/v1".into(),
            model: Some("gpt-4o".into()),
            api_key_env_or_plain: "sk-test-key".into(),
        },
    )
    .unwrap();

    // User's custom section must survive unchanged
    assert!(
        result.contains("[some_user_custom_section]"),
        "unknown section must survive: {result}"
    );
    assert!(
        result.contains("my_setting = true"),
        "unknown field must survive: {result}"
    );
    assert!(
        result.contains("another_value = 42"),
        "unknown field must survive: {result}"
    );
    // MCP config must survive
    assert!(
        result.contains("[mcp_servers]"),
        "mcp_servers must survive: {result}"
    );
}

#[test]
fn projection_updates_model_provider_fields() {
    let result = apply_provider_projection(
        CONFIG_WITH_UNKNOWN_FIELDS,
        &ProviderProjection {
            base_url: "https://api.new.io/v1".into(),
            model: Some("claude-opus-5".into()),
            api_key_env_or_plain: "sk-new-key".into(),
        },
    )
    .unwrap();

    assert!(
        result.contains("https://api.new.io/v1"),
        "new base_url must be in config: {result}"
    );
    assert!(
        result.contains("claude-opus-5"),
        "new model must be in config: {result}"
    );
}

// ── 官方登录材料不被 Chimera 投影覆盖 ──────────────────────────────────────

#[test]
fn official_login_section_is_not_overwritten() {
    // When Chimera applies a custom provider projection, it must NOT delete
    // or overwrite the [auth] section that belongs to the official Codex login.
    let result = apply_provider_projection(
        CONFIG_WITH_OFFICIAL_LOGIN,
        &ProviderProjection {
            base_url: "https://api.custom.io/v1".into(),
            model: Some("gpt-4o".into()),
            api_key_env_or_plain: "sk-chimera-key".into(),
        },
    )
    .unwrap();

    // Official [auth] section must remain (not deleted)
    assert!(
        result.contains("[auth]"),
        "official [auth] section must not be removed: {result}"
    );
}

// ── empty config 可被安全初始化 ─────────────────────────────────────────────

#[test]
fn projection_on_empty_config_produces_valid_toml() {
    let result = apply_provider_projection(
        CONFIG_EMPTY,
        &ProviderProjection {
            base_url: "https://api.chimerahub.org/v1".into(),
            model: None,
            api_key_env_or_plain: "sk-new".into(),
        },
    )
    .unwrap();

    // Must parse as valid TOML
    result
        .parse::<toml::Value>()
        .expect("result must be valid TOML");
    assert!(
        result.contains("chimerahub.org"),
        "base_url must appear in result: {result}"
    );
}

// ── revert 只删除 Chimera 拥有的字段 ──────────────────────────────────────

#[test]
fn revert_restores_pre_projection_state() {
    let original = CONFIG_WITH_UNKNOWN_FIELDS;
    let projected = apply_provider_projection(
        original,
        &ProviderProjection {
            base_url: "https://api.chimerahub.org/v1".into(),
            model: Some("gpt-4o".into()),
            api_key_env_or_plain: "sk-key".into(),
        },
    )
    .unwrap();

    let reverted = revert_provider_projection(&projected).unwrap();

    // User's unknown fields must still be present after revert
    assert!(
        reverted.contains("[some_user_custom_section]"),
        "revert must not remove unknown sections: {reverted}"
    );
    assert!(
        reverted.contains("[mcp_servers]"),
        "revert must not remove mcp_servers: {reverted}"
    );
    // Chimera's injected fields should be removed
    assert!(
        !reverted.contains("chimerahub.org"),
        "chimera base_url must be removed on revert: {reverted}"
    );
}

// ── API Key 不出现在投影后的 config（env var 引用）──────────────────────────

#[test]
fn api_key_in_plain_text_must_be_injected_as_env_or_direct() {
    // Spec 7.3: Active key may exist in plain text format that Codex supports.
    // This test just verifies the projection doesn't silently drop the key.
    let result = apply_provider_projection(
        CONFIG_EMPTY,
        &ProviderProjection {
            base_url: "https://api.example.com/v1".into(),
            model: None,
            api_key_env_or_plain: "sk-projected-key".into(),
        },
    )
    .unwrap();
    // The key (or an env reference) must appear in the output
    // so Codex can authenticate. Projection must not silently drop it.
    let has_key_ref = result.contains("sk-projected-key")
        || result.contains("CHIMERA_API_KEY")
        || result.contains("api_key");
    assert!(
        has_key_ref,
        "projected config must reference the key somehow: {result}"
    );
}
