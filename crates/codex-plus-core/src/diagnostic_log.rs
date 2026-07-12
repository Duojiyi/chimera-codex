use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};

static TEST_LOG_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
struct DiagnosticRecord {
    timestamp_ms: u64,
    pid: u32,
    event: String,
    detail: Value,
}

pub fn append_diagnostic_log(event: &str, detail: impl Serialize) -> std::io::Result<()> {
    let path = diagnostic_log_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut detail = serde_json::to_value(detail).unwrap_or_else(|error| {
        json!({
            "serialization_error": error.to_string()
        })
    });
    redact_diagnostic_strings(&mut detail);
    let record = DiagnosticRecord {
        timestamp_ms: now_ms(),
        pid: std::process::id(),
        event: event.to_string(),
        detail,
    };
    let line = serde_json::to_string(&record).unwrap_or_else(|error| {
        json!({
            "timestamp_ms": now_ms(),
            "pid": std::process::id(),
            "event": "diagnostic_log.serialization_failed",
            "detail": {
                "message": error.to_string()
            }
        })
        .to_string()
    });

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn redact_diagnostic_strings(value: &mut Value) {
    match value {
        Value::String(text) => *text = "[REDACTED]".to_string(),
        Value::Array(values) => {
            for value in values {
                redact_diagnostic_strings(value);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                redact_diagnostic_strings(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

pub fn diagnostic_log_path() -> PathBuf {
    if let Some(lock) = TEST_LOG_PATH.get() {
        if let Ok(guard) = lock.lock() {
            if let Some(path) = &*guard {
                return path.clone();
            }
        }
    }
    crate::paths::default_diagnostic_log_path()
}

#[doc(hidden)]
pub fn set_diagnostic_log_path_for_tests(path: Option<PathBuf>) {
    let lock = TEST_LOG_PATH.get_or_init(|| Mutex::new(None));
    *lock.lock().expect("test log path lock poisoned") = path;
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_log_redacts_all_string_detail_values() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("diagnostic.log");
        set_diagnostic_log_path_for_tests(Some(path.clone()));

        append_diagnostic_log(
            "diagnostic_log.redaction_test",
            json!({
                "error": "TOML line: experimental_bearer_token = \"diag-sentinel-key\"",
                "baseUrl": "https://diag-sentinel-key@example.test",
                "nested": {
                    "freeform": "diag-sentinel-key"
                },
                "safeBoolean": true,
                "safeNumber": 42
            }),
        )
        .unwrap();
        set_diagnostic_log_path_for_tests(None);

        let contents = std::fs::read_to_string(path).unwrap();
        assert!(!contents.contains("diag-sentinel-key"));
        assert!(!contents.contains("experimental_bearer_token"));
        assert!(contents.contains("\"safeBoolean\":true"));
        assert!(contents.contains("\"safeNumber\":42"));
        assert!(contents.contains("[REDACTED]"));
    }
}
