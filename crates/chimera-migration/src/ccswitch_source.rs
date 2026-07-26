//! Step 7.2 — Read-only import of CC Switch's Codex providers.
//!
//! CC Switch (farion1231/cc-switch, registered in THIRD_PARTY_SOURCES.md) is
//! a third-party tool. This reader models its on-disk config as a JSON file
//! under the user profile, per this task's design notes, with a container
//! shape (`apps.<app_type>.{providers, current}`) and a per-provider payload
//! shape (`baseUrl`/`apiKey`/`apiFormat`/`config`/`auth`) carried over from
//! the ALREADY-SHIPPED 1.x importer at
//! `crates/codex-plus-core/src/ccs_import.rs`, which parses that identical
//! payload out of a SQLite column instead of a JSON file. That inner shape is
//! real and confirmed; the outer JSON-file container is a best-effort
//! reconstruction and should be checked against a real CC Switch install
//! before this reader is wired into the desktop shell (see this crate's
//! final report, "Integration needed").
//!
//! Unknown container shapes fail closed (`CcSwitchReadError::UnknownSchema`)
//! rather than being silently treated as empty, and nothing here ever opens
//! CC Switch's config for writing — this is a one-shot, read-only import.

use crate::legacy_source::LegacyProtocol;
use crate::secret::RedactedSecret;
use serde_json::Value;
use std::path::PathBuf;
use thiserror::Error;

/// Where CC Switch's config file lives. Supplied by the caller — this crate
/// never resolves a real user profile itself.
#[derive(Debug, Clone)]
pub struct CcSwitchSourcePaths {
    pub config_path: PathBuf,
}

impl CcSwitchSourcePaths {
    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
        }
    }
}

/// One CC Switch Codex provider, reduced to what v2 can use.
#[derive(Debug, Clone)]
pub struct CcSwitchProviderCandidate {
    pub source_id: String,
    pub display_name: String,
    pub base_url: String,
    pub protocol: LegacyProtocol,
    pub is_current: bool,
    key: Option<RedactedSecret>,
}

impl CcSwitchProviderCandidate {
    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    /// The real key value, for the keychain-write step in `crate::migrate`
    /// only. Never `Debug`, log, or persist the result as-is.
    pub fn reveal_key(&self) -> Option<&str> {
        self.key.as_ref().map(RedactedSecret::reveal)
    }
}

/// Read-only result of scanning CC Switch's Codex providers.
#[derive(Debug, Clone, Default)]
pub struct CcSwitchInventory {
    pub providers: Vec<CcSwitchProviderCandidate>,
    pub current_source_id: Option<String>,
    /// Actionable, non-fatal problems found while reading (e.g. a single
    /// provider entry with no usable base URL). Never a raw Rust error or
    /// key value.
    pub warnings: Vec<String>,
}

#[derive(Debug, Error)]
pub enum CcSwitchReadError {
    /// The file exists but is not valid JSON at all. Fails closed.
    #[error("CC Switch's config is not valid and was not read: {0}")]
    Corrupt(String),
    /// Valid JSON, but not a container shape this importer recognises.
    /// Deliberately distinct from `Corrupt`: this is refused because the
    /// importer does not know how to interpret it safely, not because the
    /// JSON itself is broken.
    #[error("CC Switch's config uses a schema this importer does not recognise: {0}")]
    UnknownSchema(String),
    /// The file could not be opened at all (locked by another process,
    /// permission denied, or any other non-"missing" IO failure).
    #[error("CC Switch's config could not be read right now — it may be open in CC Switch: {0}")]
    SourceUnavailable(String),
}

/// Scan CC Switch's Codex providers. Never writes to `paths.config_path`.
///
/// `Ok(None)` means CC Switch has never run (no config file at all) — a
/// normal "nothing to import" state, not an error. `Ok(Some(inventory))`
/// with an empty provider list means CC Switch is installed but has no
/// Codex-app-type providers configured. Anything the importer cannot
/// safely interpret is refused rather than guessed at.
pub fn read_ccswitch_inventory(
    paths: &CcSwitchSourcePaths,
) -> Result<Option<CcSwitchInventory>, CcSwitchReadError> {
    let text = match std::fs::read_to_string(&paths.config_path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(CcSwitchReadError::SourceUnavailable(e.to_string())),
    };

    let raw: Value =
        serde_json::from_str(&text).map_err(|e| CcSwitchReadError::Corrupt(e.to_string()))?;
    let Value::Object(root) = raw else {
        return Err(CcSwitchReadError::UnknownSchema(
            "expected a JSON object at the top level".to_string(),
        ));
    };

    let Some(apps) = root.get("apps") else {
        return Err(CcSwitchReadError::UnknownSchema(
            "missing top-level \"apps\" key".to_string(),
        ));
    };
    let Value::Object(apps) = apps else {
        return Err(CcSwitchReadError::UnknownSchema(
            "\"apps\" was not an object".to_string(),
        ));
    };

    // No "codex" section at all is a legitimate empty state (CC Switch may
    // manage other app types only), not an unknown schema.
    let Some(codex) = apps.get("codex") else {
        return Ok(Some(CcSwitchInventory::default()));
    };
    let Value::Object(codex) = codex else {
        return Err(CcSwitchReadError::UnknownSchema(
            "\"apps.codex\" was not an object".to_string(),
        ));
    };

    let current_id = codex
        .get("current")
        .and_then(Value::as_str)
        .map(str::to_string);

    let mut inventory = CcSwitchInventory {
        providers: Vec::new(),
        current_source_id: current_id.clone(),
        warnings: Vec::new(),
    };

    match codex.get("providers") {
        None => {}
        Some(Value::Object(map)) => {
            for (id, record) in map {
                match parse_ccswitch_provider(id, record, current_id.as_deref()) {
                    Ok(candidate) => inventory.providers.push(candidate),
                    Err(warning) => inventory.warnings.push(warning),
                }
            }
        }
        Some(Value::Array(items)) => {
            for item in items {
                let id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                match parse_ccswitch_provider(&id, item, current_id.as_deref()) {
                    Ok(candidate) => inventory.providers.push(candidate),
                    Err(warning) => inventory.warnings.push(warning),
                }
            }
        }
        Some(_) => {
            return Err(CcSwitchReadError::UnknownSchema(
                "\"apps.codex.providers\" was neither an object map nor a list".to_string(),
            ));
        }
    }

    Ok(Some(inventory))
}

fn parse_ccswitch_provider(
    id: &str,
    record: &Value,
    current_id: Option<&str>,
) -> Result<CcSwitchProviderCandidate, String> {
    if id.trim().is_empty() {
        return Err("a CC Switch provider entry had no id; skipped".to_string());
    }
    let Value::Object(obj) = record else {
        return Err(format!(
            "CC Switch provider '{id}' entry was not an object; skipped"
        ));
    };
    let name = obj.get("name").and_then(Value::as_str).unwrap_or("").trim();
    let display_name = if name.is_empty() {
        id.to_string()
    } else {
        name.to_string()
    };

    let cfg = obj
        .get("settingsConfig")
        .ok_or_else(|| format!("CC Switch provider '{id}' had no settingsConfig; skipped"))?;

    let base_url = extract_base_url(cfg)
        .ok_or_else(|| format!("CC Switch provider '{id}' had no usable base URL; skipped"))?;
    let protocol = extract_protocol(cfg);
    let key = extract_api_key(cfg).map(RedactedSecret::new);

    Ok(CcSwitchProviderCandidate {
        is_current: current_id == Some(id),
        source_id: id.to_string(),
        display_name,
        base_url,
        protocol,
        key,
    })
}

// ── per-record field extraction ─────────────────────────────────────────────
// Mirrors ccs_import.rs's extract_base_url/extract_api_key/extract_protocol —
// same field names, same fallbacks, same TOML-embedded-config last resort.

fn extract_base_url(cfg: &Value) -> Option<String> {
    string_at(cfg, &["baseUrl", "base_url"])
        .or_else(|| {
            cfg.get("config")
                .and_then(Value::as_str)
                .and_then(extract_toml_base_url)
        })
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
}

fn extract_api_key(cfg: &Value) -> Option<String> {
    if let Some(key) = cfg.pointer("/auth/OPENAI_API_KEY").and_then(Value::as_str) {
        return non_empty(key);
    }
    if let Some(key) = string_at(cfg, &["apiKey", "api_key"]) {
        return non_empty(&key);
    }
    if let Some(auth_str) = cfg.get("auth").and_then(Value::as_str) {
        if let Ok(auth) = serde_json::from_str::<Value>(auth_str) {
            if let Some(key) = auth.get("OPENAI_API_KEY").and_then(Value::as_str) {
                return non_empty(key);
            }
        }
    }
    None
}

fn extract_protocol(cfg: &Value) -> LegacyProtocol {
    if let Some(fmt) = string_at(cfg, &["apiFormat", "api_format"]) {
        if is_chat_protocol(&fmt) {
            return LegacyProtocol::ChatCompletions;
        }
    }
    if let Some(wire_api) = cfg
        .get("config")
        .and_then(Value::as_str)
        .and_then(extract_toml_wire_api)
    {
        if is_chat_protocol(&wire_api) {
            return LegacyProtocol::ChatCompletions;
        }
    }
    LegacyProtocol::Responses
}

fn string_at(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn is_chat_protocol(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "chat" | "chat_completions" | "chat-completions" | "openai_chat" | "openai-chat"
    )
}

fn extract_toml_base_url(text: &str) -> Option<String> {
    extract_toml_string_value(text, "base_url")
}

fn extract_toml_wire_api(text: &str) -> Option<String> {
    extract_toml_string_value(text, "wire_api")
}

fn extract_toml_string_value(text: &str, key: &str) -> Option<String> {
    let doc: toml::Value = text.parse().ok()?;
    let providers = doc.get("model_providers")?.as_table()?;
    providers.values().find_map(|provider| {
        provider
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_base_url_prefers_the_flat_field_over_embedded_toml() {
        let cfg = serde_json::json!({"baseUrl": "https://flat.example/v1/"});
        assert_eq!(
            extract_base_url(&cfg).as_deref(),
            Some("https://flat.example/v1")
        );
    }

    #[test]
    fn extract_base_url_falls_back_to_embedded_toml() {
        let cfg = serde_json::json!({
            "config": "model_provider = 'custom'\n[model_providers.custom]\nbase_url = 'https://toml.example/v1'\n"
        });
        assert_eq!(
            extract_base_url(&cfg).as_deref(),
            Some("https://toml.example/v1")
        );
    }

    #[test]
    fn extract_protocol_defaults_to_responses() {
        let cfg = serde_json::json!({});
        assert_eq!(extract_protocol(&cfg), LegacyProtocol::Responses);
    }
}
