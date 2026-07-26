// Step 8.4 regression — the fingerprint must not collide on selector boundaries.
//
// Found by an adversarial review. The digest joined selectors with `\n`, so
// `["a\nb"]` and `["a", "b"]` produced the same value. A separator only avoids
// boundary collisions for inputs that cannot contain it, and a CSS selector
// can contain a newline.
//
// This is the worst collision this particular value can have. The fingerprint
// exists to notice that Codex's UI changed; two different selector sets sharing
// a digest means a changed UI reporting as unchanged, so the fuse never trips
// and a skin keeps injecting into a shell it was never validated against.

use chimera_theme::fingerprint::{ProbeInput, compute_fingerprint};

fn probe(selectors: &[&str]) -> ProbeInput {
    ProbeInput {
        codex_version: "26.721".to_string(),
        observed_selectors: selectors.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn a_selector_containing_the_old_separator_does_not_collide() {
    let joined = compute_fingerprint(&probe(&["a\nb"]));
    let split = compute_fingerprint(&probe(&["a", "b"]));
    assert_ne!(
        joined.digest_hex, split.digest_hex,
        "[\"a\nb\"] and [\"a\", \"b\"] are different selector sets and must not share a digest"
    );
}

#[test]
fn adjacent_selectors_do_not_collide_across_their_boundary() {
    // The classic case a bare concatenation gets wrong.
    let a = compute_fingerprint(&probe(&[".ab", ".c"]));
    let b = compute_fingerprint(&probe(&[".a", ".bc"]));
    assert_ne!(a.digest_hex, b.digest_hex, "boundary collision");
}

#[test]
fn a_trailing_empty_selector_changes_the_digest() {
    // Invisible under any delimiter-only scheme, which is why the count is
    // hashed too.
    let without = compute_fingerprint(&probe(&[".a"]));
    let with = compute_fingerprint(&probe(&[".a", ""]));
    assert_ne!(
        without.digest_hex, with.digest_hex,
        "an extra empty selector must be observable in the digest"
    );
}

#[test]
fn order_and_duplicates_still_do_not_matter() {
    // The property the canonicalisation exists for must survive the fix: DOM
    // traversal order varies between runs, and a fingerprint that changed with
    // it would trip the fuse at random.
    let a = compute_fingerprint(&probe(&[".a", ".b", ".a"]));
    let b = compute_fingerprint(&probe(&[".b", ".a"]));
    assert_eq!(a.digest_hex, b.digest_hex);
}
