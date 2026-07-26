// G9 — the skin engine must never execute JavaScript.
//
// An earlier CDP client injected CSS by evaluating
// `document.createElement('style')` through `Runtime.evaluate`. It was careful
// about the CSS it embedded — JSON-encoded rather than concatenated — but
// careful about the wrong thing. G9 forbids arbitrary JavaScript outright, so
// the mechanism was the violation, not the payload. A skin engine that runs
// script to install a stylesheet breaks the rule it exists to enforce.
//
// This reads the crate's own source. That is unusual for a test and it is the
// point: the property is "no code path anywhere in this crate can execute
// script", which no behavioural test over the current API can establish —
// a future method could reintroduce it and every existing test would stay
// green.

use std::fs;
use std::path::Path;

/// Every CDP method that runs caller-supplied script in the page.
const SCRIPT_EXECUTING_METHODS: [&str; 5] = [
    "Runtime.evaluate",
    "Runtime.callFunctionOn",
    "Runtime.compileScript",
    "Page.addScriptToEvaluateOnNewDocument",
    "Page.addScriptToEvaluateOnLoad",
];

fn source_files() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).expect("src must be readable") {
            let path = entry.expect("readable entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = fs::read_to_string(&path).expect("source must be UTF-8");
                out.push((path.display().to_string(), text));
            }
        }
    }
    assert!(!out.is_empty(), "found no source files to check");
    out
}

/// Strip comments so the explanation of the old bug does not trip the check
/// that exists because of it.
fn code_only(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with("*") || trimmed.starts_with("/*") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[test]
fn no_source_file_names_a_script_executing_cdp_method() {
    for (path, text) in source_files() {
        let code = code_only(&text);
        for method in SCRIPT_EXECUTING_METHODS {
            assert!(
                !code.contains(method),
                "{path} uses {method}, which executes JavaScript in the page (G9)"
            );
        }
    }
}

#[test]
fn css_is_installed_through_the_css_domain() {
    // The counterpart to the rule above: assert the sanctioned mechanism is
    // actually present, so "no JavaScript" cannot be satisfied by a client
    // that installs nothing at all.
    let all: String = source_files().into_iter().map(|(_, t)| t).collect();
    for method in ["CSS.createStyleSheet", "CSS.setStyleSheetText"] {
        assert!(
            all.contains(method),
            "the CDP CSS domain call {method} is missing"
        );
    }
}

#[test]
fn the_check_itself_can_fail() {
    // A source-scanning test that never matches anything would pass on an
    // empty directory. Prove the matcher works on text that should trip it.
    let planted = code_only("fn x() { let m = \"Runtime.evaluate\"; }");
    assert!(
        SCRIPT_EXECUTING_METHODS.iter().any(|m| planted.contains(m)),
        "the scanner would not notice a real occurrence"
    );
    // And that a comment mentioning it does not.
    let commented = code_only("// we must never use Runtime.evaluate here\nfn x() {}");
    assert!(
        !SCRIPT_EXECUTING_METHODS
            .iter()
            .any(|m| commented.contains(m)),
        "a comment about the rule was mistaken for a violation"
    );
}
