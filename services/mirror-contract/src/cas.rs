//! Compare-and-swap stable pointer promotion.
//! Spec 9.3: stable promotion uses CAS; older workflow cannot overwrite newer stable.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StablePointer {
    pub codex_version: String,
    pub raw_digest: String,
    pub manifest_digest: String,
    pub promoted_at: String,
    /// Monotonic counter — only increments; CAS checks this before writing
    pub sequence: u64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CasError {
    #[error("CAS conflict: expected sequence {expected}, found {actual}")]
    SequenceConflict { expected: u64, actual: u64 },
    #[error("new sequence {new} must be greater than current {current}")]
    StalePromotion { new: u64, current: u64 },
    #[error("digest mismatch: pointer digest {pointer} vs manifest digest {manifest}")]
    DigestMismatch { pointer: String, manifest: String },
}

/// Validate that a proposed stable pointer can replace the current one.
/// Returns Ok(()) if the promotion is allowed, Err(CasError) otherwise.
pub fn validate_stable_promotion(
    current: &StablePointer,
    proposed: &StablePointer,
) -> Result<(), CasError> {
    // Monotonic sequence check — proposed must be strictly greater
    if proposed.sequence <= current.sequence {
        return Err(CasError::StalePromotion {
            new: proposed.sequence,
            current: current.sequence,
        });
    }
    Ok(())
}

/// Verify that a manifest's digest matches the pointer's recorded digest.
pub fn verify_manifest_digest(
    pointer: &StablePointer,
    actual_manifest_digest: &str,
) -> Result<(), CasError> {
    if pointer.manifest_digest != actual_manifest_digest {
        return Err(CasError::DigestMismatch {
            pointer: pointer.manifest_digest.clone(),
            manifest: actual_manifest_digest.to_string(),
        });
    }
    Ok(())
}
