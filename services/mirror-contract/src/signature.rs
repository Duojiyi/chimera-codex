//! Ed25519 signature verification for mirror manifests (Spec 9.3, ADR-005).
//!
//! The mirror publishes a manifest alongside a detached Ed25519 signature. A
//! client must verify that signature against a pinned trust anchor before it
//! reads a single field, because every downstream decision — which asset to
//! download, which digest to expect, which capability manifest to pair — comes
//! from the manifest body.
//!
//! Verification fails closed. An unknown key id, a malformed signature, an
//! empty trust anchor, and a tampered payload are all errors; none of them can
//! be mistaken for a pass. There is deliberately no "skip verification" path.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

/// A raw 32-byte Ed25519 public key, exactly as published by the mirror gate.
///
/// Kept as a plain byte array rather than `ed25519_dalek::VerifyingKey` so
/// callers can hold and compare trust-anchor material without depending on
/// the crypto crate's own type, and so a malformed key is a value this type
/// can represent (checked lazily, at verification time) rather than a panic
/// at construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyingKeyBytes(pub [u8; 32]);

/// A manifest as published: the exact bytes that were signed, plus the detached
/// signature and the id of the key that produced it.
///
/// `payload` is stored as the original string rather than a parsed structure so
/// verification runs over the bytes the mirror actually signed. Re-serialising a
/// parsed value could reorder keys or change spacing and invalidate the
/// signature for reasons that have nothing to do with authenticity.
#[derive(Debug, Clone)]
pub struct SignedManifest {
    /// The manifest JSON exactly as published.
    pub payload: String,
    /// Which trusted key signed it.
    pub key_id: String,
    /// Detached Ed25519 signature, lowercase hex (128 chars / 64 bytes).
    pub signature_hex: String,
}

/// One public key the client is willing to trust.
#[derive(Debug, Clone)]
pub struct TrustedKey {
    pub key_id: String,
    /// Raw 32-byte Ed25519 public key.
    pub public_key: [u8; 32],
}

/// The set of keys this build trusts, pinned at compile time or shipped in the
/// installer. Never fetched from the mirror it validates — a trust anchor the
/// mirror can replace provides no protection.
#[derive(Debug, Clone)]
pub struct TrustAnchor {
    keys: Vec<TrustedKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SignatureError {
    #[error("no trusted key with id {key_id}")]
    UnknownKeyId { key_id: String },

    #[error("signature is not a valid 64-byte Ed25519 encoding")]
    MalformedSignature,

    #[error("public key bytes are not a valid Ed25519 verifying key")]
    MalformedKey,

    #[error("signature does not match the payload")]
    BadSignature,

    #[error("signature does not match the payload")]
    Mismatch,
}

/// Verify a detached Ed25519 signature over the exact bytes of a manifest.
///
/// This is the primitive the crate previously lacked entirely: `TrustAnchor`
/// resolves a key id and canonicalises a payload string, but underneath it —
/// and for any caller that already has raw bytes and a raw key — this
/// function does the actual cryptographic check with no string comparison,
/// no digest comparison, only a real Ed25519 verification.
///
/// `manifest_json` is verified byte-for-byte as given; callers are
/// responsible for passing the exact bytes that were signed.
pub fn verify_manifest_signature(
    manifest_json: &[u8],
    signature: &[u8],
    key: &VerifyingKeyBytes,
) -> Result<(), SignatureError> {
    let verifying_key =
        VerifyingKey::from_bytes(&key.0).map_err(|_| SignatureError::MalformedKey)?;

    let sig_bytes: [u8; 64] = signature
        .try_into()
        .map_err(|_| SignatureError::MalformedSignature)?;
    let sig = Signature::from_bytes(&sig_bytes);

    verifying_key
        .verify_strict(manifest_json, &sig)
        .map_err(|_| SignatureError::Mismatch)
}

impl TrustAnchor {
    pub fn new(keys: Vec<TrustedKey>) -> Self {
        Self { keys }
    }

    /// How many keys this anchor trusts. An empty anchor verifies nothing.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Verify a signed manifest.
    ///
    /// Returns `Ok(())` only when a trusted key with the claimed id exists and
    /// its signature over the canonical payload bytes is valid.
    pub fn verify(&self, signed: &SignedManifest) -> Result<(), SignatureError> {
        // Look the key up by the id the manifest claims. An unknown id is an
        // error rather than a search across all keys: the id is part of what
        // the mirror asserts, and silently trying other keys would let a
        // manifest signed by any trusted key pass under any id.
        let trusted = self
            .keys
            .iter()
            .find(|k| k.key_id == signed.key_id)
            .ok_or_else(|| SignatureError::UnknownKeyId {
                key_id: signed.key_id.clone(),
            })?;

        let raw = hex::decode(signed.signature_hex.trim())
            .map_err(|_| SignatureError::MalformedSignature)?;

        // Delegate the actual cryptography to `verify_manifest_signature` so
        // there is exactly one code path in this crate that calls into
        // ed25519-dalek's verifier.
        verify_manifest_signature(
            &canonical_bytes(&signed.payload),
            &raw,
            &VerifyingKeyBytes(trusted.public_key),
        )
        .map_err(|err| match err {
            SignatureError::Mismatch => SignatureError::BadSignature,
            other => other,
        })
    }
}

/// The exact bytes a signature covers.
///
/// Currently the payload string's UTF-8 bytes, with surrounding whitespace
/// removed so a trailing newline added by a file write does not invalidate an
/// otherwise valid signature. This is a single function so signer and verifier
/// can never disagree about what was signed.
pub fn canonical_bytes(payload: &str) -> Vec<u8> {
    payload.trim().as_bytes().to_vec()
}
