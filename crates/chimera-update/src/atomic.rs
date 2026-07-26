//! Step 9.2 — crash-safe writes for settings / ownership / transaction state.
//!
//! Three properties, and every one of them is tested:
//!
//! - **Atomic.** The primary file is only ever replaced by a rename of a
//!   fully-written, fsync'd temp file. A crash at any point before the rename
//!   leaves the old primary untouched; a crash after leaves the new one
//!   intact. There is no window in which the primary is half-written, because
//!   the rename is the only operation that touches it.
//! - **Recoverable.** Exactly one `.bak` generation is kept — the primary as
//!   it stood immediately before the write that just happened. This is not
//!   for crashes in this process (the rename already handles those); it is
//!   for corruption this process did not cause — bit rot, a hand-edit, a
//!   different program writing to the same path by mistake. Fail-closed means
//!   a corrupt primary is treated as loudly as it deserves, but not so loudly
//!   that a single bad byte throws away otherwise-good state when a known-good
//!   copy is sitting right next to it.
//! - **Migratable.** Every stored document is tagged with the schema version
//!   that wrote it. An older tag is upgraded in place on read — see
//!   [`Migratable::upgrade`]. A *newer* tag is refused outright: it means a
//!   newer Chimera wrote this file, and guessing how to strip its fields back
//!   down to what this binary understands would silently lose data belonging
//!   to a version this binary has never seen.
//!
//! Modelled on [`crate::cache::UpdateCache`]'s temp-file-then-rename writes
//! and on `chimera-runtime::update`'s `write_current_pointer`/transaction
//! journal, generalised here to arbitrary documents with a schema version
//! instead of one hardcoded pointer shape. `chimera-domain`'s
//! `InstallOwnership`, `TransactionState` and `UpdateState` are the intended
//! first callers of [`AtomicStore`] — see this crate's report for what wiring
//! that still needs (a `Migratable` impl lives with the type it upgrades, in
//! `chimera-domain`, not here).

use std::fs::{self, File};
use std::io::Write;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Everything that can go wrong persisting or loading an atomic document.
///
/// Deliberately free of any dynamic error text, path, or field value: this
/// crate's diagnostics path (`crate::diagnostics`) exists precisely because
/// raw errors are exactly the kind of thing that must never reach a user-
/// facing surface unredacted, and the simplest way to guarantee that for
/// *this* type is to never give it anything to leak in the first place.
#[derive(Debug, Error)]
pub enum AtomicError {
    /// A filesystem operation failed — permissions, a missing parent
    /// directory, a full disk. The underlying `io::Error`'s `Display` is safe
    /// to show (it never contains data from the document itself), but never
    /// its `Debug`-formatted path, which callers must not surface directly.
    #[error("could not access the saved data on disk")]
    Io(#[from] std::io::Error),

    /// The document could not be serialised. Only reachable if a caller's
    /// `Serialize` impl fails structurally (e.g. a non-string map key); no
    /// document shape this module ships with can trigger it, but a fallible
    /// step still gets a typed outcome rather than an assumption that it
    /// cannot fail.
    #[error("could not encode the data to save it")]
    Encode,

    /// Primary and backup were both unreadable, malformed, or failed
    /// migration. The only fail-closed outcome: never fall back to a default
    /// value, and never panic.
    #[error("the saved data is corrupt and no valid backup could be recovered")]
    Corrupt,

    /// The stored schema version is newer than this binary understands.
    /// Refused rather than guessed at — see the module doc comment.
    #[error(
        "this file was saved by a newer version of Chimera (schema {found}); this version only supports up to schema {max_supported}"
    )]
    FutureSchema { found: u32, max_supported: u32 },
}

/// A document format that knows its own on-disk schema and how to bring an
/// older copy of itself up to date.
///
/// This module owns the write/read/backup/version-gate machinery; a concrete
/// document (settings, ownership, transaction state) supplies only this.
pub trait Migratable: Serialize + DeserializeOwned {
    /// The schema version this binary writes, and the newest one it can read.
    const CURRENT_VERSION: u32;

    /// Reshape a JSON value written by `from_version` into the current shape.
    ///
    /// `from_version` is always strictly less than `CURRENT_VERSION` — the
    /// equal-version case is parsed directly by [`AtomicStore::read`] without
    /// calling this at all, and a greater version never reaches here because
    /// it is refused before any parsing is attempted. An implementation only
    /// has to bridge the single step it is actually handed; one covering more
    /// than one historical version can loop internally between steps.
    ///
    /// Returns `None` on any value this cannot upgrade — treated the same as
    /// a corrupt document, i.e. eligible for `.bak` fallback, never a panic.
    fn upgrade(from_version: u32, value: Value) -> Option<Self>;
}

/// On-disk envelope: every document is tagged with the schema version that
/// wrote it, independent of whatever fields `data` happens to hold. Kept as a
/// raw [`Value`] rather than `T` directly so an older or newer document still
/// parses as *an* envelope — only the `data` payload's shape is
/// version-dependent, never this wrapper's.
#[derive(Debug, Serialize, Deserialize)]
struct Envelope {
    schema_version: u32,
    data: Value,
}

/// Outcome of parsing one file's text into `T`, before any `.bak` fallback
/// decision is made. Kept internal: callers only ever see [`AtomicError`].
enum Parsed<T> {
    Ok(T),
    /// A schema version this binary has never written. Distinguished from
    /// `Bad` because it must never trigger a `.bak` fallback — that could
    /// silently serve an *older* version's data in place of one only a newer
    /// Chimera has ever seen, which is the exact downgrade this type refuses.
    Future {
        found: u32,
        max_supported: u32,
    },
    /// Malformed JSON, an envelope whose `data` does not match `T`, or a
    /// migration that declined to produce a value. All three collapse to the
    /// same outcome: this copy cannot be trusted, try the other one.
    Bad,
}

fn parse_versioned<T: Migratable>(text: &str) -> Parsed<T> {
    let Ok(envelope) = serde_json::from_str::<Envelope>(text) else {
        return Parsed::Bad;
    };
    if envelope.schema_version > T::CURRENT_VERSION {
        return Parsed::Future {
            found: envelope.schema_version,
            max_supported: T::CURRENT_VERSION,
        };
    }
    if envelope.schema_version == T::CURRENT_VERSION {
        return match serde_json::from_value(envelope.data) {
            Ok(v) => Parsed::Ok(v),
            Err(_) => Parsed::Bad,
        };
    }
    match T::upgrade(envelope.schema_version, envelope.data) {
        Some(v) => Parsed::Ok(v),
        None => Parsed::Bad,
    }
}

/// Read a whole file as text. A missing file is `Ok(None)` — normal on a
/// first run — but any other I/O failure (permissions, a directory in the
/// way) is reported, not silently treated as absence.
fn read_text(path: &Path) -> Result<Option<String>, AtomicError> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AtomicError::Io(e)),
    }
}

/// A crash-safe, schema-versioned, single-document store at one path.
///
/// `path` is expected to end in `.json`; the temp and backup siblings are
/// derived from it with [`Path::with_extension`], the same convention
/// [`crate::cache::UpdateCache`] and `chimera-runtime::update` use, so all
/// three land next to each other and are recognisable at a glance.
pub struct AtomicStore<T> {
    path: PathBuf,
    _document: PhantomData<T>,
}

impl<T: Migratable> AtomicStore<T> {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            _document: PhantomData,
        }
    }

    fn tmp_path(&self) -> PathBuf {
        self.path.with_extension("json.tmp")
    }

    fn bak_path(&self) -> PathBuf {
        self.path.with_extension("json.bak")
    }

    /// Read the current document, or `None` if nothing has been saved yet.
    ///
    /// A corrupt primary transparently falls back to the single `.bak`
    /// generation; both failing is [`AtomicError::Corrupt`], never a default
    /// value manufactured out of thin air.
    pub fn read(&self) -> Result<Option<T>, AtomicError> {
        let Some(primary) = read_text(&self.path)? else {
            return Ok(None);
        };

        match parse_versioned::<T>(&primary) {
            Parsed::Ok(v) => Ok(Some(v)),
            Parsed::Future {
                found,
                max_supported,
            } => Err(AtomicError::FutureSchema {
                found,
                max_supported,
            }),
            Parsed::Bad => match read_text(&self.bak_path())? {
                Some(bak) => match parse_versioned::<T>(&bak) {
                    Parsed::Ok(v) => Ok(Some(v)),
                    _ => Err(AtomicError::Corrupt),
                },
                None => Err(AtomicError::Corrupt),
            },
        }
    }

    /// Persist `value`, tagged with `T::CURRENT_VERSION`.
    ///
    /// Order matters: the existing primary (if any) is copied to `.bak`
    /// *before* the new content is written, and only the final step is a
    /// rename. A crash before the rename leaves the old primary and the just-
    /// refreshed `.bak` both intact (redundant, but never wrong); a crash
    /// after leaves the new primary and the previous generation's `.bak`
    /// intact. There is no ordering of steps that loses both.
    pub fn write(&self, value: &T) -> Result<(), AtomicError> {
        let data = serde_json::to_value(value).map_err(|_| AtomicError::Encode)?;
        let envelope = Envelope {
            schema_version: T::CURRENT_VERSION,
            data,
        };
        let json = serde_json::to_string_pretty(&envelope).map_err(|_| AtomicError::Encode)?;

        if self.path.exists() {
            // The one and only backup generation. Overwriting it every write
            // is what keeps it singular rather than accumulating history.
            fs::copy(&self.path, self.bak_path())?;
        }

        let tmp = self.tmp_path();
        {
            let mut f = File::create(&tmp)?;
            f.write_all(json.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}
