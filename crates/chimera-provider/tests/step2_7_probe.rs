// Step 2.7 RED — real endpoint probe.
//
// Spec 7.2 / Task 3 Step 3.1: a provider may only be activated after a
// successful probe. Client-side URL validation is not verification — it cannot
// tell a working endpoint from a typo'd one with a valid shape.
//
// The status→health mapping is a pure function and is tested exhaustively here.
// The live-network path is gated behind CHIMERA_TEST_NETWORK=1 so CI stays
// hermetic.
use chimera_domain::ProviderHealth;
use chimera_provider::probe::{
    ProbeOutcome, classify_probe_status, classify_transport_error, redirect_verdict,
};

// ── Status code → health verdict ─────────────────────────────────────────────

#[test]
fn success_with_model_list_is_healthy() {
    let out = classify_probe_status(200, Some(&["gpt-4o".to_string(), "o3".to_string()]));
    assert_eq!(out.health, ProviderHealth::Healthy);
    assert_eq!(out.discovered_models, vec!["gpt-4o", "o3"]);
    assert!(out.ok, "a 200 with a model list must be usable");
}

#[test]
fn unauthorised_is_auth_failed_not_unreachable() {
    // 401 means we reached the endpoint — the key is wrong. Reporting
    // "unreachable" would send the user to debug their network instead of
    // their key.
    for status in [401, 403] {
        let out = classify_probe_status(status, None);
        assert_eq!(
            out.health,
            ProviderHealth::AuthFailed,
            "status {status} must be auth_failed"
        );
        assert!(!out.ok);
    }
}

#[test]
fn not_found_is_incompatible_because_the_endpoint_answered() {
    // A 404 means the host is live but does not serve the OpenAI-compatible
    // route — the base URL is wrong, not the key.
    let out = classify_probe_status(404, None);
    assert_eq!(out.health, ProviderHealth::Incompatible);
    assert!(!out.ok);
}

#[test]
fn server_error_is_unreachable() {
    for status in [500, 502, 503] {
        let out = classify_probe_status(status, None);
        assert_eq!(
            out.health,
            ProviderHealth::Unreachable,
            "status {status} must be unreachable"
        );
    }
}

#[test]
fn success_without_a_model_list_is_healthy_but_reports_no_models() {
    // Some gateways return 200 with a non-standard body. The endpoint works,
    // so it is healthy; we simply have no catalog to offer.
    let out = classify_probe_status(200, None);
    assert_eq!(out.health, ProviderHealth::Healthy);
    assert!(out.discovered_models.is_empty());
    assert!(out.ok);
}

// ── Transport failures ───────────────────────────────────────────────────────

#[test]
fn timeout_is_unreachable_with_an_actionable_message() {
    let out = classify_transport_error(true, false);
    assert_eq!(out.health, ProviderHealth::Unreachable);
    assert!(!out.ok);
    // Must not surface a raw reqwest Debug string.
    assert!(
        !out.message.contains("reqwest"),
        "raw error leaked: {}",
        out.message
    );
    assert!(
        !out.message.contains("Error {"),
        "raw Debug leaked: {}",
        out.message
    );
    assert!(out.message.len() > 10, "message must be actionable");
}

#[test]
fn tls_failure_is_distinguished_from_a_plain_timeout() {
    // A TLS failure and a timeout need different user advice, so they must not
    // collapse into one message.
    let tls = classify_transport_error(false, true);
    let timeout = classify_transport_error(true, false);
    assert_eq!(tls.health, ProviderHealth::Unreachable);
    assert_ne!(
        tls.message, timeout.message,
        "TLS and timeout must give different guidance"
    );
}

// ── Cross-origin redirects must not carry the key ────────────────────────────
// Spec 7.2 bans following a redirect to another origin. The probe sends the key
// in an Authorization header, so a redirect that leaves the origin the user
// approved would hand their credential to a host they never named — the exact
// exfiltration a hostile or compromised endpoint would attempt.

#[test]
fn same_origin_redirect_is_followed() {
    // A gateway relocating /v1/models to /v1/models/ on its own host is
    // ordinary and must keep working.
    assert!(redirect_verdict(
        "https://api.example.com/v1/models",
        "https://api.example.com/v1/models/",
    ));
}

#[test]
fn cross_host_redirect_is_refused() {
    assert!(
        !redirect_verdict(
            "https://api.example.com/v1/models",
            "https://evil.example.net/collect",
        ),
        "a redirect to another host must not carry the Authorization header"
    );
}

#[test]
fn scheme_downgrade_redirect_is_refused() {
    // https -> http on the same host still puts the key on the wire in clear.
    assert!(!redirect_verdict(
        "https://api.example.com/v1/models",
        "http://api.example.com/v1/models",
    ));
}

#[test]
fn port_change_redirect_is_refused() {
    // A different port is a different origin, even on the same host.
    assert!(!redirect_verdict(
        "https://api.example.com/v1/models",
        "https://api.example.com:8443/v1/models",
    ));
}

#[test]
fn subdomain_redirect_is_refused() {
    // Same registrable domain is not the same origin.
    assert!(!redirect_verdict(
        "https://api.example.com/v1/models",
        "https://api.internal.example.com/v1/models",
    ));
}

#[test]
fn a_refused_redirect_is_reported_as_a_redirect_not_a_compatibility_problem() {
    // Stopping at the redirect leaves the 3xx as the observed status. The
    // message must name the real cause, or the user goes off debugging a
    // protocol mismatch that does not exist.
    for status in [301, 302, 307, 308] {
        let out = classify_probe_status(status, None);
        assert!(!out.ok, "status {status} must not count as verified");
        assert!(
            out.message.to_lowercase().contains("redirect"),
            "status {status} message must explain the redirect: {}",
            out.message
        );
    }
}

#[test]
fn unparseable_redirect_target_is_refused() {
    // Fail closed: if the destination cannot be understood, do not send the key.
    assert!(!redirect_verdict(
        "https://api.example.com/v1/models",
        "not a url",
    ));
}

// ── The probe never leaks the key ────────────────────────────────────────────

#[test]
fn probe_outcome_debug_never_contains_key_material() {
    let out = ProbeOutcome {
        ok: false,
        health: ProviderHealth::AuthFailed,
        message: "Authentication failed. Check that the API key is correct.".to_string(),
        discovered_models: vec![],
    };
    let dbg = format!("{out:?}");
    assert!(!dbg.contains("sk-"), "key-shaped material in Debug: {dbg}");
}
