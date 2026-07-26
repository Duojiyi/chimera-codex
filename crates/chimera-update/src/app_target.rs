//! `chimera-app-latest.json` treated strictly as a pinned target (Step 9.1
//! "app version/downgrade decision").
//!
//! [`crate::trust::verify_chain`] establishes that the *targets metadata* is
//! trustworthy — the map of file names to length+hash. It says nothing about
//! the bytes of any one file, because those are fetched separately (see
//! [`crate::fetch::MetadataFetcher::fetch_target_file`]) and are not
//! themselves signed. This module is the bridge from "the map is trustworthy"
//! to "these particular bytes are the thing the map says they are": look up
//! the pinned length+hash for `chimera-app-latest.json`, check the bytes
//! against *both* before doing anything else with them, and only then parse.
//!
//! That ordering is load-bearing, not stylistic. If the JSON parser ran
//! first, a parser bug (or an intentionally malformed document crafted to
//! exploit one) would be reachable by anyone who can answer an HTTP request,
//! signature or no signature. Checking length+hash first means the only
//! bytes that ever reach `serde_json` are bytes a signed, verified chain has
//! already vouched for byte-for-byte.
//!
//! The other hazard this module owns is downgrade. A version chain in the
//! wild being replayed to reinstall an old, vulnerable release is the
//! textbook TUF rollback attack — except here it cannot be caught by
//! [`crate::trust`]'s version-monotonicity check, because that check is about
//! *metadata* versions (root/timestamp/snapshot/targets), and an attacker
//! doesn't need an old metadata version to serve an old *app* version: the
//! newest, most current targets metadata can legitimately point at
//! `chimera-app-latest.json` describing app version 1.0.0 while the machine
//! asking already runs 2.0.0, simply because that is what the server has
//! configured right now. Refusing that requires comparing the *app* version
//! in the target's own body against what is installed, which is this
//! module's job, not `trust`'s.

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::trust::VerifiedChain;

/// The one target this module ever looks up. A constant rather than a
/// caller-supplied string so `crate::fetch::MetadataFetcher::fetch_target_file`
/// and this module's lookup can never drift apart into fetching one path and
/// checking another.
pub const APP_TARGET_PATH: &str = "chimera-app-latest.json";

/// The parsed body of `chimera-app-latest.json`.
///
/// Never constructed from untrusted bytes except through [`decide`], which
/// checks length and digest against the signed targets entry before this is
/// ever handed to a JSON parser.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppLatest {
    /// Semver string, e.g. `"2.1.0"`. Kept as the raw string here rather than
    /// a parsed `Version` so a document with an unparseable version is still
    /// representable — [`decide`] turns a bad version into a first-class
    /// [`AppTargetError::InvalidVersion`] refusal, not a deserialisation
    /// failure indistinguishable from a truncated download.
    pub version: String,
    /// Where the installer for `version` can be fetched. Not used by
    /// [`decide`] itself — carried through to whatever caller actually
    /// downloads and installs it.
    pub download_url: String,
    /// Installer digest. Pins the *installer*, a separate artefact from this
    /// target document; the caller that downloads it is responsible for
    /// checking this before running anything.
    pub sha256_hex: String,
    pub length: u64,
    /// Present only on a release that intentionally supersedes a *newer* one
    /// that shipped and was pulled (a bad release walked back). [`decide`]
    /// refuses any downgrade whose installed version does not match this
    /// field exactly — see the module doc comment for why nothing upstream
    /// of this document can make that call instead.
    pub downgrade_authorized_from: Option<String>,
}

/// The result of evaluating a verified `chimera-app-latest.json` against the
/// version currently installed. Inert data — see this crate's top-level doc
/// comment: acting on it belongs to a caller in `apps/chimera-desktop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    /// The installed version already matches `chimera-app-latest.json`.
    UpToDate { installed: Version },
    /// A newer version is available, or a downgrade was explicitly
    /// authorised for exactly the installed version (see
    /// [`AppLatest::downgrade_authorized_from`]).
    UpdateAvailable {
        installed: Version,
        latest: AppLatest,
    },
    /// `chimera-app-latest.json` names a version older than the one
    /// installed, and did not explicitly authorise replacing this exact
    /// installed version. Refused rather than silently ignored, so a caller
    /// can surface *why* nothing happened instead of the check appearing to
    /// have done nothing.
    DowngradeRefused {
        installed: Version,
        offered: Version,
    },
}

/// Everything that can go wrong turning target bytes into an [`UpdateDecision`].
/// Every variant is a refusal to proceed past that point — there is no
/// "assume it's fine" outcome.
#[derive(Debug, Error)]
pub enum AppTargetError {
    /// The signed targets metadata has no entry for [`APP_TARGET_PATH`] at
    /// all. Refused rather than treated as "no update" — an update chain that
    /// no longer mentions its own pointer file has a shape nothing signed for
    /// this release should ever take.
    #[error("the update pointer file is missing from the signed release manifest")]
    UnknownTarget,

    /// Checked before the digest — a length mismatch is cheaper to detect and
    /// already sufficient to refuse, so there is no reason to hash bytes that
    /// are already known not to match.
    #[error("the update pointer file's size does not match the signed release manifest")]
    LengthMismatch { expected: u64, actual: u64 },

    /// The bytes' sha256 does not match what the signed targets metadata
    /// pinned. Checked before any JSON parsing is attempted — see the module
    /// doc comment.
    #[error("the update pointer file's contents do not match the signed release manifest")]
    DigestMismatch,

    /// The bytes matched their pin exactly but are not valid JSON, or not the
    /// shape [`AppLatest`] expects. Never includes the underlying serde
    /// error text, which could echo back arbitrary attacker-controlled bytes.
    #[error("the update pointer file is not in a format this version of Chimera understands")]
    Malformed(String),

    /// `version` is not a value [`semver::Version`] can parse.
    #[error("the update pointer file names a version that is not a valid version number")]
    InvalidVersion(String),
}

/// Verify `raw_target_bytes` against the pinned length and digest for
/// [`APP_TARGET_PATH`] in `chain.targets`, then decide whether it describes
/// an upgrade, a no-op, or a downgrade to refuse.
///
/// `chain` must already have come out of [`crate::trust::verify_chain`] — this
/// function does not re-check signatures, expiry, or rollback on the
/// metadata itself; it only extends that already-established trust to one
/// specific file's bytes.
pub fn decide(
    chain: &VerifiedChain,
    raw_target_bytes: &[u8],
    installed_version: &Version,
) -> Result<UpdateDecision, AppTargetError> {
    let entry = chain
        .targets
        .targets
        .get(APP_TARGET_PATH)
        .ok_or(AppTargetError::UnknownTarget)?;

    // Length first: cheaper than hashing, and a mismatch alone is already
    // grounds for refusal, so there is nothing to gain from computing a
    // digest over bytes already known not to match.
    let actual_len = raw_target_bytes.len() as u64;
    if actual_len != entry.length {
        return Err(AppTargetError::LengthMismatch {
            expected: entry.length,
            actual: actual_len,
        });
    }

    let actual_hex = format!("{:x}", Sha256::digest(raw_target_bytes));
    if !actual_hex.eq_ignore_ascii_case(&entry.sha256_hex) {
        return Err(AppTargetError::DigestMismatch);
    }

    // Only now — after length and digest both matched the signed pin — does
    // this touch a JSON parser.
    let latest: AppLatest = serde_json::from_slice(raw_target_bytes)
        .map_err(|e| AppTargetError::Malformed(e.to_string()))?;

    let latest_version = Version::parse(&latest.version)
        .map_err(|e| AppTargetError::InvalidVersion(e.to_string()))?;

    use std::cmp::Ordering;
    match latest_version.cmp(installed_version) {
        Ordering::Equal => Ok(UpdateDecision::UpToDate {
            installed: installed_version.clone(),
        }),
        Ordering::Greater => Ok(UpdateDecision::UpdateAvailable {
            installed: installed_version.clone(),
            latest,
        }),
        Ordering::Less => {
            let authorised = latest
                .downgrade_authorized_from
                .as_deref()
                .and_then(|v| Version::parse(v).ok())
                .is_some_and(|v| &v == installed_version);
            if authorised {
                Ok(UpdateDecision::UpdateAvailable {
                    installed: installed_version.clone(),
                    latest,
                })
            } else {
                Ok(UpdateDecision::DowngradeRefused {
                    installed: installed_version.clone(),
                    offered: latest_version,
                })
            }
        }
    }
}
