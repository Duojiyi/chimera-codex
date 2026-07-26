//! Step 7.4 — read-only detection of other managers of the same Codex
//! install, plus the explicit, confirmation-gated operation to take
//! ownership of it anyway.
//!
//! Every fact this module needs (lock holder, ownership-manifest owner,
//! running processes) comes from a [`CoexistencePort`] the caller supplies —
//! nothing here ever opens a real lock file, parses a real manifest, or
//! lists real OS processes, so [`detect_coexistence`] is fully testable with
//! an in-memory double and never touches a real machine. The desktop shell
//! wires the real filesystem/process access — see this crate's final
//! report, "Integration needed".

use std::path::{Path, PathBuf};
use thiserror::Error;

/// One concrete piece of evidence that another manager currently owns (or is
/// using) this Codex install. Kept in a `Vec` on the verdict rather than
/// stopping at the first hit, so the UI can show the user everything found
/// instead of a single arbitrary reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictEvidence {
    /// Another process (not us) currently holds the operation lock.
    LockHeld {
        lock_path: PathBuf,
        holder_pid: Option<u32>,
    },
    /// The ownership manifest on disk names an owner that is not Chimera.
    OwnershipManifestNamesAnotherOwner {
        manifest_path: PathBuf,
        owner: String,
    },
    /// A running process looks like another manager, not us.
    RunningProcess { process_name: String, pid: u32 },
}

impl ConflictEvidence {
    /// i18n key for this specific piece of evidence, so the UI can render a
    /// detailed per-reason line under the general conflict banner. English
    /// and Chinese copy lives in the desktop shell's i18n resources — this
    /// crate defines only the stable key (see "Integration needed").
    pub fn i18n_key(&self) -> &'static str {
        match self {
            Self::LockHeld { .. } => "migration.coexistence.evidence.lock_held",
            Self::OwnershipManifestNamesAnotherOwner { .. } => {
                "migration.coexistence.evidence.ownership_manifest"
            }
            Self::RunningProcess { .. } => "migration.coexistence.evidence.running_process",
        }
    }
}

/// Read-only verdict of a coexistence check. Nothing in this module can
/// reach [`CoexistencePort::claim_ownership`] except [`take_ownership`], and
/// that only after an explicit confirmation — see that function's docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoexistenceVerdict {
    /// No other manager was detected.
    Clear,
    /// At least one other manager was detected.
    ConflictDetected {
        /// Best-effort human label for who else is involved — the ownership
        /// manifest's named owner if present, otherwise a description
        /// derived from whatever evidence was found. Never a raw path.
        who: String,
        evidence: Vec<ConflictEvidence>,
    },
}

impl CoexistenceVerdict {
    /// i18n key for the top-level banner. Per-evidence detail keys come from
    /// [`ConflictEvidence::i18n_key`].
    pub fn i18n_key(&self) -> &'static str {
        match self {
            Self::Clear => "migration.coexistence.clear",
            Self::ConflictDetected { .. } => "migration.coexistence.conflict_detected",
        }
    }
}

/// Who (if anyone) holds a lock, as reported by a [`CoexistencePort`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockHolder {
    /// `None` means "held, but the holder's pid could not be determined" —
    /// treated as a conflict (fail closed) rather than assumed to be us.
    pub pid: Option<u32>,
}

/// One running process a [`CoexistencePort`] believes might be a manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningProcess {
    pub pid: u32,
    pub process_name: String,
}

/// A probe-side failure — the check itself could not be completed, as
/// distinct from the check completing and finding a conflict. The wrapped
/// `String` is caller-supplied and, like `ports::PortError`, MUST NOT be a
/// raw Rust error, absolute path, username, or key material.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProbeError {
    #[error("could not check whether another program is using this Codex install: {0}")]
    Io(String),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoexistenceError {
    #[error("could not check for other Codex managers: {0}")]
    CheckFailed(String),
    #[error("could not record Chimera as the owner of this Codex install: {0}")]
    ClaimFailed(String),
}

impl From<ProbeError> for CoexistenceError {
    fn from(e: ProbeError) -> Self {
        Self::CheckFailed(e.to_string())
    }
}

/// Facts this module needs about the local machine, gathered by whatever
/// the desktop shell wires (a real lock file, a real ownership manifest, a
/// real process list). Every method is read-only except
/// `claim_ownership` — [`detect_coexistence`] never calls it; only
/// [`take_ownership`] does, and only after an explicit confirmation.
pub trait CoexistencePort {
    /// Who (if anyone) currently holds the operation lock at `lock_path`.
    /// `Ok(None)` means the lock is free.
    fn lock_holder(&self, lock_path: &Path) -> Result<Option<LockHolder>, ProbeError>;

    /// The owner named in the ownership manifest at `manifest_path`, if the
    /// manifest exists and is readable. Never parsed beyond that one field.
    fn ownership_owner(&self, manifest_path: &Path) -> Result<Option<String>, ProbeError>;

    /// Running processes that look like a Codex/Chimera manager, by name.
    /// A conforming implementation excludes the caller's own process, but
    /// `detect_coexistence` also filters by pid as defense in depth.
    fn running_manager_processes(&self) -> Result<Vec<RunningProcess>, ProbeError>;

    /// Write `owner_label` into the ownership manifest at `manifest_path` as
    /// the new owner. The only write this trait exposes — reached
    /// exclusively through [`take_ownership`], never through
    /// [`detect_coexistence`].
    fn claim_ownership(&self, manifest_path: &Path, owner_label: &str) -> Result<(), ProbeError>;
}

/// What [`detect_coexistence`] needs to know about *this* Chimera instance,
/// so it can tell "our own lock/manifest/process" apart from someone else's.
#[derive(Debug, Clone)]
pub struct CoexistenceCheckInput {
    pub lock_path: PathBuf,
    pub ownership_manifest_path: PathBuf,
    /// The owner label Chimera itself writes into the manifest (e.g.
    /// "Chimera++"). A manifest naming anything else is a conflict.
    pub expected_owner_label: String,
    pub current_pid: u32,
}

/// Detect whether another manager currently owns or is using the same Codex
/// install. Purely read-only: nothing here can reach `claim_ownership`.
pub fn detect_coexistence(
    input: &CoexistenceCheckInput,
    probe: &dyn CoexistencePort,
) -> Result<CoexistenceVerdict, CoexistenceError> {
    let mut evidence = Vec::new();

    if let Some(holder) = probe.lock_holder(&input.lock_path)? {
        // Fail closed: a lock held by an unidentifiable pid is treated as
        // someone else's, never silently assumed to be our own.
        if holder.pid != Some(input.current_pid) {
            evidence.push(ConflictEvidence::LockHeld {
                lock_path: input.lock_path.clone(),
                holder_pid: holder.pid,
            });
        }
    }

    let manifest_owner = probe.ownership_owner(&input.ownership_manifest_path)?;
    if let Some(owner) = &manifest_owner {
        if owner != &input.expected_owner_label {
            evidence.push(ConflictEvidence::OwnershipManifestNamesAnotherOwner {
                manifest_path: input.ownership_manifest_path.clone(),
                owner: owner.clone(),
            });
        }
    }

    for process in probe.running_manager_processes()? {
        // Defense in depth: even if a probe implementation forgets to
        // exclude ourselves, a process sharing our own pid is never
        // treated as "another" manager.
        if process.pid != input.current_pid {
            evidence.push(ConflictEvidence::RunningProcess {
                process_name: process.process_name.clone(),
                pid: process.pid,
            });
        }
    }

    if evidence.is_empty() {
        return Ok(CoexistenceVerdict::Clear);
    }

    let who = manifest_owner.unwrap_or_else(|| describe(&evidence[0]));
    Ok(CoexistenceVerdict::ConflictDetected { who, evidence })
}

/// Best-effort human label for one piece of evidence, used when no
/// ownership-manifest owner name is available to describe "who" more
/// specifically.
fn describe(evidence: &ConflictEvidence) -> String {
    match evidence {
        ConflictEvidence::RunningProcess { process_name, .. } => process_name.clone(),
        ConflictEvidence::LockHeld {
            holder_pid: Some(pid),
            ..
        } => format!("another process (pid {pid})"),
        _ => "an unidentified external manager".to_string(),
    }
}

/// Proof of explicit user confirmation to take ownership of an install
/// another manager currently claims.
///
/// Has no `Default` impl, and the private `()` field means the only way to
/// construct one outside this module is [`Self::user_explicitly_confirmed`]
/// — there is no code path that produces this value by inference or
/// fall-through. That is what makes "taking ownership by accident" a
/// compile-time impossibility rather than a runtime check a call site could
/// forget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnershipTakeoverConfirmation(());

impl OwnershipTakeoverConfirmation {
    /// The only way to produce a confirmed instance — named loudly so a
    /// call site reads as a deliberate decision, never an oversight.
    pub fn user_explicitly_confirmed() -> Self {
        Self(())
    }
}

/// Take ownership of the install described by `verdict`, recording
/// `owner_label` as the new owner. Requires an
/// [`OwnershipTakeoverConfirmation`], which only exists once a caller has
/// explicitly called [`OwnershipTakeoverConfirmation::user_explicitly_confirmed`].
pub fn take_ownership(
    verdict: &CoexistenceVerdict,
    _confirmation: OwnershipTakeoverConfirmation,
    manifest_path: &Path,
    owner_label: &str,
    probe: &dyn CoexistencePort,
) -> Result<(), CoexistenceError> {
    if matches!(verdict, CoexistenceVerdict::Clear) {
        // Nothing to take over: Chimera already owns (or nobody else
        // claims) this install. Writing a claim here would be a needless
        // manifest rewrite for no behavioural change, so this is a no-op,
        // not an error.
        return Ok(());
    }
    probe
        .claim_ownership(manifest_path, owner_label)
        .map_err(|e| CoexistenceError::ClaimFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i18n_keys_are_stable_and_distinct() {
        // A regression test, not a behavioural one: these strings are a
        // contract with the desktop shell's i18n resource files (a
        // forbidden file for this crate to touch), so an accidental rename
        // here must fail loudly instead of silently breaking UI wiring.
        let clear = CoexistenceVerdict::Clear.i18n_key();
        let conflict = CoexistenceVerdict::ConflictDetected {
            who: "x".into(),
            evidence: vec![],
        }
        .i18n_key();
        assert_eq!(clear, "migration.coexistence.clear");
        assert_eq!(conflict, "migration.coexistence.conflict_detected");
        assert_ne!(clear, conflict);

        let lock = ConflictEvidence::LockHeld {
            lock_path: PathBuf::from("x"),
            holder_pid: None,
        }
        .i18n_key();
        let manifest = ConflictEvidence::OwnershipManifestNamesAnotherOwner {
            manifest_path: PathBuf::from("x"),
            owner: "x".into(),
        }
        .i18n_key();
        let process = ConflictEvidence::RunningProcess {
            process_name: "x".into(),
            pid: 1,
        }
        .i18n_key();
        assert_eq!(lock, "migration.coexistence.evidence.lock_held");
        assert_eq!(
            manifest,
            "migration.coexistence.evidence.ownership_manifest"
        );
        assert_eq!(process, "migration.coexistence.evidence.running_process");
    }
}
