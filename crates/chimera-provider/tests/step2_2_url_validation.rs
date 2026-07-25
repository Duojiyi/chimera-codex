// Step 2.2 RED — URL validation and probe security rules.
// These tests enforce the spec's security requirements for provider URL input.
use chimera_provider::probe::{validate_provider_url, UrlValidationError};
use url::Url;

// ── HTTPS enforcement ────────────────────────────────────────────────────────

#[test]
fn http_url_is_rejected_outside_loopback() {
    let r = validate_provider_url("http://api.example.com/v1", false);
    assert!(r.is_err(), "non-loopback HTTP must be rejected");
    let e = r.unwrap_err();
    assert!(matches!(e, UrlValidationError::InsecureScheme { .. }));
}

#[test]
fn http_loopback_allowed_only_in_dev_mode() {
    // dev_mode = false → reject
    assert!(validate_provider_url("http://127.0.0.1:8080/v1", false).is_err());
    // dev_mode = true → allow loopback HTTP
    assert!(validate_provider_url("http://127.0.0.1:8080/v1", true).is_ok());
    assert!(validate_provider_url("http://localhost:8080/v1", true).is_ok());
}

#[test]
fn https_url_is_accepted() {
    let r = validate_provider_url("https://api.example.com/v1", false);
    assert!(r.is_ok(), "valid HTTPS must be accepted: {:?}", r);
}

// ── userinfo and fragment bans ────────────────────────────────────────────────

#[test]
fn userinfo_in_url_is_rejected() {
    let r = validate_provider_url("https://user:pass@api.example.com/v1", false);
    assert!(r.is_err());
    assert!(matches!(r.unwrap_err(), UrlValidationError::ContainsUserinfo));
}

#[test]
fn fragment_in_url_is_rejected() {
    // Fragments are stripped by browsers but must be rejected in API endpoints
    // (they indicate a confused / misconfigured URL)
    let r = validate_provider_url("https://api.example.com/v1#frag", false);
    assert!(r.is_err());
    assert!(matches!(r.unwrap_err(), UrlValidationError::ContainsFragment));
}

// ── scheme validation ────────────────────────────────────────────────────────

#[test]
fn ftp_scheme_is_rejected() {
    let r = validate_provider_url("ftp://files.example.com/v1", false);
    assert!(r.is_err());
    assert!(matches!(r.unwrap_err(), UrlValidationError::InsecureScheme { .. }));
}

#[test]
fn file_scheme_is_rejected() {
    let r = validate_provider_url("file:///etc/passwd", false);
    assert!(r.is_err());
}

// ── path normalisation ────────────────────────────────────────────────────────

#[test]
fn url_with_explicit_v1_path_is_accepted_verbatim() {
    // If the user already provided a full path, use it as-is
    let r = validate_provider_url("https://api.example.com/v1", false);
    assert!(r.is_ok());
    let validated = r.unwrap();
    assert_eq!(validated.base_url.path(), "/v1");
}

#[test]
fn url_with_origin_only_returns_candidate_with_v1() {
    // origin-only → probe suggests /v1 as a candidate but must not silently write it
    let r = validate_provider_url("https://api.example.com", false);
    assert!(r.is_ok());
    let validated = r.unwrap();
    // The validated struct should expose whether we added /v1 as a candidate
    assert!(validated.v1_candidate.is_some());
}

#[test]
fn empty_url_is_rejected() {
    assert!(validate_provider_url("", false).is_err());
}

#[test]
fn url_parse_failure_returns_error() {
    assert!(validate_provider_url("not a url at all!!", false).is_err());
}

// ── cross-origin redirect rule ────────────────────────────────────────────────

#[test]
fn validated_url_exposes_origin_for_cross_origin_check() {
    let r = validate_provider_url("https://api.chimerahub.io/v1", false).unwrap();
    assert_eq!(r.base_url.origin(), Url::parse("https://api.chimerahub.io/v1").unwrap().origin());
}
