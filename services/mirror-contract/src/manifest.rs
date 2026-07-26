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

    // ── Capability manifest binding (Spec 9.2) ──────────────────────────────
    // A stable entry must name the exact capability manifest generated for the
    // same raw digest, so a client can never pair a runtime with skin
    // capabilities computed for a different build. All three are required
    // together: a URL without its size and digest cannot be verified before
    // use, which is what makes the binding meaningful rather than advisory.
    /// Where the bound capability manifest is published.
    /// Absent on the raw channel: capabilities are computed at promotion time,
    /// so a raw entry legitimately has none yet.
    #[serde(default)]
    pub capability_manifest_url: Option<String>,
    /// Expected byte length, checked before parsing to bound the read.
    #[serde(default)]
    pub capability_manifest_size_bytes: Option<u64>,
    /// Expected SHA-256 of the capability manifest bytes, lowercase hex.
    #[serde(default)]
    pub capability_manifest_sha256: Option<String>,
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

/// True iff `s` is exactly 64 lowercase hex characters.
///
/// A capability digest that fails this shape check can never be a real
/// SHA-256 hex digest, so `binds_capability_manifest` treats it the same as
/// a missing digest rather than trusting it.
fn is_lowercase_hex_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Why a capability manifest failed to bind to a stable entry.
///
/// Every variant carries both sides of the comparison so the mirror gate's log
/// says what was wrong, not merely that something was.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BindingError {
    #[error("this manifest declares no capability manifest, so nothing can be bound")]
    NotDeclared,
    #[error("capability digest mismatch: manifest declares {declared}, file hashes to {actual}")]
    DigestMismatch { declared: String, actual: String },
    #[error("capability size mismatch: manifest declares {declared} bytes, file is {actual}")]
    SizeMismatch { declared: u64, actual: u64 },
    #[error(
        "capability is bound to raw digest {capability_bound_to}, but this entry was promoted \
         from {entry_promoted_from}"
    )]
    RawDigestMismatch {
        capability_bound_to: String,
        entry_promoted_from: String,
    },
}

impl MirrorManifest {
    /// True when this entry may be handed to a client as a usable stable build.
    ///
    /// Requires the capability triple as well as channel and status: Spec 9.2
    /// makes the binding part of what "stable" means, because activating a skin
    /// against a build with no capability record is exactly the case ADR-005
    /// exists to prevent. A stable entry missing the triple is malformed, and
    /// reporting it as compatible would push that decision onto every caller.
    pub fn is_stable_compatible(&self) -> bool {
        self.channel == "stable"
            && self.compatibility_status == CompatibilityStatus::Compatible
            && self.binds_capability_manifest()
    }

    /// True when this entry binds a complete, well-formed capability triple.
    ///
    /// A partial triple counts as absent: a URL with no digest cannot be
    /// verified, so treating it as declared would invite an unverified fetch.
    /// The digest is also shape-checked here (64 lowercase hex chars) so a
    /// malformed value can never be treated as a real binding.
    pub fn binds_capability_manifest(&self) -> bool {
        self.capability_manifest_url.is_some()
            && self.capability_manifest_size_bytes.is_some()
            && self
                .capability_manifest_sha256
                .as_deref()
                .is_some_and(is_lowercase_hex_sha256)
    }

    /// True when this entry declares a complete capability triple.
    ///
    /// Kept as an alias of [`Self::binds_capability_manifest`] for existing
    /// callers; prefer the latter in new code.
    pub fn declares_capability(&self) -> bool {
        self.binds_capability_manifest()
    }

    /// Verify a fetched capability manifest against what this entry declares.
    ///
    /// Fails closed (Spec 9.2, ADR-005): an entry that declares no capability
    /// binding is an error rather than a silent pass, so a stable entry can
    /// never activate a skin against a build whose capabilities are unknown.
    ///
    /// `actual_sha256` and `actual_size` describe the bytes the caller fetched.
    /// The caller must hash the file it actually received — passing the declared
    /// value back in would make this check vacuous.
    pub fn verify_capability_binding(
        &self,
        actual_sha256: &str,
        actual_size: u64,
        capability: &crate::capability::CapabilityManifest,
    ) -> Result<(), BindingError> {
        if !self.declares_capability() {
            return Err(BindingError::NotDeclared);
        }

        let declared_sha = self
            .capability_manifest_sha256
            .as_deref()
            .unwrap_or_default();
        if !declared_sha.eq_ignore_ascii_case(actual_sha256) {
            return Err(BindingError::DigestMismatch {
                declared: declared_sha.to_string(),
                actual: actual_sha256.to_string(),
            });
        }

        let declared_size = self.capability_manifest_size_bytes.unwrap_or_default();
        if declared_size != actual_size {
            return Err(BindingError::SizeMismatch {
                declared: declared_size,
                actual: actual_size,
            });
        }

        // The capability manifest names the raw build it was generated for. If
        // this entry was promoted from a different raw build, the pair is
        // mismatched even though the file itself hashed correctly.
        if let Some(ref promoted_from) = self.promoted_from_raw_digest {
            if &capability.bound_raw_digest != promoted_from {
                return Err(BindingError::RawDigestMismatch {
                    capability_bound_to: capability.bound_raw_digest.clone(),
                    entry_promoted_from: promoted_from.clone(),
                });
            }
        }

        Ok(())
    }
}
