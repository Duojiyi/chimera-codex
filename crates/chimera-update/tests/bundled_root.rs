// The compiled-in root is a production trust anchor. The private root key is
// intentionally absent from the repository; only its signed public metadata
// is shipped to fresh installations.

use chimera_update::bundled_root::{bundled_root, is_development_root};
use chimera_update::metadata::{APP_TRUST_DOMAIN, Role, parse_root, verify_threshold};

#[test]
fn the_bundled_root_parses_and_is_in_the_app_trust_domain() {
    let signed = bundled_root().expect("the bundled root must construct");
    let root = parse_root(&signed.payload).expect("the bundled root must parse");
    assert_eq!(root.domain, APP_TRUST_DOMAIN);
    assert_eq!(root.version, 1);
    assert!(!is_development_root(&root));
}

#[test]
fn the_bundled_root_is_self_signed_consistently() {
    let signed = bundled_root().expect("the bundled root must construct");
    let root = parse_root(&signed.payload).expect("the bundled root must parse");
    let candidates = root.resolve_keys(Role::Root);
    verify_threshold(
        &signed.payload,
        &signed.signatures,
        &candidates,
        root.role(Role::Root).threshold,
        "root",
    )
    .expect("the bundled root must be self-signed");
}

#[test]
fn every_role_has_an_independent_production_key() {
    let signed = bundled_root().expect("the bundled root must construct");
    let root = parse_root(&signed.payload).expect("the bundled root must parse");
    let ids = [
        &root.root.key_ids[0],
        &root.targets.key_ids[0],
        &root.snapshot.key_ids[0],
        &root.timestamp.key_ids[0],
    ];
    assert_eq!(
        ids.len(),
        ids.iter().collect::<std::collections::BTreeSet<_>>().len()
    );
    assert!(ids.iter().all(|id| id.starts_with("chimera-app-")));
}
