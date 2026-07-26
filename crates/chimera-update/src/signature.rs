//! Ed25519 signature verification for Chimera's own app-update trust chain.
//!
//! This is a deliberate duplicate of
//! `services/mirror-contract/src/signature.rs`, not a shared dependency. G15
//! requires the app trust domain and the Codex payload trust domain to stay
//! independent all the way down, including the code that verifies them: a
//! shared verification routine is a shared blast radius, and a bug fixed (or
//! a compromise contained) in one domain would only protect the other by
//! coincidence. Two small, independently reviewable copies is the intent, not
//! an oversight — see ADR-006.
//!
//! Verification fails closed: a malformed key, a malformed signature and a
//! tampered payload are all errors, and none of them can be mistaken for a
//! pass.

use ed25519_dalek::{Signature, VerifyingKey};
use thiserror::Error;

/// A raw 32-byte Ed25519 public key, exactly as pinned in root metadata.
///
/// Kept as a plain byte array rather than `ed25519_dalek::VerifyingKey` so a
/// malformed key is a value this type can represent (checked lazily, at
/// verification time) rather than a panic at construction, and so metadata
/// parsing never has to reach into the crypto crate's own types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyingKeyBytes(pub [u8; 32]);

/// Everything that can go wrong verifying a signature. Every variant is a
/// refusal — there is deliberately no variant that means "skip verification".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SignatureError {
    #[error("signature is not a valid 64-byte Ed25519 encoding")]
    MalformedSignature,

    #[error("public key bytes are not a valid Ed25519 verifying key")]
    MalformedKey,

    #[error("signature does not match the payload")]
    Mismatch,
}

/// Verify a detached Ed25519 signature over the exact bytes given.
///
/// `payload` must be the exact bytes that were signed — see [`canonical_bytes`].
/// This function does no parsing of its own and trusts no string comparison,
/// only the underlying cryptographic check.
pub fn verify_bytes(
    payload: &[u8],
    signature_hex: &str,
    key: &VerifyingKeyBytes,
) -> Result<(), SignatureError> {
    let verifying_key =
        VerifyingKey::from_bytes(&key.0).map_err(|_| SignatureError::MalformedKey)?;

    let raw = hex::decode(signature_hex.trim()).map_err(|_| SignatureError::MalformedSignature)?;
    let sig_bytes: [u8; 64] = raw
        .try_into()
        .map_err(|_| SignatureError::MalformedSignature)?;
    let sig = Signature::from_bytes(&sig_bytes);

    verifying_key
        .verify_strict(payload, &sig)
        .map_err(|_| SignatureError::Mismatch)
}

/// The exact bytes a signature covers: the payload string's UTF-8 bytes with
/// surrounding whitespace removed, so a trailing newline picked up by a file
/// write or an editor does not invalidate an otherwise-valid signature. This
/// is a single function so every signer and every verifier in this crate
/// agree, byte for byte, on what "the payload" means.
pub fn canonical_bytes(payload: &str) -> Vec<u8> {
    payload.trim().as_bytes().to_vec()
}
