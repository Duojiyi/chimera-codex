//! Step 2.2 — Provider URL validation and security checks.
//! Rules (Spec 7.2):
//! - HTTPS required (HTTP only on explicit loopback + dev mode)
//! - No userinfo, no fragment
//! - Origin-only URL → expose /v1 as a candidate, never silently persist
//! - Cross-origin redirect ban enforced at probe time

use thiserror::Error;
use url::Url;

/// Result of URL validation — does NOT yet contain a key.
#[derive(Debug, Clone)]
pub struct ValidatedUrl {
    /// The URL exactly as entered (or with scheme normalised to lowercase).
    pub base_url: Url,
    /// When the user supplied an origin-only URL, this holds the /v1 candidate.
    /// Caller must ask user to confirm before persisting.
    pub v1_candidate: Option<Url>,
    pub dev_mode: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UrlValidationError {
    #[error("URL parse error: {0}")]
    Parse(String),

    #[error("insecure scheme '{scheme}' — HTTPS required (use dev mode for loopback HTTP)")]
    InsecureScheme { scheme: String },

    #[error("URL must not contain userinfo (user:pass@host)")]
    ContainsUserinfo,

    #[error("URL must not contain a fragment (#...)")]
    ContainsFragment,

    #[error("URL is empty")]
    Empty,
}

/// Validate a provider base URL against Spec 7.2 security rules.
///
/// `dev_mode` allows `http://127.0.0.1` and `http://localhost` for local testing.
/// In production, any non-HTTPS URL is rejected.
pub fn validate_provider_url(
    raw: &str,
    dev_mode: bool,
) -> Result<ValidatedUrl, UrlValidationError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(UrlValidationError::Empty);
    }

    let url = Url::parse(raw).map_err(|e| UrlValidationError::Parse(e.to_string()))?;

    // Ban userinfo
    if !url.username().is_empty() || url.password().is_some() {
        return Err(UrlValidationError::ContainsUserinfo);
    }

    // Ban fragment
    if url.fragment().is_some() {
        return Err(UrlValidationError::ContainsFragment);
    }

    // Scheme check
    match url.scheme() {
        "https" => { /* always allowed */ }
        "http" => {
            let is_loopback = url.host_str()
                .map(|h| h == "127.0.0.1" || h == "localhost" || h == "::1")
                .unwrap_or(false);
            if !(dev_mode && is_loopback) {
                return Err(UrlValidationError::InsecureScheme {
                    scheme: "http".to_string(),
                });
            }
        }
        other => {
            return Err(UrlValidationError::InsecureScheme {
                scheme: other.to_string(),
            });
        }
    }

    // Detect origin-only (no path, or path is just "/")
    let path = url.path();
    let v1_candidate = if path == "/" || path.is_empty() {
        let mut candidate = url.clone();
        candidate.set_path("/v1");
        Some(candidate)
    } else {
        None
    };

    Ok(ValidatedUrl { base_url: url, v1_candidate, dev_mode })
}
