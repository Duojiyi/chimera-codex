//! Step 9.3 — error classification, diagnostics, and log rotation.
//!
//! A diagnostic bundle is the one artifact this app builds specifically so it
//! can leave the machine. Everything here is arranged around that: fields are
//! redacted on the way in, the rendered form is redacted again, and the bundle
//! can state whether it is clean so the UI can refuse to send rather than hope.
//!
//! Redaction runs twice on purpose. The preview a user approves and the text
//! that would actually be sent must be identical, and the only way to know that
//! is to apply the same function to the finished artifact.

use crate::redact::{contains_secret, redact};
use std::fs;
use std::path::Path;

/// What kind of problem a message describes.
///
/// Coarse by design: the point is to route a user to the right next action
/// ("check your key" / "check your network" / "check your disk"), not to
/// enumerate causes. A taxonomy nobody can act on differently is noise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    Credentials,
    Network,
    Storage,
    Unknown,
}

/// Classify a user-facing message.
///
/// Takes the message, not the underlying error, because the error's text is
/// not a stable API and because a caller holding a raw error is exactly the
/// caller most likely to still have a credential attached to it. Callers pass
/// the redacted form; redaction preserves enough wording to classify.
pub fn classify(message: &str) -> ErrorClass {
    let m = message.to_ascii_lowercase();
    if m.contains("key") || m.contains("auth") || m.contains("credential") || m.contains("rejected")
    {
        return ErrorClass::Credentials;
    }
    if m.contains("reach") || m.contains("network") || m.contains("timed out") || m.contains("dns")
    {
        return ErrorClass::Network;
    }
    if m.contains("write") || m.contains("disk") || m.contains("space") || m.contains("folder") {
        return ErrorClass::Storage;
    }
    ErrorClass::Unknown
}

/// Everything a bundle is built from, before redaction.
#[derive(Debug, Clone)]
pub struct DiagnosticInput {
    pub app_version: String,
    pub os: String,
    pub last_error: Option<String>,
    pub recent_log_lines: Vec<String>,
    /// Host only, never a full provider URL with a path.
    pub provider_host: Option<String>,
    pub runtime_version: Option<String>,
}

/// A redacted diagnostic report.
///
/// Construction is the only way to get one, and construction redacts. There is
/// no way to build a bundle holding raw input.
#[derive(Debug, Clone)]
pub struct DiagnosticBundle {
    app_version: String,
    os: String,
    last_error: Option<String>,
    error_class: ErrorClass,
    recent_log_lines: Vec<String>,
    provider_host: Option<String>,
    runtime_version: Option<String>,
}

impl DiagnosticBundle {
    /// The text a user sees and would send.
    ///
    /// Redacted a second time over the assembled whole. Concatenation can
    /// create a pattern that was not present in any single field — a key split
    /// across a wrapped log line is the obvious case — and the first pass
    /// cannot see across fields it processed separately.
    pub fn render(&self) -> String {
        let dash = "—";
        let body = format!(
            "Chimera++ {}\nOS: {}\nCodex runtime: {}\nProvider host: {}\nLast error: {} [{:?}]\n\nRecent log:\n{}\n",
            self.app_version,
            self.os,
            self.runtime_version.as_deref().unwrap_or(dash),
            self.provider_host.as_deref().unwrap_or(dash),
            self.last_error.as_deref().unwrap_or(dash),
            self.error_class,
            self.recent_log_lines.join("\n"),
        );
        redact(&body)
    }

    /// Whether the rendered form is free of anything redaction recognises.
    ///
    /// Uses the same function as the redactor, so a disagreement between "was
    /// redacted" and "is clean" is impossible by construction.
    pub fn is_clean(&self) -> bool {
        !contains_secret(&self.render())
    }

    pub fn error_class(&self) -> ErrorClass {
        self.error_class
    }
}

/// Build a bundle, redacting every field on the way in.
pub fn build_bundle(input: &DiagnosticInput) -> DiagnosticBundle {
    let last_error = input.last_error.as_deref().map(redact);
    // Classified from the redacted text, so a caller that passed a raw error
    // cannot have its credential reach the classifier's matching either.
    let error_class = last_error
        .as_deref()
        .map(classify)
        .unwrap_or(ErrorClass::Unknown);

    DiagnosticBundle {
        app_version: redact(&input.app_version),
        os: redact(&input.os),
        last_error,
        error_class,
        recent_log_lines: input.recent_log_lines.iter().map(|l| redact(l)).collect(),
        provider_host: input.provider_host.as_deref().map(redact),
        runtime_version: input.runtime_version.as_deref().map(redact),
    }
}

// ── Log rotation ───────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum RotationError {
    #[error("could not read the log directory")]
    Read,
}

/// Trim a log directory to `max_files` and `max_total_bytes`, oldest first.
///
/// Never removes the last remaining file, even when it alone exceeds the
/// budget. Deleting the current log to make room destroys the evidence of
/// whatever is filling it, which is worse than a directory slightly over
/// budget — and the file is about to be reopened and appended to anyway.
///
/// An absent directory is a no-op: this runs at startup, before anything has
/// necessarily written a log.
pub fn rotate_logs(
    dir: &Path,
    max_files: usize,
    max_total_bytes: u64,
) -> Result<(), RotationError> {
    if !dir.is_dir() {
        return Ok(());
    }

    let mut files: Vec<(std::path::PathBuf, u64, std::time::SystemTime)> = fs::read_dir(dir)
        .map_err(|_| RotationError::Read)?
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            Some((e.path(), meta.len(), modified))
        })
        .collect();

    // Oldest first. Ties fall back to the name so the order is deterministic
    // on filesystems with coarse timestamps — otherwise which file survives
    // would vary between runs and be untestable.
    files.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(&b.0)));

    let mut total: u64 = files.iter().map(|f| f.1).sum();
    let mut count = files.len();

    for (path, size, _) in files.iter() {
        if count <= 1 {
            break;
        }
        if count <= max_files && total <= max_total_bytes {
            break;
        }
        // A file we cannot delete is not fatal: the next run tries again, and
        // reporting it would send the user to chase a log they cannot act on.
        if fs::remove_file(path).is_ok() {
            total = total.saturating_sub(*size);
            count -= 1;
        }
    }

    Ok(())
}
