//! Step 2.5 — CAS switch transaction + write-ahead journal.
//! Spec 7.4 state machine:
//! acquire lock → snapshot+hash → render → journal → stage → CAS → atomic replace
//!             → verify → mark active → clear journal

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::keychain::{KeychainPort, SecretRef};
use crate::projection::{apply_provider_projection, ProviderProjection};
use chimera_platform::lock::{LockError, OperationLock};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TransactionOutcome {
    Committed,
    Conflict(CasConflict),
}

#[derive(Debug, Clone)]
pub struct CasConflict {
    pub snapshot_hash: String,
    pub current_hash: String,
}

/// Write-ahead journal entry, persisted to disk before any atomic rename.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub state: JournalState,
    pub config_path: PathBuf,
    pub staged_path: Option<PathBuf>,
    pub snapshot_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalState {
    Pending,
    Cleared,
}

impl JournalEntry {
    pub fn is_cleared(&self) -> bool {
        self.state == JournalState::Cleared
    }
}

#[derive(Debug, Error)]
pub enum TxError {
    #[error("lock error: {0}")]
    Lock(#[from] LockError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("projection error: {0}")]
    Projection(#[from] crate::projection::ProjectionError),
    #[error("keychain error: {0}")]
    Keychain(String),
    #[error("secret not found in keychain")]
    SecretMissing,
}

// ── Hashing ────────────────────────────────────────────────────────────────────

/// SHA-256 hex digest of file contents. Returns hex string.
pub fn snapshot_hash(path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

fn hash_str(s: &str) -> String {
    let digest = Sha256::digest(s.as_bytes());
    format!("{digest:x}")
}

// ── SwitchTransaction ─────────────────────────────────────────────────────────

pub struct SwitchTransaction {
    config_path: PathBuf,
    lock_path: PathBuf,
    journal_path: PathBuf,
}

impl SwitchTransaction {
    pub fn new(
        config_path: impl Into<PathBuf>,
        lock_path: impl Into<PathBuf>,
        journal_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            config_path: config_path.into(),
            lock_path: lock_path.into(),
            journal_path: journal_path.into(),
        }
    }

    /// Execute the full CAS transaction.
    pub fn execute(
        &self,
        projection: &ProviderProjection,
        kc: &dyn KeychainPort,
        secret_ref: &SecretRef,
    ) -> Result<TransactionOutcome, TxError> {
        self.execute_with_pre_cas_hook(projection, kc, secret_ref, Box::new(|| {}))
    }

    /// Execute with a hook that runs between snapshot and CAS check (for testing).
    pub fn execute_with_pre_cas_hook(
        &self,
        projection: &ProviderProjection,
        kc: &dyn KeychainPort,
        secret_ref: &SecretRef,
        pre_cas_hook: Box<dyn FnOnce()>,
    ) -> Result<TransactionOutcome, TxError> {
        // 1. Acquire cross-process operation lock
        let lock = OperationLock::new(&self.lock_path);
        let _guard = lock.try_acquire("switch_provider")?;

        // 2. Read current config; snapshot content hash
        let existing_config = if self.config_path.exists() {
            fs::read_to_string(&self.config_path)?
        } else {
            String::new()
        };
        let snapshot_hash = hash_str(&existing_config);

        // 3. Retrieve secret from keychain
        let secret = kc.retrieve(secret_ref)
            .map_err(|e| TxError::Keychain(e.to_string()))?
            .ok_or(TxError::SecretMissing)?;

        // 4. Render candidate config (unknown fields preserved by projection)
        let mut proj_with_key = projection.clone();
        proj_with_key.api_key_env_or_plain = secret;
        let candidate_config = apply_provider_projection(&existing_config, &proj_with_key)?;

        // 5. Write journal (before any file mutation)
        let staged_path = self.config_path.with_extension("toml.staged");
        let journal = JournalEntry {
            state: JournalState::Pending,
            config_path: self.config_path.clone(),
            staged_path: Some(staged_path.clone()),
            snapshot_hash: snapshot_hash.clone(),
        };
        self.write_journal(&journal)?;

        // 6. Write staged file
        fs::write(&staged_path, &candidate_config)?;

        // Pre-CAS hook (test injection point to simulate external write)
        pre_cas_hook();

        // 7. CAS check: re-read live config, compare hash to snapshot
        let live_config = if self.config_path.exists() {
            fs::read_to_string(&self.config_path)?
        } else {
            String::new()
        };
        let live_hash = hash_str(&live_config);

        if live_hash != snapshot_hash {
            // External write detected — enter conflict state, do NOT overwrite
            let _ = fs::remove_file(&staged_path);
            return Ok(TransactionOutcome::Conflict(CasConflict {
                snapshot_hash,
                current_hash: live_hash,
            }));
        }

        // 8. Atomic rename staged → config
        fs::rename(&staged_path, &self.config_path)?;

        // 9. Clear journal
        self.clear_journal(&journal)?;

        Ok(TransactionOutcome::Committed)
    }

    fn write_journal(&self, entry: &JournalEntry) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(entry).unwrap();
        let tmp = self.journal_path.with_extension("journal.tmp");
        let mut f = fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.flush()?;
        drop(f);
        fs::rename(tmp, &self.journal_path)?;
        Ok(())
    }

    fn clear_journal(&self, entry: &JournalEntry) -> std::io::Result<()> {
        let cleared = JournalEntry {
            state: JournalState::Cleared,
            ..entry.clone()
        };
        let json = serde_json::to_string_pretty(&cleared).unwrap();
        fs::write(&self.journal_path, json)?;
        Ok(())
    }
}
