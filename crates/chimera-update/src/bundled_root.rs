//! The compiled-in trust seed a fresh install starts from (Step 9.1
//! bootstrap).
//!
//! Every other document in [`crate::trust`]'s chain is verified against a
//! root that was, at some point, itself accepted — either fetched and rotated
//! into place via [`crate::trust::accept_root_rotation`], or, on a completely
//! fresh install with no prior state, this one. That makes this module the
//! base case the entire chain's trust is grounded in: if this file lies about
//! which keys are authoritative, nothing downstream can catch it, because
//! downstream is defined as "whatever this vouches for".
//!
//! # THIS IS A DEVELOPMENT PLACEHOLDER — NOT A PRODUCTION TRUST ANCHOR
//!
//! No production signing key exists yet. [`development_root`] embeds a
//! keypair derived from a fixed, publicly-known seed
//! ([`DEV_INSECURE_ROOT_SEED`]) directly in source, which means anyone who
//! can read this repository — which is everyone, since it is source — can
//! forge a root the exact same code would accept. That is fine for the
//! purpose this serves today (exercising the verification chain end to end
//! in tests) and would be a total compromise of every install if it ever
//! shipped as the seed for a real release.
//!
//! Before release this must be replaced with a root produced by a real
//! offline root key, generated and stored the way ADR-006 requires (not in
//! source control). Until then:
//!
//! - Every identifying string this module produces — the key id, and this
//!   constant's own name — says "dev" and "insecure" so a grep or a glance at
//!   signed JSON gives it away.
//! - [`is_development_root`] is a runtime check a release bootstrap can call
//!   on whatever root it is about to trust from cold (i.e. with no prior
//!   cached state to compare against) and hard-refuse to start if it answers
//!   `true` outside of an explicit development mode. Wiring that refusal into
//!   `apps/chimera-desktop`'s startup path is release-blocking work this
//!   crate cannot do on its own behalf — see this crate's report for exactly
//!   what remains.

use ed25519_dalek::{Signer, SigningKey};
use thiserror::Error;

use crate::metadata::{
    APP_TRUST_DOMAIN, KeyEntry, MetaSignature, RoleKeys, RootMetadata, SignedPayload,
};
use crate::signature::canonical_bytes;

/// Fixed seed for the development root's signing key. Not a secret — it is
/// checked into source deliberately, because there is nothing to protect
/// yet: no production install has ever trusted a root derived from this seed,
/// and none ever should. Its only job is to make [`development_root`]
/// reproducible across runs and machines.
const DEV_INSECURE_ROOT_SEED: [u8; 32] = [0x44; 32]; // 0x44 = ASCII 'D'(evelopment), mnemonic only.

/// Key id for the development root's sole key. Deliberately verbose: this
/// string ends up in signed JSON that a support bundle or a bug report could
/// surface verbatim, so it has to explain itself without any other context.
const DEV_INSECURE_ROOT_KEY_ID: &str = "chimera-dev-insecure-DO-NOT-SHIP-root-1";

/// Version of the bootstrap root. Always 1 — a fresh install has no history
/// to have rotated through yet; rotation only ever moves forward from here.
const DEV_INSECURE_ROOT_VERSION: u64 = 1;

/// Far enough out that no test or development run expires it, but not so far
/// that it reads as a considered production value — 2099 is deliberately a
/// round, obviously-placeholder date rather than "10 years from whenever this
/// was written".
const DEV_INSECURE_ROOT_EXPIRES: i64 = 4_070_908_800; // 2099-01-01T00:00:00Z

/// Everything that can go wrong constructing the bundled root. In practice
/// unreachable for the fixed, well-formed data this module builds — kept
/// typed rather than assumed-infallible, matching this crate's rule that a
/// serialisation step is a `Result`, not an `unwrap`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BundledRootError {
    #[error("the compiled-in trust seed could not be encoded")]
    Encode,
}

fn dev_signing_key() -> SigningKey {
    SigningKey::from_bytes(&DEV_INSECURE_ROOT_SEED)
}

/// The development root document, signed by itself.
///
/// A fresh install with no cached trust state (see [`crate::cache`]) starts
/// its very first [`crate::trust::verify_chain`] call from this. Every
/// subsequent run instead uses whatever root was last cached — this function
/// is consulted exactly once in the life of a real install, on the first run,
/// which is also exactly why a placeholder here is tolerable today: nothing
/// in this crate's own tests depends on the key material being any
/// particular value, only on it being internally consistent.
pub fn development_root() -> Result<SignedPayload, BundledRootError> {
    let signing = dev_signing_key();
    let key_entry = KeyEntry {
        key_id: DEV_INSECURE_ROOT_KEY_ID.to_string(),
        public_key_hex: hex::encode(signing.verifying_key().to_bytes()),
    };
    // One key, every role — there are no other keys yet to split roles
    // across. A production root MUST NOT reuse a single key across roles;
    // see the module doc comment for what has to change before release.
    let role = RoleKeys {
        key_ids: vec![DEV_INSECURE_ROOT_KEY_ID.to_string()],
        threshold: 1,
    };
    let root = RootMetadata {
        domain: APP_TRUST_DOMAIN.to_string(),
        version: DEV_INSECURE_ROOT_VERSION,
        expires: DEV_INSECURE_ROOT_EXPIRES,
        keys: vec![key_entry],
        root: role.clone(),
        targets: role.clone(),
        snapshot: role.clone(),
        timestamp: role,
    };

    let payload = serde_json::to_string(&root).map_err(|_| BundledRootError::Encode)?;
    let signature_hex = hex::encode(signing.sign(&canonical_bytes(&payload)).to_bytes());
    let signatures = vec![MetaSignature {
        key_id: DEV_INSECURE_ROOT_KEY_ID.to_string(),
        signature_hex,
    }];

    Ok(SignedPayload {
        payload,
        signatures,
    })
}

/// Does this root describe the development placeholder rather than a real
/// trust anchor?
///
/// Checked by key id rather than by comparing the whole document: the key id
/// is the one field a rotated-in production root can never accidentally
/// share with this placeholder (a real root generated by a real offline key
/// ceremony has no reason to ever choose this exact string), whereas
/// `version`/`expires` are plain integers a production root could
/// legitimately match by coincidence.
pub fn is_development_root(root: &RootMetadata) -> bool {
    root.keys
        .iter()
        .any(|k| k.key_id == DEV_INSECURE_ROOT_KEY_ID)
}
