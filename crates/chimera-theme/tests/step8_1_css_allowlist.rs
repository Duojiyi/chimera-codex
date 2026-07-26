// Step 8.1 RED — CSS property allowlist (ADR-005).
//
// Deny-by-default: a property, at-rule, or value shape not explicitly
// permitted is refused, not silently dropped and not silently kept. The
// package-level import treats any CSS validation failure as "refuse the
// whole package" (see step8_1_import.rs) — this file tests the allowlist in
// isolation, in terms of what it accepts/refuses.

use chimera_theme::css_allowlist::{CssError, validate_css};
use std::collections::HashSet;

fn assets(names: &[&str]) -> HashSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

#[test]
fn plain_allowed_declarations_pass() {
    let css = ".title { color: #fff; background-color: rgba(0,0,0,0.5); font-size: 14px; }";
    assert!(validate_css(css, &assets(&[])).is_ok());
}

#[test]
fn a_url_pointing_at_a_bundled_asset_is_allowed() {
    let css = ".bg { background-image: url(\"images/bg.png\"); }";
    assert!(validate_css(css, &assets(&["images/bg.png"])).is_ok());
}

#[test]
fn a_url_not_in_the_bundled_asset_set_is_refused() {
    let css = ".bg { background-image: url(\"images/missing.png\"); }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::UnbundledUrl(_))));
}

#[test]
fn a_disallowed_property_is_refused() {
    // `behavior` is an IE binding hook and was never meant to be reachable.
    let css = ".x { behavior: url(evil.htc); }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::DisallowedProperty(p)) if p == "behavior"));
}

#[test]
fn position_fixed_is_not_on_the_allowlist_by_default() {
    // `position` is not in the allowlist at all: a skin author cannot pin an
    // element over Codex's own UI chrome.
    let css = ".x { position: fixed; top: 0; }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::DisallowedProperty(_))));
}

// ── remote / absolute URLs are refused regardless of scheme ────────────────

#[test]
fn an_https_url_is_refused() {
    let css = ".bg { background-image: url(https://evil.example/track.png); }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::UnbundledUrl(_))));
}

#[test]
fn a_protocol_relative_url_is_refused() {
    let css = ".bg { background-image: url(//evil.example/track.png); }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::UnbundledUrl(_))));
}

#[test]
fn an_absolute_path_url_is_refused() {
    let css = ".bg { background-image: url(/etc/passwd); }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::UnbundledUrl(_))));
}

#[test]
fn a_url_with_traversal_is_refused_even_if_it_lands_on_a_bundled_name() {
    let css = ".bg { background-image: url(images/../images/bg.png); }";
    let result = validate_css(css, &assets(&["images/bg.png"]));
    assert!(matches!(result, Err(CssError::UnbundledUrl(_))));
}

#[test]
fn a_javascript_uri_is_refused() {
    let css = ".x { background: url(javascript:alert(1)); }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::UnbundledUrl(_))));
}

#[test]
fn a_data_uri_is_refused_even_for_an_image_mime_type() {
    // Bundled assets already provide a safe path for images; there is no
    // legitimate reason a skin needs an inline data: URI, and "does this
    // data: URI carry a script" is not reliably decidable in general, so all
    // of them are refused.
    let css = ".x { background: url(data:image/png;base64,iVBORw0KGgo=); }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::UnbundledUrl(_))));
}

// ── explicit script vectors ─────────────────────────────────────────────────

#[test]
fn css_expression_is_refused() {
    let css = ".x { width: expression(alert(1)); }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::ScriptVector(_))));
}

#[test]
fn import_is_refused() {
    let css = "@import url(\"https://evil.example/style.css\"); .x { color: red; }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::AtRuleRefused(_))));
}

#[test]
fn any_at_rule_is_refused() {
    let css = "@media (min-width: 1px) { .x { color: red; } }";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::AtRuleRefused(_))));
}

#[test]
fn unclosed_block_is_refused_rather_than_silently_truncated() {
    let css = ".x { color: red;";
    let result = validate_css(css, &assets(&[]));
    assert!(matches!(result, Err(CssError::UnbalancedBraces)));
}

#[test]
fn empty_css_is_valid() {
    assert!(validate_css("", &assets(&[])).is_ok());
}

#[test]
fn comments_are_stripped_before_scanning_so_they_cannot_hide_script_vectors() {
    // A naive scanner might only look at declaration values and be fooled by
    // a vector split across a comment boundary; stripping comments first
    // means there is nothing left to hide behind.
    let css = "/* @import url(evil.css); */ .x { color: red; }";
    assert!(validate_css(css, &assets(&[])).is_ok());
}

#[test]
fn a_string_containing_a_brace_does_not_desync_the_block_tracker() {
    let css = ".x::after { content: \"{\"; color: red; }";
    assert!(validate_css(css, &assets(&[])).is_ok());
}
