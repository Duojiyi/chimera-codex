// Step 9.1 RED — the verification chain.
//
// Everything else in this crate parses documents. This is the part that
// decides whether to believe them, and therefore the part that decides what
// code runs on the user's machine next.
//
// The attacks it must survive are the standard TUF set, and each gets its own
// test because each fails differently:
//
//   rollback  — serving an older-but-validly-signed document to undo a fix
//   freeze    — serving a stale-but-unexpired-looking document forever
//   mix-and-match — serving a targets list that a different snapshot vouched for
//   key compromise — an online key signing something only root may authorise
//   cross-domain — a Codex mirror document satisfying the app chain (G8/G15)
//
// A chain that only ever gets valid input proves nothing, so every test below
// is an attack except the first.

use chimera_update::clock::FixedClock;
use chimera_update::metadata::{
    APP_TRUST_DOMAIN, KeyEntry, MetaSignature, Role, RoleKeys, RootMetadata, SignedPayload,
    SnapshotMetadata, TargetEntry, TargetsMetadata, TimestampMetadata,
};
use chimera_update::signature::canonical_bytes;
use chimera_update::trust::{TrustError, verify_chain};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const NOW: i64 = 1_800_000_000;
const LATER: i64 = NOW + 86_400;

// ── Test key material ───────────────────────────────────────────────────────

struct Key {
    id: String,
    signing: SigningKey,
}

impl Key {
    fn new(id: &str, seed: u8) -> Self {
        Self {
            id: id.to_string(),
            signing: SigningKey::from_bytes(&[seed; 32]),
        }
    }
    fn entry(&self) -> KeyEntry {
        KeyEntry {
            key_id: self.id.clone(),
            public_key_hex: hex::encode(self.signing.verifying_key().to_bytes()),
        }
    }
    fn sign(&self, payload: &str) -> MetaSignature {
        MetaSignature {
            key_id: self.id.clone(),
            signature_hex: hex::encode(self.signing.sign(&canonical_bytes(payload)).to_bytes()),
        }
    }
}

fn single(key_id: &str) -> RoleKeys {
    RoleKeys {
        key_ids: vec![key_id.to_string()],
        threshold: 1,
    }
}

fn sha256_hex(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

/// A complete, internally consistent set of documents.
struct Fixture {
    root_key: Key,
    ts_key: Key,
    snap_key: Key,
    tgt_key: Key,
}

impl Fixture {
    fn new() -> Self {
        Self {
            root_key: Key::new("root-1", 1),
            ts_key: Key::new("ts-1", 2),
            snap_key: Key::new("snap-1", 3),
            tgt_key: Key::new("tgt-1", 4),
        }
    }

    fn root(&self, version: u64, expires: i64) -> RootMetadata {
        RootMetadata {
            domain: APP_TRUST_DOMAIN.to_string(),
            version,
            expires,
            keys: vec![
                self.root_key.entry(),
                self.ts_key.entry(),
                self.snap_key.entry(),
                self.tgt_key.entry(),
            ],
            root: single("root-1"),
            targets: single("tgt-1"),
            snapshot: single("snap-1"),
            timestamp: single("ts-1"),
        }
    }

    fn targets(&self, version: u64, expires: i64) -> TargetsMetadata {
        let mut map = BTreeMap::new();
        map.insert(
            "chimera-app-latest.json".to_string(),
            TargetEntry {
                sha256_hex: "a".repeat(64),
                length: 512,
            },
        );
        TargetsMetadata {
            domain: APP_TRUST_DOMAIN.to_string(),
            version,
            expires,
            targets: map,
        }
    }

    fn signed(&self, value: &impl serde::Serialize, key: &Key) -> SignedPayload {
        let payload = serde_json::to_string(value).unwrap();
        let signatures = vec![key.sign(&payload)];
        SignedPayload {
            payload,
            signatures,
        }
    }

    /// Root, timestamp, snapshot, targets that all agree with each other.
    fn consistent(&self) -> (SignedPayload, SignedPayload, SignedPayload, SignedPayload) {
        let root = self.signed(&self.root(1, LATER), &self.root_key);

        let targets = self.targets(5, LATER);
        let targets_payload = serde_json::to_string(&targets).unwrap();
        let signed_targets = SignedPayload {
            payload: targets_payload.clone(),
            signatures: vec![self.tgt_key.sign(&targets_payload)],
        };

        let snapshot = SnapshotMetadata {
            domain: APP_TRUST_DOMAIN.to_string(),
            version: 9,
            expires: LATER,
            targets_version: 5,
            targets_sha256_hex: sha256_hex(&targets_payload),
        };
        let snapshot_payload = serde_json::to_string(&snapshot).unwrap();
        let signed_snapshot = SignedPayload {
            payload: snapshot_payload.clone(),
            signatures: vec![self.snap_key.sign(&snapshot_payload)],
        };

        let timestamp = TimestampMetadata {
            domain: APP_TRUST_DOMAIN.to_string(),
            version: 20,
            expires: LATER,
            snapshot_version: 9,
            snapshot_sha256_hex: sha256_hex(&snapshot_payload),
        };
        let signed_timestamp = self.signed(&timestamp, &self.ts_key);

        (root, signed_timestamp, signed_snapshot, signed_targets)
    }
}

// ── The chain accepts a consistent set ──────────────────────────────────────

#[test]
fn a_fully_consistent_chain_verifies() {
    let f = Fixture::new();
    let (root, ts, snap, tgt) = f.consistent();

    let verified = verify_chain(&root, &ts, &snap, &tgt, &FixedClock(NOW), None)
        .expect("a consistent chain must verify");

    assert!(
        verified
            .targets
            .targets
            .contains_key("chimera-app-latest.json")
    );
    assert_eq!(verified.root.version, 1);
}

// ── Expiry ──────────────────────────────────────────────────────────────────

#[test]
fn an_expired_timestamp_is_refused() {
    // The freeze attack: keep serving yesterday's timestamp so the client
    // never learns a newer snapshot exists. Expiry is the only thing that
    // makes "stale" distinguishable from "unchanged".
    let f = Fixture::new();
    let (_, ts, snap, tgt) = f.consistent();
    // Root outlives everything else, so the only thing expired at the moment
    // of the check is the timestamp. Root legitimately expires first when they
    // share a date, and the chain would then refuse for that reason instead —
    // correct, but it would not be testing what this test claims to test.
    let long_lived_root = f.signed(&f.root(1, LATER * 2), &f.root_key);

    let err = verify_chain(
        &long_lived_root,
        &ts,
        &snap,
        &tgt,
        &FixedClock(LATER + 1),
        None,
    )
    .unwrap_err();

    assert!(
        matches!(
            err,
            TrustError::Expired {
                role: Role::Timestamp,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn every_role_has_its_expiry_enforced() {
    // Not just the timestamp: a chain that checked one document's expiry and
    // not the others would let a stale targets list through behind a fresh
    // timestamp.
    let f = Fixture::new();
    for role in [Role::Root, Role::Snapshot, Role::Targets] {
        let (mut root, ts, mut snap, mut tgt) = f.consistent();
        match role {
            Role::Root => root = f.signed(&f.root(1, NOW - 1), &f.root_key),
            Role::Snapshot => {
                let mut s: SnapshotMetadata = serde_json::from_str(&snap.payload).unwrap();
                s.expires = NOW - 1;
                let p = serde_json::to_string(&s).unwrap();
                snap = SignedPayload {
                    signatures: vec![f.snap_key.sign(&p)],
                    payload: p,
                };
            }
            Role::Targets => {
                let mut t: TargetsMetadata = serde_json::from_str(&tgt.payload).unwrap();
                t.expires = NOW - 1;
                let p = serde_json::to_string(&t).unwrap();
                tgt = SignedPayload {
                    signatures: vec![f.tgt_key.sign(&p)],
                    payload: p,
                };
            }
            Role::Timestamp => unreachable!(),
        }
        // Snapshot and targets are re-signed, so the pins above them no longer
        // match; either refusal is correct, but it must not verify.
        let result = verify_chain(&root, &ts, &snap, &tgt, &FixedClock(NOW), None);
        assert!(result.is_err(), "{role:?} expiry was not enforced");
    }
}

// ── Rollback ────────────────────────────────────────────────────────────────

#[test]
fn a_timestamp_older_than_the_one_already_trusted_is_refused() {
    // Rollback attack. Every document here is validly signed — the only thing
    // wrong is that it is old, which is exactly what makes this class of
    // attack invisible to signature checking alone.
    let f = Fixture::new();
    let (root, ts, snap, tgt) = f.consistent();

    let previous = chimera_update::trust::TrustedVersions {
        root: 1,
        timestamp: 999, // we have already seen a much newer timestamp
        snapshot: 9,
        targets: 5,
    };

    let err = verify_chain(&root, &ts, &snap, &tgt, &FixedClock(NOW), Some(&previous)).unwrap_err();

    assert!(
        matches!(
            err,
            TrustError::Rollback {
                role: Role::Timestamp,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn an_equal_version_is_accepted_but_a_lower_one_is_not() {
    // Re-fetching the same version is normal — nothing changed. Only a
    // decrease is an attack.
    let f = Fixture::new();
    let (root, ts, snap, tgt) = f.consistent();
    let same = chimera_update::trust::TrustedVersions {
        root: 1,
        timestamp: 20,
        snapshot: 9,
        targets: 5,
    };

    verify_chain(&root, &ts, &snap, &tgt, &FixedClock(NOW), Some(&same))
        .expect("an unchanged chain must still verify");
}

#[test]
fn a_root_older_than_the_one_already_trusted_is_refused() {
    let f = Fixture::new();
    let (root, ts, snap, tgt) = f.consistent();
    let previous = chimera_update::trust::TrustedVersions {
        root: 7,
        timestamp: 0,
        snapshot: 0,
        targets: 0,
    };

    let err = verify_chain(&root, &ts, &snap, &tgt, &FixedClock(NOW), Some(&previous)).unwrap_err();

    assert!(
        matches!(
            err,
            TrustError::Rollback {
                role: Role::Root,
                ..
            }
        ),
        "got {err:?}"
    );
}

// ── Mix and match ───────────────────────────────────────────────────────────

#[test]
fn a_targets_list_the_snapshot_did_not_vouch_for_is_refused() {
    // The whole reason snapshot exists. Both documents are validly signed by
    // their correct keys; they simply do not describe the same release.
    let f = Fixture::new();
    let (root, ts, snap, _) = f.consistent();

    let other = f.targets(5, LATER);
    let mut swapped = other.clone();
    swapped.targets.insert(
        "evil.exe".to_string(),
        TargetEntry {
            sha256_hex: "f".repeat(64),
            length: 1,
        },
    );
    let payload = serde_json::to_string(&swapped).unwrap();
    let signed = SignedPayload {
        signatures: vec![f.tgt_key.sign(&payload)],
        payload,
    };

    let err = verify_chain(&root, &ts, &snap, &signed, &FixedClock(NOW), None).unwrap_err();

    assert!(
        matches!(
            err,
            TrustError::HashMismatch {
                role: Role::Targets,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn a_snapshot_the_timestamp_did_not_vouch_for_is_refused() {
    let f = Fixture::new();
    let (root, ts, _, tgt) = f.consistent();

    let other = SnapshotMetadata {
        domain: APP_TRUST_DOMAIN.to_string(),
        version: 9,
        expires: LATER,
        targets_version: 5,
        targets_sha256_hex: sha256_hex(&tgt.payload),
    };
    // Same fields but a different serialisation would still hash differently;
    // change a field so the difference is unambiguous.
    let mut tampered = other.clone();
    tampered.targets_version = 6;
    let payload = serde_json::to_string(&tampered).unwrap();
    let signed = SignedPayload {
        signatures: vec![f.snap_key.sign(&payload)],
        payload,
    };

    let err = verify_chain(&root, &ts, &signed, &tgt, &FixedClock(NOW), None).unwrap_err();

    assert!(
        matches!(
            err,
            TrustError::HashMismatch {
                role: Role::Snapshot,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn a_version_number_that_disagrees_with_the_pin_is_refused() {
    // The hash pin and the version pin must BOTH hold. A chain that checked
    // only the hash would accept a document whose version field lies, and the
    // rollback check above runs on that field.
    let f = Fixture::new();
    let (root, ts, _, tgt) = f.consistent();

    let snapshot = SnapshotMetadata {
        domain: APP_TRUST_DOMAIN.to_string(),
        version: 9,
        expires: LATER,
        targets_version: 99, // the timestamp pinned targets_version 5
        targets_sha256_hex: sha256_hex(&tgt.payload),
    };
    let payload = serde_json::to_string(&snapshot).unwrap();
    let signed = SignedPayload {
        signatures: vec![f.snap_key.sign(&payload)],
        payload,
    };

    let result = verify_chain(&root, &ts, &signed, &tgt, &FixedClock(NOW), None);
    assert!(result.is_err(), "a lying version pin must not verify");
}

// ── Wrong signer ────────────────────────────────────────────────────────────

#[test]
fn a_document_signed_by_the_wrong_role_key_is_refused() {
    // Key compromise containment: an online timestamp key must not be able to
    // authorise a targets list, however valid its signature is.
    let f = Fixture::new();
    let (root, ts, snap, tgt) = f.consistent();

    let payload = tgt.payload.clone();
    let wrong = SignedPayload {
        signatures: vec![f.ts_key.sign(&payload)],
        payload,
    };

    let err = verify_chain(&root, &ts, &snap, &wrong, &FixedClock(NOW), None).unwrap_err();

    assert!(
        matches!(
            err,
            TrustError::Signature {
                role: Role::Targets,
                ..
            }
        ),
        "got {err:?}"
    );
}

#[test]
fn an_unsigned_document_is_refused() {
    let f = Fixture::new();
    let (root, ts, snap, tgt) = f.consistent();
    let unsigned = SignedPayload {
        payload: tgt.payload.clone(),
        signatures: vec![],
    };

    let err = verify_chain(&root, &ts, &snap, &unsigned, &FixedClock(NOW), None).unwrap_err();

    assert!(matches!(err, TrustError::Signature { .. }), "got {err:?}");
}

#[test]
fn a_root_not_signed_by_itself_is_refused() {
    // Root is self-signed by definition: it is the document that says which
    // key root is. Accepting one signed by anything else means accepting a
    // stranger's claim about who we should trust.
    let f = Fixture::new();
    let (_, ts, snap, tgt) = f.consistent();

    let payload = serde_json::to_string(&f.root(1, LATER)).unwrap();
    let not_self_signed = SignedPayload {
        signatures: vec![f.snap_key.sign(&payload)],
        payload,
    };

    let err = verify_chain(&not_self_signed, &ts, &snap, &tgt, &FixedClock(NOW), None).unwrap_err();

    assert!(
        matches!(
            err,
            TrustError::Signature {
                role: Role::Root,
                ..
            }
        ),
        "got {err:?}"
    );
}

// ── Trust domain ────────────────────────────────────────────────────────────

#[test]
fn a_document_from_the_codex_mirror_domain_cannot_satisfy_the_app_chain() {
    // G8/G15. If the two domains could ever satisfy each other, compromising
    // the mirror's online key would let an attacker replace Chimera itself,
    // and rotating one root would silently affect the other.
    let f = Fixture::new();
    let (_, ts, snap, tgt) = f.consistent();

    let mut foreign = f.root(1, LATER);
    foreign.domain = "codex-mirror.v1".to_string();
    let payload = serde_json::to_string(&foreign).unwrap();
    let signed = SignedPayload {
        signatures: vec![f.root_key.sign(&payload)],
        payload,
    };

    let err = verify_chain(&signed, &ts, &snap, &tgt, &FixedClock(NOW), None).unwrap_err();

    assert!(
        matches!(err, TrustError::Metadata(_)),
        "a foreign trust domain must be refused at parse time: {err:?}"
    );
}

// ── Reported versions feed the next run's rollback check ────────────────────

#[test]
fn a_successful_verification_reports_the_versions_it_accepted() {
    // These become the next run's rollback floor. If they were not returned,
    // every run would start from zero and rollback protection would be a
    // property of nothing.
    let f = Fixture::new();
    let (root, ts, snap, tgt) = f.consistent();

    let verified = verify_chain(&root, &ts, &snap, &tgt, &FixedClock(NOW), None).unwrap();

    assert_eq!(verified.versions.root, 1);
    assert_eq!(verified.versions.timestamp, 20);
    assert_eq!(verified.versions.snapshot, 9);
    assert_eq!(verified.versions.targets, 5);
}
