// Step 8.2 — the CDP transport's decidable parts.
//
// What a unit test can honestly establish here is the protocol: which endpoint
// is contacted, what a target list parses to, which frames are responses, and
// what Codex is launched with. That a real Codex build answers these exact
// messages is not something a fake can stand in for, and this file does not
// pretend otherwise — that belongs to the clean-VM run.
//
// Nothing here opens a socket or spawns a process.

use chimera_theme::cdp_transport::{
    build_command, is_top_level_navigation, launch_args, parse_targets, response_for,
    target_socket_url, targets_endpoint,
};

// ── Everything is loopback, by construction ─────────────────────────────────

#[test]
fn every_endpoint_is_the_ipv4_loopback_literal() {
    // Never "localhost": it can resolve to ::1 first on a dual-stack machine
    // while the browser was told to bind IPv4. Hard-coding the literal removes
    // a resolver from the path entirely.
    for url in [targets_endpoint(9222), target_socket_url(9222, "T1")] {
        assert!(url.contains("127.0.0.1"), "not loopback: {url}");
        assert!(!url.contains("localhost"), "resolver in the path: {url}");
        assert!(!url.contains("0.0.0.0"), "wildcard bind: {url}");
    }
}

#[test]
fn the_socket_url_is_built_never_taken_from_the_browsers_answer() {
    // CDP publishes a webSocketDebuggerUrl per target. Trusting it would let a
    // spoofed /json/list response point the session at another host, which is
    // exactly what the random loopback port exists to prevent.
    let url = target_socket_url(4711, "ABC123");
    assert_eq!(url, "ws://127.0.0.1:4711/devtools/page/ABC123");
}

#[test]
fn the_launch_arguments_pin_the_debug_address() {
    // Some Chromium builds bind every interface when only the port is given,
    // which would put the debug endpoint on the LAN.
    let args = launch_args(4711, "C:/tmp/profile");
    assert!(args.iter().any(|a| a == "--remote-debugging-port=4711"));
    assert!(
        args.iter()
            .any(|a| a == "--remote-debugging-address=127.0.0.1")
    );
    assert!(
        args.iter().any(|a| a.starts_with("--user-data-dir=")),
        "a session must not run in the user's real Codex profile"
    );
}

// ── Target parsing ──────────────────────────────────────────────────────────

#[test]
fn a_normal_target_list_parses() {
    let body = r#"[
      {"id":"A","type":"page","url":"https://codex.local/"},
      {"id":"B","type":"worker","url":"https://codex.local/w.js"}
    ]"#;
    let targets = parse_targets(body).unwrap();
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].id, "A");
    assert_eq!(targets[0].kind, "page");
}

#[test]
fn one_malformed_entry_does_not_lose_the_whole_list() {
    // A browser reporting one odd target should not make the skin engine
    // unusable.
    let body = r#"[{"id":"A","type":"page","url":"u"},{"nope":true},{"id":"C","type":"page"}]"#;
    let targets = parse_targets(body).unwrap();
    let ids: Vec<&str> = targets.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["A", "C"]);
}

#[test]
fn a_response_that_is_not_a_list_is_an_error_not_an_empty_list() {
    // Empty would read as "Codex has no windows open" and send the user to
    // debug the wrong thing.
    assert!(parse_targets(r#"{"error":"nope"}"#).is_err());
    assert!(parse_targets("not json at all").is_err());
}

// ── Request/response correlation ────────────────────────────────────────────

#[test]
fn a_command_carries_its_id_and_method() {
    let frame = build_command(7, "CSS.enable", serde_json::json!({}));
    let v: serde_json::Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(v["id"], 7);
    assert_eq!(v["method"], "CSS.enable");
}

#[test]
fn only_the_matching_id_is_treated_as_a_response() {
    // Events and other calls' replies share the stream. Accepting the wrong
    // one would return another command's result as if it were ours.
    assert!(response_for(r#"{"id":7,"result":{"ok":true}}"#, 7).is_some());
    assert!(response_for(r#"{"id":8,"result":{}}"#, 7).is_none());
    assert!(response_for(r#"{"method":"Page.frameNavigated","params":{}}"#, 7).is_none());
    assert!(response_for("not json", 7).is_none());
}

#[test]
fn a_cdp_error_reply_is_an_error_without_echoing_its_message() {
    // A CDP error message can name a target or a URL, and the user cannot act
    // on either.
    let reply =
        r#"{"id":7,"error":{"code":-32000,"message":"No frame with given id http://internal/x"}}"#;
    let err = response_for(reply, 7).unwrap().unwrap_err();
    let shown = err.to_string();
    assert!(
        !shown.contains("internal"),
        "the CDP message leaked: {shown}"
    );
    assert!(
        shown.contains("-32000"),
        "the code is the actionable part: {shown}"
    );
}

// ── Navigation ──────────────────────────────────────────────────────────────

#[test]
fn only_a_top_level_navigation_counts() {
    // A subframe navigation does not clear the page's stylesheets, so treating
    // one as a reason to reinject would push the skin again on every iframe.
    let top = r#"{"method":"Page.frameNavigated","params":{"frame":{"id":"F1","url":"u"}}}"#;
    let sub = r#"{"method":"Page.frameNavigated","params":{"frame":{"id":"F2","parentId":"F1"}}}"#;
    assert!(is_top_level_navigation(top));
    assert!(!is_top_level_navigation(sub));
}

#[test]
fn an_unrelated_event_is_not_a_navigation() {
    assert!(!is_top_level_navigation(
        r#"{"method":"CSS.styleSheetAdded","params":{}}"#
    ));
    assert!(!is_top_level_navigation(r#"{"id":1,"result":{}}"#));
    assert!(!is_top_level_navigation("garbage"));
}
