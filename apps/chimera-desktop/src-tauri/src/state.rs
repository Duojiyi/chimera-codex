//! Managed application state.
//!
//! ADR-001 (single writer per domain): the provider DB handle and the runtime
//! layout live here behind a mutex so exactly one writer touches each domain.
//! Commands borrow the state; they never open their own connection.

use std::path::PathBuf;
use std::sync::Mutex;

use chimera_provider::db::ProviderDb;
use chimera_runtime::update::RuntimeLayout;

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
    pub runtime: RuntimeLayout,
}

impl AppState {
    /// Open the DB and initialise the runtime layout.
    ///
    /// Returns an actionable message on failure; the caller surfaces it in the
    /// UI rather than panicking, so a corrupt DB does not brick startup.
    pub fn initialise() -> Result<Self, String> {
        let paths = Paths::resolve();

        std::fs::create_dir_all(&paths.data_root)
            .map_err(|e| format!("Could not create data directory: {e}"))?;

        let db = ProviderDb::open(paths.provider_db())
            .map_err(|e| format!("Could not open the provider database: {e}"))?;

        let runtime = RuntimeLayout::new(paths.runtime_root());
        runtime
            .initialise()
            .map_err(|e| format!("Could not prepare the runtime directory: {e}"))?;

        Ok(Self {
            paths,
            db: Mutex::new(db),
            runtime,
        })
    }
}
