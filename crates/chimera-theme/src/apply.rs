//! try / apply / restore-default skin-state transaction — Step 8.3 (ADR-005,
//! T48).
//!
//! Two directories exist in this problem and this module only ever knows
//! about one of them: the caller-supplied `state_dir` (Chimera's own data
//! dir — never named, guessed, or defaulted here; see
//! [`SkinStateTransaction::open`]). **There is no parameter, field, or
//! function anywhere in this file that names the official Codex install
//! directory.** That is deliberate, not an oversight: a module that never
//! receives a path cannot write to it, so "no official file is ever
//! modified" holds by construction, not merely by care — see
//! `tests/step8_3_apply.rs`'s
//! `a_full_apply_try_cancel_restore_cycle_never_touches_an_unrelated_official_dir`
//! for the empirical version of the same claim (record every byte of an
//! unrelated directory before a full apply/try/cancel/restore cycle, assert
//! it is byte-identical after).
//!
//! The three operations and why each is safe even when something fails
//! partway through:
//!   - [`SkinStateTransaction::try_skin`] never touches disk or
//!     `self.current` at all — it only pushes CSS into the live
//!     [`SkinApplier`]. That is what makes
//!     [`SkinStateTransaction::cancel_try`] exactly reverse it: cancelling
//!     just re-drives the live session back to whatever was already the
//!     last *committed* state, which never moved.
//!   - [`SkinStateTransaction::apply_and_commit`] pushes CSS live **before**
//!     writing anything to disk. If the live push fails, nothing below it
//!     runs — the previously committed skin's files and `skin-state.json`
//!     are left completely untouched, which is what makes
//!     [`SkinStateTransaction::restore_default`] reliable "even after a
//!     failed apply": there is nothing inconsistent left behind to recover
//!     from in the first place.
//!   - [`SkinStateTransaction::restore_default`] unconditionally records
//!     [`SkinState::Default`] and clears the live session; it does not
//!     consult or depend on whatever the previous operation's outcome was.

use std::fs;
use std::path::PathBuf;

use chimera_platform::CanonicalPath;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::package::{ImportError, SkinPackage};
use crate::session::{BrowserProcess, CdpClient, CdpSession, SessionError};

const STATE_FILE: &str = "skin-state.json";
const CURRENT_DIR: &str = "current";
const STAGING_DIR: &str = "current.staging";

/// Why a skin-state transaction operation failed.
#[derive(Debug, Error)]
pub enum ApplyError {
    /// A [`SkinApplier::apply`]/[`SkinApplier::clear`] call against the live
    /// session failed. Never leaves `skin-state.json` or the committed
    /// skin's files touched — see the module docs.
    #[error("could not apply the skin to the live session: {0}")]
    Session(#[from] SessionError),
    #[error("could not read or write Chimera's own skin-state directory: {0}")]
    Io(String),
    #[error("skin-state.json is corrupted: {0}")]
    CorruptState(String),
    #[error("could not extract skin package into the skin-state directory: {0}")]
    Package(#[from] ImportError),
}

impl From<std::io::Error> for ApplyError {
    fn from(e: std::io::Error) -> Self {
        ApplyError::Io(e.to_string())
    }
}

/// Applies (or clears) a skin's CSS against the live, Chimera-owned browser
/// session. Production code implements this via [`CdpSession`] (see the
/// blanket impl below); tests use a fake that never touches a socket.
pub trait SkinApplier {
    /// Push `css` live, replacing whatever was previously showing.
    fn apply(&mut self, css: &str) -> Result<(), ApplyError>;
    /// Remove whatever CSS is currently showing, restoring Codex's own
    /// default appearance.
    fn clear(&mut self) -> Result<(), ApplyError>;
}

impl<P: BrowserProcess, C: CdpClient> SkinApplier for CdpSession<P, C> {
    fn apply(&mut self, css: &str) -> Result<(), ApplyError> {
        self.apply_css(css).map_err(ApplyError::from)
    }

    fn clear(&mut self) -> Result<(), ApplyError> {
        self.clear_css().map_err(ApplyError::from)
    }
}

/// Persisted skin-state — the entire contents of `skin-state.json`.
///
/// Deliberately does *not* carry the CSS text itself: the committed skin's
/// actual files live under `state_dir/current/` (written by
/// [`SkinPackage::write_to`]), and `entry_css` here is just the relative
/// path to the entry point within that directory — enough for
/// [`SkinStateTransaction::cancel_try`] to re-read the exact committed bytes
/// without re-parsing `theme.json` first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SkinState {
    /// No skin applied — Codex's own default appearance.
    Default,
    Applied {
        name: String,
        version: String,
        entry_css: String,
    },
}

/// A try/apply/restore transaction rooted at a single skin-state directory.
pub struct SkinStateTransaction<A: SkinApplier> {
    state_dir: PathBuf,
    applier: A,
    current: SkinState,
}

impl<A: SkinApplier> SkinStateTransaction<A> {
    /// Open (or initialise) a transaction rooted at `state_dir` — always a
    /// directory Chimera itself owns, supplied by the caller, and never the
    /// official Codex install (see module docs). [`CanonicalPath`] is the
    /// parameter type specifically so a relative or traversal-bearing path
    /// can never reach this far in the first place.
    ///
    /// A missing state file reads as [`SkinState::Default`] rather than an
    /// error: a machine that has never applied a skin is not in a
    /// corrupted state, it is in the ordinary starting one.
    pub fn open(state_dir: &CanonicalPath, applier: A) -> Result<Self, ApplyError> {
        let state_dir = state_dir.as_path().to_path_buf();
        fs::create_dir_all(&state_dir)?;

        let state_path = state_dir.join(STATE_FILE);
        let current = if state_path.exists() {
            let bytes = fs::read(&state_path)?;
            serde_json::from_slice(&bytes).map_err(|e| ApplyError::CorruptState(e.to_string()))?
        } else {
            SkinState::Default
        };

        Ok(Self {
            state_dir,
            applier,
            current,
        })
    }

    /// Mutable access to the applier.
    ///
    /// Narrow on purpose: the caller that owns the live CDP session needs to
    /// ask whether the browser is still running, and that question belongs to
    /// the session rather than to this transaction. The transaction
    /// deliberately cannot end the process itself — it pushes CSS and records
    /// state, and giving it the power to kill Codex would make `Drop` order
    /// something skin logic had to reason about.
    pub fn applier_mut(&mut self) -> &mut A {
        &mut self.applier
    }

    /// The last *committed* state — never reflects an in-progress
    /// [`Self::try_skin`], which is precisely the point (see module docs).
    pub fn current(&self) -> &SkinState {
        &self.current
    }

    /// Preview `package` on the live session without persisting anything.
    /// Neither `skin-state.json` nor `self.current` changes — the only
    /// effect is on whatever [`SkinApplier`] the transaction was opened
    /// with. Call [`Self::cancel_try`] to undo it, or [`Self::apply_and_commit`]
    /// to keep it.
    pub fn try_skin(&mut self, package: &SkinPackage) -> Result<(), ApplyError> {
        self.applier.apply(&package.entry_css)?;
        Ok(())
    }

    /// Undo an in-progress [`Self::try_skin`] by re-driving the live session
    /// back to whatever is actually committed: [`SkinState::Default`]
    /// clears it, [`SkinState::Applied`] re-reads that skin's own saved CSS
    /// from `state_dir/current/` and re-pushes it. Because `try_skin` never
    /// touched the committed state, this always lands back exactly where a
    /// caller who had never called `try_skin` at all would be.
    pub fn cancel_try(&mut self) -> Result<(), ApplyError> {
        match self.current.clone() {
            SkinState::Default => self.applier.clear().map_err(ApplyError::from),
            SkinState::Applied { entry_css, .. } => {
                let css_path = self.state_dir.join(CURRENT_DIR).join(&entry_css);
                let css = fs::read_to_string(&css_path)?;
                self.applier.apply(&css).map_err(ApplyError::from)
            }
        }
    }

    /// Stage to disk, push live, then publish — in that order.
    ///
    /// Two properties have to hold at once, and an adversarial review found
    /// that neither naive ordering gives both:
    ///
    /// - A failed apply must not destroy the previously committed skin. Writing
    ///   straight into `current/` breaks this.
    /// - The live session and `skin-state.json` must never disagree. Pushing
    ///   live *first* breaks this: a later disk failure left the browser showing
    ///   the failed package while the recorded state still named the previous
    ///   one, so restore-default would restore the wrong thing and the user
    ///   would be looking at a skin the app believed was not applied.
    ///
    /// Three phases satisfy both. Staging touches nothing the user can see, so
    /// a failure there is free. The live push comes next, while `current/` and
    /// the recorded state are still untouched — a failure there removes the
    /// staging directory and leaves everything exactly as it was. Only once the
    /// user can actually see the new skin does it become the committed one.
    ///
    /// The remaining window is publish-after-a-successful-live-push. It cannot
    /// be eliminated — two systems, one commit — so it is converged instead:
    /// both sides fall back to Default. Not what the user asked for, but a
    /// state where what they see and what is recorded agree, and one retry away
    /// from what they wanted. Disagreement is the outcome no retry can fix.
    pub fn apply_and_commit(&mut self, package: &SkinPackage) -> Result<(), ApplyError> {
        let dest = self.state_dir.join(CURRENT_DIR);
        let staging = self.state_dir.join(STAGING_DIR);
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }

        // Phase 1 — staging. A fresh sibling directory rather than `current/`
        // itself, so a crash mid-write cannot leave `current` half old and half
        // new for a later `cancel_try` to read from.
        package.write_to(&staging)?;

        // Phase 2 — live. Still nothing committed: on failure the staging
        // directory goes and the previous skin remains exactly as it was.
        if let Err(live_error) = self.applier.apply(&package.entry_css) {
            let _ = fs::remove_dir_all(&staging);
            return Err(ApplyError::from(live_error));
        }

        // Phase 3 — publish. The user can already see it; make it official.
        let published = (|| -> Result<(), ApplyError> {
            if dest.exists() {
                fs::remove_dir_all(&dest)?;
            }
            fs::rename(&staging, &dest)?;
            self.current = SkinState::Applied {
                name: package.manifest.name.clone(),
                version: package.manifest.version.clone(),
                entry_css: package.manifest.entry_css.clone(),
            };
            self.persist_state()
        })();

        if let Err(publish_error) = published {
            // Converge rather than leave the two disagreeing. Best-effort on
            // the way down: if clearing or persisting also fails there is
            // nothing more this call can do, and the publish error is the one
            // worth reporting.
            let _ = self.applier.clear();
            self.current = SkinState::Default;
            let _ = self.persist_state();
            let _ = fs::remove_dir_all(&staging);
            let _ = fs::remove_dir_all(&dest);
            return Err(publish_error);
        }

        Ok(())
    }

    /// Unconditionally return to Codex's own default appearance: clear the
    /// live session and record [`SkinState::Default`]. Does not consult, or
    /// depend on, whether the previous operation on this transaction
    /// succeeded — that is what makes it safe to call after a failed
    /// [`Self::apply_and_commit`].
    pub fn restore_default(&mut self) -> Result<(), ApplyError> {
        let clear_result = self.applier.clear();
        self.current = SkinState::Default;
        self.persist_state()?;
        // Best-effort: a leftover `current/` directory is stale data, not
        // an incorrect *state* (skin-state.json already says Default), so
        // a failure to remove it must not turn a successful restore into a
        // reported error.
        let _ = fs::remove_dir_all(self.state_dir.join(CURRENT_DIR));
        clear_result.map_err(ApplyError::from)
    }

    fn persist_state(&self) -> Result<(), ApplyError> {
        let bytes = serde_json::to_vec_pretty(&self.current)
            .map_err(|e| ApplyError::CorruptState(e.to_string()))?;
        fs::write(self.state_dir.join(STATE_FILE), bytes)?;
        Ok(())
    }
}
