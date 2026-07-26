// Task 4 — Mirror contract schema and CAS validation tests.
use ed25519_dalek::{Signer, SigningKey};
use getrandom::{SysRng, rand_core::UnwrapErr};
use mirror_contract::capability::{CapabilityManifest, SkinCompatibility};
use mirror_contract::cas::{
    CasError, StablePointer, validate_stable_promotion, verify_manifest_digest,
};
use mirror_contract::manifest::{
    CompatibilityStatus, MirrorManifest, OfficialIdentity, SourceProvenance,
};
use mirror_contract::signature::{SignatureError, VerifyingKeyBytes, verify_manifest_signature};

// ── Manifest ──────────────────────────────────────────────────────────────────

#[test]
fn stable_compatible_manifest_is_recognised() {
    let m = sample_manifest("stable", CompatibilityStatus::Compatible);
    assert!(
        m.is_stable_compatible(),
        "stable+compatible must be recognised"
    );
}

#[test]
fn raw_manifest_is_not_stable_compatible() {
    let m = sample_manifest("raw", CompatibilityStatus::Compatible);
    assert!(
        !m.is_stable_compatible(),
        "raw channel must not pass is_stable_compatible"
    );
}

#[test]
fn incompatible_stable_manifest_is_not_stable_compatible() {
    let m = sample_manifest(
        "stable",
        CompatibilityStatus::Incompatible {
            reason: "bad".into(),
        },
    );
    assert!(!m.is_stable_compatible());
}

#[test]
fn manifest_roundtrips_json() {
    let m = sample_manifest("stable", CompatibilityStatus::Compatible);
    let json = serde_json::to_string_pretty(&m).unwrap();
    let m2: MirrorManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(m.codex_version, m2.codex_version);
    assert_eq!(m.sha256, m2.sha256);
    assert_eq!(m.channel, m2.channel);
}

// ── CAS promotion ─────────────────────────────────────────────────────────────

#[test]
fn higher_sequence_is_accepted() {
    let current = pointer(1);
    let proposed = pointer(2);
    assert!(validate_stable_promotion(&current, &proposed).is_ok());
}

#[test]
fn same_sequence_is_rejected_as_stale() {
    let current = pointer(5);
    let proposed = pointer(5);
    let err = validate_stable_promotion(&current, &proposed).unwrap_err();
    assert!(matches!(err, CasError::StalePromotion { .. }));
}

#[test]
fn lower_sequence_is_rejected_prevents_rollback_attack() {
    let current = pointer(10);
    let proposed = pointer(3);
    let err = validate_stable_promotion(&current, &proposed).unwrap_err();
    assert!(
        matches!(err, CasError::StalePromotion { .. }),
        "lower sequence must be rejected (anti-rollback): {:?}",
        err
    );
}

#[test]
fn digest_verification_passes_on_match() {
    let ptr = StablePointer {
        codex_version: "26.721".into(),
        raw_digest: "sha256:abc".into(),
        manifest_digest: "sha256:manifest123".into(),
        promoted_at: "2026-07-26T00:00:00Z".into(),
        sequence: 1,
    };
    assert!(verify_manifest_digest(&ptr, "sha256:manifest123").is_ok());
}

#[test]
fn digest_verification_fails_on_mismatch() {
    let ptr = StablePointer {
        codex_version: "26.721".into(),
        raw_digest: "sha256:abc".into(),
        manifest_digest: "sha256:correct".into(),
        promoted_at: "2026-07-26T00:00:00Z".into(),
        sequence: 1,
    };
    let err = verify_manifest_digest(&ptr, "sha256:wrong").unwrap_err();
    assert!(matches!(err, CasError::DigestMismatch { .. }));
}

// ── Capability manifest ────────────────────────────────────────────────────────

#[test]
fn capability_matches_bound_digest() {
    let cap = CapabilityManifest {
        schema_version: 1,
        bound_raw_digest: "sha256:abc123".into(),
        codex_version: "26.721".into(),
        generated_at: "2026-07-26T00:00:00Z".into(),
        skin_compat: SkinCompatibility {
            compatible: true,
            checks: vec![],
        },
    };
    assert!(cap.matches_digest("sha256:abc123"));
    assert!(!cap.matches_digest("sha256:different"));
}

#[test]
fn capability_manifest_roundtrips_json() {
    let cap = CapabilityManifest {
        schema_version: 1,
        bound_raw_digest: "sha256:xyz".into(),
        codex_version: "26.721".into(),
        generated_at: "2026-07-26T00:00:00Z".into(),
        skin_compat: SkinCompatibility {
            compatible: false,
            checks: vec![],
        },
    };
    let json = serde_json::to_string_pretty(&cap).unwrap();
    let cap2: CapabilityManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(cap.bound_raw_digest, cap2.bound_raw_digest);
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn sample_manifest(channel: &str, compat: CompatibilityStatus) -> MirrorManifest {
    MirrorManifest {
        schema_version: 1,
        channel: channel.to_string(),
        codex_version: "26.721.41059".to_string(),
        published_at: "2026-07-26T00:00:00Z".to_string(),
        platform: "windows".to_string(),
        arch: "x64".to_string(),
        asset_url: "https://mirror.chimera.io/windows/x64/codex-26.721.msix".to_string(),
        size_bytes: 123_456_789,
        sha256: "0".repeat(64),
        official_identity: OfficialIdentity {
            signer: "OpenAI Authenticode".to_string(),
            subject: Some("OpenAI, Inc.".to_string()),
            team_id: None,
        },
        minimum_chimera_version: "2.0.0".to_string(),
        compatibility_status: compat,
        promoted_from_raw_digest: None,
        rollback_target: None,
        source_provenance: SourceProvenance {
            source_url: "https://winget.cdn.microsoft.com/packages/OpenAI.Codex.msix".to_string(),
            etag: Some("abc123".to_string()),
            observed_at: "2026-07-26T00:00:00Z".to_string(),
        },
        // Absent here on purpose: the capability triple is generated at
        // promotion time, so a bare sample carries none. capability_binding.rs
        // covers the declared case.
        // Spec 9.2: a real stable entry always names its capability manifest.
        // Raw entries have none, because capabilities are computed at promotion.
        capability_manifest_url: if channel == "stable" {
            Some("https://mirror.chimera.io/windows/x64/capability-26.721.json".to_string())
        } else {
            None
        },
        capability_manifest_size_bytes: if channel == "stable" { Some(512) } else { None },
        capability_manifest_sha256: if channel == "stable" {
            Some("b".repeat(64))
        } else {
            None
        },
    }
}

fn pointer(seq: u64) -> StablePointer {
    StablePointer {
        codex_version: "26.721".into(),
        raw_digest: "sha256:raw".into(),
        manifest_digest: "sha256:manifest".into(),
        promoted_at: "2026-07-26T00:00:00Z".into(),
        sequence: seq,
    }
}

/// Generate a fresh Ed25519 keypair using the system CSPRNG, for tests that
/// need a real signature rather than a fixed one.
fn generate_signing_key() -> SigningKey {
    let mut csprng = UnwrapErr(SysRng);
    SigningKey::generate(&mut csprng)
}

// ── Capability manifest binding (Spec 9.2) ─────────────────────────────────────

#[test]
fn stable_manifest_that_binds_capability_manifest_is_stable_compatible() {
    let m = sample_manifest("stable", CompatibilityStatus::Compatible);
    assert!(m.binds_capability_manifest());
    assert!(
        m.is_stable_compatible(),
        "a stable manifest that binds a capability manifest must be stable-compatible"
    );
}

#[test]
fn stable_manifest_that_does_not_bind_capability_manifest_is_not_stable_compatible() {
    let mut m = sample_manifest("stable", CompatibilityStatus::Compatible);
    // Strip the capability triple entirely: a stable entry with nothing to
    // bind must not be treated as usable, even though channel and
    // compatibility_status both look fine on their own.
    m.capability_manifest_url = None;
    m.capability_manifest_size_bytes = None;
    m.capability_manifest_sha256 = None;

    assert!(!m.binds_capability_manifest());
    assert!(
        !m.is_stable_compatible(),
        "a stable manifest with no capability binding must not be stable-compatible"
    );
}

#[test]
fn binds_capability_manifest_rejects_wrong_length_digest() {
    let mut m = sample_manifest("stable", CompatibilityStatus::Compatible);
    m.capability_manifest_sha256 = Some("b".repeat(63)); // one char short
    assert!(!m.binds_capability_manifest());
}

#[test]
fn binds_capability_manifest_rejects_non_hex_digest() {
    let mut m = sample_manifest("stable", CompatibilityStatus::Compatible);
    // 64 chars, but 'g' is not a hex digit.
    m.capability_manifest_sha256 = Some(format!("g{}", "b".repeat(63)));
    assert!(!m.binds_capability_manifest());
}

// ── Signature verification ─────────────────────────────────────────────────────

#[test]
fn valid_ed25519_signature_over_manifest_bytes_verifies() {
    let signing_key = generate_signing_key();
    let verifying_key = VerifyingKeyBytes(signing_key.verifying_key().to_bytes());
    let manifest_json = br#"{"schema_version":1,"channel":"stable"}"#;

    let signature = signing_key.sign(manifest_json);

    assert!(
        verify_manifest_signature(manifest_json, &signature.to_bytes(), &verifying_key).is_ok(),
        "a genuine signature over the exact bytes must verify"
    );
}

#[test]
fn signature_over_different_bytes_fails_with_mismatch() {
    let signing_key = generate_signing_key();
    let verifying_key = VerifyingKeyBytes(signing_key.verifying_key().to_bytes());
    let original = br#"{"schema_version":1,"channel":"stable"}"#;
    let tampered = br#"{"schema_version":2,"channel":"stable"}"#;

    let signature = signing_key.sign(original);

    let err = verify_manifest_signature(tampered, &signature.to_bytes(), &verifying_key)
        .expect_err("a signature over the wrong bytes must not verify");
    assert!(matches!(err, SignatureError::Mismatch));
}

#[test]
fn malformed_key_bytes_return_malformed_key_error_not_a_panic() {
    let signing_key = generate_signing_key();
    let manifest_json = br#"{"schema_version":1}"#;
    let signature = signing_key.sign(manifest_json);

    // A high last byte with the rest zeroed decodes to a y-coordinate that is
    // not less than the field prime, so it is not a valid compressed Edwards
    // point. Key decoding must fail cleanly rather than panicking.
    let mut bad_bytes = [0u8; 32];
    bad_bytes[31] = 0xFF;
    let bad_key = VerifyingKeyBytes(bad_bytes);

    let err = verify_manifest_signature(manifest_json, &signature.to_bytes(), &bad_key)
        .expect_err("a malformed key must not verify");
    assert!(matches!(err, SignatureError::MalformedKey));
}

#[test]
fn malformed_signature_bytes_return_malformed_signature_error_not_a_panic() {
    let signing_key = generate_signing_key();
    let verifying_key = VerifyingKeyBytes(signing_key.verifying_key().to_bytes());
    let manifest_json = br#"{"schema_version":1}"#;

    // Only 3 bytes: nowhere near the required 64-byte signature length.
    let bad_signature = [1u8, 2, 3];

    let err = verify_manifest_signature(manifest_json, &bad_signature, &verifying_key)
        .expect_err("a malformed signature must not verify");
    assert!(matches!(err, SignatureError::MalformedSignature));
}
