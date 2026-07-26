//! Persisted trust state for the app-update chain, kept physically apart
//! from the Codex mirror's own cache (Step 9.1, G8/G15).
//!
//! Every one of the four TUF documents this crate trusts is written to its
//! own file under a single, fixed subdirectory name
//! ([`APP_TRUST_CACHE_DIRNAME`]) rather than directly at whatever base path a
//! caller supplies. That is deliberate: even if a future wiring mistake
//! passed this crate the very same base directory the Codex mirror's own
//! state lives under, the two would still land in sibling directories with
//! different names, never the same file. Cross-contamination between the two
//! trust domains cannot happen through a shared path if there is no path
//! this crate will ever accept unmodified.
//!
//! Reads fail closed: a missing file means "nothing persisted yet" (a normal
//! first run), but a file that exists and does not parse is
//! [`CacheError::Corrupt`], a different and louder outcome. Treating corrupt
//! the same as missing would let anything that can write to this directory —
//! a bug, a disk fault, a local attacker — force the client back to
//! bootstrapping from the bundled initial root on demand.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::metadata::SignedPayload;

/// The fixed subdirectory every `UpdateCache` writes under. Never derived
/// from configuration — see the module doc comment.
pub const APP_TRUST_CACHE_DIRNAME: &str = "chimera-app-trust";

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("cached trust state is corrupt: {0}")]
    Corrupt(String),
}

/// On-disk shape of one cached document. `SignedPayload` itself has no
/// schema version, so the cache wraps it in one — the same instinct as Step
/// 9.2's atomic stores, applied here to trust state rather than settings.
#[derive(Debug, Serialize, Deserialize)]
struct CachedDocument {
    schema_version: u32,
    payload: String,
    signatures: Vec<crate::metadata::MetaSignature>,
}

const CACHED_DOCUMENT_SCHEMA_VERSION: u32 = 1;

impl From<&SignedPayload> for CachedDocument {
    fn from(sp: &SignedPayload) -> Self {
        Self {
            schema_version: CACHED_DOCUMENT_SCHEMA_VERSION,
            payload: sp.payload.clone(),
            signatures: sp.signatures.clone(),
        }
    }
}

impl From<CachedDocument> for SignedPayload {
    fn from(cd: CachedDocument) -> Self {
        Self {
            payload: cd.payload,
            signatures: cd.signatures,
        }
    }
}

/// The on-disk trust-state cache for one installation of the app updater.
pub struct UpdateCache {
    dir: PathBuf,
}

impl UpdateCache {
    /// `base_dir` is wherever the caller's platform layer keeps app data —
    /// this type always joins its own fixed subdirectory onto it, so callers
    /// cannot accidentally point two different trust domains at the same
    /// physical directory.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: base_dir.into().join(APP_TRUST_CACHE_DIRNAME),
        }
    }

    pub fn initialise(&self) -> Result<(), CacheError> {
        fs::create_dir_all(&self.dir)?;
        Ok(())
    }

    /// Exposed for tests that need to assert on the namespacing property, or
    /// simulate a corrupt file by writing to it directly.
    pub fn root_dir_for_test(&self) -> &Path {
        &self.dir
    }
    pub fn root_path_for_test(&self) -> PathBuf {
        self.root_path()
    }

    fn root_path(&self) -> PathBuf {
        self.dir.join("root.json")
    }
    fn timestamp_path(&self) -> PathBuf {
        self.dir.join("timestamp.json")
    }
    fn snapshot_path(&self) -> PathBuf {
        self.dir.join("snapshot.json")
    }
    fn targets_path(&self) -> PathBuf {
        self.dir.join("targets.json")
    }

    fn read(path: &Path) -> Result<Option<SignedPayload>, CacheError> {
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(path)?;
        let doc: CachedDocument = serde_json::from_str(&text)
            .map_err(|e| CacheError::Corrupt(format!("{}: {e}", path.display())))?;
        Ok(Some(doc.into()))
    }

    fn write(path: &Path, value: &SignedPayload) -> Result<(), CacheError> {
        let doc = CachedDocument::from(value);
        let json =
            serde_json::to_string_pretty(&doc).map_err(|e| CacheError::Corrupt(e.to_string()))?;
        // Temp file + rename: a crash mid-write must never leave a truncated
        // trust document that `read` would then have to treat as corrupt.
        let tmp = path.with_extension("json.tmp");
        {
            let mut f = File::create(&tmp)?;
            f.write_all(json.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn read_root(&self) -> Result<Option<SignedPayload>, CacheError> {
        Self::read(&self.root_path())
    }
    pub fn write_root(&self, value: &SignedPayload) -> Result<(), CacheError> {
        Self::write(&self.root_path(), value)
    }

    pub fn read_timestamp(&self) -> Result<Option<SignedPayload>, CacheError> {
        Self::read(&self.timestamp_path())
    }
    pub fn write_timestamp(&self, value: &SignedPayload) -> Result<(), CacheError> {
        Self::write(&self.timestamp_path(), value)
    }

    pub fn read_snapshot(&self) -> Result<Option<SignedPayload>, CacheError> {
        Self::read(&self.snapshot_path())
    }
    pub fn write_snapshot(&self, value: &SignedPayload) -> Result<(), CacheError> {
        Self::write(&self.snapshot_path(), value)
    }

    pub fn read_targets(&self) -> Result<Option<SignedPayload>, CacheError> {
        Self::read(&self.targets_path())
    }
    pub fn write_targets(&self, value: &SignedPayload) -> Result<(), CacheError> {
        Self::write(&self.targets_path(), value)
    }
}
