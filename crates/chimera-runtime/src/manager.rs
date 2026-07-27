//! Codex App Manager integration for release discovery and install planning.
//!
//! The Windows package parser and installer are reused from the pinned MIT
//! `codex-win-engine`; this module keeps Chimera's public contract small and
//! prevents Tauri commands from depending on engine-specific details.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Default China-reachable Codex mirror managed by the reference project.
pub const DEFAULT_MIRROR_BASE: &str = "https://codexapp.agentsmirror.com";

/// How the application chooses where Codex update bytes come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSource {
    /// Choose the best supported source for this platform. On Windows the
    /// official endpoint does not expose a complete installer contract, so
    /// automatic mode currently resolves to the verified mirror.
    Auto,
    /// Always use the configured mirror endpoints.
    Mirror,
}

impl FromStr for UpdateSource {
    type Err = ManagerError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "mirror" => Ok(Self::Mirror),
            _ => Err(ManagerError::UnsupportedSource),
        }
    }
}

/// Windows installation strategy selected by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallMode {
    /// Register the official MSIX through Windows AppX deployment.
    Standard,
    /// Extract the official signed MSIX into a Chimera-managed user directory.
    Portable,
}

impl FromStr for InstallMode {
    type Err = ManagerError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "portable" => Ok(Self::Portable),
            _ => Err(ManagerError::UnsupportedInstallMode),
        }
    }
}

/// URLs needed to discover and download one Windows release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorEndpoints {
    pub manifest_url: String,
    pub checksums_url: String,
    pub package_url: String,
}

/// Resolve the stable endpoints for the selected architecture.
pub fn mirror_endpoints(source: UpdateSource, architecture: Option<&str>) -> MirrorEndpoints {
    let _ = source;
    let package = match architecture.map(|value| value.trim().to_ascii_lowercase()) {
        Some(value) if value == "arm64" || value == "aarch64" => "win-arm64",
        Some(value) if value == "x64" || value == "x86_64" || value == "amd64" => "win-x64",
        _ => "win",
    };
    MirrorEndpoints {
        manifest_url: format!("{DEFAULT_MIRROR_BASE}/latest/manifest"),
        checksums_url: format!("{DEFAULT_MIRROR_BASE}/latest/checksums"),
        package_url: format!("{DEFAULT_MIRROR_BASE}/latest/{package}"),
    }
}

/// A checksum-bound Windows package ready for comparison or installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsReleasePlan {
    /// Human-facing Codex app version, for example `26.721.41059`.
    pub version: String,
    /// Four-part MSIX deployment version, for example `26.721.4979.0`.
    pub package_version: String,
    pub package_moniker: String,
    pub package_url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub released_at: Option<String>,
}

impl WindowsReleasePlan {
    /// Whether installing this plan would advance the detected application.
    pub fn is_update_available(&self, current: Option<&str>) -> bool {
        let Some(current) = current.map(str::trim).filter(|value| !value.is_empty()) else {
            return true;
        };
        if current.eq_ignore_ascii_case(&self.version)
            || current.eq_ignore_ascii_case(&self.package_version)
        {
            return false;
        }
        codex_win_engine::version::compare_versions(current, &self.version).is_lt()
    }
}

/// Errors surfaced by the manager service without local paths or credentials.
#[derive(Debug, Error)]
pub enum ManagerError {
    #[error("unsupported Codex update source")]
    UnsupportedSource,
    #[error("unsupported Codex installation mode")]
    UnsupportedInstallMode,
    #[error("the Codex release manifest is invalid")]
    InvalidManifest,
    #[error("the Codex release checksum is missing or invalid")]
    InvalidChecksum,
    #[error("the Codex release does not declare a download size")]
    MissingSize,
    #[error("the Codex release could not be fetched")]
    Fetch,
}

/// Parse the mirror manifest and bind it to its declared MSIX checksum.
pub fn parse_windows_release_plan(
    manifest: &str,
    checksums: &str,
    source: UpdateSource,
    architecture: Option<&str>,
) -> Result<WindowsReleasePlan, ManagerError> {
    let release = codex_win_engine::manifest::parse_manifest_for_arch(manifest, architecture)
        .map_err(|_| ManagerError::InvalidManifest)?;
    let sha256 = codex_win_engine::find_msix_sha256(checksums, &release.package_moniker)
        .map_err(|_| ManagerError::InvalidChecksum)?;
    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ManagerError::InvalidChecksum);
    }
    let endpoints = mirror_endpoints(source, release.download_architecture.as_deref());
    Ok(WindowsReleasePlan {
        version: release.version,
        package_version: release.package_version,
        package_moniker: release.package_moniker,
        package_url: endpoints.package_url,
        sha256: sha256.to_ascii_lowercase(),
        size_bytes: release.content_length.ok_or(ManagerError::MissingSize)?,
        released_at: release.released_at,
    })
}

/// Fetch and parse the latest Windows release using the reference engine's
/// bounded network implementation.
pub fn fetch_windows_release_plan(
    source: UpdateSource,
    architecture: Option<&str>,
) -> Result<WindowsReleasePlan, ManagerError> {
    let endpoints = mirror_endpoints(source, architecture);
    let manifest =
        codex_win_engine::fetch_text(&endpoints.manifest_url).map_err(|_| ManagerError::Fetch)?;
    let checksums =
        codex_win_engine::fetch_text(&endpoints.checksums_url).map_err(|_| ManagerError::Fetch)?;
    parse_windows_release_plan(&manifest, &checksums, source, architecture)
}
