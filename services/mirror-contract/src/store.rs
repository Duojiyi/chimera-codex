//! Storage layer for the stable pointer — Spec 9.4.
//!
//! `cas.rs` decides *whether* a promotion is allowed. This module makes that
//! decision safe under concurrency: without a lock, two mirror workflows can
//! both read sequence N, both conclude N+1 is valid, and both write — so the
//! CAS check passes twice and one promotion is silently lost.
//!
//! The whole read-validate-write window is held under an exclusive file lock,
//! and the pointer is written via temp-file + atomic rename so a crash mid-write
//! can never leave a truncated pointer.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cas::{CasError, StablePointer, validate_stable_promotion};

const POINTER_FILE: &str = "stable-pointer.json";
const LOCK_FILE: &str = "promotion.lock";
const LOG_FILE: &str = "promotions.jsonl";

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("another promotion holds the lock")]
    Locked { holder_pid: Option<u32> },

    #[error(
        "promotion refused: sequence {attempted} does not supersede the stored sequence {stored}"
    )]
    SequenceConflict { stored: u64, attempted: u64 },

    #[error("stored pointer is corrupt: {0}")]
    PointerCorrupt(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(String),
}

/// One line of the append-only promotion audit log.
///
/// Only accepted promotions are appended. A refused promotion must not appear,
/// or the log would imply a state change that never happened.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionRecord {
    pub sequence: u64,
    pub codex_version: String,
    pub raw_digest: String,
    pub manifest_digest: String,
    pub promoted_at: String,
}

/// Guard proving the promotion lock is held. Releases on drop.
pub struct PromotionGuard {
    file: File,
}

impl Drop for PromotionGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// The on-disk stable pointer store for one mirror.
pub struct StableStore {
    root: PathBuf,
}

impl StableStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn pointer_path(&self) -> PathBuf {
        self.root.join(POINTER_FILE)
    }

    pub fn lock_path(&self) -> PathBuf {
        self.root.join(LOCK_FILE)
    }

    pub fn log_path(&self) -> PathBuf {
        self.root.join(LOG_FILE)
    }

    pub fn initialise(&self) -> Result<(), StoreError> {
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    /// Acquire the exclusive promotion lock.
    ///
    /// Exposed so tests can simulate a concurrent workflow holding it. The
    /// promote path takes the same lock, so the test exercises the real
    /// contention rather than a stand-in.
    pub fn lock_for_test(&self) -> Result<PromotionGuard, StoreError> {
        self.acquire()
    }

    fn acquire(&self) -> Result<PromotionGuard, StoreError> {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.lock_path())?;

        match file.try_lock_exclusive() {
            Ok(()) => {
                // Record the holder so a stuck lock is diagnosable.
                let _ = file.set_len(0);
                let _ = write!(file, "{{\"pid\":{}}}", std::process::id());
                let _ = file.flush();
                Ok(PromotionGuard { file })
            }
            Err(_) => Err(StoreError::Locked {
                holder_pid: read_holder_pid(&self.lock_path()),
            }),
        }
    }

    /// Read the stored pointer. `None` means no promotion has happened yet,
    /// which is a normal initial state rather than an error.
    pub fn read_pointer(&self) -> Result<Option<StablePointer>, StoreError> {
        let path = self.pointer_path();
        if !path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path)?;
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| StoreError::PointerCorrupt(e.to_string()))
    }

    /// Promote a new stable pointer under the exclusive lock.
    ///
    /// The lock spans read → validate → write, which is what makes the CAS
    /// check meaningful: two workflows cannot both observe the same sequence
    /// and both conclude their promotion is valid.
    pub fn promote(&self, next: &StablePointer) -> Result<(), StoreError> {
        let _guard = self.acquire()?;

        if let Some(current) = self.read_pointer()? {
            validate_stable_promotion(&current, next).map_err(|e| match e {
                CasError::SequenceConflict { actual, .. } => StoreError::SequenceConflict {
                    stored: actual,
                    attempted: next.sequence,
                },
                CasError::StalePromotion { current, new } => StoreError::SequenceConflict {
                    stored: current,
                    attempted: new,
                },
                CasError::DigestMismatch { .. } => StoreError::PointerCorrupt(e.to_string()),
            })?;
        }

        // Write the pointer first, then append the log. If a crash lands
        // between them the pointer is still valid and the promotion can be
        // re-run; the reverse order would claim a promotion that never landed.
        self.write_pointer_atomically(next)?;
        self.append_log(next)?;
        Ok(())
    }

    fn write_pointer_atomically(&self, p: &StablePointer) -> Result<(), StoreError> {
        let json = serde_json::to_string_pretty(p).map_err(|e| StoreError::Json(e.to_string()))?;
        let final_path = self.pointer_path();
        let tmp = final_path.with_extension("json.tmp");

        {
            let mut f = File::create(&tmp)?;
            f.write_all(json.as_bytes())?;
            // Durability before the rename: a rename that beats the data to
            // disk would expose an empty pointer after power loss.
            f.sync_all()?;
        }
        fs::rename(&tmp, &final_path)?;
        Ok(())
    }

    fn append_log(&self, p: &StablePointer) -> Result<(), StoreError> {
        let record = PromotionRecord {
            sequence: p.sequence,
            codex_version: p.codex_version.clone(),
            raw_digest: p.raw_digest.clone(),
            manifest_digest: p.manifest_digest.clone(),
            promoted_at: p.promoted_at.clone(),
        };
        let line = serde_json::to_string(&record).map_err(|e| StoreError::Json(e.to_string()))?;

        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())?;
        writeln!(f, "{line}")?;
        Ok(())
    }

    /// Read the append-only promotion log, oldest first.
    pub fn read_log(&self) -> Result<Vec<PromotionRecord>, StoreError> {
        let path = self.log_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&path)?;
        let mut out = Vec::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let rec: PromotionRecord = serde_json::from_str(line)
                .map_err(|e| StoreError::PointerCorrupt(e.to_string()))?;
            out.push(rec);
        }
        Ok(out)
    }
}

fn read_holder_pid(path: &Path) -> Option<u32> {
    let content = fs::read_to_string(path).ok()?;
    let i = content.find("\"pid\":")?;
    content[i + 6..]
        .split([',', '}'])
        .next()?
        .trim()
        .parse()
        .ok()
}
