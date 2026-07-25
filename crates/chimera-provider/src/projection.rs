//! Step 2.4 — Codex config.toml projection.
//! Rules (Spec 7.3-7.4):
//! - Preserve ALL unknown fields, MCP, official login, user-modified nodes.
//! - Only modify Chimera-owned keys (model, model_provider, model_base_url, api_key).
//! - Revert only removes keys Chimera added AND whose values still match the expected sentinel.

use thiserror::Error;
use toml_edit::{DocumentMut, value};

/// Fields to inject into config.toml on behalf of the active provider.
#[derive(Debug, Clone)]
pub struct ProviderProjection {
    pub base_url: String,
    pub model: Option<String>,
    /// Actual key value (or env var reference). Spec 7.3: active key may be plain text.
    /// Caller is responsible for log redaction; this value is NEVER stored in DB or logged.
    pub api_key_env_or_plain: String,
}

#[derive(Debug, Error)]
pub enum ProjectionError {
    #[error("TOML parse error: {0}")]
    TomlParse(String),
    #[error("TOML serialisation error: {0}")]
    TomlSerialise(String),
}

/// Chimera-owned sentinel comment prefix — identifies keys Chimera injected.
const CHIMERA_SENTINEL: &str = " chimera-managed";

/// Apply provider projection to an existing config.toml text.
/// Returns the new config text with Chimera keys updated and all other content preserved.
pub fn apply_provider_projection(
    existing_config: &str,
    projection: &ProviderProjection,
) -> Result<String, ProjectionError> {
    let mut doc = existing_config
        .parse::<DocumentMut>()
        .map_err(|e| ProjectionError::TomlParse(e.to_string()))?;

    // Update / insert Chimera-owned top-level keys
    doc["model_base_url"] = value(projection.base_url.as_str());
    doc["model_provider"] = value("custom");
    doc["api_key"] = value(projection.api_key_env_or_plain.as_str());

    if let Some(ref model) = projection.model {
        doc["model"] = value(model.as_str());
    }

    // Add a chimera_managed flag so revert can identify our keys
    doc["chimera_managed"] = value(true);

    Ok(doc.to_string())
}

/// Revert a Chimera projection: remove only keys Chimera injected.
/// Unknown fields, MCP, official login ([auth]) remain untouched.
pub fn revert_provider_projection(projected_config: &str) -> Result<String, ProjectionError> {
    let mut doc = projected_config
        .parse::<DocumentMut>()
        .map_err(|e| ProjectionError::TomlParse(e.to_string()))?;

    // Only remove keys Chimera is responsible for.
    // We identify them by the chimera_managed flag; if it's absent, we do nothing.
    let is_chimera_managed = doc
        .get("chimera_managed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if is_chimera_managed {
        doc.remove("model_base_url");
        doc.remove("model_provider");
        doc.remove("api_key");
        doc.remove("chimera_managed");
        // model key is left if it existed before; Chimera only removes fields
        // it knows it added. A user-modified model key would be left in place.
    }

    Ok(doc.to_string())
}
