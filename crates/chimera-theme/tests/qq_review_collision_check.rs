use chimera_theme::fingerprint::{ProbeInput, compute_fingerprint};

#[test]
fn review_selector_newline_boundary_collision() {
    let a = ProbeInput {
        codex_version: "1.2.3".to_string(),
        observed_selectors: vec!["a\nb".to_string()],
    };
    let b = ProbeInput {
        codex_version: "1.2.3".to_string(),
        observed_selectors: vec!["a".to_string(), "b".to_string()],
    };

    let fa = compute_fingerprint(&a);
    let fb = compute_fingerprint(&b);

    assert_ne!(
        a.observed_selectors, b.observed_selectors,
        "genuinely different selector sets"
    );
    assert_eq!(fa.digest_hex, fb.digest_hex, "collision reproduced");
}
