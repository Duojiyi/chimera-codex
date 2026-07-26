//! First-run bootstrap: preflight, then fetch the Codex payload.
//!
//! D6 (revised 2026-07-26) removed the payload from our package, so the app now
//! has a first run that does real work before it can be useful. The ordering
//! here is the contract:
//!
//!   1. preflight — WebView2, writable directories, free space
//!   2. only if all pass, download and verify against the signed manifest
//!   3. only if verification passes, commit into the managed runtime
//!
//! A preflight failure must leave the machine untouched (ADR-009). A download
//! failure must leave the runtime untouched (`chimera_runtime::download`
//! guarantees this). Between them, a user who cannot run Chimera is a user
//! whose machine is exactly as it was before they tried.

use serde::Serialize;
use tauri::State;

use chimera_platform::webview2;
use chimera_runtime::download::{
    HttpPayloadSource, PayloadSpec, Preflight, fetch_payload, preflight,
};
use chimera_runtime::update::RuntimeLayout;

use crate::state::AppState;

/// What the first-run screen needs to render.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightDto {
    /// True when every check passed and the download may start.
    pub ok: bool,
    /// i18n keys for the problems found, in the order to show them.
    ///
    /// Keys rather than sentences: the backend holds no translated text, and
    /// V17 cannot see strings that live in Rust.
    pub blocking_keys: Vec<String>,
    /// Present only when WebView2 is missing, so the UI can offer the link
    /// without hardcoding it in two places.
    pub webview2_download_url: Option<String>,
    /// Bytes free where the runtime will live, when the platform reports it.
    pub free_bytes: Option<u64>,
}

/// Free space on the volume holding `path`.
///
/// `fs2` rather than a per-platform call: it already ships in this workspace
/// and answers on both targets, which keeps ADR-007's "one entrypoint on
/// Windows and macOS" true here instead of adding a `#[cfg]` fork on day one.
///
/// Returns `None` rather than guessing when the filesystem does not answer.
/// `preflight` treats unknown space as acceptable — the write already fails
/// safely, and refusing to install because a disk could not be measured is
/// worse than letting it try.
fn free_bytes_at(path: &std::path::Path) -> Option<u64> {
    fs2::available_space(path).ok()
}

/// Run every check that must pass before the app touches anything.
///
/// `payload_bytes` comes from the signed manifest, so the space requirement is
/// the real one rather than a guess.
#[tauri::command]
pub fn run_preflight(
    state: State<'_, AppState>,
    payload_bytes: u64,
) -> Result<PreflightDto, String> {
    let mut blocking_keys = Vec::new();
    let mut webview2_download_url = None;

    let wv = webview2::check_installed();
    if !wv.is_present() {
        blocking_keys.push(wv.user_message_key().to_string());
        webview2_download_url = Some(webview2::download_url().to_string());
    }

    let runtime_root = state.paths.runtime_root();
    let free =
        free_bytes_at(&runtime_root).or_else(|| runtime_root.parent().and_then(free_bytes_at));

    match preflight(&state.runtime, payload_bytes, free) {
        Preflight::Ok => {}
        Preflight::InsufficientSpace { .. } => {
            blocking_keys.push("preflight.insufficientSpace".to_string());
        }
        Preflight::NotWritable { .. } => {
            // Deliberately no path in the key or the message: it contains the
            // account name and is not something the user can act on.
            blocking_keys.push("preflight.notWritable".to_string());
        }
    }

    Ok(PreflightDto {
        ok: blocking_keys.is_empty(),
        blocking_keys,
        webview2_download_url,
        free_bytes: free,
    })
}

/// Fetch the Codex payload described by a verified manifest entry.
///
/// The caller supplies the digest and size from the manifest rather than this
/// command reading them from the network: the whole point of D6's inversion is
/// that the bytes are checked against something signed, and a command that
/// fetched both the payload and its expectations from the same place would
/// verify nothing.
///
/// Returns the staged file's name on success. The caller commits it into the
/// managed runtime as a separate step, so a download that succeeded and an
/// install that failed remain distinguishable.
#[tauri::command]
pub async fn fetch_codex_payload(
    state: State<'_, AppState>,
    version: String,
    url: String,
    size_bytes: u64,
    sha256: String,
) -> Result<String, String> {
    // Only the root is captured: `State` cannot cross into the blocking pool,
    // and rebuilding the layout there costs nothing.
    let root = state.paths.runtime_root();

    // The download is blocking I/O. Running it on the async runtime's worker
    // would stall every other command for the length of a multi-megabyte
    // transfer — including the ones the progress UI needs.
    tauri::async_runtime::spawn_blocking(move || {
        let layout = RuntimeLayout::new(root);
        let spec = PayloadSpec {
            version,
            url,
            size_bytes,
            sha256,
        };
        fetch_payload(&layout, &spec, &HttpPayloadSource::new())
            // DownloadError's Display is already actionable and carries no URL
            // or raw io text; anything else here would undo that.
            .map_err(|e| e.to_string())
            .map(|path| {
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
    })
    .await
    .map_err(|_| "The download was interrupted. Nothing was installed; try again.".to_string())?
}

/// Compile-time proof that the DTO the frontend destructures is camelCase.
///
/// A snake_case field reads as `undefined` in the webview and the first-run
/// screen silently shows "everything is fine" on a machine that cannot run.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_dto_is_camel_case() {
        let json = serde_json::to_string(&PreflightDto {
            ok: false,
            blocking_keys: vec!["preflight.webview2Missing".into()],
            webview2_download_url: Some("https://example".into()),
            free_bytes: Some(1),
        })
        .unwrap();
        assert!(json.contains("blockingKeys"), "{json}");
        assert!(json.contains("webview2DownloadUrl"), "{json}");
        assert!(json.contains("freeBytes"), "{json}");
    }

    #[test]
    fn an_unused_payload_spec_still_compiles_against_the_runtime_contract() {
        // Keeps this module honest about the shape it will pass to
        // fetch_payload once the HTTP source exists: a field rename in
        // chimera-runtime should break here, not at integration time.
        let _ = PayloadSpec {
            version: "26.721".into(),
            url: "https://example/x".into(),
            size_bytes: 1,
            sha256: "0".repeat(64),
        };
    }
}
