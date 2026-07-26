//! Step 7.1 — Read-only inventory of a Chimera++ 1.x install.
//!
//! Field names below are copied from the real 1.x schema
//! (`crates/codex-plus-core/src/settings.rs::BackendSettings`/`RelayProfile`,
//! `crates/codex-plus-core/src/paths.rs::APP_STATE_DIR`), not invented. This
//! crate cannot depend on `codex-plus-core` (v2 must not depend on the 1.x
//! tree — scripts/verify-v2-architecture.mjs G2), so the shapes are mirrored
//! independently and read permissively via `serde_json::Value` rather than a
//! strict `#[derive(Deserialize)]`, matching how `settings.rs` itself reads
//! its own file (field-by-field `.and_then`, tolerant of drift) rather than
//! failing the whole document over one bad field.
//!
//! Two historical profile shapes must both keep working:
//!   - old: flat `baseUrl` / `apiKey` fields (`RelayProfile` still
//!     deserializes them for back-compat even though it stopped writing them)
//!   - new: `upstreamBaseUrl` for the URL, with the key folded into
//!     `authContents` (JSON `OPENAI_API_KEY`) or `configContents` (TOML
//!     `experimental_bearer_token`, the "official mix" path)
//!
//! Everything here is read-only: `read_legacy_inventory` never opens
//! `settings_path` (or any auxiliary marker) for writing, and auxiliary
//! markers are checked for *existence only* — their content is never parsed,
//! which is what makes it structurally impossible for the dropped 1.x
//! features (N1-N6) to leak into the inventory.

use crate::secret::RedactedSecret;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Where a 1.x install's settings.json lives, plus optional presence-only
/// markers for the advanced features v2 does not migrate (N4-N6). All paths
/// are supplied by the caller — this crate never resolves a real user
/// profile itself, so tests never touch one.
#[derive(Debug, Clone)]
pub struct LegacySourcePaths {
    pub settings_path: PathBuf,
    pub auxiliary: LegacyAuxiliaryMarkers,
}

impl LegacySourcePaths {
    pub fn new(settings_path: impl Into<PathBuf>) -> Self {
        Self {
            settings_path: settings_path.into(),
            auxiliary: LegacyAuxiliaryMarkers::default(),
        }
    }

    pub fn with_auxiliary(mut self, auxiliary: LegacyAuxiliaryMarkers) -> Self {
        self.auxiliary = auxiliary;
        self
    }
}

/// Marker paths for the 1.x subsystems v2 deliberately dropped (N4-N6:
/// user scripts, session database, watcher). Only `.exists()` is ever
/// checked — never the content — so a corrupt or foreign file at these
/// paths cannot influence, let alone populate, the migrated output.
#[derive(Debug, Clone, Default)]
pub struct LegacyAuxiliaryMarkers {
    pub user_scripts_config_path: Option<PathBuf>,
    pub watcher_disabled_flag_path: Option<PathBuf>,
    pub session_database_paths: Vec<PathBuf>,
}

/// A 1.x relay profile's protocol. v2's provider engine only ever commits to
/// the OpenAI Responses API (see `chimera_provider::probe` doc comment), so
/// `ChatCompletions` candidates are inventoried but flagged, never silently
/// treated as Responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyProtocol {
    Responses,
    ChatCompletions,
}

/// A 1.x advanced feature v2 deliberately does not migrate. Detected for
/// transparency (so a preview can tell the user what was left behind); its
/// payload is never read into any migrated field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DroppedFeature {
    /// N1: MCP servers / skills / plugins context sync.
    McpSkillsAndPluginContextSync,
    /// N2: Chat Completions <-> Responses protocol conversion / aggregation.
    ProtocolConversion,
    /// N3: Plugin marketplace unlock.
    PluginMarketplaceUnlock,
    /// N4: User scripts.
    UserScripts,
    /// N5: Local session database.
    SessionDatabase,
    /// N6: Watcher subsystem.
    Watcher,
}

impl DroppedFeature {
    /// Short, user-facing description. Never includes any of the source
    /// data — only the fact that the feature existed and was left behind.
    pub fn description(&self) -> &'static str {
        match self {
            Self::McpSkillsAndPluginContextSync => {
                "MCP server / skill / plugin context sync (not carried over)"
            }
            Self::ProtocolConversion => {
                "Chat Completions <-> Responses protocol conversion (v2 only supports Responses)"
            }
            Self::PluginMarketplaceUnlock => "Plugin marketplace unlock (not carried over)",
            Self::UserScripts => "User scripts (not carried over)",
            Self::SessionDatabase => "Local session database (not carried over)",
            Self::Watcher => "Watcher subsystem (not carried over)",
        }
    }
}

/// One 1.x relay profile, reduced to what v2 can use.
///
/// Deliberately holds nothing beyond display_name/base_url/protocol/active
/// plus a redacted key: everything else `RelayProfile` carries (dream skin,
/// stepwise, context selection, aggregate config, ...) is 1.x-only and is
/// never copied onto this type in the first place — there is no field here
/// for it to leak through.
#[derive(Debug, Clone)]
pub struct LegacyProviderCandidate {
    pub source_id: String,
    pub display_name: String,
    pub base_url: String,
    pub protocol: LegacyProtocol,
    pub is_active: bool,
    key: Option<RedactedSecret>,
}

impl LegacyProviderCandidate {
    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    /// The real key value, for the one caller allowed to see it: the
    /// keychain-write step in `crate::migrate`. Never call this to log,
    /// `Debug`, or persist the result as-is.
    pub fn reveal_key(&self) -> Option<&str> {
        self.key.as_ref().map(RedactedSecret::reveal)
    }
}

/// Read-only result of scanning a 1.x install. Contains no key material by
/// construction — `LegacyProviderCandidate::key` redacts itself in `Debug`.
#[derive(Debug, Clone, Default)]
pub struct LegacyInventory {
    pub providers: Vec<LegacyProviderCandidate>,
    pub active_source_id: Option<String>,
    pub dropped_features: Vec<DroppedFeature>,
    /// Actionable, non-fatal problems found while reading (e.g. "profile
    /// 'x' had no base URL and was skipped"). Never a raw Rust error, path,
    /// or key value.
    pub warnings: Vec<String>,
}

#[derive(Debug, Error)]
pub enum LegacyReadError {
    /// The settings file exists but could not be safely interpreted at all
    /// (invalid JSON, or valid JSON that is not the expected top-level
    /// object). Fails closed rather than guessing.
    #[error("the 1.x settings file is not valid and was not read: {0}")]
    Corrupt(String),
    #[error("the 1.x settings file could not be read: {0}")]
    Io(String),
}

const APP_STATE_DIR_NAME: &str = ".codex-session-delete";
const SETTINGS_FILE_NAME: &str = "settings.json";

/// Where 1.x wrote `settings.json`, mirroring
/// `codex-plus-core::paths::default_settings_path` exactly (same directory
/// name, same file name) so an upgrade finds the file the old app actually
/// wrote. Pure: takes the home directory explicitly instead of resolving it,
/// so this is unit-testable without touching the real user profile.
pub fn resolve_legacy_settings_path(home_dir: &Path) -> PathBuf {
    home_dir.join(APP_STATE_DIR_NAME).join(SETTINGS_FILE_NAME)
}

/// Scan a 1.x install and return a read-only inventory. Never writes to
/// `paths.settings_path` or any auxiliary marker.
///
/// An absent settings file is a normal "nothing to migrate" state, not an
/// error (mirrors `RuntimeLayout::read_current_pointer` returning `Ok(None)`
/// for a missing pointer). A settings file that exists but cannot be safely
/// interpreted at all is refused (fail closed); a single bad profile entry
/// only skips that entry, recorded as a warning.
pub fn read_legacy_inventory(
    paths: &LegacySourcePaths,
) -> Result<LegacyInventory, LegacyReadError> {
    let text = match std::fs::read_to_string(&paths.settings_path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LegacyInventory::default());
        }
        Err(e) => return Err(LegacyReadError::Io(e.to_string())),
    };

    let raw: Value =
        serde_json::from_str(&text).map_err(|e| LegacyReadError::Corrupt(e.to_string()))?;
    let Value::Object(map) = raw else {
        return Err(LegacyReadError::Corrupt(
            "expected a JSON object at the top level".to_string(),
        ));
    };

    let mut inventory = LegacyInventory::default();

    let active_relay_id = map
        .get("activeRelayId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !active_relay_id.is_empty() {
        inventory.active_source_id = Some(active_relay_id.clone());
    }

    detect_top_level_dropped_features(&map, &mut inventory.dropped_features);

    match map.get("relayProfiles") {
        None => {}
        Some(Value::Array(items)) => {
            for item in items {
                match parse_profile(item, &active_relay_id) {
                    Ok(candidate) => {
                        if candidate.protocol == LegacyProtocol::ChatCompletions {
                            push_unique(
                                &mut inventory.dropped_features,
                                DroppedFeature::ProtocolConversion,
                            );
                        }
                        if profile_has_context_selection(item) {
                            push_unique(
                                &mut inventory.dropped_features,
                                DroppedFeature::McpSkillsAndPluginContextSync,
                            );
                        }
                        inventory.providers.push(candidate);
                    }
                    Err(warning) => inventory.warnings.push(warning),
                }
            }
        }
        Some(_) => inventory.warnings.push(
            "relayProfiles was present but was not a list; no providers were imported from it"
                .to_string(),
        ),
    }

    detect_auxiliary_dropped_features(&paths.auxiliary, &mut inventory.dropped_features);

    Ok(inventory)
}

fn detect_top_level_dropped_features(map: &Map<String, Value>, out: &mut Vec<DroppedFeature>) {
    if map
        .get("aggregateRelayProfiles")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty())
    {
        push_unique(out, DroppedFeature::ProtocolConversion);
    }
    for key in [
        "codexAppPluginMarketplaceUnlock",
        "codexAppPluginAutoExpand",
        "codexAppModelWhitelistUnlock",
    ] {
        if map.get(key).and_then(Value::as_bool) == Some(true) {
            push_unique(out, DroppedFeature::PluginMarketplaceUnlock);
        }
    }
    if map
        .get("relayContextConfigContents")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty())
    {
        push_unique(out, DroppedFeature::McpSkillsAndPluginContextSync);
    }
}

fn detect_auxiliary_dropped_features(aux: &LegacyAuxiliaryMarkers, out: &mut Vec<DroppedFeature>) {
    if aux
        .user_scripts_config_path
        .as_deref()
        .is_some_and(Path::exists)
    {
        push_unique(out, DroppedFeature::UserScripts);
    }
    if aux
        .watcher_disabled_flag_path
        .as_deref()
        .is_some_and(Path::exists)
    {
        push_unique(out, DroppedFeature::Watcher);
    }
    if aux.session_database_paths.iter().any(|p| p.exists()) {
        push_unique(out, DroppedFeature::SessionDatabase);
    }
}

fn push_unique(list: &mut Vec<DroppedFeature>, item: DroppedFeature) {
    if !list.contains(&item) {
        list.push(item);
    }
}

fn profile_has_context_selection(item: &Value) -> bool {
    let Some(selection) = item.get("contextSelection") else {
        return false;
    };
    ["mcpServers", "skills", "plugins"].iter().any(|key| {
        selection
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
    })
}

/// Parse one `relayProfiles[]` entry, or return a human-readable reason it
/// was skipped. Skipping one profile never fails the whole read.
fn parse_profile(item: &Value, active_relay_id: &str) -> Result<LegacyProviderCandidate, String> {
    let Value::Object(obj) = item else {
        return Err("a relay profile entry was not an object; skipped".to_string());
    };

    let id = obj
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if id.is_empty() {
        return Err("a relay profile entry had no id; skipped".to_string());
    }

    let name = obj.get("name").and_then(Value::as_str).unwrap_or("").trim();
    let display_name = if name.is_empty() {
        id.clone()
    } else {
        name.to_string()
    };

    // Historical shape: `upstreamBaseUrl` is current; `baseUrl` is the old
    // flat field, still deserializable for back-compat (settings.rs kept
    // `deserialize_with` for it even after making it `skip_serializing`).
    let upstream = obj
        .get("upstreamBaseUrl")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let legacy_base = obj
        .get("baseUrl")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let base_url = if !upstream.is_empty() {
        upstream
    } else {
        legacy_base
    };
    if base_url.is_empty() {
        return Err(format!("relay profile '{id}' had no base URL; skipped"));
    }

    let protocol = match obj.get("protocol").and_then(Value::as_str) {
        Some("chatCompletions") => LegacyProtocol::ChatCompletions,
        _ => LegacyProtocol::Responses,
    };

    Ok(LegacyProviderCandidate {
        is_active: !active_relay_id.is_empty() && active_relay_id == id,
        source_id: id,
        display_name,
        base_url: base_url.to_string(),
        protocol,
        key: extract_legacy_key(obj),
    })
}

/// Three-way key lookup mirroring `relay_profile_has_usable_key` in
/// settings.rs: flat `apiKey` first, then an `experimental_bearer_token`
/// folded into `configContents` TOML (the official-mix path), then
/// `OPENAI_API_KEY` folded into `authContents` JSON. The first usable value
/// wins; any source that is absent, empty, or unparseable is skipped rather
/// than treated as an error, since a missing key is a normal, testable state
/// (a provider imported with no key yet).
fn extract_legacy_key(obj: &Map<String, Value>) -> Option<RedactedSecret> {
    if let Some(key) = non_empty_str(obj.get("apiKey")) {
        return Some(RedactedSecret::new(key));
    }
    if let Some(config_contents) = obj.get("configContents").and_then(Value::as_str) {
        if let Some(token) = extract_experimental_bearer_token(config_contents) {
            return Some(RedactedSecret::new(token));
        }
    }
    if let Some(auth_contents) = obj.get("authContents").and_then(Value::as_str) {
        if let Ok(auth) = serde_json::from_str::<Value>(auth_contents) {
            if let Some(key) = non_empty_str(auth.get("OPENAI_API_KEY")) {
                return Some(RedactedSecret::new(key));
            }
        }
    }
    None
}

fn non_empty_str(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Pull `model_providers.*.experimental_bearer_token` out of an embedded
/// config.toml string. Unparseable or missing-field TOML degrades to `None`
/// rather than an error — `configContents` is only one of three key sources,
/// so a malformed one just means "try the next source".
fn extract_experimental_bearer_token(config_contents: &str) -> Option<String> {
    let doc: toml::Value = config_contents.parse().ok()?;
    let providers = doc.get("model_providers")?.as_table()?;
    providers.values().find_map(|provider| {
        provider
            .get("experimental_bearer_token")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    })
}
