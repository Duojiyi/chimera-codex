//! Structured Codex `config.toml` provider detection and projection.
//!
//! Chimera owns one namespaced provider table while it is active. All other
//! configuration, including MCP servers, profiles and official login state,
//! remains untouched.

use thiserror::Error;
use toml_edit::{DocumentMut, Item, Table, value};

const CHIMERA_OWNERSHIP_FLAG: &str = "chimera_managed";
const PREVIOUS_PROVIDER_KEY: &str = "chimera_previous_model_provider";
const CHIMERA_PROVIDER_ID: &str = "chimera";

/// Fields needed to activate a saved provider in Codex.
#[derive(Debug, Clone)]
pub struct ProviderProjection {
    pub base_url: String,
    pub model: Option<String>,
    /// The active Codex configuration may contain the credential. It is never
    /// persisted in Chimera's database or returned over IPC.
    pub api_key_env_or_plain: String,
}

/// The provider that the current Codex configuration will actually use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveProvider {
    /// No explicit compatible API endpoint is selected.
    Official,
    /// A configured OpenAI-compatible API endpoint.
    Custom {
        provider_id: String,
        display_name: String,
        base_url: String,
        managed_by_chimera: bool,
    },
}

#[derive(Debug, Error)]
pub enum ProjectionError {
    #[error("TOML parse error: {0}")]
    TomlParse(String),
    #[error("TOML serialisation error: {0}")]
    TomlSerialise(String),
    #[error("model_providers.chimera already belongs to another tool")]
    ReservedProviderConflict,
}

/// Parse the effective provider without reading authentication material.
///
/// Current Codex uses `model_provider` plus a matching
/// `[model_providers.<id>]` table. Older switchers used the top-level
/// `model_base_url`; that shape remains detectable for migration.
pub fn detect_active_provider(config: &str) -> Result<ActiveProvider, ProjectionError> {
    let doc = config
        .parse::<DocumentMut>()
        .map_err(|error| ProjectionError::TomlParse(error.to_string()))?;
    let managed_by_chimera = doc
        .get(CHIMERA_OWNERSHIP_FLAG)
        .and_then(Item::as_bool)
        .unwrap_or(false);
    let requested_id = doc
        .get("model_provider")
        .and_then(Item::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let providers = doc.get("model_providers").and_then(Item::as_table);
    let selected = requested_id
        .as_deref()
        .and_then(|id| {
            providers
                .and_then(|table| table.get(id))
                .map(|item| (id, item))
        })
        .or_else(|| {
            let table = providers?;
            (requested_id.is_none() && table.len() == 1)
                .then(|| table.iter().next())
                .flatten()
        });

    if let Some((id, item)) = selected {
        let provider = item.as_table_like();
        let base_url = provider
            .and_then(|table| table.get("base_url"))
            .and_then(Item::as_str)
            .map(normalize_url)
            .unwrap_or_default();
        if !base_url.is_empty() {
            let display_name = provider
                .and_then(|table| table.get("name"))
                .and_then(Item::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or(id)
                .to_string();
            return Ok(ActiveProvider::Custom {
                provider_id: id.to_string(),
                display_name,
                base_url,
                managed_by_chimera,
            });
        }
    }

    let legacy_url = doc
        .get("model_base_url")
        .and_then(Item::as_str)
        .map(normalize_url)
        .unwrap_or_default();
    if !legacy_url.is_empty() {
        let provider_id = requested_id.unwrap_or_else(|| "custom".to_string());
        return Ok(ActiveProvider::Custom {
            display_name: provider_id.clone(),
            provider_id,
            base_url: legacy_url,
            managed_by_chimera,
        });
    }

    Ok(ActiveProvider::Official)
}

/// Activate a provider using Codex's current provider-table format.
pub fn apply_provider_projection(
    existing_config: &str,
    projection: &ProviderProjection,
) -> Result<String, ProjectionError> {
    let mut doc = existing_config
        .parse::<DocumentMut>()
        .map_err(|error| ProjectionError::TomlParse(error.to_string()))?;
    let already_managed = doc
        .get(CHIMERA_OWNERSHIP_FLAG)
        .and_then(Item::as_bool)
        .unwrap_or(false);

    if !already_managed {
        if let Some(previous) = doc
            .get("model_provider")
            .and_then(Item::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
        {
            doc[PREVIOUS_PROVIDER_KEY] = value(previous);
        } else {
            doc.remove(PREVIOUS_PROVIDER_KEY);
        }
    } else if doc.get(PREVIOUS_PROVIDER_KEY).is_none() {
        // Upgrade the legacy Chimera projection. Its top-level credential and
        // URL must not remain as stale authentication material.
        doc.remove("model_base_url");
        doc.remove("api_key");
    }

    if doc.get("model_providers").is_none() {
        doc["model_providers"] = Item::Table(Table::new());
    }
    let providers = doc
        .get_mut("model_providers")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| ProjectionError::TomlParse("model_providers must be a table".into()))?;
    if providers.contains_key(CHIMERA_PROVIDER_ID) && !already_managed {
        return Err(ProjectionError::ReservedProviderConflict);
    }
    providers[CHIMERA_PROVIDER_ID] = Item::Table(Table::new());
    let provider = providers[CHIMERA_PROVIDER_ID]
        .as_table_mut()
        .ok_or_else(|| ProjectionError::TomlParse("Chimera provider must be a table".into()))?;
    provider["name"] = value("Chimera++");
    provider["wire_api"] = value("responses");
    provider["requires_openai_auth"] = value(true);
    provider["base_url"] = value(normalize_url(&projection.base_url));
    provider["experimental_bearer_token"] = value(projection.api_key_env_or_plain.as_str());

    // Provider switching only needs URL + key. Keep the user's model choice
    // intact so restoring or changing endpoints never rewrites unrelated UI.
    doc["model_provider"] = value(CHIMERA_PROVIDER_ID);
    doc[CHIMERA_OWNERSHIP_FLAG] = value(true);
    Ok(doc.to_string())
}

/// Restore the provider selected before Chimera took control.
pub fn revert_provider_projection(projected_config: &str) -> Result<String, ProjectionError> {
    let mut doc = projected_config
        .parse::<DocumentMut>()
        .map_err(|error| ProjectionError::TomlParse(error.to_string()))?;
    let managed = doc
        .get(CHIMERA_OWNERSHIP_FLAG)
        .and_then(Item::as_bool)
        .unwrap_or(false);
    if !managed {
        return Ok(doc.to_string());
    }

    let previous = doc
        .get(PREVIOUS_PROVIDER_KEY)
        .and_then(Item::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(previous) = previous {
        doc["model_provider"] = value(previous);
    } else {
        doc.remove("model_provider");
    }

    if let Some(providers) = doc.get_mut("model_providers").and_then(Item::as_table_mut) {
        providers.remove(CHIMERA_PROVIDER_ID);
        if providers.is_empty() {
            doc.remove("model_providers");
        }
    }

    // Legacy Chimera builds wrote these at the root. Removing them during
    // restore prevents an old plaintext credential from surviving an upgrade.
    doc.remove("model_base_url");
    doc.remove("api_key");
    doc.remove(CHIMERA_OWNERSHIP_FLAG);
    doc.remove(PREVIOUS_PROVIDER_KEY);
    Ok(doc.to_string())
}

fn normalize_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}
