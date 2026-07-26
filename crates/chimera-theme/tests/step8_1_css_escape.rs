// Step 8.1 regression — CSS escape sequences must not smuggle a url() past
// the allowlist.
//
// Found by an adversarial review, reproduced end to end through the real
// `import_codexskin`. The scanner searched for the literal four bytes `url(`.
// CSS's escaped-code-point grammar means `\75rl(` decodes to `url(` in every
// standards-compliant tokenizer — including the Chromium engine this crate
// injects into over CDP — so the argument was never handed to the check that
// rejects remote schemes. A skin could load remote content, which is exactly
// what G9 forbids.
//
// The fix is not to teach the scanner CSS escapes. That is an arms race
// (`\75`, `\000075`, `\75 rl(`, mixed case, …) against a tokenizer whose
// behaviour is not ours to define. A skin has no legitimate need for an escape
// sequence, so the allowlist refuses backslashes outright — which removes the
// attack surface rather than chasing it.

use chimera_theme::css_allowlist::{CssError, validate_css};
use std::collections::HashSet;

fn assets() -> HashSet<String> {
    let mut s = HashSet::new();
    s.insert("bg.png".to_string());
    s
}

#[test]
fn an_escaped_url_function_is_refused() {
    let css = r".x{background-image:\75rl(https://evil.example/x.png)}";
    let err = validate_css(css, &assets()).unwrap_err();
    assert!(
        matches!(err, CssError::EscapeRefused(_)),
        "an escaped url() must be refused, got {err:?}"
    );
}

#[test]
fn every_escape_spelling_is_refused_not_just_the_one_that_was_reported() {
    // Whatever the tokenizer accepts, we do not have to enumerate: any
    // backslash is refused, so all of these fail for the same reason.
    for css in [
        r".x{background-image:\75rl(https://evil.example/x.png)}",
        r".x{background-image:\000075rl(https://evil.example/x.png)}",
        r".x{background-image:\75 rl(https://evil.example/x.png)}",
        r".x{background-image:u\72l(https://evil.example/x.png)}",
        r".x{background-image:url(\68ttps://evil.example/x.png)}",
    ] {
        let err = validate_css(css, &assets()).unwrap_err();
        assert!(
            matches!(err, CssError::EscapeRefused(_)),
            "not refused: {css} -> {err:?}"
        );
    }
}

#[test]
fn a_bundled_asset_url_still_works() {
    // The fix must not break the one thing url() is for.
    validate_css(".x{background-image:url(bg.png)}", &assets())
        .expect("a bundled asset must still be allowed");
}

#[test]
fn ordinary_declarations_are_unaffected() {
    validate_css(".x{color:#112233;margin:0 auto}", &assets()).expect("plain CSS must pass");
}

#[test]
fn an_escaped_property_name_is_refused_by_default_deny() {
    // The property side needs no separate escape check: an escaped name simply
    // is not on the allowlist, and the allowlist denies by default. Asserted
    // rather than assumed, because "it happens to be safe" and "it is safe by
    // construction" are different claims and only one survives a refactor.
    let css = r".x{\62 ackground-image:url(bg.png)}";
    let err = validate_css(css, &assets()).unwrap_err();
    assert!(
        matches!(err, CssError::DisallowedProperty(_)),
        "an escaped property name must not reach the allowlist as its decoded form: {err:?}"
    );
}
