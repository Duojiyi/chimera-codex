//! Step 9.1 — the verification chain.
//!
//! Every other module here parses or fetches. This one decides whether to
//! believe what was parsed, which makes it the module that decides what code
//! runs on the user's machine next.
//!
//! Signature checking alone is not enough, and that is the entire reason this
//! file is more than a loop over `verify_threshold`. Each of the following
//! attacks uses documents whose signatures are perfectly valid:
//!
//! - **Rollback** — serve an older release to undo a fix. Beaten by refusing
//!   any version below one already trusted.
//! - **Freeze** — serve yesterday's timestamp forever so the client never
//!   learns a newer snapshot exists. Beaten by expiry, which is the only thing
//!   that distinguishes "stale" from "unchanged".
//! - **Mix and match** — pair a targets list with a snapshot that vouched for
//!   a different one. Beaten by the hash and version pins between layers.
//! - **Key compromise** — an online timestamp key authorising a targets list.
//!   Beaten by only ever offering a role its own keys as candidates.
//!
//! Ordering is deliberate: root first, because it is the document that says
//! which keys the other three roles have; then timestamp, snapshot, targets,
//! each checked against the pin the layer above it published. A check moved
//! earlier than its prerequisite would validate against something not yet
//! established as trustworthy.

use sha2::{Digest, Sha256};

use crate::clock::Clock;
use crate::metadata::{
    MetadataError, Role, RootMetadata, SignedPayload, SnapshotMetadata, TargetsMetadata,
    TimestampMetadata, parse_root, parse_snapshot, parse_targets, parse_timestamp,
    verify_threshold,
};

/// Highest version already accepted for each role.
///
/// Persisted between runs (see [`crate::cache`]) and fed back in as the
/// rollback floor. Without it every run would start from zero and rollback
/// protection would be a property of nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrustedVersions {
    pub root: u64,
    pub timestamp: u64,
    pub snapshot: u64,
    pub targets: u64,
}

/// A chain that passed every check, and the versions to record for next time.
#[derive(Debug, Clone)]
pub struct VerifiedChain {
    pub root: RootMetadata,
    pub timestamp: TimestampMetadata,
    pub snapshot: SnapshotMetadata,
    pub targets: TargetsMetadata,
    pub versions: TrustedVersions,
}

/// Every way the chain can refuse. There is no non-refusal variant: this type
/// exists only to say why something was not believed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TrustError {
    #[error("{0}")]
    Metadata(#[from] MetadataError),

    #[error("{role:?} metadata expired at {expires} (now {now})")]
    Expired { role: Role, expires: i64, now: i64 },

    #[error("{role:?} offered version {offered}, older than the trusted {have}")]
    Rollback { role: Role, have: u64, offered: u64 },

    #[error("{role:?} does not match the digest the layer above pinned")]
    HashMismatch {
        role: Role,
        expected: String,
        actual: String,
    },

    #[error("{role:?} version {actual} does not match the pinned {expected}")]
    VersionMismatch {
        role: Role,
        expected: u64,
        actual: u64,
    },

    #[error("{role:?} signatures do not satisfy its role")]
    Signature { role: Role },

    #[error(
        "the compiled-in development trust root is not usable in a release build; \
         it must be replaced by a root from a real offline key ceremony"
    )]
    DevelopmentRootRefused,
}

/// What a caller is willing to trust.
///
/// Exists for one reason: a review found that `bundled_root::is_development_root`
/// was defined and never called, so the placeholder root shipped in the binary
/// would have been accepted in production exactly like a real one. A detection
/// hook nobody invokes is not a control.
///
/// Kept as a parameter rather than a `cfg!(debug_assertions)` check inside the
/// verifier, because a rule that only exists in release builds is a rule no
/// test can ever observe firing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustPolicy {
    /// Only ever true for a development build talking to a development mirror.
    pub allow_development_root: bool,
}

impl TrustPolicy {
    /// What a shipped build uses.
    pub const RELEASE: Self = Self {
        allow_development_root: false,
    };

    /// What `verify_chain` selects automatically. A debug build may bootstrap
    /// from the bundled placeholder; a release build never can.
    pub fn for_this_build() -> Self {
        Self {
            allow_development_root: cfg!(debug_assertions),
        }
    }
}

fn sha256_hex(payload: &str) -> String {
    format!("{:x}", Sha256::digest(payload.as_bytes()))
}

fn check_expiry(role: Role, expires: i64, now: i64) -> Result<(), TrustError> {
    if now > expires {
        return Err(TrustError::Expired { role, expires, now });
    }
    Ok(())
}

fn check_rollback(role: Role, have: Option<u64>, offered: u64) -> Result<(), TrustError> {
    // Equal is fine: re-fetching an unchanged document is the normal case, and
    // treating it as an attack would break every run in which nothing shipped.
    if let Some(have) = have {
        if offered < have {
            return Err(TrustError::Rollback {
                role,
                have,
                offered,
            });
        }
    }
    Ok(())
}

/// Check the digest and version a higher layer published against what arrived.
fn check_pin(
    role: Role,
    pinned_digest: &str,
    pinned_version: u64,
    payload: &str,
    actual_version: u64,
) -> Result<(), TrustError> {
    let actual = sha256_hex(payload);
    if !actual.eq_ignore_ascii_case(pinned_digest) {
        return Err(TrustError::HashMismatch {
            role,
            expected: pinned_digest.to_string(),
            actual,
        });
    }
    // Both pins must hold. Checking only the digest would admit a document
    // whose version field lies — and the rollback check reads exactly that
    // field, so a lie there disables rollback protection for the role.
    if actual_version != pinned_version {
        return Err(TrustError::VersionMismatch {
            role,
            expected: pinned_version,
            actual: actual_version,
        });
    }
    Ok(())
}

/// Verify a document's signatures against the keys `root` assigns to `role`.
///
/// Candidates come from the role's own key list, never the whole key set: a
/// signature that is cryptographically valid but made by a key belonging to a
/// different role must not count, and confining the candidate set is what
/// makes that a property of the code rather than of caller discipline.
fn check_signatures(
    root: &RootMetadata,
    role: Role,
    signed: &SignedPayload,
) -> Result<(), TrustError> {
    let candidates = root.resolve_keys(role);
    let threshold = root.role(role).threshold;
    verify_threshold(
        &signed.payload,
        &signed.signatures,
        &candidates,
        threshold,
        role.name(),
    )
    .map_err(|_| TrustError::Signature { role })
}

/// Verify a full root → timestamp → snapshot → targets chain.
///
/// `previous` is the versions already trusted, or `None` on a first run from
/// the bundled root. Returns the parsed documents plus the versions to persist.
pub fn verify_chain(
    root_doc: &SignedPayload,
    timestamp_doc: &SignedPayload,
    snapshot_doc: &SignedPayload,
    targets_doc: &SignedPayload,
    clock: &dyn Clock,
    previous: Option<&TrustedVersions>,
) -> Result<VerifiedChain, TrustError> {
    verify_chain_with_policy(
        root_doc,
        timestamp_doc,
        snapshot_doc,
        targets_doc,
        clock,
        previous,
        TrustPolicy::for_this_build(),
    )
}

/// [`verify_chain`] with the trust policy stated explicitly.
///
/// The only reason to call this directly is to assert release behaviour from a
/// test that is itself running in a debug build.
pub fn verify_chain_with_policy(
    root_doc: &SignedPayload,
    timestamp_doc: &SignedPayload,
    snapshot_doc: &SignedPayload,
    targets_doc: &SignedPayload,
    clock: &dyn Clock,
    previous: Option<&TrustedVersions>,
    policy: TrustPolicy,
) -> Result<VerifiedChain, TrustError> {
    let now = clock.now();

    // ── Root ────────────────────────────────────────────────────────────────
    // Parsing checks the trust domain before anything else, so a Codex mirror
    // document is refused here rather than later by a signature mismatch that
    // could be misread as "the mirror rotated a key" (G8/G15).
    let root = parse_root(&root_doc.payload)?;

    // Before any cryptography: the compiled-in placeholder is a valid,
    // self-consistent root, so every signature check below would pass on it.
    // Only this refuses it.
    if !policy.allow_development_root && crate::bundled_root::is_development_root(&root) {
        return Err(TrustError::DevelopmentRootRefused);
    }

    // Root is self-signed by definition — it is the document that declares
    // which key root is. Accepting one signed by anything else would mean
    // accepting a stranger's claim about whom to trust.
    check_signatures(&root, Role::Root, root_doc)?;
    check_expiry(Role::Root, root.expires, now)?;
    check_rollback(Role::Root, previous.map(|p| p.root), root.version)?;

    // ── Timestamp ───────────────────────────────────────────────────────────
    let timestamp = parse_timestamp(&timestamp_doc.payload)?;
    check_signatures(&root, Role::Timestamp, timestamp_doc)?;
    check_expiry(Role::Timestamp, timestamp.expires, now)?;
    check_rollback(
        Role::Timestamp,
        previous.map(|p| p.timestamp),
        timestamp.version,
    )?;

    // ── Snapshot, pinned by the timestamp ───────────────────────────────────
    let snapshot = parse_snapshot(&snapshot_doc.payload)?;
    check_signatures(&root, Role::Snapshot, snapshot_doc)?;
    check_expiry(Role::Snapshot, snapshot.expires, now)?;
    check_pin(
        Role::Snapshot,
        &timestamp.snapshot_sha256_hex,
        timestamp.snapshot_version,
        &snapshot_doc.payload,
        snapshot.version,
    )?;
    check_rollback(
        Role::Snapshot,
        previous.map(|p| p.snapshot),
        snapshot.version,
    )?;

    // ── Targets, pinned by the snapshot ─────────────────────────────────────
    let targets = parse_targets(&targets_doc.payload)?;
    check_signatures(&root, Role::Targets, targets_doc)?;
    check_expiry(Role::Targets, targets.expires, now)?;
    check_pin(
        Role::Targets,
        &snapshot.targets_sha256_hex,
        snapshot.targets_version,
        &targets_doc.payload,
        targets.version,
    )?;
    check_rollback(Role::Targets, previous.map(|p| p.targets), targets.version)?;

    let versions = TrustedVersions {
        root: root.version,
        timestamp: timestamp.version,
        snapshot: snapshot.version,
        targets: targets.version,
    };

    Ok(VerifiedChain {
        root,
        timestamp,
        snapshot,
        targets,
        versions,
    })
}

/// Accept a replacement root only if it is the immediate successor of the one
/// already trusted, and is signed by BOTH.
///
/// Signed by the old root because that is what authorises the change, and by
/// the new one because a root that cannot sign for itself is unusable
/// afterwards. Requiring consecutive versions is what stops an attacker
/// holding one compromised historical root key from jumping a client straight
/// to a root of their own: every intermediate rotation has to check out too.
pub fn accept_root_rotation(
    current: &RootMetadata,
    candidate_doc: &SignedPayload,
    clock: &dyn Clock,
) -> Result<RootMetadata, TrustError> {
    let candidate = parse_root(&candidate_doc.payload)?;

    if candidate.version != current.version + 1 {
        // A skipped version and a replay both land here. Reported as rollback
        // because the effect is identical: the client would end up trusting a
        // root the current one never authorised.
        return Err(TrustError::Rollback {
            role: Role::Root,
            have: current.version + 1,
            offered: candidate.version,
        });
    }

    check_signatures(current, Role::Root, candidate_doc)?;
    check_signatures(&candidate, Role::Root, candidate_doc)?;
    check_expiry(Role::Root, candidate.expires, clock.now())?;

    Ok(candidate)
}
