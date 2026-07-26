//! `.codexskin` manifest (`theme.json`) schema — Step 8.1 (ADR-005).
//!
//! Kept separate from [`crate::package`] (the zip extraction pipeline) so the
//! *shape* of a valid manifest is testable without a single byte of zip
//! machinery, and separate from [`crate::css_allowlist`] so CSS content rules
//! don't get tangled up with "does this JSON even describe a plausible skin".

use serde::Deserialize;
use thiserror::Error;

/// Only schema version this build understands.
///
/// An unknown version is refused rather than best-effort parsed: a future
/// author's manifest may rely on fields this build has never validated, so
/// "the JSON happens to parse" must never be mistaken for "safe to import".
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Parsed and validated contents of `theme.json`.
///
/// Constructing one always means [`SkinManifest::validate`] has already
/// passed — there is no way to obtain a `SkinManifest` that hasn't been
/// checked, because [`SkinManifest::parse`] is the only public constructor.
#[derive(Debug, Clone, Deserialize)]
pub struct SkinManifest {
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    /// Relative path, inside the package, to the single CSS entry point.
    /// Never a URL, never absolute, never able to escape the package root —
    /// see [`validate_entry_css`].
    pub entry_css: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Why a `theme.json` was refused.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    /// Covers "not JSON", "not UTF-8", and "JSON but missing/mistyped a
    /// required field" — serde_json's own message already names the field,
    /// and re-deriving that logic here would just be a second, potentially
    /// out-of-sync, source of the same information.
    #[error("theme.json could not be read: {0}")]
    Malformed(String),
    #[error("unsupported schema_version {found} (this build understands {supported})")]
    UnsupportedSchemaVersion { found: u32, supported: u32 },
    #[error("name must be non-empty")]
    EmptyName,
    #[error("version must be non-empty")]
    EmptyVersion,
    #[error("entry_css must be a relative .css path within the package, got {0:?}")]
    InvalidEntryCss(String),
}

impl SkinManifest {
    /// Parse and validate `theme.json` bytes in one step.
    ///
    /// Parsing and validating are never split into two public calls: a
    /// manifest that parsed but hadn't yet been validated would be a value a
    /// caller could accidentally act on.
    pub fn parse(bytes: &[u8]) -> Result<Self, ManifestError> {
        let text =
            std::str::from_utf8(bytes).map_err(|e| ManifestError::Malformed(e.to_string()))?;
        let manifest: SkinManifest =
            serde_json::from_str(text).map_err(|e| ManifestError::Malformed(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedSchemaVersion {
                found: self.schema_version,
                supported: SUPPORTED_SCHEMA_VERSION,
            });
        }
        if self.name.trim().is_empty() {
            return Err(ManifestError::EmptyName);
        }
        if self.version.trim().is_empty() {
            return Err(ManifestError::EmptyVersion);
        }
        validate_entry_css(&self.entry_css)?;
        Ok(())
    }
}

/// Validate that `path` names a plain relative `.css` file with no way out of
/// the package: no scheme (`https:`, `javascript:`, ...), no absolute prefix
/// on either Windows or POSIX, no `..` traversal, and no backslash — the zip
/// format itself only ever uses `/`, so a backslash here is already a sign
/// the string was crafted for a Windows path parser rather than a zip path.
fn validate_entry_css(path: &str) -> Result<(), ManifestError> {
    let bad = || Err(ManifestError::InvalidEntryCss(path.to_string()));

    if path.is_empty() || !path.ends_with(".css") {
        return bad();
    }
    if path.contains('\\') || path.contains("..") {
        return bad();
    }
    if path.starts_with('/') {
        return bad();
    }
    // A URL scheme ("https://", "javascript:", "data:", ...) always has a
    // colon before the first slash (if any); a bare Windows drive letter
    // ("C:\...") also has a colon in the first two characters. Rejecting any
    // colon at all is simpler than allow-listing schemes and is exactly as
    // safe, since a legitimate in-package relative path never contains one.
    if path.contains(':') {
        return bad();
    }
    Ok(())
}
