//! The network seam for the app-update trust chain.
//!
//! A trait rather than a concrete HTTP client so every test in this crate —
//! including the offline test and every attack simulation — runs without a
//! socket. A real implementation (reqwest-based, matching the pattern in
//! `chimera-provider::probe`) belongs to the caller in `apps/chimera-desktop`;
//! this crate only defines the contract it verifies against.

use thiserror::Error;

use crate::metadata::SignedPayload;

/// A transport-level failure. Kept small and typed (no raw error strings)
/// so a user-facing message can be built from it without ever risking a
/// leaked URL, stack trace, or key.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FetchError {
    #[error("could not reach the update server")]
    Offline,

    #[error("the update server returned an error")]
    Server(String),

    #[error("no such target file: {0}")]
    NotFound(String),
}

/// Everything the trust chain needs to pull over the network, one call per
/// TUF role plus the actual target file bytes.
pub trait MetadataFetcher {
    /// The next root document after `after_version`, if the server has one.
    /// `Ok(None)` means rotation is complete — the caller already holds the
    /// newest root the server will offer. Whatever version the returned
    /// document *claims* to be is not trusted here; verifying it is exactly
    /// one more than `after_version` is [`crate::trust`]'s job.
    fn fetch_root_next(&self, after_version: u64) -> Result<Option<SignedPayload>, FetchError>;

    fn fetch_timestamp(&self) -> Result<SignedPayload, FetchError>;

    fn fetch_snapshot(&self) -> Result<SignedPayload, FetchError>;

    fn fetch_targets(&self) -> Result<SignedPayload, FetchError>;

    /// Raw bytes of a target file (e.g. `chimera-app-latest.json`). Never
    /// trusted on the strength of this call alone — the caller must hash the
    /// result and compare it against what signed targets metadata declared.
    fn fetch_target_file(&self, path: &str) -> Result<Vec<u8>, FetchError>;
}
