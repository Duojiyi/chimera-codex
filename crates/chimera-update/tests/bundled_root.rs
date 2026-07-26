// The compiled-in trust seed a fresh install starts from.
//
// No production signing key exists yet, so this is a clearly-labelled
// DEVELOPMENT root. The point of these tests is not just "it parses" — it is
// that shipping this placeholder as if it were a real trust anchor is
// something the crate itself can detect and refuse, not just a naming
// convention a reviewer has to remember to check.

use chimera_update::bundled_root::{development_root, is_development_root};
use chimera_update::metadata::{APP_TRUST_DOMAIN, Role, parse_root, verify_threshold};

#[test]
fn the_development_root_parses_and_is_in_the_app_trust_domain() {
    let signed = development_root().expect("the bundled dev root must construct");
    let root = parse_root(&signed.payload).expect("the bundled dev root must parse");
    assert_eq!(root.domain, APP_TRUST_DOMAIN);
}

#[test]
fn the_development_root_is_self_signed_consistently() {
    // Root is self-signed by definition (see `trust::verify_chain`): the
    // candidates for a root signature check are root's own declared root
    // keys, and threshold must be met by real signatures over these exact
    // bytes — not merely present.
    let signed = development_root().expect("the bundled dev root must construct");
    let root = parse_root(&signed.payload).expect("the bundled dev root must parse");

    let candidates = root.resolve_keys(Role::Root);
    let threshold = root.role(Role::Root).threshold;
    verify_threshold(
        &signed.payload,
        &signed.signatures,
        &candidates,
        threshold,
        "root",
    )
    .expect("the bundled dev root must be self-signed consistently");
}

#[test]
fn the_development_root_is_flagged_as_development_not_production() {
    // This is the hook a release build's bootstrap must call before trusting
    // any root it did not just fetch and verify through a full chain: refuse
    // to proceed if this ever reports true outside of an explicit dev mode.
    let signed = development_root().expect("the bundled dev root must construct");
    let root = parse_root(&signed.payload).expect("the bundled dev root must parse");
    assert!(
        is_development_root(&root),
        "the bundled root must be identifiable as a development placeholder"
    );
}

#[test]
fn every_role_key_id_in_the_development_root_says_so_on_its_face() {
    // Belt and braces alongside `is_development_root`: even someone who only
    // greps the JSON for a key id, without running any code, should not be
    // able to mistake this for a production key.
    let signed = development_root().expect("the bundled dev root must construct");
    let root = parse_root(&signed.payload).expect("the bundled dev root must parse");
    for key in &root.keys {
        assert!(
            key.key_id.to_ascii_lowercase().contains("dev")
                && key.key_id.to_ascii_lowercase().contains("insecure"),
            "key id does not self-identify as an insecure development key: {}",
            key.key_id
        );
    }
}
