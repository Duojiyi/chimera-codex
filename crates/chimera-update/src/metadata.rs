//! TUF-style metadata document shapes for Chimera's own app-update chain
//! (Step 9.1, ADR-006).
//!
//! Four roles, exactly as in real TUF: root (who the other three roles are),
//! targets (what files exist and their hashes — including the
//! `chimera-app-latest.json` pointer, which is a target and nothing more),
//! snapshot (pins the targets version+hash so the two cannot be served out of
//! sync), timestamp (pins the snapshot version+hash and is the one document
//! fetched on every check, so freshness has a cheap, frequently-rotated
//! anchor). Each document is a plain, signed JSON payload — see
//! [`SignedPayload`] — kept as a string until its signatures are checked, for
//! the same reason `mirror_contract::signature::SignedManifest` does: a
//! reparsed-and-reserialised value can silently reorder fields and invalidate
//! every signature over it.
//!
//! `domain` on every one of the four types is the cross-contamination gate
//! (G8/G15): every `parse_*` function below checks it against
//! [`APP_TRUST_DOMAIN`] before anything else, including before a single
//! signature is inspected. A Codex mirror document — or a bug that points
//! this crate's fetcher at the mirror's endpoint — is refused at parse time,
//! not discovered later by a signature mismatch that could be mistaken for
//! "the mirror rotated a key".

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The only trust domain this crate's parsers ever accept. Hardcoded, not
/// configurable — a value read from a config file could be edited to match
/// whatever the caller happens to be fetching, which would turn the domain
/// check into decoration rather than a real gate.
pub const APP_TRUST_DOMAIN: &str = "chimera-app-update.v1";

/// One public key a root document has assigned an id to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyEntry {
    pub key_id: String,
    /// Lowercase hex, 64 chars / 32 bytes.
    pub public_key_hex: String,
}

/// Which of the four TUF roles a check is being performed for. Carried as a
/// value (rather than each caller passing a bare `&str` role name) so a typo
/// in a role name cannot silently create a role that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Root,
    Targets,
    Snapshot,
    Timestamp,
}

impl Role {
    pub fn name(&self) -> &'static str {
        match self {
            Role::Root => "root",
            Role::Targets => "targets",
            Role::Snapshot => "snapshot",
            Role::Timestamp => "timestamp",
        }
    }
}

/// Which keys may act for a role, and how many of them must agree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleKeys {
    pub key_ids: Vec<String>,
    pub threshold: u32,
}

/// The root role: the only document that says which keys the other three
/// roles trust. Rotating it (Step 9.1 "consecutive root rotation") lives in
/// [`crate::trust`]; this type only knows how to describe one version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RootMetadata {
    pub domain: String,
    /// Monotonic; a client must never accept a root numbered lower than one
    /// it has already trusted (Step 9.1 "skipped root version" / rollback).
    pub version: u64,
    pub expires: i64,
    pub keys: Vec<KeyEntry>,
    pub root: RoleKeys,
    pub targets: RoleKeys,
    pub snapshot: RoleKeys,
    pub timestamp: RoleKeys,
}

impl RootMetadata {
    pub fn role(&self, role: Role) -> &RoleKeys {
        match role {
            Role::Root => &self.root,
            Role::Targets => &self.targets,
            Role::Snapshot => &self.snapshot,
            Role::Timestamp => &self.timestamp,
        }
    }

    /// Look up the actual key material for a role's declared key ids.
    ///
    /// An id a role names but that is missing from `keys` is caught earlier,
    /// by [`validate_shape`](Self::validate_shape) — by the time this runs,
    /// every id is expected to resolve.
    pub fn resolve_keys(&self, role: Role) -> Vec<&KeyEntry> {
        self.role(role)
            .key_ids
            .iter()
            .filter_map(|id| self.keys.iter().find(|k| &k.key_id == id))
            .collect()
    }

    /// Structural sanity checks that have nothing to do with cryptography:
    /// a role with no way to ever be satisfied, or a role naming a key that
    /// does not exist, is a malformed document regardless of who signed it.
    pub fn validate_shape(&self) -> Result<(), MetadataError> {
        for role in [Role::Root, Role::Targets, Role::Snapshot, Role::Timestamp] {
            let rk = self.role(role);
            if rk.threshold == 0 {
                return Err(MetadataError::ZeroThreshold {
                    role: role.name().to_string(),
                    threshold: rk.threshold,
                });
            }
            for id in &rk.key_ids {
                if !self.keys.iter().any(|k| &k.key_id == id) {
                    return Err(MetadataError::UnknownRoleKey {
                        role: role.name().to_string(),
                        key_id: id.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// The timestamp role: pins which snapshot version+hash is current. Meant to
/// be small and fetched on every check, so it rotates most frequently of the
/// four and gives freshness without re-fetching the (larger) targets list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimestampMetadata {
    pub domain: String,
    pub version: u64,
    pub expires: i64,
    pub snapshot_version: u64,
    pub snapshot_sha256_hex: String,
}

/// The snapshot role: pins which targets version+hash is current, so the two
/// can never be served out of sync with each other.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotMetadata {
    pub domain: String,
    pub version: u64,
    pub expires: i64,
    pub targets_version: u64,
    pub targets_sha256_hex: String,
}

/// One file the targets role vouches for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetEntry {
    pub sha256_hex: String,
    pub length: u64,
}

/// The targets role: what files exist and their hashes.
/// `chimera-app-latest.json` is one entry among these — never a
/// self-authenticating document, only ever trusted through this map.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetsMetadata {
    pub domain: String,
    pub version: u64,
    pub expires: i64,
    pub targets: BTreeMap<String, TargetEntry>,
}

/// One signature over a metadata payload, attributed to a specific key id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaSignature {
    pub key_id: String,
    pub signature_hex: String,
}

/// A metadata document as published: the exact signed bytes, plus one or
/// more detached signatures. `payload` is a string rather than a parsed
/// struct so it can be verified byte-for-byte as received; see the module
/// doc comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPayload {
    pub payload: String,
    pub signatures: Vec<MetaSignature>,
}

/// Everything that can go wrong turning bytes on the wire into a metadata
/// document this crate is willing to reason about further. Every variant is
/// a refusal.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MetadataError {
    #[error("metadata json is malformed: {0}")]
    Malformed(String),

    #[error("role {role} has threshold {threshold}, which can never be satisfied")]
    ZeroThreshold { role: String, threshold: u32 },

    #[error("role {role} names key id {key_id}, which is not in the document's key set")]
    UnknownRoleKey { role: String, key_id: String },

    #[error("metadata belongs to a different trust domain: expected {expected}, found {found}")]
    WrongDomain { expected: String, found: String },

    #[error("signature threshold for role {role} not met: needed {needed}, got {got}")]
    ThresholdNotMet { role: String, needed: u32, got: u32 },
}

fn check_domain(found: &str) -> Result<(), MetadataError> {
    if found != APP_TRUST_DOMAIN {
        return Err(MetadataError::WrongDomain {
            expected: APP_TRUST_DOMAIN.to_string(),
            found: found.to_string(),
        });
    }
    Ok(())
}

/// Parse and structurally validate a root document. Does not check
/// signatures — that requires knowing which key set is authoritative for
/// this exact document, which is a rotation-chain question handled by
/// [`crate::trust`].
pub fn parse_root(payload: &str) -> Result<RootMetadata, MetadataError> {
    let root: RootMetadata =
        serde_json::from_str(payload).map_err(|e| MetadataError::Malformed(e.to_string()))?;
    check_domain(&root.domain)?;
    root.validate_shape()?;
    Ok(root)
}

pub fn parse_timestamp(payload: &str) -> Result<TimestampMetadata, MetadataError> {
    let ts: TimestampMetadata =
        serde_json::from_str(payload).map_err(|e| MetadataError::Malformed(e.to_string()))?;
    check_domain(&ts.domain)?;
    Ok(ts)
}

pub fn parse_snapshot(payload: &str) -> Result<SnapshotMetadata, MetadataError> {
    let snap: SnapshotMetadata =
        serde_json::from_str(payload).map_err(|e| MetadataError::Malformed(e.to_string()))?;
    check_domain(&snap.domain)?;
    Ok(snap)
}

pub fn parse_targets(payload: &str) -> Result<TargetsMetadata, MetadataError> {
    let targets: TargetsMetadata =
        serde_json::from_str(payload).map_err(|e| MetadataError::Malformed(e.to_string()))?;
    check_domain(&targets.domain)?;
    Ok(targets)
}

fn decode_public_key(hex_str: &str) -> Option<[u8; 32]> {
    hex::decode(hex_str.trim()).ok()?.try_into().ok()
}

/// Verify that enough of `candidates` signed `payload` to meet `threshold`.
///
/// Deliberately takes the candidate set as an explicit parameter rather than
/// resolving it internally: a signature that is cryptographically valid but
/// from a key outside the role being checked (say, the snapshot key
/// presented against a root check) must not count, and the only way to make
/// that a property of this function rather than of caller discipline is to
/// never let it see keys outside the role in the first place.
pub fn verify_threshold(
    payload: &str,
    signatures: &[MetaSignature],
    candidates: &[&KeyEntry],
    threshold: u32,
    role_name: &str,
) -> Result<(), MetadataError> {
    if threshold == 0 {
        return Err(MetadataError::ZeroThreshold {
            role: role_name.to_string(),
            threshold,
        });
    }

    let bytes = crate::signature::canonical_bytes(payload);
    let mut satisfied: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();

    for sig in signatures {
        let Some(candidate) = candidates.iter().find(|k| k.key_id == sig.key_id) else {
            continue;
        };
        let Some(raw_key) = decode_public_key(&candidate.public_key_hex) else {
            continue;
        };
        let key = crate::signature::VerifyingKeyBytes(raw_key);
        if crate::signature::verify_bytes(&bytes, &sig.signature_hex, &key).is_ok() {
            satisfied.insert(candidate.key_id.as_str());
        }
    }

    if satisfied.len() as u32 >= threshold {
        Ok(())
    } else {
        Err(MetadataError::ThresholdNotMet {
            role: role_name.to_string(),
            needed: threshold,
            got: satisfied.len() as u32,
        })
    }
}
