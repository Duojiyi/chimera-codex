// `chimera-app-latest.json` treated strictly as a pinned target.
//
// Two hazards this file guards against:
//
// - Parsing before verifying. If the JSON parser ran on unverified bytes
//   first, a parser bug becomes a remotely triggerable one — the whole point
//   of pinning length+hash in signed metadata is that nothing downstream
//   needs to trust the parser with anything that has not already been
//   checked byte-for-byte.
// - A downgrade sliding through silently. `version` in the target is not
//   itself signed by a role that can singlehandedly say "yes, really go
//   backwards" — only the target's own body can, via an explicit field named
//   for exactly this, and only when it names the version actually installed.

use chimera_update::app_target::{
    APP_TARGET_PATH, AppLatest, AppTargetError, UpdateDecision, decide,
};
use chimera_update::metadata::{
    APP_TRUST_DOMAIN, RoleKeys, RootMetadata, SnapshotMetadata, TargetEntry, TargetsMetadata,
    TimestampMetadata,
};
use chimera_update::trust::{TrustedVersions, VerifiedChain};
use semver::Version;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// A `VerifiedChain` this crate's own test can construct directly — this
/// module only ever looks at `.targets`, but the function signature takes the
/// whole chain (that is what a real caller has in hand after `verify_chain`),
/// so the fixture has to be a complete, if mostly-inert, one.
fn chain_with_target(entry: Option<TargetEntry>) -> VerifiedChain {
    let mut targets = BTreeMap::new();
    if let Some(entry) = entry {
        targets.insert(APP_TARGET_PATH.to_string(), entry);
    }
    let empty_role = RoleKeys {
        key_ids: vec![],
        threshold: 1,
    };
    VerifiedChain {
        root: RootMetadata {
            domain: APP_TRUST_DOMAIN.to_string(),
            version: 1,
            expires: 0,
            keys: vec![],
            root: empty_role.clone(),
            targets: empty_role.clone(),
            snapshot: empty_role.clone(),
            timestamp: empty_role,
        },
        timestamp: TimestampMetadata {
            domain: APP_TRUST_DOMAIN.to_string(),
            version: 1,
            expires: 0,
            snapshot_version: 1,
            snapshot_sha256_hex: String::new(),
        },
        snapshot: SnapshotMetadata {
            domain: APP_TRUST_DOMAIN.to_string(),
            version: 1,
            expires: 0,
            targets_version: 1,
            targets_sha256_hex: String::new(),
        },
        targets: TargetsMetadata {
            domain: APP_TRUST_DOMAIN.to_string(),
            version: 1,
            expires: 0,
            targets,
        },
        versions: TrustedVersions {
            root: 1,
            timestamp: 1,
            snapshot: 1,
            targets: 1,
        },
    }
}

fn pinned_entry(bytes: &[u8]) -> TargetEntry {
    TargetEntry {
        sha256_hex: format!("{:x}", Sha256::digest(bytes)),
        length: bytes.len() as u64,
    }
}

fn latest_json(version: &str, downgrade_authorized_from: Option<&str>) -> Vec<u8> {
    let doc = AppLatest {
        version: version.to_string(),
        download_url: "https://updates.example/chimera-2.0.0.msi".to_string(),
        sha256_hex: "a".repeat(64),
        length: 1024,
        downgrade_authorized_from: downgrade_authorized_from.map(str::to_string),
    };
    serde_json::to_vec(&doc).unwrap()
}

// ── Verify length + hash BEFORE parsing ─────────────────────────────────

#[test]
fn bytes_that_do_not_match_the_pinned_hash_are_refused_without_being_parsed() {
    let bytes = b"not even valid json, and also the wrong hash".to_vec();
    let mut entry = pinned_entry(&bytes);
    entry.sha256_hex = "f".repeat(64); // deliberately wrong
    let chain = chain_with_target(Some(entry));

    let err = decide(&chain, &bytes, &Version::new(1, 0, 0)).unwrap_err();
    assert!(
        matches!(err, AppTargetError::DigestMismatch),
        "expected DigestMismatch (proving the hash check ran before any parse attempt), got {err:?}"
    );
}

#[test]
fn bytes_that_do_not_match_the_pinned_length_are_refused() {
    let bytes = latest_json("2.0.0", None);
    let mut entry = pinned_entry(&bytes);
    entry.length += 1;
    let chain = chain_with_target(Some(entry));

    let err = decide(&chain, &bytes, &Version::new(1, 0, 0)).unwrap_err();
    assert!(
        matches!(err, AppTargetError::LengthMismatch { .. }),
        "got {err:?}"
    );
}

#[test]
fn a_target_missing_from_the_signed_targets_map_is_refused() {
    let bytes = latest_json("2.0.0", None);
    let chain = chain_with_target(None);

    let err = decide(&chain, &bytes, &Version::new(1, 0, 0)).unwrap_err();
    assert!(matches!(err, AppTargetError::UnknownTarget), "got {err:?}");
}

#[test]
fn malformed_json_that_still_matches_the_pin_is_refused_as_malformed() {
    let bytes = b"{ not valid json".to_vec();
    let entry = pinned_entry(&bytes);
    let chain = chain_with_target(Some(entry));

    let err = decide(&chain, &bytes, &Version::new(1, 0, 0)).unwrap_err();
    assert!(matches!(err, AppTargetError::Malformed(_)), "got {err:?}");
}

#[test]
fn an_unparseable_version_string_is_refused() {
    let bytes = latest_json("not-a-version", None);
    let entry = pinned_entry(&bytes);
    let chain = chain_with_target(Some(entry));

    let err = decide(&chain, &bytes, &Version::new(1, 0, 0)).unwrap_err();
    assert!(
        matches!(err, AppTargetError::InvalidVersion(_)),
        "got {err:?}"
    );
}

// ── Decision outcomes ────────────────────────────────────────────────────

#[test]
fn the_same_version_is_up_to_date() {
    let bytes = latest_json("1.0.0", None);
    let entry = pinned_entry(&bytes);
    let chain = chain_with_target(Some(entry));

    let decision = decide(&chain, &bytes, &Version::new(1, 0, 0)).unwrap();
    assert!(
        matches!(decision, UpdateDecision::UpToDate { .. }),
        "{decision:?}"
    );
}

#[test]
fn a_higher_version_is_an_update_available() {
    let bytes = latest_json("1.2.0", None);
    let entry = pinned_entry(&bytes);
    let chain = chain_with_target(Some(entry));

    let decision = decide(&chain, &bytes, &Version::new(1, 0, 0)).unwrap();
    match decision {
        UpdateDecision::UpdateAvailable { latest, .. } => assert_eq!(latest.version, "1.2.0"),
        other => panic!("expected UpdateAvailable, got {other:?}"),
    }
}

#[test]
fn a_lower_version_with_no_authorisation_is_a_refused_downgrade() {
    let bytes = latest_json("0.9.0", None);
    let entry = pinned_entry(&bytes);
    let chain = chain_with_target(Some(entry));

    let decision = decide(&chain, &bytes, &Version::new(1, 0, 0)).unwrap();
    match decision {
        UpdateDecision::DowngradeRefused { installed, offered } => {
            assert_eq!(installed, Version::new(1, 0, 0));
            assert_eq!(offered, Version::new(0, 9, 0));
        }
        other => panic!("expected DowngradeRefused, got {other:?}"),
    }
}

#[test]
fn a_lower_version_explicitly_authorised_for_the_installed_version_is_accepted() {
    // A release pulled for a critical bug needs a way to walk installs back
    // down. The one document positioned to say "yes, really" is this one —
    // nothing else in the chain speaks to a specific installed version.
    let bytes = latest_json("0.9.0", Some("1.0.0"));
    let entry = pinned_entry(&bytes);
    let chain = chain_with_target(Some(entry));

    let decision = decide(&chain, &bytes, &Version::new(1, 0, 0)).unwrap();
    assert!(
        matches!(decision, UpdateDecision::UpdateAvailable { .. }),
        "an authorised downgrade must be offered as an update, got {decision:?}"
    );
}

#[test]
fn a_downgrade_authorisation_for_a_different_version_does_not_apply() {
    // The authorisation names 1.0.0; the machine asking is on 1.1.0. That is
    // not the version this release was cleared to replace.
    let bytes = latest_json("0.9.0", Some("1.0.0"));
    let entry = pinned_entry(&bytes);
    let chain = chain_with_target(Some(entry));

    let decision = decide(&chain, &bytes, &Version::new(1, 1, 0)).unwrap();
    assert!(
        matches!(decision, UpdateDecision::DowngradeRefused { .. }),
        "an authorisation for a different installed version must not apply, got {decision:?}"
    );
}
