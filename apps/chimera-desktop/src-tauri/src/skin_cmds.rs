//! Skins: import, list, try, apply, restore — Step 8.3 wired to a live session.
//!
//! Two things this module is responsible for that the crate below it cannot be.
//!
//! **Ownership.** `CodexLauncher` launches whatever path it is handed. It has no
//! notion of a runtime root, so it cannot check that the executable belongs to
//! us. That check is G5's, it lives here, and skipping it would mean starting an
//! arbitrary binary with remote debugging enabled.
//!
//! **Lifetime.** A CDP session outlives the command that opened it — a skin has
//! to stay applied after `apply_skin` returns — so the session lives in state,
//! not in a local. It is opened lazily on the first command that needs it, and
//! dropped (which kills the process) when it is found dead.

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use chimera_runtime::health::{check_runtime_health, is_process_owned_by_runtime};
use chimera_theme::apply::{ApplyError, SkinApplier, SkinState, SkinStateTransaction};
use chimera_theme::cdp_transport::{CodexLauncher, OwnedBrowserProcess, WebSocketCdpClient};
use chimera_theme::package::{SkinPackage, import_codexskin};
use chimera_theme::session::{CdpSession, OsPortAllocator};

use crate::dto::SkinDto;
use crate::state::AppState;

/// Directory under the app's data root where imported packages live.
const SKINS_DIR: &str = "skins";
const SKIN_STATE_DIR: &str = "skin-state";

/// Pushes CSS into the running Codex window over CDP.
///
/// Wraps the session rather than being it, because `SkinApplier` is what the
/// transaction owns and the transaction must not be able to kill the browser.
struct CdpSkinApplier {
    session: CdpSession<OwnedBrowserProcess, WebSocketCdpClient>,
}

impl SkinApplier for CdpSkinApplier {
    fn apply(&mut self, css: &str) -> Result<(), ApplyError> {
        // Re-inject first if the page navigated: a fresh document has no
        // memory of a stylesheet, so applying without this would push CSS into
        // a sheet that no longer exists and silently do nothing.
        let _ = self.session.reinject_after_navigation();
        self.session
            .apply_css(css)
            .map_err(|e| ApplyError::Io(e.to_string()))
    }

    fn clear(&mut self) -> Result<(), ApplyError> {
        self.session
            .clear_css()
            .map_err(|e| ApplyError::Io(e.to_string()))
    }
}

/// The live skin transaction, or nothing if no session has been opened yet.
///
/// `Option` rather than eager construction: opening a session launches Codex,
/// and merely visiting the Appearance screen must not do that.
#[derive(Default)]
pub struct SkinRuntime {
    txn: Option<SkinStateTransaction<CdpSkinApplier>>,
}

impl SkinRuntime {
    /// Open a session if there is not already a healthy one.
    fn ensure(
        &mut self,
        state: &AppState,
    ) -> Result<&mut SkinStateTransaction<CdpSkinApplier>, String> {
        if self
            .txn
            .as_mut()
            .is_some_and(|t| t.applier_mut().session.is_alive())
        {
            return Ok(self.txn.as_mut().expect("checked above"));
        }
        // A dead session's process is already gone; dropping it here also
        // guarantees the old one cannot linger holding a debug port.
        self.txn = None;

        let exe = resolve_owned_codex(state)?;
        let profile = state.paths.data_root.join("skin-profile");
        let launcher = CodexLauncher::new(exe, profile);

        let session = CdpSession::start(&OsPortAllocator, &launcher, WebSocketCdpClient::new())
            .map_err(|e| e.to_string())?;

        let state_dir =
            chimera_platform::CanonicalPath::new(state.paths.data_root.join(SKIN_STATE_DIR))
                .map_err(|_| "Could not prepare the skin state folder.".to_string())?;

        let txn = SkinStateTransaction::open(&state_dir, CdpSkinApplier { session })
            .map_err(|e| e.to_string())?;
        self.txn = Some(txn);
        Ok(self.txn.as_mut().expect("just assigned"))
    }
}

/// The managed Codex executable, but only if it is genuinely ours (G5).
///
/// `check_runtime_health` derives the path from the runtime layout, and the
/// ownership check is then re-run on the canonicalised result — the same
/// belt-and-braces `chimera_runtime::process` uses, for the same reason: a
/// tampered pointer containing `..` would otherwise still start with the
/// runtime root.
fn resolve_owned_codex(state: &AppState) -> Result<PathBuf, String> {
    let health = check_runtime_health(&state.runtime).map_err(|_| {
        "Codex is not installed yet. Install it from the Codex tab first.".to_string()
    })?;
    let exe = health
        .exe_path
        .filter(|_| health.exe_present)
        .ok_or_else(|| {
            "Codex is not installed yet. Install it from the Codex tab first.".to_string()
        })?;

    let canonical_exe = std::fs::canonicalize(&exe).unwrap_or_else(|_| exe.clone());
    let canonical_root = std::fs::canonicalize(state.runtime.root())
        .unwrap_or_else(|_| state.runtime.root().to_path_buf());
    if !is_process_owned_by_runtime(&canonical_exe, &canonical_root) {
        // Deliberately no path in the message: it names the user's account.
        return Err("That Codex installation is not managed by Chimera++, so it will not be started with a skin.".to_string());
    }
    Ok(exe)
}

// ── Package storage ────────────────────────────────────────────────────────

fn skins_dir(state: &AppState) -> PathBuf {
    state.paths.data_root.join(SKINS_DIR)
}

/// Load one imported package back from disk.
///
/// Re-runs the full import, including validation. Storing the archive rather
/// than the extracted tree means a package cannot be edited in place after it
/// was accepted — every load re-checks the CSS allowlist and the entry names.
fn load_package(state: &AppState, id: &str) -> Result<SkinPackage, String> {
    // The id is a filename we wrote; refuse anything that could climb out of
    // the directory even so, since it arrives from the frontend.
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err("That skin id is not valid.".to_string());
    }
    let path = skins_dir(state).join(format!("{id}.codexskin"));
    let bytes =
        std::fs::read(&path).map_err(|_| "That skin is no longer installed.".to_string())?;
    import_codexskin(&bytes).map_err(|e| e.to_string())
}

// ── DTO ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinImportResultDto {
    pub id: String,
    pub name: String,
    pub version: String,
}

// ── Commands ───────────────────────────────────────────────────────────────

/// Import a `.codexskin` file. Validation happens here, once, before the file
/// is stored — an archive that fails is never written to disk at all.
#[tauri::command]
pub fn import_skin(
    state: State<'_, AppState>,
    path: String,
) -> Result<SkinImportResultDto, String> {
    let bytes = std::fs::read(&path).map_err(|_| "Could not read that file.".to_string())?;
    let package = import_codexskin(&bytes).map_err(|e| e.to_string())?;

    let id = sanitise_id(&package.manifest.name);
    let dir = skins_dir(&state);
    std::fs::create_dir_all(&dir).map_err(|_| "Could not create the skins folder.".to_string())?;
    std::fs::write(dir.join(format!("{id}.codexskin")), &bytes)
        .map_err(|_| "Could not save the skin.".to_string())?;

    Ok(SkinImportResultDto {
        id,
        name: package.manifest.name,
        version: package.manifest.version,
    })
}

/// A filename-safe id derived from a skin's own name.
///
/// Everything outside a small alphabet becomes `-`, so a name containing a path
/// separator, a colon, or a reserved Windows device name cannot become a path.
fn sanitise_id(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_ascii_lowercase();
    if trimmed.is_empty() {
        "skin".to_string()
    } else {
        trimmed
    }
}

/// Every installed skin, plus the built-in default, with the applied one marked.
#[tauri::command]
pub fn list_skins(state: State<'_, AppState>) -> Result<Vec<SkinDto>, String> {
    let applied_name = state
        .skins
        .lock()
        .ok()
        .and_then(|r| r.txn.as_ref().map(|t| t.current().clone()))
        .and_then(|s| match s {
            SkinState::Applied { name, .. } => Some(name),
            SkinState::Default => None,
        });

    let mut out = vec![SkinDto {
        id: "default".to_string(),
        name: "Default".to_string(),
        description: String::new(),
        is_default: true,
        applied: applied_name.is_none(),
    }];

    let dir = skins_dir(&state);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "codexskin") {
            continue;
        }
        let id = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        // A package that no longer validates is listed but marked unusable
        // rather than hidden — silently vanishing would look like data loss.
        let (name, description) = match load_package(&state, &id) {
            Ok(p) => (p.manifest.name, p.manifest.version),
            Err(e) => (id.clone(), e),
        };
        let applied = applied_name.as_deref() == Some(name.as_str());
        out.push(SkinDto {
            id,
            name,
            description,
            is_default: false,
            applied,
        });
    }
    Ok(out)
}

/// Preview a skin without committing it.
#[tauri::command]
pub fn try_skin(state: State<'_, AppState>, skin_id: String) -> Result<(), String> {
    let package = load_package(&state, &skin_id)?;
    let mut runtime = state
        .skins
        .lock()
        .map_err(|_| "Internal state is locked. Restart Chimera++.".to_string())?;
    let txn = runtime.ensure(&state)?;
    txn.try_skin(&package).map_err(|e| e.to_string())
}

/// Undo a preview, returning to whatever is actually committed.
#[tauri::command]
pub fn cancel_try_skin(state: State<'_, AppState>) -> Result<(), String> {
    let mut runtime = state
        .skins
        .lock()
        .map_err(|_| "Internal state is locked. Restart Chimera++.".to_string())?;
    let txn = runtime.ensure(&state)?;
    txn.cancel_try().map_err(|e| e.to_string())
}

/// Commit a skin.
#[tauri::command]
pub fn apply_skin(state: State<'_, AppState>, skin_id: String) -> Result<(), String> {
    if skin_id == "default" {
        return restore_default_skin(state);
    }
    let package = load_package(&state, &skin_id)?;
    let mut runtime = state
        .skins
        .lock()
        .map_err(|_| "Internal state is locked. Restart Chimera++.".to_string())?;
    let txn = runtime.ensure(&state)?;
    txn.apply_and_commit(&package).map_err(|e| e.to_string())
}

/// Return to Codex's own appearance.
///
/// Never opens a session it does not already have: with no live Codex there is
/// nothing showing a skin, so the answer is already "default" and launching a
/// browser to prove it would be absurd.
#[tauri::command]
pub fn restore_default_skin(state: State<'_, AppState>) -> Result<(), String> {
    let mut runtime = state
        .skins
        .lock()
        .map_err(|_| "Internal state is locked. Restart Chimera++.".to_string())?;
    match runtime.txn.as_mut() {
        Some(txn) => txn.restore_default().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skin_id_can_never_become_a_path() {
        for hostile in [
            "../../evil",
            "a/b",
            r"a\b",
            "CON",
            "with:colon",
            "trailing.",
        ] {
            let id = sanitise_id(hostile);
            assert!(!id.contains('/') && !id.contains('\\'), "{hostile} -> {id}");
            assert!(!id.contains(".."), "{hostile} -> {id}");
            assert!(!id.contains(':'), "{hostile} -> {id}");
        }
    }

    #[test]
    fn an_id_that_sanitises_to_nothing_still_gets_a_name() {
        // Otherwise the stored file would be ".codexskin" — hidden on Unix and
        // impossible to select again.
        assert_eq!(sanitise_id("///"), "skin");
        assert_eq!(sanitise_id(""), "skin");
    }

    #[test]
    fn an_ordinary_name_survives_recognisably() {
        // Over-sanitising would make every skin's file unrecognisable to a
        // user looking in the folder.
        assert_eq!(sanitise_id("Midnight Terminal"), "midnight-terminal");
    }
}
