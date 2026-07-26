// Spec 9.2 RED — the stable manifest must bind the capability manifest.
//
// Without these fields there is nothing to bind a capability digest to, so
// Step 4.5 ("bind capability digest to the stable manifest in the same CAS
// transaction") has no field to write and the requirement is unimplementable.

use mirror_contract::capability::CapabilityManifest;
use mirror_contract::manifest::{BindingError, MirrorManifest};

fn stable_with_capability(cap_sha: &str) -> MirrorManifest {
    let mut m = fixture_stable();
    m.capability_manifest_url = Some("https://mirror.example/cap/26.732.json".to_string());
    m.capability_manifest_size_bytes = Some(512);
    m.capability_manifest_sha256 = Some(cap_sha.to_string());
    m
}

fn fixture_stable() -> MirrorManifest {
    serde_json::from_str(include_str!("fixtures/stable.json")).expect("fixture must parse")
}

#[test]
fn stable_manifest_carries_the_three_capability_fields() {
    let m = stable_with_capability(&"a".repeat(64));
    assert!(m.capability_manifest_url.is_some());
    assert_eq!(m.capability_manifest_size_bytes, Some(512));
    assert_eq!(
        m.capability_manifest_sha256.as_deref(),
        Some(&*"a".repeat(64))
    );
}

#[test]
fn a_stable_manifest_without_capability_binding_is_rejected() {
    // A stable entry with no capability binding must not be treated as usable:
    // the skin engine would have no compatibility record for this exact build.
    let m = fixture_stable();
    assert!(
        !m.is_stable_compatible(),
        "stable without capability binding must not be stable-compatible"
    );
}

#[test]
fn binding_verifies_when_digest_and_size_both_match() {
    let cap_sha = "b".repeat(64);
    let m = stable_with_capability(&cap_sha);
    let cap: CapabilityManifest =
        serde_json::from_str(include_str!("fixtures/capability.json")).unwrap();

    assert!(
        m.verify_capability_binding(&cap_sha, 512, &cap).is_ok(),
        "matching digest and size must verify"
    );
}

#[test]
fn binding_fails_when_the_capability_digest_differs() {
    let m = stable_with_capability(&"c".repeat(64));
    let cap: CapabilityManifest =
        serde_json::from_str(include_str!("fixtures/capability.json")).unwrap();

    let err = m
        .verify_capability_binding(&"d".repeat(64), 512, &cap)
        .expect_err("a different digest must not verify");
    assert!(matches!(err, BindingError::DigestMismatch { .. }));
}

#[test]
fn binding_fails_when_the_size_differs() {
    let cap_sha = "e".repeat(64);
    let m = stable_with_capability(&cap_sha);
    let cap: CapabilityManifest =
        serde_json::from_str(include_str!("fixtures/capability.json")).unwrap();

    let err = m
        .verify_capability_binding(&cap_sha, 999, &cap)
        .expect_err("a size mismatch must not verify");
    assert!(matches!(err, BindingError::SizeMismatch { .. }));
}

#[test]
fn binding_fails_when_the_capability_is_bound_to_a_different_raw_digest() {
    // The capability manifest records which raw build it was generated for.
    // If that does not match the stable entry's raw provenance, the pair is
    // mismatched even when the file itself hashes correctly.
    let cap_sha = "f".repeat(64);
    let mut m = stable_with_capability(&cap_sha);
    m.promoted_from_raw_digest = Some("sha256:raw-A".to_string());

    let mut cap: CapabilityManifest =
        serde_json::from_str(include_str!("fixtures/capability.json")).unwrap();
    cap.bound_raw_digest = "sha256:raw-B".to_string();

    let err = m
        .verify_capability_binding(&cap_sha, 512, &cap)
        .expect_err("a capability bound to another raw build must not verify");
    assert!(matches!(err, BindingError::RawDigestMismatch { .. }));
}

#[test]
fn binding_fails_closed_when_the_manifest_declares_no_capability() {
    let m = fixture_stable();
    let cap: CapabilityManifest =
        serde_json::from_str(include_str!("fixtures/capability.json")).unwrap();

    let err = m
        .verify_capability_binding(&"a".repeat(64), 512, &cap)
        .expect_err("absent binding must be an error, never a silent pass");
    assert!(matches!(err, BindingError::NotDeclared));
}
