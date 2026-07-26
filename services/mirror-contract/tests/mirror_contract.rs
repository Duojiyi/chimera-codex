// Task 4 — Mirror contract schema and CAS validation tests.
use mirror_contract::capability::{CapabilityManifest, SkinCompatibility};
use mirror_contract::cas::{
    CasError, StablePointer, validate_stable_promotion, verify_manifest_digest,
};
use mirror_contract::manifest::{
    CompatibilityStatus, MirrorManifest, OfficialIdentity, SourceProvenance,
};

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
