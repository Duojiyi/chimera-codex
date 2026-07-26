//! Steps 5.3/5.4 — Runtime directory layout, current pointer, staging, commit, rollback.
//! Spec 8.2: versions/<v>/, staging/<tx>/, backup/<v>/, current.json, operation.lock

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("no previous version available for rollback")]
    NoPreviousVersion,
    #[error("version directory already exists: {0}")]
    AlreadyInstalled(PathBuf),
    #[error("current.json is missing or corrupt: {0}")]
    PointerCorrupt(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(String),
}

/// Active version pointer, written as `current.json` in the runtime root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePointer {
    pub active_version: String,
    pub source_manifest_digest: String,
    /// Previous version (used for rollback).
    pub previous_version: Option<String>,
}

/// Directory layout for the managed runtime.
pub struct RuntimeLayout {
    root: PathBuf,
}

impl RuntimeLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn staging_dir(&self) -> PathBuf {
        self.root.join("staging")
    }
    pub fn backup_dir(&self) -> PathBuf {
        self.root.join("backup")
    }
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir().join(version)
    }
    pub fn current_pointer_path(&self) -> PathBuf {
        self.root.join("current.json")
    }

    pub fn initialise(&self) -> Result<(), UpdateError> {
        fs::create_dir_all(self.versions_dir())?;
        fs::create_dir_all(self.staging_dir())?;
        fs::create_dir_all(self.backup_dir())?;
        Ok(())
    }

    pub fn read_current_pointer(&self) -> Result<Option<UpdatePointer>, UpdateError> {
        let p = self.current_pointer_path();
        if !p.exists() {
            return Ok(None);
        }
        let data = fs::read(&p)?;
        serde_json::from_slice(&data)
            .map_err(|e| UpdateError::PointerCorrupt(e.to_string()))
            .map(Some)
    }

    fn write_current_pointer(&self, pointer: &UpdatePointer) -> Result<(), UpdateError> {
        let json =
            serde_json::to_string_pretty(pointer).map_err(|e| UpdateError::Json(e.to_string()))?;
        let tmp = self.current_pointer_path().with_extension("json.tmp");
        let mut f = fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.flush()?;
        drop(f);
        fs::rename(tmp, self.current_pointer_path())?;
        Ok(())
    }
}

/// Create a staging directory for a new version.
/// Returns the staging directory path.
pub fn stage_version(layout: &RuntimeLayout, version: &str) -> Result<PathBuf, UpdateError> {
    let staged = layout.staging_dir().join(version);
    fs::create_dir_all(&staged)?;
    Ok(staged)
}

/// Commit a staged version: move staged → versions/<v>, update current.json.
pub fn commit_version(
    layout: &RuntimeLayout,
    version: &str,
    source_manifest_digest: &str,
) -> Result<UpdatePointer, UpdateError> {
    let staged = layout.staging_dir().join(version);
    let version_dir = layout.version_dir(version);

    if version_dir.exists() {
        // Remove old version dir first (for reinstall/repair)
        fs::remove_dir_all(&version_dir)?;
    }

    // Atomic rename: staging → versions/<v>
    fs::rename(&staged, &version_dir)?;

    let previous = layout
        .read_current_pointer()
        .ok()
        .flatten()
        .map(|p| p.active_version);

    let pointer = UpdatePointer {
        active_version: version.to_string(),
        source_manifest_digest: source_manifest_digest.to_string(),
        previous_version: previous,
    };
    layout.write_current_pointer(&pointer)?;
    Ok(pointer)
}

/// Roll back to the previous version.
pub fn rollback_to_last_known(layout: &RuntimeLayout) -> Result<UpdatePointer, UpdateError> {
    let current = layout
        .read_current_pointer()?
        .ok_or(UpdateError::NoPreviousVersion)?;

    let prev_version = current
        .previous_version
        .ok_or(UpdateError::NoPreviousVersion)?;

    // Verify the previous version directory still exists
    let prev_dir = layout.version_dir(&prev_version);
    if !prev_dir.exists() {
        return Err(UpdateError::NoPreviousVersion);
    }

    let pointer = UpdatePointer {
        active_version: prev_version.clone(),
        source_manifest_digest: String::new(),
        previous_version: None, // after rollback, no further rollback available
    };
    layout.write_current_pointer(&pointer)?;
    Ok(pointer)
}
