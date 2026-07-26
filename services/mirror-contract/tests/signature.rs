// Step 4.6 RED — Ed25519 signature verification for mirror manifests.
//
// Spec 9.2 / ADR-005: a client must never act on a manifest it cannot attribute
// to the mirror gate's signing key. Verification fails closed — an unsigned
// manifest, an unknown key id, or a tampered byte all refuse.
//
// Uses a fixed seed rather than a generated keypair so the test is
// deterministic and needs no RNG feature.
use ed25519_dalek::{Signer, SigningKey};
use mirror_contract::signature::{
    SignatureError, SignedManifest, TrustAnchor, TrustedKey, canonical_bytes,
};

/// Deterministic signer. A real deployment key never appears in the repo.
fn test_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

fn anchor_trusting(key_id: &str, signing: &SigningKey) -> TrustAnchor {
    TrustAnchor::new(vec![TrustedKey {
        key_id: key_id.to_string(),
        public_key: signing.verifying_key().to_bytes(),
    }])
}

fn sign(payload: &str, key_id: &str, signing: &SigningKey) -> SignedManifest {
    let sig = signing.sign(&canonical_bytes(payload));
    SignedManifest {
        payload: payload.to_string(),
        key_id: key_id.to_string(),
        signature_hex: hex::encode(sig.to_bytes()),
    }
}

// ── The happy path ───────────────────────────────────────────────────────────

#[test]
fn a_correctly_signed_manifest_verifies() {
    let k = test_key();
    let signed = sign(r#"{"schema_version":1}"#, "mirror-2026", &k);
    let anchor = anchor_trusting("mirror-2026", &k);

    assert!(
        anchor.verify(&signed).is_ok(),
        "a manifest signed by a trusted key must verify"
    );
}

// ── Fail-closed cases ────────────────────────────────────────────────────────

#[test]
fn a_tampered_payload_fails_even_by_one_byte() {
    let k = test_key();
    let mut signed = sign(r#"{"schema_version":1}"#, "mirror-2026", &k);
    let anchor = anchor_trusting("mirror-2026", &k);

    // Flip a single character in the payload; the signature no longer covers it.
    signed.payload = r#"{"schema_version":2}"#.to_string();

    let err = anchor
        .verify(&signed)
        .expect_err("a tampered payload must not verify");
    assert!(matches!(err, SignatureError::BadSignature));
}

#[test]
fn an_unknown_key_id_is_refused_rather_than_tried_against_every_key() {
    // Trying every key would let an attacker substitute any trusted key's
    // signature for another's identity, so the id must be resolved first.
    let k = test_key();
    let signed = sign(r#"{"schema_version":1}"#, "attacker-key", &k);
    let anchor = anchor_trusting("mirror-2026", &k);

    let err = anchor
        .verify(&signed)
        .expect_err("an unknown key id must not verify");
    assert!(matches!(err, SignatureError::UnknownKeyId { .. }));
}

#[test]
fn a_signature_from_a_different_key_fails() {
    let real = test_key();
    let impostor = SigningKey::from_bytes(&[9u8; 32]);
    // Signed by the impostor but claiming the trusted key's id.
    let signed = sign(r#"{"schema_version":1}"#, "mirror-2026", &impostor);
    let anchor = anchor_trusting("mirror-2026", &real);

    let err = anchor
        .verify(&signed)
        .expect_err("a signature from another key must not verify");
    assert!(matches!(err, SignatureError::BadSignature));
}

#[test]
fn malformed_signature_hex_is_an_error_not_a_panic() {
    let k = test_key();
    let mut signed = sign(r#"{"schema_version":1}"#, "mirror-2026", &k);
    let anchor = anchor_trusting("mirror-2026", &k);

    signed.signature_hex = "not-hex".to_string();
    let err = anchor
        .verify(&signed)
        .expect_err("malformed hex must be an error");
    assert!(matches!(err, SignatureError::MalformedSignature));
}

#[test]
fn an_empty_trust_anchor_verifies_nothing() {
    // A misconfigured client with no trusted keys must reject everything rather
    // than accept everything.
    let k = test_key();
    let signed = sign(r#"{"schema_version":1}"#, "mirror-2026", &k);
    let anchor = TrustAnchor::new(vec![]);

    assert!(
        anchor.verify(&signed).is_err(),
        "an empty trust anchor must never verify a manifest"
    );
}

// ── Canonicalisation ─────────────────────────────────────────────────────────

#[test]
fn canonical_bytes_are_stable_for_the_same_input() {
    let a = canonical_bytes(r#"{"b":1,"a":2}"#);
    let b = canonical_bytes(r#"{"b":1,"a":2}"#);
    assert_eq!(a, b, "the same payload must produce the same signed bytes");
}

#[test]
fn canonical_bytes_differ_for_different_input() {
    let a = canonical_bytes(r#"{"a":1}"#);
    let b = canonical_bytes(r#"{"a":2}"#);
    assert_ne!(a, b, "different payloads must not sign identically");
}
