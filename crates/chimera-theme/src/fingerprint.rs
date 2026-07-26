//! Versioned compatibility fingerprint + fail-closed interpreter + skin-only
//! fuse — Step 8.4 (ADR-005, T48).
//!
//! Layering seam (see `lib.rs` module docs): this crate may not depend on
//! `services/mirror-contract` (also L2), so the shapes below are a **local
//! mirror**, not an import, of that crate's
//! `capability_manifest_url` / `_size_bytes` / `_sha256` binding triple and
//! its `binds_capability_manifest()` idea — read
//! `services/mirror-contract/src/manifest.rs` and `src/capability.rs` before
//! touching this file. [`ExpectedFingerprint`] is this crate's own narrow
//! read of "what a trusted capability manifest declared the skin surface
//! should look like"; the mirror gate is the only thing that ever produces
//! the trust-bearing manifest this reads *from*.
//!
//! **This crate produces CANDIDATE evidence only.** [`compute_fingerprint`]
//! returns a [`CandidateFingerprint`] — never signed, never published as a
//! manifest. That is not merely "undone" here, it is impossible by
//! construction: there is no signing-key type anywhere in this module (only
//! [`ed25519_dalek::VerifyingKey`], which can verify a signature but cannot
//! produce one), no `sign_*` function, and no code path that serialises a
//! `CandidateFingerprint` into anything resembling
//! `mirror_contract::capability::CapabilityManifest`. Promoting a candidate
//! into a trusted, signed capability manifest is Step 4.5's job, running on
//! the mirror gate, over a fleet of candidates — never a single client
//! deciding its own trust.
//!
//! A fingerprint mismatch (or a verified kill switch) only ever flips
//! [`SkinFuse`], and [`SkinFuse::skin_enabled`] is the *only* thing that
//! type can affect — there is no method here that touches Codex's own
//! process lifecycle, so tripping it cannot, even in principle, stop Codex
//! from launching (see `a_tripped_fuse_disables_only_the_skin_never_codex_launch`
//! in `tests/step8_4_fingerprint.rs`).

use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Schema version of the probe algorithm itself (not of any manifest).
///
/// Bumped whenever the set of signals folded into the digest changes, so a
/// fingerprint computed under an old algorithm is never compared against an
/// `ExpectedFingerprint` written for a new one — see
/// [`ExpectedFingerprint::validate`].
pub const PROBE_SCHEMA_VERSION: u32 = 1;

/// Raw, local signals this build's probe observed about the running Codex
/// UI shell, just before a skin would be injected.
///
/// Deliberately narrow: only the running Codex version and the CSS
/// selectors a shipped skin actually targets (i.e. exactly what
/// [`crate::css_allowlist`] could ever let a skin touch) — a compatibility
/// signal, not a general device/browser fingerprinting surface.
#[derive(Debug, Clone)]
pub struct ProbeInput {
    pub codex_version: String,
    /// CSS selectors observed live in the Codex shell DOM. Order does not
    /// matter — [`compute_fingerprint`] sorts and de-duplicates before
    /// hashing, so two probes that saw the same selectors in a different
    /// order (as DOM traversal order can vary run to run) still agree.
    pub observed_selectors: Vec<String>,
}

/// A versioned digest of a [`ProbeInput`] — candidate evidence only (see
/// module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateFingerprint {
    pub schema_version: u32,
    pub codex_version: String,
    /// Lowercase hex SHA-256 digest of the canonicalised probe input.
    pub digest_hex: String,
}

/// Fold a [`ProbeInput`] into a [`CandidateFingerprint`].
///
/// Every variable-length field is length-prefixed rather than delimited. A
/// separator only avoids boundary collisions for inputs that cannot contain
/// it, and a CSS selector can contain a newline — an adversarial review
/// demonstrated `["a\nb"]` and `["a", "b"]` hashing identically under the
/// earlier `\n`-delimited scheme.
///
/// That is the worst possible collision for this particular value. The
/// fingerprint's whole job is to notice that Codex's UI changed; two different
/// selector sets sharing a digest means a changed UI reporting as unchanged,
/// so the fuse never trips and a skin keeps injecting into a shell it was
/// never validated against.
pub fn compute_fingerprint(input: &ProbeInput) -> CandidateFingerprint {
    let mut selectors = input.observed_selectors.clone();
    selectors.sort();
    selectors.dedup();

    let mut hasher = Sha256::new();
    hasher.update(PROBE_SCHEMA_VERSION.to_le_bytes());
    hasher.update((input.codex_version.len() as u64).to_le_bytes());
    hasher.update(input.codex_version.as_bytes());
    // Length-prefixed, not newline-delimited. A selector may legitimately
    // contain a newline, and with a plain separator `["a\nb"]` and
    // `["a", "b"]` hash identically. For a value whose entire job is to notice
    // that Codex's UI changed, a collision means a changed UI reporting as
    // unchanged — the fuse silently fails to trip, which is the one outcome it
    // exists to prevent. The count is included for the same reason: without it
    // a trailing empty selector is invisible.
    hasher.update((selectors.len() as u64).to_le_bytes());
    for selector in &selectors {
        hasher.update((selector.len() as u64).to_le_bytes());
        hasher.update(selector.as_bytes());
    }
    let digest = hasher.finalize();

    CandidateFingerprint {
        schema_version: PROBE_SCHEMA_VERSION,
        codex_version: input.codex_version.clone(),
        digest_hex: hex::encode(digest),
    }
}

/// What a trusted capability manifest (produced elsewhere — see module
/// docs) declares this build's skin surface should fingerprint to.
///
/// Constructing one always means [`ExpectedFingerprint::validate`] already
/// passed, for the same reason as [`crate::schema::SkinManifest`]:
/// [`ExpectedFingerprint::parse`] is the only public constructor that
/// exists as a parsing entry point, so a caller reading one off the wire
/// cannot skip validation by construction... though see its doc comment for
/// the one honest caveat shared with `SkinManifest`.
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedFingerprint {
    pub schema_version: u32,
    pub codex_version: String,
    /// Lowercase hex SHA-256, exactly 64 characters — checked eagerly at
    /// parse time (shape), then compared case-insensitively against a
    /// candidate's digest at [`Self::matches`] time (value). Two different
    /// checks on purpose: shape failing early means a malformed value is
    /// refused before it is ever compared to anything, and cannot be
    /// mistaken for "just didn't match this candidate".
    pub digest_hex: String,
}

/// Why an [`ExpectedFingerprint`] was refused, or why it did not match a
/// [`CandidateFingerprint`].
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum FingerprintError {
    /// Covers "not JSON", "not UTF-8", and "JSON but missing/mistyped a
    /// required field" — see [`crate::schema::ManifestError::Malformed`]
    /// for why this crate does not re-derive serde_json's own message.
    #[error("expected fingerprint could not be read: {0}")]
    Malformed(String),
    #[error(
        "expected fingerprint schema_version {found} is not this build's supported version {supported}"
    )]
    UnsupportedSchemaVersion { found: u32, supported: u32 },
    #[error("expected codex_version must be non-empty")]
    EmptyCodexVersion,
    #[error("expected digest_hex must be exactly 64 lowercase hex characters, got {0:?}")]
    MalformedDigest(String),
    #[error("codex_version mismatch: expected {expected:?}, observed {observed:?}")]
    CodexVersionMismatch { expected: String, observed: String },
    #[error("fingerprint digest mismatch")]
    DigestMismatch,
}

/// True iff `s` is exactly 64 lowercase hex characters.
///
/// Mirrors `mirror_contract::manifest::is_lowercase_hex_sha256` byte for
/// byte (see module docs on why this crate cannot simply import it).
fn is_lowercase_hex_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl ExpectedFingerprint {
    /// Parse and validate an expected-fingerprint record in one step —
    /// never two public calls, so a value that parsed but was not yet
    /// validated is not a state a caller can hold.
    pub fn parse(bytes: &[u8]) -> Result<Self, FingerprintError> {
        let text =
            std::str::from_utf8(bytes).map_err(|e| FingerprintError::Malformed(e.to_string()))?;
        let value: ExpectedFingerprint =
            serde_json::from_str(text).map_err(|e| FingerprintError::Malformed(e.to_string()))?;
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), FingerprintError> {
        if self.schema_version != PROBE_SCHEMA_VERSION {
            return Err(FingerprintError::UnsupportedSchemaVersion {
                found: self.schema_version,
                supported: PROBE_SCHEMA_VERSION,
            });
        }
        if self.codex_version.trim().is_empty() {
            return Err(FingerprintError::EmptyCodexVersion);
        }
        if !is_lowercase_hex_sha256(&self.digest_hex) {
            return Err(FingerprintError::MalformedDigest(self.digest_hex.clone()));
        }
        Ok(())
    }

    /// Compare against a freshly computed candidate. Fails closed: every
    /// field that could disagree is checked, and the first disagreement
    /// refuses rather than being averaged away or ignored.
    pub fn matches(&self, candidate: &CandidateFingerprint) -> Result<(), FingerprintError> {
        if self.codex_version != candidate.codex_version {
            return Err(FingerprintError::CodexVersionMismatch {
                expected: self.codex_version.clone(),
                observed: candidate.codex_version.clone(),
            });
        }
        if !self.digest_hex.eq_ignore_ascii_case(&candidate.digest_hex) {
            return Err(FingerprintError::DigestMismatch);
        }
        Ok(())
    }
}

// ── The fuse: skin-only, never Codex ────────────────────────────────────────

/// Why [`SkinFuse`] tripped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TripReason {
    FingerprintMismatch(FingerprintError),
    KillSwitch { reason: String },
}

/// Disables the skin enhancement and nothing else.
///
/// There is deliberately no variant, method, or field here that names a
/// process, a window, or Codex's own lifecycle — see module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkinFuse {
    Engaged,
    Tripped(TripReason),
}

impl SkinFuse {
    /// The starting state: skin enhancement allowed.
    pub fn engaged() -> Self {
        SkinFuse::Engaged
    }

    /// Trip the fuse because of a fingerprint mismatch.
    pub fn trip_on_mismatch(reason: FingerprintError) -> Self {
        SkinFuse::Tripped(TripReason::FingerprintMismatch(reason))
    }

    /// Whether the skin enhancement may run right now. The only externally
    /// observable effect of this type — nothing here can be asked "may
    /// Codex launch", because nothing here reads this fuse but the skin
    /// path.
    pub fn skin_enabled(&self) -> bool {
        matches!(self, SkinFuse::Engaged)
    }

    /// Why the fuse is tripped, if it is.
    pub fn trip_reason(&self) -> Option<&TripReason> {
        match self {
            SkinFuse::Engaged => None,
            SkinFuse::Tripped(reason) => Some(reason),
        }
    }

    /// Apply a server-side kill switch signal.
    ///
    /// `self` is left completely unchanged whenever `signed` fails
    /// verification (unknown key, malformed encoding, tampered payload) —
    /// obeying an unverified instruction would let anyone who can merely
    /// *reach* the client (no server compromise required — a
    /// man-in-the-middle, a replayed request to the wrong endpoint) disable
    /// the feature for every user, which is a strictly worse failure mode
    /// than "the kill switch didn't work this one time".
    pub fn apply_kill_switch(
        &mut self,
        anchor: &KillSwitchTrustAnchor,
        signed: &SignedKillSwitch,
    ) -> Result<(), KillSwitchError> {
        let payload = anchor.verify(signed)?;
        if payload.disable_skin {
            *self = SkinFuse::Tripped(TripReason::KillSwitch {
                reason: payload.reason,
            });
        }
        Ok(())
    }
}

// ── Signed kill switch ──────────────────────────────────────────────────────

/// A server-published instruction to disable the skin enhancement — never
/// Codex itself. Trusted only once [`KillSwitchTrustAnchor::verify`] passes.
///
/// Shape mirrors `services/mirror-contract::signature::SignedManifest`
/// (payload + key_id + detached hex signature) deliberately: one
/// signed-envelope shape across the codebase rather than a second bespoke
/// one invented here (see module docs on the layering that forbids
/// importing that crate's type directly).
#[derive(Debug, Clone)]
pub struct SignedKillSwitch {
    /// The kill-switch JSON body exactly as published — verified byte for
    /// byte (after trimming surrounding whitespace), never a re-serialised
    /// reconstruction, so re-ordering/re-formatting can never invalidate a
    /// genuine signature or paper over a tampered one.
    pub payload: String,
    pub key_id: String,
    /// Detached Ed25519 signature, lowercase or uppercase hex (128 chars /
    /// 64 bytes).
    pub signature_hex: String,
}

/// The JSON body a [`SignedKillSwitch`] carries, once verified.
///
/// Returned by [`KillSwitchTrustAnchor::verify`] so a caller can inspect
/// `reason` (e.g. for a diagnostics log) even when `disable_skin` is
/// `false` — verifying a signal and acting on it are deliberately separate
/// steps.
#[derive(Debug, Clone, Deserialize)]
pub struct KillSwitchPayload {
    /// Read for shape/forward-compatibility only; not yet compared against
    /// anything (the kill switch has no versioned variants yet).
    pub schema_version: u32,
    pub disable_skin: bool,
    pub reason: String,
}

/// The set of keys this build trusts for kill-switch signals, pinned at
/// compile time or shipped in the installer — never fetched from the same
/// server it is meant to hold accountable.
#[derive(Debug, Clone)]
pub struct KillSwitchTrustAnchor {
    keys: Vec<(String, [u8; 32])>,
}

/// Why a [`SignedKillSwitch`] was refused (and therefore ignored — see
/// [`SkinFuse::apply_kill_switch`]).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KillSwitchError {
    #[error("no trusted key with id {key_id}")]
    UnknownKeyId { key_id: String },
    #[error("signature is not a valid 64-byte Ed25519 encoding")]
    MalformedSignature,
    #[error("public key bytes are not a valid Ed25519 verifying key")]
    MalformedKey,
    #[error("signature does not match the payload")]
    BadSignature,
    #[error("kill switch payload could not be parsed: {0}")]
    MalformedPayload(String),
}

impl KillSwitchTrustAnchor {
    pub fn new(keys: Vec<(String, [u8; 32])>) -> Self {
        Self { keys }
    }

    /// Verify a signed kill switch and, only on success, return its parsed
    /// payload. An unknown key id is resolved to a specific key first
    /// (never tried against every trusted key in turn) — trying every key
    /// would let a signature genuinely produced by one trusted key verify
    /// under a different key's claimed identity.
    pub fn verify(&self, signed: &SignedKillSwitch) -> Result<KillSwitchPayload, KillSwitchError> {
        let (_, public_key) = self
            .keys
            .iter()
            .find(|(id, _)| id == &signed.key_id)
            .ok_or_else(|| KillSwitchError::UnknownKeyId {
                key_id: signed.key_id.clone(),
            })?;

        let sig_bytes = hex::decode(signed.signature_hex.trim())
            .map_err(|_| KillSwitchError::MalformedSignature)?;
        let sig_array: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| KillSwitchError::MalformedSignature)?;
        let signature = Signature::from_bytes(&sig_array);

        let verifying_key =
            VerifyingKey::from_bytes(public_key).map_err(|_| KillSwitchError::MalformedKey)?;

        verifying_key
            .verify_strict(signed.payload.trim().as_bytes(), &signature)
            .map_err(|_| KillSwitchError::BadSignature)?;

        serde_json::from_str(signed.payload.trim())
            .map_err(|e| KillSwitchError::MalformedPayload(e.to_string()))
    }
}
