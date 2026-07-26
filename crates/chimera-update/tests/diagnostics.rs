// Step 9.3 RED — diagnostics and log rotation.
//
// The canary test is the point of this file. Everything else is supporting
// structure: if a planted, key-shaped value can reach the bundle, nothing else
// here matters.

use chimera_update::diagnostics::{
    DiagnosticInput, ErrorClass, build_bundle, classify, rotate_logs,
};
use std::fs;
use tempfile::TempDir;

fn canary() -> String {
    ["sk", "-", "CANARYzzzz1111222233334444555566667777"].concat()
}

// ── The canary ──────────────────────────────────────────────────────────────

#[test]
fn a_canary_planted_in_every_field_reaches_none_of_the_output() {
    // Fail-closed: not "the fields we remembered to redact", every field.
    let c = canary();
    let input = DiagnosticInput {
        app_version: format!("2.0.0 {c}"),
        os: format!("windows 11 {c}"),
        last_error: Some(format!("request failed {c}")),
        recent_log_lines: vec![
            format!("line one {c}"),
            format!("line two https://user:{c}@api.example.com/v1"),
        ],
        provider_host: Some(format!("api.example.com {c}")),
        runtime_version: Some(format!("26.721 {c}")),
    };

    let bundle = build_bundle(&input);
    let rendered = bundle.render();

    assert!(
        !rendered.contains(&c),
        "the canary reached the diagnostic output:\n{rendered}"
    );
}

#[test]
fn the_canary_test_would_fail_if_redaction_did_nothing() {
    // Guards the guard: if `render` ever stopped including the fields at all,
    // the test above would pass vacuously. This asserts the fields are there.
    let input = DiagnosticInput {
        app_version: "2.0.0".into(),
        os: "windows 11".into(),
        last_error: Some("request failed".into()),
        recent_log_lines: vec!["a distinctive log line".into()],
        provider_host: Some("api.example.com".into()),
        runtime_version: Some("26.721".into()),
    };
    let rendered = build_bundle(&input).render();
    for expected in [
        "2.0.0",
        "windows 11",
        "request failed",
        "a distinctive log line",
        "26.721",
    ] {
        assert!(
            rendered.contains(expected),
            "field missing from output: {expected}"
        );
    }
}

#[test]
fn redaction_is_applied_twice_and_the_second_pass_changes_nothing() {
    // The user approves a preview; what is sent must be identical to it.
    let input = DiagnosticInput {
        app_version: "2.0.0".into(),
        os: "windows 11".into(),
        last_error: Some(format!("auth failed for {}", canary())),
        recent_log_lines: vec![r"C:\Users\jdoe\AppData\Local\Chimera\app.log".into()],
        provider_host: Some("api.example.com".into()),
        runtime_version: None,
    };
    let once = build_bundle(&input).render();
    let twice = chimera_update::redact::redact(&once);
    assert_eq!(once, twice, "the preview and what would be sent differ");
}

#[test]
fn a_bundle_reports_whether_it_is_clean() {
    // The UI needs to be able to refuse to send rather than hoping.
    let input = DiagnosticInput {
        app_version: "2.0.0".into(),
        os: "windows 11".into(),
        last_error: Some(canary()),
        recent_log_lines: vec![],
        provider_host: None,
        runtime_version: None,
    };
    assert!(
        build_bundle(&input).is_clean(),
        "a redacted bundle must report clean"
    );
}

// ── Error classification ────────────────────────────────────────────────────

#[test]
fn errors_are_classified_into_actionable_groups() {
    assert_eq!(
        classify("The API key was rejected."),
        ErrorClass::Credentials
    );
    assert_eq!(
        classify("Could not reach the endpoint."),
        ErrorClass::Network
    );
    assert_eq!(
        classify("Could not write to the Chimera folder."),
        ErrorClass::Storage
    );
    assert_eq!(
        classify("something nobody anticipated"),
        ErrorClass::Unknown
    );
}

#[test]
fn classification_never_reads_the_secret_it_is_classifying() {
    // A classifier keyed on message text must not be fed raw credentials by a
    // caller that assumed it was safe. It takes the redacted form.
    let c = canary();
    let class = classify(&chimera_update::redact::redact(&format!(
        "key {c} rejected"
    )));
    assert_ne!(
        class,
        ErrorClass::Unknown,
        "redaction should not destroy classifiability"
    );
}

// ── Log rotation ────────────────────────────────────────────────────────────

fn write_log(dir: &TempDir, name: &str, bytes: usize) {
    fs::write(dir.path().join(name), "x".repeat(bytes)).unwrap();
}

#[test]
fn rotation_keeps_the_newest_and_removes_the_oldest() {
    let dir = TempDir::new().unwrap();
    for i in 1..=5 {
        write_log(&dir, &format!("app.{i}.log"), 100);
    }

    rotate_logs(dir.path(), 3, 10_000).unwrap();

    let remaining: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(remaining.len(), 3, "expected 3 files, got {remaining:?}");
}

#[test]
fn rotation_enforces_a_total_size_budget() {
    let dir = TempDir::new().unwrap();
    for i in 1..=4 {
        write_log(&dir, &format!("app.{i}.log"), 1_000);
    }

    rotate_logs(dir.path(), 10, 2_500).unwrap();

    let total: u64 = fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();
    assert!(total <= 2_500, "budget exceeded: {total}");
}

#[test]
fn rotation_never_removes_the_only_log() {
    // Losing the current log to make room is the one outcome worse than a log
    // directory slightly over budget: it deletes the evidence of whatever is
    // filling it.
    let dir = TempDir::new().unwrap();
    write_log(&dir, "app.1.log", 10_000);

    rotate_logs(dir.path(), 3, 100).unwrap();

    assert!(
        dir.path().join("app.1.log").exists(),
        "the only log was deleted"
    );
}

#[test]
fn rotation_on_an_absent_directory_is_not_an_error() {
    // It runs at startup, before anything has necessarily written a log.
    let dir = TempDir::new().unwrap();
    rotate_logs(&dir.path().join("nope"), 3, 100).expect("absent dir must be a no-op");
}
