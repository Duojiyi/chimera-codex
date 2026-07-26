// Step 9.3 RED — secret redaction.
//
// Diagnostics exist so a user can send us something useful when Chimera
// misbehaves. That makes them the one artifact designed to leave the machine,
// which makes this the last place a credential can escape.
//
// The requirement is not "usually catches keys". It is fail-closed: a canary
// planted anywhere in the input must not appear anywhere in the output, and
// running redaction twice must change nothing the first pass already handled.
//
// Fixture strings are assembled at runtime rather than written literally.
// scripts/verify-no-secrets.mjs scans this repository and cannot tell a
// convincing test fixture from a real leak — which is exactly the property
// that makes it worth having, so the fixtures work around it instead of
// teaching it an exception it would then apply to a genuine key.

use chimera_update::redact::{contains_secret, redact};

fn api_key() -> String {
    ["sk", "-", "PROJqL8xTn4mWvZ2bKcR7aYd9eHgJfMnQpSt"].concat()
}
fn github_token() -> String {
    ["ghp", "_", "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789"].concat()
}
fn fine_grained() -> String {
    [
        "github",
        "_pat_",
        "11AAAAAAA0",
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWX",
    ]
    .concat()
}
fn jwt() -> String {
    [
        "eyJhbGciOiJIUzI1NiJ9",
        ".",
        "eyJzdWIiOiIxMjM0NTY3ODkwIn0",
        ".",
        "dBjftJeZ4CVPmB92K27uhbUJU1p1r_wW1gFWFOEjXk",
    ]
    .concat()
}

// ── Each pattern is caught ──────────────────────────────────────────────────

#[test]
fn an_api_key_is_removed() {
    let out = redact(&format!("Authorization: Bearer {}", api_key()));
    assert!(!out.contains(&api_key()), "key survived: {out}");
}

#[test]
fn github_tokens_of_both_shapes_are_removed() {
    for token in [github_token(), fine_grained()] {
        let out = redact(&format!("token={token}"));
        assert!(!out.contains(&token), "token survived: {out}");
    }
}

#[test]
fn a_jwt_is_removed() {
    let out = redact(&format!("cookie: session={}", jwt()));
    assert!(!out.contains(&jwt()), "jwt survived: {out}");
}

#[test]
fn credentials_embedded_in_a_url_are_removed() {
    let out = redact("endpoint=https://alice:hunter2@api.example.com/v1");
    assert!(!out.contains("hunter2"), "password survived: {out}");
    assert!(!out.contains("alice"), "username survived: {out}");
    // The host is the useful part and must stay, or the diagnostic says nothing.
    assert!(out.contains("api.example.com"), "host was destroyed: {out}");
}

#[test]
fn a_windows_user_path_keeps_its_shape_but_loses_the_name() {
    let out = redact(r"C:\Users\jdoe\AppData\Local\Chimera\logs\app.log");
    assert!(!out.contains("jdoe"), "username survived: {out}");
    assert!(
        out.contains("AppData"),
        "the useful part of the path was destroyed: {out}"
    );
    assert!(out.contains("app.log"), "the filename was destroyed: {out}");
}

#[test]
fn unix_home_paths_lose_the_name_too() {
    for path in [
        "/Users/jdoe/Library/Logs/app.log",
        "/home/jdoe/.config/chimera/app.log",
    ] {
        let out = redact(path);
        assert!(!out.contains("jdoe"), "username survived in {path}: {out}");
        assert!(out.contains("app.log"), "filename destroyed: {out}");
    }
}

#[test]
fn an_email_address_is_removed() {
    let out = redact("signed in as jane.doe+chimera@example.com today");
    assert!(!out.contains("jane.doe"), "email survived: {out}");
    assert!(out.contains("today"), "surrounding text destroyed: {out}");
}

// ── Properties, not just cases ──────────────────────────────────────────────

#[test]
fn redaction_is_idempotent() {
    // The diagnostics path applies it twice. If the second pass changed
    // already-clean output, the preview a user approves would not match what
    // is actually sent.
    let input = format!(
        "key={} path=C:\\Users\\jdoe\\x.log url=https://a:b@h.example/v1 mail=x@y.example jwt={}",
        api_key(),
        jwt()
    );
    let once = redact(&input);
    let twice = redact(&once);
    assert_eq!(once, twice, "second pass changed the output");
}

#[test]
fn ordinary_text_is_left_alone() {
    // Over-redaction makes diagnostics useless, which makes people stop
    // sending them — a slower path to the same place as leaking.
    let input = "Codex 26.721 failed to start: exit code 3221225781 after 12s on windows/x64";
    assert_eq!(redact(input), input);
}

#[test]
fn multiple_secrets_on_one_line_are_all_removed() {
    // A regex that returns after its first match leaves the rest in place.
    let input = format!("a={} b={} c={}", api_key(), github_token(), jwt());
    let out = redact(&input);
    for secret in [api_key(), github_token(), jwt()] {
        assert!(
            !out.contains(&secret),
            "one of several secrets survived: {out}"
        );
    }
}

#[test]
fn contains_secret_agrees_with_redact() {
    // The canary check and the redactor must not disagree: a detector that
    // saw something the redactor did not remove would report a clean bundle
    // that is not clean.
    let dirty = format!("key={}", api_key());
    assert!(contains_secret(&dirty));
    assert!(
        !contains_secret(&redact(&dirty)),
        "redact left something contains_secret can see"
    );
    assert!(!contains_secret("nothing sensitive here"));
}

#[test]
fn an_empty_input_is_handled() {
    assert_eq!(redact(""), "");
    assert!(!contains_secret(""));
}
