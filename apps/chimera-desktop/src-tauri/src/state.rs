//! Managed application state.
//!
//! ADR-001 (single writer per domain): the provider DB handle and the runtime
//! layout live here behind a mutex so exactly one writer touches each domain.
//! Commands borrow the state; they never open their own connection.

use std::path::PathBuf;
use std::sync::Mutex;

use chimera_provider::db::ProviderDb;
use chimera_provider::keychain::OsKeychain;
use chimera_runtime::update::RuntimeLayout;
use chimera_update::bundled_root::{bundled_root, is_development_root};
use chimera_update::metadata::{Role, parse_root, verify_threshold};

/// Resolved on-disk locations for this installation.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Chimera's own data root (`%LOCALAPPDATA%/ChimeraPlusPlus` on Windows).
    pub data_root: PathBuf,
    /// Codex config home (`CODEX_HOME` or `~/.codex`).
    pub codex_home: PathBuf,
}

impl Paths {
    /// Resolve from the environment.
    ///
    /// `CHIMERA_DATA_ROOT` and `CODEX_HOME` override the defaults so tests and
    /// portable installs can redirect writes without touching real user data.
    pub fn resolve() -> Self {
        let data_root = std::env::var_os("CHIMERA_DATA_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let base = std::env::var_os("LOCALAPPDATA")
                    .or_else(|| std::env::var_os("HOME"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."));
                base.join("ChimeraPlusPlus")
            });

        let codex_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var_os("USERPROFILE")
                    .or_else(|| std::env::var_os("HOME"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."));
                home.join(".codex")
            });

        Self {
            data_root,
            codex_home,
        }
    }

    /// Provider database file.
    pub fn provider_db(&self) -> PathBuf {
        self.data_root.join("providers.sqlite")
    }

    /// Managed Codex runtime root.
    pub fn runtime_root(&self) -> PathBuf {
        self.data_root.join("runtime")
    }

    /// Live Codex `config.toml` that the provider projection writes.
    pub fn codex_config(&self) -> PathBuf {
        self.codex_home.join("config.toml")
    }

    /// Cross-process operation lock file.
    pub fn operation_lock(&self) -> PathBuf {
        self.data_root.join("operation.lock")
    }

    /// User preferences. Plain JSON: no secrets live here — provider keys are
    /// in the OS keychain and only referenced by handle (G4).
    pub fn settings(&self) -> PathBuf {
        self.data_root.join("settings.json")
    }

    /// Transaction journal for crash recovery.
    pub fn journal(&self) -> PathBuf {
        self.data_root.join("switch.journal")
    }
}

/// State handed to every command via `tauri::State`.
pub struct AppState {
    pub paths: Paths,
    /// Provider DB behind a mutex — single writer (ADR-001).
    pub db: Mutex<ProviderDb>,
    /// Real OS credential store. Held here so every command shares one
    /// backend and no command can silently fall back to an in-memory double.
    pub keychain: OsKeychain,
    pub runtime: RuntimeLayout,
    /// The live skin session, opened lazily.
    ///
    /// Behind a mutex because a CDP session is a single owned browser process:
    /// two commands driving it concurrently would interleave stylesheet
    /// commands on one socket.
    pub skins: Mutex<crate::skin_cmds::SkinRuntime>,
}

impl AppState {
    /// Open the DB and initialise the runtime layout.
    ///
    /// Returns an actionable message on failure; the caller surfaces it in the
    /// UI rather than panicking, so a corrupt DB does not brick startup.
    pub fn initialise() -> Result<Self, String> {
        let paths = Paths::resolve();

        // A fresh install has no cached update trust state yet. Validate the
        // compiled root before opening any user data so a development or
        // malformed trust anchor can never reach the release update path.
        let root_doc = bundled_root()
            .map_err(|_| "The Chimera update trust root is unavailable.".to_string())?;
        let root = parse_root(&root_doc.payload)
            .map_err(|_| "The Chimera update trust root is malformed.".to_string())?;
        if is_development_root(&root) {
            return Err("This build contains a development update trust root.".to_string());
        }
        let root_candidates = root.resolve_keys(Role::Root);
        verify_threshold(
            &root_doc.payload,
            &root_doc.signatures,
            &root_candidates,
            root.role(Role::Root).threshold,
            Role::Root.name(),
        )
        .map_err(|_| "The Chimera update trust root signature is invalid.".to_string())?;

        std::fs::create_dir_all(&paths.data_root)
            .map_err(|e| format!("Could not create data directory: {e}"))?;

        let db = ProviderDb::open(paths.provider_db())
            .map_err(|e| format!("Could not open the provider database: {e}"))?;

        let runtime = RuntimeLayout::new(paths.runtime_root());
        runtime
            .initialise()
            .map_err(|e| format!("Could not prepare the runtime directory: {e}"))?;

        // G6: an update interrupted by a crash or power loss must return to the
        // last known good version by itself. This is the only place that can
        // happen — the user cannot be asked to repair a runtime that will not
        // start. A failure here is reported rather than fatal: refusing to open
        // the app would strand them with no way to reach Diagnose or Rollback.
        if let Err(e) = chimera_runtime::update::recover_if_interrupted(&runtime) {
            eprintln!("runtime recovery could not complete: {e}");
        }

        Ok(Self {
            paths,
            db: Mutex::new(db),
            keychain: OsKeychain::new(),
            runtime,
            skins: Mutex::new(Default::default()),
        })
    }

    /// Persisted settings, or the shipped defaults.
    ///
    /// Deliberately infallible: the callers that need settings before the
    /// window exists — tray setup, initial visibility — have no way to report a
    /// read failure, and a missing or corrupt file is a recoverable state, not
    /// a reason to refuse to start.
    pub fn settings(&self) -> crate::dto::SettingsDto {
        let text = match std::fs::read_to_string(self.paths.settings()) {
            Ok(text) => text,
            Err(_) => return crate::dto::SettingsDto::default(),
        };

        // Settings are stored by chimera-update::AtomicStore as a versioned
        // envelope. Keep a plain JSON fallback for the one-time 1.x migration
        // and for a startup path that must remain available before Tauri
        // commands can surface a recovery message.
        serde_json::from_str::<crate::dto::SettingsDto>(&text)
            .ok()
            .or_else(|| {
                serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|value| value.get("data").cloned())
                    .and_then(|value| serde_json::from_value(value).ok())
            })
            .unwrap_or_default()
    }
}
