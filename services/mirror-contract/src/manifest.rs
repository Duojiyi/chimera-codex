//! Mirror manifest schema — Spec 9.2 minimum fields.
use serde::{Deserialize, Serialize};

/// A single entry in the stable/raw manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorManifest {
    pub schema_version: u32,
    /// "raw" | "stable" | "candidate"
    pub channel: String,
    pub codex_version: String,
    pub published_at: String,
    pub platform: String,
    pub arch: String,
    pub asset_url: String,
    pub size_bytes: u64,
    pub sha256: String,
    /// Official platform signature metadata (Authenticode subject / Sparkle team id)
    pub official_identity: OfficialIdentity,
    /// Minimum Chimera version required to consume this manifest
    pub minimum_chimera_version: String,
    pub compatibility_status: CompatibilityStatus,
    /// raw digest this stable entry was promoted from (None for raw channel)
    pub promoted_from_raw_digest: Option<String>,
    /// Version to roll back to if this entry is retracted
    pub rollback_target: Option<String>,
    /// Source provenance: original download URL, ETag, headers
    pub source_provenance: SourceProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfficialIdentity {
    pub signer: String,
    pub subject: Option<String>,
    pub team_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    Pending,
    Compatible,
    Incompatible { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceProvenance {
    pub source_url: String,
    pub etag: Option<String>,
    pub observed_at: String,
}

impl MirrorManifest {
    pub fn is_stable_compatible(&self) -> bool {
        self.channel == "stable" && self.compatibility_status == CompatibilityStatus::Compatible
    }
}
