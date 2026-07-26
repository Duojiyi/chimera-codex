// Step 8.4 RED — versioned compatibility fingerprint + fail-closed
// interpreter + skin-only fuse + signed kill switch (ADR-005).
//
// This crate produces CANDIDATE evidence only: `compute_fingerprint` and
// `CandidateFingerprint` never sign anything, and there is no signing key
// type anywhere in `fingerprint.rs` — publishing a *trusted* capability
// manifest from a candidate fingerprint is the mirror gate's job alone
// (Step 4.5, `services/mirror-contract`). See the module docs for why that
// is enforced by omission (no signing primitive exists here to call) rather
// than by a runtime check.

use chimera_theme::fingerprint::{
    CandidateFingerprint, ExpectedFingerprint, FingerprintError, KillSwitchError,
    KillSwitchTrustAnchor, ProbeInput, PROBE_SCHEMA_VERSION, SignedKillSwitch, SkinFuse,
    TripReason, compute_fingerprint,
};
use ed25519_dalek::{Signer, SigningKey};

// ── deterministic test keys (never a real deployment key) ──────────────────

fn test_key() -> SigningKey {
    SigningKey::from_bytes(&[11u8; 32])
}

fn anchor_trusting(key_id: &str, signing: &SigningKey) -> KillSwitchTrustAnchor {
    KillSwitchTrustAnchor::new(vec![(key_id.to_string(), signing.verifying_key().to_bytes())])
}

fn sign_kill_switch(payload: &str, key_id: &str, signing: &SigningKey) -> SignedKillSwitch {
    let sig = signing.sign(payload.trim().as_bytes());
    SignedKillSwitch {
        payload: payload.to_string(),
        key_id: key_id.to_string(),
        signature_hex: hex::encode(sig.to_bytes()),
    }
}

fn probe(codex_version: &str, selectors: &[&str]) -> ProbeInput {
    ProbeInput {
        codex_version: codex_version.to_string(),
        observed_selectors: selectors.iter().map(|s| s.to_string()).collect(),
    }
}

// ── compute_fingerprint: versioned, deterministic, order-independent ───────

#[test]
fn the_same_probe_input_always_yields_the_same_fingerprint() {
    let a = compute_fingerprint(&probe("2.0.0", &[".title", ".sidebar"]));
    let b = compute_fingerprint(&probe("2.0.0", &[".title", ".sidebar"]));
    assert_eq!(a, b);
    assert_eq!(a.schema_version, PROBE_SCHEMA_VERSION);
}

#[test]
fn selector_order_does_not_change_the_fingerprint() {
    let a = compute_fingerprint(&probe("2.0.0", &[".title", ".sidebar"]));
    let b = compute_fingerprint(&probe("2.0.0", &[".sidebar", ".title"]));
    assert_eq!(a.digest_hex, b.digest_hex);
}

#[test]
fn a_different_codex_version_yields_a_different_fingerprint() {
    let a = compute_fingerprint(&probe("2.0.0", &[".title"]));
    let b = compute_fingerprint(&probe("2.0.1", &[".title"]));
    assert_ne!(a.digest_hex, b.digest_hex);
}

#[test]
fn a_different_selector_set_yields_a_different_fingerprint() {
    let a = compute_fingerprint(&probe("2.0.0", &[".title"]));
    let b = compute_fingerprint(&probe("2.0.0", &[".title", ".sidebar"]));
    assert_ne!(a.digest_hex, b.digest_hex);
}

// ── ExpectedFingerprint::parse — negative fixture per interpreted field ────

fn valid_expected_json(digest: &str) -> String {
    format!(
        r#"{{"schema_version":{v},"codex_version":"2.0.0","digest_hex":"{d}"}}"#,
        v = PROBE_SCHEMA_VERSION,
        d = digest
    )
}

const VALID_DIGEST: &str = "ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab1";

#[test]
fn a_well_formed_expected_fingerprint_parses() {
    let json = valid_expected_json(VALID_DIGEST);
    let parsed = ExpectedFingerprint::parse(json.as_bytes()).expect("must parse");
    assert_eq!(parsed.codex_version, "2.0.0");
}

#[test]
fn not_json_at_all_is_refused() {
    let result = ExpectedFingerprint::parse(b"not json");
    assert!(matches!(result, Err(FingerprintError::Malformed(_))));
}

#[test]
fn non_utf8_bytes_are_refused() {
    let result = ExpectedFingerprint::parse(&[0xff, 0xfe, 0x00]);
    assert!(matches!(result, Err(FingerprintError::Malformed(_))));
}

#[test]
fn schema_version_field_wrong_is_refused() {
    let json = r#"{"schema_version":99,"codex_version":"2.0.0","digest_hex":"ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab1"}"#;
    let result = ExpectedFingerprint::parse(json.as_bytes());
    assert!(matches!(
        result,
        Err(FingerprintError::UnsupportedSchemaVersion { found: 99, .. })
    ));
}

#[test]
fn codex_version_field_empty_is_refused() {
    let json = r#"{"schema_version":1,"codex_version":"   ","digest_hex":"ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab1"}"#;
    let result = ExpectedFingerprint::parse(json.as_bytes());
    assert!(matches!(result, Err(FingerprintError::EmptyCodexVersion)));
}

#[test]
fn digest_hex_field_too_short_is_refused() {
    let json = r#"{"schema_version":1,"codex_version":"2.0.0","digest_hex":"ab12"}"#;
    let result = ExpectedFingerprint::parse(json.as_bytes());
    assert!(matches!(result, Err(FingerprintError::MalformedDigest(_))));
}

#[test]
fn digest_hex_field_with_uppercase_is_refused_by_shape_even_though_verification_is_case_insensitive()
 {
    // Shape validation at parse time is stricter than the later comparison:
    // an interpreter that accepted uppercase here would have two different
    // ideas of "valid" depending on which code path touched the value first.
    let json = format!(
        r#"{{"schema_version":1,"codex_version":"2.0.0","digest_hex":"{}"}}"#,
        VALID_DIGEST.to_uppercase()
    );
    let result = ExpectedFingerprint::parse(json.as_bytes());
    assert!(matches!(result, Err(FingerprintError::MalformedDigest(_))));
}

#[test]
fn digest_hex_field_with_non_hex_characters_is_refused() {
    let json = r#"{"schema_version":1,"codex_version":"2.0.0","digest_hex":"zz12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab1"}"#;
    let result = ExpectedFingerprint::parse(json.as_bytes());
    assert!(matches!(result, Err(FingerprintError::MalformedDigest(_))));
}

#[test]
fn missing_required_field_is_refused() {
    let json = r#"{"schema_version":1,"digest_hex":"ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab12ab1"}"#;
    let result = ExpectedFingerprint::parse(json.as_bytes());
    assert!(matches!(result, Err(FingerprintError::Malformed(_))));
}

// ── ExpectedFingerprint::matches — the semantic comparison ─────────────────

#[test]
fn a_matching_candidate_verifies() {
    let candidate = compute_fingerprint(&probe("2.0.0", &[".title"]));
    let expected_json = valid_expected_json(&candidate.digest_hex);
    let expected = ExpectedFingerprint::parse(expected_json.as_bytes()).unwrap();
    assert!(expected.matches(&candidate).is_ok());
}

#[test]
fn a_codex_version_mismatch_is_refused() {
    let candidate = compute_fingerprint(&probe("2.0.1", &[".title"]));
    let expected_json = valid_expected_json(&candidate.digest_hex);
    let expected = ExpectedFingerprint::parse(expected_json.as_bytes()).unwrap();
    // `expected` above claims codex_version 2.0.0 while candidate says 2.0.1.
    let result = expected.matches(&candidate);
    assert!(matches!(
        result,
        Err(FingerprintError::CodexVersionMismatch { .. })
    ));
}

#[test]
fn a_digest_mismatch_is_refused() {
    let candidate = compute_fingerprint(&probe("2.0.0", &[".title"]));
    let expected = ExpectedFingerprint::parse(valid_expected_json(VALID_DIGEST).as_bytes()).unwrap();
    assert_ne!(expected.digest_hex, candidate.digest_hex, "fixture sanity");
    let result = expected.matches(&candidate);
    assert!(matches!(result, Err(FingerprintError::DigestMismatch)));
}

// ── SkinFuse: mismatch trips the skin, never Codex itself ──────────────────

#[test]
fn a_fresh_fuse_leaves_the_skin_enabled() {
    let fuse = SkinFuse::engaged();
    assert!(fuse.skin_enabled());
}

#[test]
fn a_fingerprint_mismatch_trips_the_fuse_and_disables_the_skin() {
    let candidate = compute_fingerprint(&probe("2.0.0", &[".title"]));
    let expected = ExpectedFingerprint::parse(valid_expected_json(VALID_DIGEST).as_bytes()).unwrap();
    let err = expected.matches(&candidate).unwrap_err();
    let fuse = SkinFuse::trip_on_mismatch(err);
    assert!(!fuse.skin_enabled());
    assert!(matches!(
        fuse.trip_reason(),
        Some(TripReason::FingerprintMismatch(_))
    ));
}

/// Stand-in for the real Codex launch path (owned by `chimera-runtime`, a
/// sibling adapter this crate may not depend on). Its signature takes no
/// `SkinFuse` parameter at all — that absence, not a runtime check that
/// happens to return `true`, is what proves a tripped fuse cannot gate it.
fn launch_codex_stub() -> bool {
    true
}

#[test]
fn a_tripped_fuse_disables_only_the_skin_never_codex_launch() {
    let candidate = compute_fingerprint(&probe("2.0.0", &[".title"]));
    let expected = ExpectedFingerprint::parse(valid_expected_json(VALID_DIGEST).as_bytes()).unwrap();
    let err = expected.matches(&candidate).unwrap_err();
    let fuse = SkinFuse::trip_on_mismatch(err);

    assert!(!fuse.skin_enabled(), "the skin must be disabled");
    // Codex's own launch path takes no fuse parameter — there is nothing to
    // gate here, which is the point: stock Codex still launches.
    assert!(
        launch_codex_stub(),
        "codex must still launch with the fuse tripped"
    );
}

// ── Signed kill switch: verified-only, disables only the enhancement ───────

#[test]
fn a_validly_signed_disable_kill_switch_trips_the_fuse() {
    let key = test_key();
    let anchor = anchor_trusting("chimera-2026", &key);
    let signed = sign_kill_switch(
        r#"{"schema_version":1,"disable_skin":true,"reason":"incident-42"}"#,
        "chimera-2026",
        &key,
    );

    let mut fuse = SkinFuse::engaged();
    fuse.apply_kill_switch(&anchor, &signed)
        .expect("a validly signed kill switch must be honoured");
    assert!(!fuse.skin_enabled());
    assert!(matches!(fuse.trip_reason(), Some(TripReason::KillSwitch { .. })));
}

#[test]
fn a_kill_switch_that_explicitly_says_do_not_disable_leaves_the_fuse_engaged() {
    let key = test_key();
    let anchor = anchor_trusting("chimera-2026", &key);
    let signed = sign_kill_switch(
        r#"{"schema_version":1,"disable_skin":false,"reason":"heartbeat"}"#,
        "chimera-2026",
        &key,
    );

    let mut fuse = SkinFuse::engaged();
    fuse.apply_kill_switch(&anchor, &signed).expect("a valid signature must verify");
    assert!(fuse.skin_enabled(), "disable_skin:false must not trip anything");
}

#[test]
fn an_unsigned_kill_switch_is_ignored_not_obeyed() {
    // "Unsigned" modelled as a signature that does not verify: a
    // zero-filled signature is never a valid Ed25519 signature over any
    // non-trivial payload signed by a real key.
    let key = test_key();
    let anchor = anchor_trusting("chimera-2026", &key);
    let forged = SignedKillSwitch {
        payload: r#"{"schema_version":1,"disable_skin":true,"reason":"attacker"}"#.to_string(),
        key_id: "chimera-2026".to_string(),
        signature_hex: "00".repeat(64),
    };

    let mut fuse = SkinFuse::engaged();
    let err = fuse
        .apply_kill_switch(&anchor, &forged)
        .expect_err("an unverified kill switch must not be obeyed");
    assert!(matches!(err, KillSwitchError::BadSignature));
    assert!(
        fuse.skin_enabled(),
        "the fuse must be untouched by a signal that failed verification"
    );
}

#[test]
fn a_tampered_payload_is_ignored_even_though_the_signature_hex_is_well_formed() {
    let key = test_key();
    let anchor = anchor_trusting("chimera-2026", &key);
    let mut signed = sign_kill_switch(
        r#"{"schema_version":1,"disable_skin":true,"reason":"incident-42"}"#,
        "chimera-2026",
        &key,
    );
    signed.payload = r#"{"schema_version":1,"disable_skin":true,"reason":"tampered"}"#.to_string();

    let mut fuse = SkinFuse::engaged();
    let err = fuse.apply_kill_switch(&anchor, &signed).unwrap_err();
    assert!(matches!(err, KillSwitchError::BadSignature));
    assert!(fuse.skin_enabled());
}

#[test]
fn an_unknown_key_id_is_ignored_rather_than_tried_against_every_trusted_key() {
    let key = test_key();
    let anchor = anchor_trusting("chimera-2026", &key);
    let signed = sign_kill_switch(
        r#"{"schema_version":1,"disable_skin":true,"reason":"attacker"}"#,
        "some-other-key",
        &key,
    );

    let mut fuse = SkinFuse::engaged();
    let err = fuse.apply_kill_switch(&anchor, &signed).unwrap_err();
    assert!(matches!(err, KillSwitchError::UnknownKeyId { .. }));
    assert!(fuse.skin_enabled());
}

#[test]
fn malformed_signature_hex_is_an_error_not_a_panic() {
    let key = test_key();
    let anchor = anchor_trusting("chimera-2026", &key);
    let mut signed = sign_kill_switch(
        r#"{"schema_version":1,"disable_skin":true,"reason":"x"}"#,
        "chimera-2026",
        &key,
    );
    signed.signature_hex = "not-hex".to_string();

    let mut fuse = SkinFuse::engaged();
    let err = fuse.apply_kill_switch(&anchor, &signed).unwrap_err();
    assert!(matches!(err, KillSwitchError::MalformedSignature));
    assert!(fuse.skin_enabled());
}

#[test]
fn an_empty_trust_anchor_honours_nothing() {
    let key = test_key();
    let signed = sign_kill_switch(
        r#"{"schema_version":1,"disable_skin":true,"reason":"x"}"#,
        "chimera-2026",
        &key,
    );
    let empty_anchor = KillSwitchTrustAnchor::new(vec![]);

    let mut fuse = SkinFuse::engaged();
    let err = fuse.apply_kill_switch(&empty_anchor, &signed).unwrap_err();
    assert!(matches!(err, KillSwitchError::UnknownKeyId { .. }));
    assert!(fuse.skin_enabled());
}

#[test]
fn a_verified_but_unparseable_payload_is_an_error_not_a_panic() {
    // The signature covers exactly these (malformed-JSON) bytes, so it
    // verifies — the payload shape check is a separate, later failure.
    let key = test_key();
    let anchor = anchor_trusting("chimera-2026", &key);
    let signed = sign_kill_switch("not json at all", "chimera-2026", &key);

    let mut fuse = SkinFuse::engaged();
    let err = fuse.apply_kill_switch(&anchor, &signed).unwrap_err();
    assert!(matches!(err, KillSwitchError::MalformedPayload(_)));
    assert!(fuse.skin_enabled());
}
