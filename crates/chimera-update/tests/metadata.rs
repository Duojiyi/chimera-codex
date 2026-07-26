// Step 9.1 RED — TUF-style metadata shapes and threshold verification.
//
// This module is the first line of defence for cross-contamination (G8/G15):
// every parse function checks the payload's `domain` tag against this
// crate's hardcoded APP_TRUST_DOMAIN before anything else, so a Codex mirror
// document is refused before a single signature is even inspected.

use chimera_update::metadata::{
    APP_TRUST_DOMAIN, KeyEntry, MetaSignature, MetadataError, Role, RoleKeys, RootMetadata,
    parse_root, parse_snapshot, parse_targets, parse_timestamp, verify_threshold,
};
use chimera_update::signature::canonical_bytes;
use ed25519_dalek::{Signer, SigningKey};

fn key(id: &str, seed: u8) -> (KeyEntry, SigningKey) {
    let signing = SigningKey::from_bytes(&[seed; 32]);
    (
        KeyEntry {
            key_id: id.to_string(),
            public_key_hex: hex::encode(signing.verifying_key().to_bytes()),
        },
        signing,
    )
}

fn well_formed_root_json(domain: &str, root_key_id: &str) -> String {
    format!(
        r#"{{"domain":"{domain}","version":1,"expires":9999999999,
        "keys":[{{"key_id":"{root_key_id}","public_key_hex":"{}"}}],
        "root":{{"key_ids":["{root_key_id}"],"threshold":1}},
        "targets":{{"key_ids":["{root_key_id}"],"threshold":1}},
        "snapshot":{{"key_ids":["{root_key_id}"],"threshold":1}},
        "timestamp":{{"key_ids":["{root_key_id}"],"threshold":1}}
        }}"#,
        "a".repeat(64)
    )
}

// ── parse_root ───────────────────────────────────────────────────────────────

#[test]
fn parse_root_accepts_a_well_formed_document() {
    let json = well_formed_root_json(APP_TRUST_DOMAIN, "root-2026");
    let root: RootMetadata = parse_root(&json).expect("well-formed root must parse");
    assert_eq!(root.version, 1);
    assert_eq!(root.domain, APP_TRUST_DOMAIN);
}

#[test]
fn parse_root_rejects_a_document_from_a_different_trust_domain() {
    // This is the concrete cross-contamination gate: even a perfectly
    // well-formed document is refused if it was not minted for this app's
    // own chain — e.g. the Codex mirror's root, or a copy-paste mistake.
    let json = well_formed_root_json("codex-mirror-payload.v1", "root-2026");
    let err = parse_root(&json).expect_err("a foreign-domain root must be refused");
    assert!(matches!(err, MetadataError::WrongDomain { .. }));
}

#[test]
fn parse_root_rejects_malformed_json() {
    let err = parse_root("{ not json").expect_err("garbage must not parse");
    assert!(matches!(err, MetadataError::Malformed(_)));
}

#[test]
fn parse_root_rejects_a_zero_threshold_role() {
    let json = format!(
        r#"{{"domain":"{APP_TRUST_DOMAIN}","version":1,"expires":9999999999,
        "keys":[{{"key_id":"k1","public_key_hex":"{}"}}],
        "root":{{"key_ids":["k1"],"threshold":0}},
        "targets":{{"key_ids":["k1"],"threshold":1}},
        "snapshot":{{"key_ids":["k1"],"threshold":1}},
        "timestamp":{{"key_ids":["k1"],"threshold":1}}
        }}"#,
        "a".repeat(64)
    );
    let err = parse_root(&json).expect_err("a zero threshold can never be satisfied");
    assert!(matches!(err, MetadataError::ZeroThreshold { .. }));
}

#[test]
fn parse_root_rejects_a_role_that_names_a_key_not_in_the_key_set() {
    let json = format!(
        r#"{{"domain":"{APP_TRUST_DOMAIN}","version":1,"expires":9999999999,
        "keys":[{{"key_id":"k1","public_key_hex":"{}"}}],
        "root":{{"key_ids":["ghost-key"],"threshold":1}},
        "targets":{{"key_ids":["k1"],"threshold":1}},
        "snapshot":{{"key_ids":["k1"],"threshold":1}},
        "timestamp":{{"key_ids":["k1"],"threshold":1}}
        }}"#,
        "a".repeat(64)
    );
    let err = parse_root(&json).expect_err("a dangling key reference must be refused");
    assert!(matches!(err, MetadataError::UnknownRoleKey { .. }));
}

// ── parse_timestamp / parse_snapshot / parse_targets ────────────────────────

#[test]
fn parse_timestamp_rejects_a_document_from_a_different_trust_domain() {
    let json = r#"{"domain":"codex-mirror-payload.v1","version":1,"expires":9999999999,
        "snapshot_version":1,"snapshot_sha256_hex":"deadbeef"}"#;
    let err = parse_timestamp(json).expect_err("foreign-domain timestamp must be refused");
    assert!(matches!(err, MetadataError::WrongDomain { .. }));
}

#[test]
fn parse_snapshot_rejects_a_document_from_a_different_trust_domain() {
    let json = r#"{"domain":"codex-mirror-payload.v1","version":1,"expires":9999999999,
        "targets_version":1,"targets_sha256_hex":"deadbeef"}"#;
    let err = parse_snapshot(json).expect_err("foreign-domain snapshot must be refused");
    assert!(matches!(err, MetadataError::WrongDomain { .. }));
}

#[test]
fn parse_targets_rejects_a_document_from_a_different_trust_domain() {
    let json = r#"{"domain":"codex-mirror-payload.v1","version":1,"expires":9999999999,
        "targets":{}}"#;
    let err = parse_targets(json).expect_err("foreign-domain targets must be refused");
    assert!(matches!(err, MetadataError::WrongDomain { .. }));
}

#[test]
fn parse_targets_round_trips_a_target_entry() {
    let json = format!(
        r#"{{"domain":"{APP_TRUST_DOMAIN}","version":1,"expires":9999999999,
        "targets":{{"chimera-app-latest.json":{{"sha256_hex":"abc123","length":42}}}}
        }}"#
    );
    let parsed = parse_targets(&json).expect("well-formed targets must parse");
    let entry = parsed.targets.get("chimera-app-latest.json").unwrap();
    assert_eq!(entry.sha256_hex, "abc123");
    assert_eq!(entry.length, 42);
}

// ── verify_threshold ─────────────────────────────────────────────────────────

#[test]
fn verify_threshold_passes_when_enough_valid_signatures_are_present() {
    let (entry, signing) = key("k1", 1);
    let payload = r#"{"version":1}"#;
    let sig = signing.sign(&canonical_bytes(payload));
    let sigs = vec![MetaSignature {
        key_id: "k1".to_string(),
        signature_hex: hex::encode(sig.to_bytes()),
    }];

    verify_threshold(payload, &sigs, &[&entry], 1, "root").expect("threshold of 1 must be met");
}

#[test]
fn verify_threshold_fails_when_below_threshold() {
    let (entry, signing) = key("k1", 1);
    let payload = r#"{"version":1}"#;
    let sig = signing.sign(&canonical_bytes(payload));
    let sigs = vec![MetaSignature {
        key_id: "k1".to_string(),
        signature_hex: hex::encode(sig.to_bytes()),
    }];

    let err = verify_threshold(payload, &sigs, &[&entry], 2, "root")
        .expect_err("one signature cannot meet a threshold of two");
    assert!(matches!(err, MetadataError::ThresholdNotMet { .. }));
}

#[test]
fn verify_threshold_ignores_a_cryptographically_valid_signature_from_a_key_outside_the_role() {
    // A key that is real and its signature genuinely verifies — but it was
    // never listed as a candidate for this role, e.g. it is the snapshot
    // key being replayed against a root check. It must not count.
    let (_root_entry, root_signing) = key("root-key", 1);
    let (outside_entry, outside_signing) = key("outside-key", 2);
    let payload = r#"{"version":1}"#;
    let sig = outside_signing.sign(&canonical_bytes(payload));
    let sigs = vec![MetaSignature {
        key_id: "outside-key".to_string(),
        signature_hex: hex::encode(sig.to_bytes()),
    }];
    let _ = root_signing; // only used to make the intent ("a real other key") obvious

    let err = verify_threshold(payload, &sigs, &[&outside_entry], 1, "root");
    // outside_entry IS the candidate here in a degenerate sense; re-run with
    // the real candidate set excluding it to prove the exclusion, not the key.
    assert!(
        err.is_ok(),
        "sanity: signature is valid against its own key"
    );

    let candidates: Vec<&KeyEntry> = vec![];
    let err = verify_threshold(payload, &sigs, &candidates, 1, "root")
        .expect_err("a signature from a key absent from the candidate set must not count");
    assert!(matches!(err, MetadataError::ThresholdNotMet { .. }));
}

#[test]
fn verify_threshold_counts_each_key_only_once_even_with_duplicate_signatures() {
    let (entry, signing) = key("k1", 1);
    let payload = r#"{"version":1}"#;
    let sig = signing.sign(&canonical_bytes(payload));
    let one = MetaSignature {
        key_id: "k1".to_string(),
        signature_hex: hex::encode(sig.to_bytes()),
    };
    let sigs = vec![one.clone(), one];

    let err = verify_threshold(payload, &sigs, &[&entry], 2, "root")
        .expect_err("the same key signing twice must not satisfy a threshold of two");
    assert!(matches!(err, MetadataError::ThresholdNotMet { .. }));
}

#[test]
fn verify_threshold_rejects_a_zero_threshold_outright() {
    let sigs: Vec<MetaSignature> = vec![];
    let candidates: Vec<&KeyEntry> = vec![];
    let err = verify_threshold("{}", &sigs, &candidates, 0, "root")
        .expect_err("a zero threshold must never be treated as satisfied");
    assert!(matches!(err, MetadataError::ZeroThreshold { .. }));
}

#[test]
fn role_keys_shape_is_plain_data() {
    let rk = RoleKeys {
        key_ids: vec!["a".to_string()],
        threshold: 1,
    };
    assert_eq!(rk.key_ids.len(), 1);
    let _ = Role::Root;
}
