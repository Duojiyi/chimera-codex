//! Step 2.2 — Provider URL validation and security checks.
//! Rules (Spec 7.2):
//! - HTTPS required (HTTP only on explicit loopback + dev mode)
//! - No userinfo, no fragment
//! - Origin-only URL → expose /v1 as a candidate, never silently persist
//! - Cross-origin redirect ban enforced at probe time

use chimera_domain::ProviderHealth;
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
            let is_loopback = url
                .host_str()
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

    Ok(ValidatedUrl {
        base_url: url,
        v1_candidate,
        dev_mode,
    })
}

// ── Real endpoint probe (Step 2.7) ───────────────────────────────────────────
// Spec 7.2 / G2: a provider must be *verified* before it is activated. URL
// validation alone proves nothing about whether the endpoint answers, so the
// add flow calls this before any row is written.
//
// The classification functions are deliberately separated from the network call
// so the status/error mapping is unit-testable without a live server.

/// What a probe concluded, in terms the UI can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutcome {
    pub ok: bool,
    pub health: ProviderHealth,
    /// Already-actionable text. Never a raw reqwest/HTTP error string.
    pub message: String,
    /// Models the endpoint reported, when it lists any.
    pub discovered_models: Vec<String>,
}

/// Map an HTTP status to a health verdict.
///
/// The distinction that matters: 401/403 means we reached the right server and
/// the key is wrong (actionable by the user), whereas 5xx means the server is
/// broken (not the user's fault). Conflating them sends people to re-check a
/// key that was fine.
pub fn classify_probe_status(status: u16, models: Option<&[String]>) -> ProbeOutcome {
    // Only a success carries a model list; an error body is never trusted as one.
    let listed = |ok: bool| {
        if ok {
            models.unwrap_or(&[]).to_vec()
        } else {
            Vec::new()
        }
    };
    match status {
        200..=299 => ProbeOutcome {
            ok: true,
            health: ProviderHealth::Healthy,
            message: "Connected successfully.".to_string(),
            discovered_models: listed(true),
        },
        401 | 403 => ProbeOutcome {
            ok: false,
            health: ProviderHealth::AuthFailed,
            message: "The API key was rejected. Check that it is correct and still active."
                .to_string(),
            discovered_models: Vec::new(),
        },
        404 => ProbeOutcome {
            ok: false,
            health: ProviderHealth::Incompatible,
            message: "The endpoint answered but has no models list. Check the URL includes the correct path, for example /v1."
                .to_string(),
            discovered_models: Vec::new(),
        },
        // The probe stopped at a redirect instead of following it, which only
        // happens when the destination left the origin the user approved.
        // Saying "responded in a way this type does not support" would send
        // them to debug a compatibility problem they do not have.
        300..=399 => ProbeOutcome {
            ok: false,
            health: ProviderHealth::Incompatible,
            message:
                "The endpoint redirected to a different host. For your key's safety the check stopped there. Enter the final URL directly."
                    .to_string(),
            discovered_models: Vec::new(),
        },
        429 => ProbeOutcome {
            ok: false,
            health: ProviderHealth::Unreachable,
            message: "The provider is rate limiting this key. Wait a moment and test again."
                .to_string(),
            discovered_models: Vec::new(),
        },
        500..=599 => ProbeOutcome {
            ok: false,
            health: ProviderHealth::Unreachable,
            message: "The provider reported a server error. This is on their side; try again later."
                .to_string(),
            discovered_models: Vec::new(),
        },
        _ => ProbeOutcome {
            ok: false,
            health: ProviderHealth::Incompatible,
            message: "The endpoint responded in a way this provider type does not support."
                .to_string(),
            discovered_models: Vec::new(),
        },
    }
}

/// Classify a transport-level failure (DNS, TLS, timeout, refused).
///
/// Takes the already-stringified error rather than `reqwest::Error` so the
/// mapping is testable and so no raw error can leak into the message.
/// Classify a transport failure from typed flags rather than error text.
///
/// Deliberately takes booleans instead of the error string: reqwest's message
/// wording is not a stable API, so sniffing it would silently regress on a
/// dependency bump. The caller derives these from reqwest's typed predicates.
pub fn classify_transport_error(is_timeout: bool, is_tls: bool) -> ProbeOutcome {
    let message = if is_timeout {
        "The endpoint did not respond in time. Check the URL and your network."
    } else if is_tls {
        "The endpoint's TLS certificate could not be verified."
    } else {
        "Could not reach the endpoint. Check the URL and your network."
    };
    ProbeOutcome {
        ok: false,
        health: ProviderHealth::Unreachable,
        message: message.to_string(),
        discovered_models: Vec::new(),
    }
}

/// Derive the flags from a reqwest error, then classify.
///
/// TLS has no typed predicate in reqwest, so it is the one case that inspects
/// the source chain — kept here so the pure classifier stays text-free.
pub fn classify_reqwest_error(e: &reqwest::Error) -> ProbeOutcome {
    let mut is_tls = false;
    let mut source: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(e);
    while let Some(s) = source {
        let text = s.to_string().to_lowercase();
        if text.contains("certificate") || text.contains("tls") || text.contains("ssl") {
            is_tls = true;
            break;
        }
        source = std::error::Error::source(s);
    }
    classify_transport_error(e.is_timeout(), is_tls)
}

/// Probe timeout. Short enough that a wrong URL fails fast in the UI, long
/// enough that a slow-but-working endpoint is not misreported as unreachable.
const PROBE_TIMEOUT_SECS: u64 = 10;

/// How many same-origin redirects to follow before giving up.
const MAX_REDIRECTS: usize = 5;

/// May a redirect from `from` to `to` be followed?
///
/// True only when both URLs share an origin — scheme, host and port. The probe
/// carries the API key in an `Authorization` header, so following a redirect
/// off-origin would deliver the user's credential to a host they never
/// approved. Relying on the HTTP client to strip the header instead would make
/// key safety a property of a transitive dependency's defaults; here it is a
/// property of our own code, with tests.
///
/// The rule itself lives in `chimera-domain` because the payload downloader in
/// `chimera-runtime` needs the same one, and adapter crates may not depend on
/// each other. Kept as a named function here so this module's tests still
/// describe the redirect decision rather than a generic URL comparison.
///
/// Fails closed: anything that cannot be parsed is refused.
pub fn redirect_verdict(from: &str, to: &str) -> bool {
    chimera_domain::same_origin(from, to)
}

/// Probe a provider endpoint with the given key.
///
/// Hits the models list, which every OpenAI-compatible endpoint exposes and
/// which is side-effect free — probing must never create or bill anything.
/// The key is sent in the Authorization header and is never logged.
pub async fn probe_provider(base_url: &str, api_key: &str) -> ProbeOutcome {
    let url = format!("{}/models", base_url.trim_end_matches('/'));

    // Enforce the cross-origin redirect ban here, not by trusting the client's
    // default header handling. `redirect_verdict` is the single rule and it is
    // unit-tested; this closure only feeds it reqwest's view of the hop.
    let redirect_policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.error("too many redirects");
        }
        let from = attempt
            .previous()
            .last()
            .map(|u| u.to_string())
            .unwrap_or_default();
        if redirect_verdict(&from, attempt.url().as_str()) {
            attempt.follow()
        } else {
            attempt.stop()
        }
    });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(PROBE_TIMEOUT_SECS))
        .redirect(redirect_policy)
        .build()
    {
        Ok(c) => c,
        Err(e) => return classify_reqwest_error(&e),
    };

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await;

    match response {
        Err(e) => classify_reqwest_error(&e),
        Ok(resp) => {
            let status = resp.status().as_u16();
            // Only read the body on success: an error body may be large, HTML,
            // or contain provider-side detail we do not want to surface.
            let models = if (200..=299).contains(&status) {
                resp.json::<serde_json::Value>()
                    .await
                    .map(|body| extract_model_ids(&body))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            classify_probe_status(status, Some(&models))
        }
    }
}

/// Pull model ids out of an OpenAI-shaped `{ "data": [ { "id": ... } ] }` body.
/// A body in another shape yields an empty list rather than an error: model
/// discovery is a convenience, not a correctness requirement.
pub fn extract_model_ids(body: &serde_json::Value) -> Vec<String> {
    body.get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|i| i.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
