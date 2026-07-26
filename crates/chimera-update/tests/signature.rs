// Step 9.1 RED — Ed25519 primitive for the app trust domain.
//
// Deliberately mirrors services/mirror-contract/tests/signature.rs in shape
// (same fixed-seed pattern) so the two domains are easy to compare side by
// side, while never importing from mirror-contract (G15).

use chimera_update::signature::{SignatureError, canonical_bytes, verify_bytes};
use ed25519_dalek::{Signer, SigningKey};

fn test_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

#[test]
fn a_correctly_signed_payload_verifies() {
    let k = test_key();
    let payload = r#"{"version":1}"#;
    let sig = k.sign(&canonical_bytes(payload));
    let pk = chimera_update::signature::VerifyingKeyBytes(k.verifying_key().to_bytes());

    assert!(verify_bytes(&canonical_bytes(payload), &hex::encode(sig.to_bytes()), &pk).is_ok());
}

#[test]
fn a_tampered_payload_fails_even_by_one_byte() {
    let k = test_key();
    let sig = k.sign(&canonical_bytes(r#"{"version":1}"#));
    let pk = chimera_update::signature::VerifyingKeyBytes(k.verifying_key().to_bytes());

    let tampered = canonical_bytes(r#"{"version":2}"#);
    let err = verify_bytes(&tampered, &hex::encode(sig.to_bytes()), &pk).unwrap_err();
    assert_eq!(err, SignatureError::Mismatch);
}

#[test]
fn a_signature_from_a_different_key_fails() {
    let real = test_key();
    let impostor = SigningKey::from_bytes(&[9u8; 32]);
    let payload = canonical_bytes(r#"{"version":1}"#);
    let sig = impostor.sign(&payload);
    let pk = chimera_update::signature::VerifyingKeyBytes(real.verifying_key().to_bytes());

    let err = verify_bytes(&payload, &hex::encode(sig.to_bytes()), &pk).unwrap_err();
    assert_eq!(err, SignatureError::Mismatch);
}

#[test]
fn malformed_signature_hex_is_an_error_not_a_panic() {
    let k = test_key();
    let pk = chimera_update::signature::VerifyingKeyBytes(k.verifying_key().to_bytes());
    let err = verify_bytes(b"payload", "not-hex", &pk).unwrap_err();
    assert_eq!(err, SignatureError::MalformedSignature);
}

#[test]
fn a_signature_of_the_wrong_length_is_malformed_not_a_panic() {
    let k = test_key();
    let pk = chimera_update::signature::VerifyingKeyBytes(k.verifying_key().to_bytes());
    // Valid hex, wrong byte count.
    let err = verify_bytes(b"payload", "aabbcc", &pk).unwrap_err();
    assert_eq!(err, SignatureError::MalformedSignature);
}

#[test]
fn malformed_key_bytes_are_an_error_not_a_panic() {
    // All-zero is not a valid compressed Edwards point in every ed25519
    // implementation's strict decoder; this crate must not panic on it.
    let bad = chimera_update::signature::VerifyingKeyBytes([0u8; 32]);
    let result = verify_bytes(b"payload", &hex::encode([0u8; 64]), &bad);
    // Either a clean MalformedKey error or a clean Mismatch is acceptable —
    // what is not acceptable is a panic, which `assert!` alone already proves
    // by virtue of this test completing.
    assert!(result.is_err());
}

#[test]
fn canonical_bytes_trims_surrounding_whitespace_so_a_trailing_newline_does_not_break_verification()
{
    let a = canonical_bytes("{\"a\":1}\n");
    let b = canonical_bytes("{\"a\":1}");
    assert_eq!(a, b);
}
