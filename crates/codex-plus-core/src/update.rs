use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Context as _;
use fs2::FileExt;
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const DEFAULT_REPOSITORY: &str = crate::branding::REPOSITORY;
pub const DEFAULT_LATEST_JSON_URL: &str = crate::branding::LATEST_JSON_URL;
pub const MAX_LATEST_JSON_BYTES: u64 = 1024 * 1024;
pub const MAX_UPDATE_ASSET_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const LATEST_JSON_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const STARTUP_LATEST_JSON_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const UPDATE_DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);
const UPDATE_LOCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const UPDATE_CONTINUATION_TTL: std::time::Duration = std::time::Duration::from_secs(2 * 60 * 60);
const MAX_RETAINED_UPDATE_ASSETS: usize = 3;
static NEXT_DOWNLOAD_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[cfg(any(target_os = "macos", test))]
fn installer_fd_restore_error(error: std::io::Error) -> anyhow::Error {
    anyhow::Error::new(error).context("failed to restore installer FD flags")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    #[serde(default)]
    pub minimum_supported_version: Option<String>,
    pub url: String,
    pub body: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    #[serde(default)]
    pub asset_sha256: Option<String>,
    #[serde(default)]
    pub asset_size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateCheck {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub minimum_supported_version: Option<String>,
    pub release_summary: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub asset_sha256: Option<String>,
    pub asset_size: Option<u64>,
    pub update_available: bool,
    pub mandatory_update: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateInstall {
    pub release: Release,
    pub installer_path: PathBuf,
    pub launched: bool,
    pub exit_current_process: bool,
    pub requires_user_confirmation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatePlatform {
    Windows,
    Macos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerLaunchPolicy {
    pub arguments: Vec<String>,
    pub exit_current_process: bool,
    pub requires_user_confirmation: bool,
}

pub fn installer_launch_policy(platform: UpdatePlatform) -> InstallerLaunchPolicy {
    match platform {
        UpdatePlatform::Windows => InstallerLaunchPolicy {
            arguments: vec!["/S".to_string()],
            exit_current_process: true,
            requires_user_confirmation: false,
        },
        UpdatePlatform::Macos => InstallerLaunchPolicy {
            arguments: vec!["attach".to_string(), "-autoopen".to_string()],
            exit_current_process: false,
            requires_user_confirmation: true,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateAction {
    None,
    Automatic,
    Mandatory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCacheStatus {
    Missing,
    Valid,
    CorruptQuarantined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedFloorSnapshot {
    pub minimum_supported_version: Option<String>,
    pub status: UpdateCacheStatus,
    pub quarantined_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateDecision {
    pub action: UpdateAction,
    pub latest_version: Option<String>,
    pub minimum_supported_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupUpdateStatus {
    pub decision: UpdateDecision,
    pub check: Option<UpdateCheck>,
    pub cache_status: UpdateCacheStatus,
    pub manifest_available: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateStateStore {
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedUpdateState {
    version: u32,
    minimum_supported_version: String,
}

#[derive(Debug, Clone)]
pub struct UpdateContinuationStore {
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedUpdateContinuation {
    version: u32,
    token: String,
    current_version: String,
    observed_floor: Option<String>,
    expires_at_unix_seconds: u64,
}

impl Default for UpdateContinuationStore {
    fn default() -> Self {
        Self::new(crate::paths::default_update_continuation_path())
    }
}

impl UpdateContinuationStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn issue(
        &self,
        current_version: &str,
        observed_floor: Option<&str>,
    ) -> anyhow::Result<String> {
        let current_version = parse_version_tag(current_version)?.to_string();
        let observed_floor = observed_floor
            .map(parse_version_tag)
            .transpose()?
            .map(|version| version.to_string());
        if decide_update(&current_version, None, observed_floor.as_deref())?.action
            == UpdateAction::Mandatory
        {
            anyhow::bail!("current version is below the trusted minimum supported version");
        }
        let token = uuid::Uuid::new_v4().to_string();
        let expires_at_unix_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .checked_add(UPDATE_CONTINUATION_TTL)
            .ok_or_else(|| anyhow::anyhow!("update continuation expiry overflow"))?
            .as_secs();
        let state = PersistedUpdateContinuation {
            version: 1,
            token: token.clone(),
            current_version,
            observed_floor,
            expires_at_unix_seconds,
        };
        self.with_lock(|| crate::settings::atomic_write(&self.path, &serde_json::to_vec(&state)?))?;
        Ok(token)
    }

    pub fn consume_if_supported(
        &self,
        token: &str,
        current_version: &str,
        update_state_store: &UpdateStateStore,
    ) -> anyhow::Result<bool> {
        self.with_lock(|| {
            let bytes = match fs::read(&self.path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(error.into()),
            };
            let Ok(state) = serde_json::from_slice::<PersistedUpdateContinuation>(&bytes) else {
                let _ = fs::remove_file(&self.path);
                return Ok(false);
            };
            if state.version != 1 || state.token != token {
                return Ok(false);
            }
            fs::remove_file(&self.path)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let current_version = parse_version_tag(current_version)?.to_string();
            if now > state.expires_at_unix_seconds || current_version != state.current_version {
                return Ok(false);
            }
            let latest = update_state_store.load_trusted_floor()?;
            let effective_floor = [
                state.observed_floor.as_deref(),
                latest.minimum_supported_version.as_deref(),
            ]
            .into_iter()
            .flatten()
            .map(parse_version_tag)
            .collect::<anyhow::Result<Vec<_>>>()?
            .into_iter()
            .max()
            .map(|version| version.to_string());
            Ok(
                decide_update(&current_version, None, effective_floor.as_deref())?.action
                    != UpdateAction::Mandatory,
            )
        })
    }

    fn with_lock<T>(&self, operation: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let lock_path = self.path.with_extension("lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)?;
        lock.lock_exclusive()?;
        let result = operation();
        let unlock = fs2::FileExt::unlock(&lock);
        match (result, unlock) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error.into()),
        }
    }
}

impl Default for UpdateStateStore {
    fn default() -> Self {
        Self::new(crate::paths::default_update_state_path())
    }
}

impl UpdateStateStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load_trusted_floor(&self) -> anyhow::Result<TrustedFloorSnapshot> {
        self.with_state_lock(|| self.load_trusted_floor_locked())
    }

    fn load_trusted_floor_locked(&self) -> anyhow::Result<TrustedFloorSnapshot> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(TrustedFloorSnapshot {
                    minimum_supported_version: None,
                    status: UpdateCacheStatus::Missing,
                    quarantined_path: None,
                });
            }
            Err(error) => return Err(error).context("failed to read update state"),
        };
        let parsed = serde_json::from_slice::<PersistedUpdateState>(&bytes)
            .ok()
            .filter(|state| state.version == 1)
            .and_then(|state| {
                parse_version_tag(&state.minimum_supported_version)
                    .ok()
                    .map(|version| version.to_string())
            });
        if let Some(minimum_supported_version) = parsed {
            return Ok(TrustedFloorSnapshot {
                minimum_supported_version: Some(minimum_supported_version),
                status: UpdateCacheStatus::Valid,
                quarantined_path: None,
            });
        }

        let quarantined_path = crate::settings::quarantine_corrupt_state_file(&self.path)?;
        Ok(TrustedFloorSnapshot {
            minimum_supported_version: None,
            status: UpdateCacheStatus::CorruptQuarantined,
            quarantined_path,
        })
    }

    pub fn record_trusted_floor(
        &self,
        minimum_supported_version: &str,
    ) -> anyhow::Result<TrustedFloorSnapshot> {
        self.with_state_lock(|| self.record_trusted_floor_locked(minimum_supported_version))
    }

    pub fn authorize_release_install<T>(
        &self,
        release: &Release,
        operation: impl FnOnce() -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        self.with_state_lock(|| {
            let latest = self.load_trusted_floor_locked()?;
            validate_release_against_trusted_floor(
                release,
                latest.minimum_supported_version.as_deref(),
            )?;
            operation()
        })
    }

    fn with_state_lock<T>(
        &self,
        operation: impl FnOnce() -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let lock_path = self.path.with_extension("lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .with_context(|| format!("failed to open update state lock {}", lock_path.display()))?;
        lock.lock_exclusive()
            .with_context(|| format!("failed to lock update state {}", lock_path.display()))?;
        let result = operation();
        let unlock_result = fs2::FileExt::unlock(&lock)
            .with_context(|| format!("failed to unlock update state {}", lock_path.display()));
        match (result, unlock_result) {
            (Ok(snapshot), Ok(())) => Ok(snapshot),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    fn record_trusted_floor_locked(
        &self,
        minimum_supported_version: &str,
    ) -> anyhow::Result<TrustedFloorSnapshot> {
        let candidate = parse_version_tag(minimum_supported_version)
            .context("minimum supported version is invalid")?
            .to_string();
        let current = self.load_trusted_floor_locked()?;
        if let Some(existing) = current.minimum_supported_version.as_deref() {
            if parse_version_tag(existing)? >= parse_version_tag(&candidate)? {
                return Ok(current);
            }
        }

        let state = PersistedUpdateState {
            version: 1,
            minimum_supported_version: candidate.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&state)?;
        crate::settings::atomic_write(&self.path, &bytes)?;
        Ok(TrustedFloorSnapshot {
            minimum_supported_version: Some(candidate),
            status: UpdateCacheStatus::Valid,
            quarantined_path: current.quarantined_path,
        })
    }
}

pub fn parse_version_tag(value: &str) -> anyhow::Result<semver::Version> {
    if value.trim() != value {
        anyhow::bail!("version must not contain surrounding whitespace: {value:?}");
    }
    let normalized = value
        .strip_prefix('v')
        .or_else(|| value.strip_prefix('V'))
        .unwrap_or(value);
    let version = semver::Version::parse(normalized)
        .map_err(|error| anyhow::anyhow!("Invalid version tag: {value} ({error})"))?;
    require_chimera_channel(&version)?;
    Ok(version)
}

fn require_chimera_channel(version: &semver::Version) -> anyhow::Result<()> {
    let pre = version.pre.as_str();
    let Some(revision) = pre.strip_prefix("chimera.") else {
        anyhow::bail!("version must use chimera channel: {version}");
    };
    if revision.is_empty() || !revision.chars().all(|ch| ch.is_ascii_digit()) {
        anyhow::bail!("invalid chimera revision: {version}");
    }
    revision
        .parse::<u64>()
        .map_err(|_| anyhow::anyhow!("chimera revision is outside u64 range: {version}"))?;
    if !version.build.is_empty() {
        anyhow::bail!("build metadata is not allowed: {version}");
    }
    Ok(())
}

pub fn is_newer_version(candidate: &str, current: &str) -> anyhow::Result<bool> {
    let left = parse_version_tag(candidate)?;
    let right = parse_version_tag(current)?;
    Ok(left > right)
}

pub fn decide_update(
    current_version: &str,
    release: Option<&Release>,
    trusted_floor: Option<&str>,
) -> anyhow::Result<UpdateDecision> {
    let current =
        parse_version_tag(current_version).context("current application version invalid")?;
    let floor = trusted_floor
        .map(parse_version_tag)
        .transpose()
        .context("trusted minimum supported version invalid")?;
    let latest = release
        .map(|release| parse_version_tag(&release.version))
        .transpose()
        .context("latest release version invalid")?;
    let minimum_supported_version = floor.as_ref().map(ToString::to_string);
    let latest_version = latest.as_ref().map(ToString::to_string);

    if floor.as_ref().is_some_and(|floor| current < *floor) {
        return Ok(UpdateDecision {
            action: UpdateAction::Mandatory,
            latest_version,
            minimum_supported_version,
        });
    }

    let automatic = release.is_some_and(release_has_complete_native_asset)
        && latest.as_ref().is_some_and(|latest| latest > &current);
    Ok(UpdateDecision {
        action: if automatic {
            UpdateAction::Automatic
        } else {
            UpdateAction::None
        },
        latest_version,
        minimum_supported_version,
    })
}

fn release_has_complete_native_asset(release: &Release) -> bool {
    release.asset_name.is_some()
        && release.asset_url.is_some()
        && release.asset_sha256.is_some()
        && release.asset_size.is_some()
}

pub fn validate_release_against_trusted_floor(
    release: &Release,
    trusted_floor: Option<&str>,
) -> anyhow::Result<()> {
    if trusted_floor
        .map(|floor| is_newer_version(floor, &release.version))
        .transpose()?
        .unwrap_or(false)
    {
        anyhow::bail!("latest release is below the trusted minimum supported version");
    }
    Ok(())
}

pub fn validate_release_for_install(release: &Release) -> anyhow::Result<()> {
    parse_version_tag(&release.version).context("release version is invalid")?;
    release_asset_metadata(release).map(|_| ())
}

pub fn validate_update_request(
    requested_version: &str,
    trusted_release: Release,
    current_version: &str,
) -> anyhow::Result<Release> {
    let requested =
        parse_version_tag(requested_version).context("请求的更新版本不是有效的 Chimera 版本")?;
    let trusted = parse_version_tag(&trusted_release.version)
        .context("可信更新源返回了无效的 Chimera 版本")?;
    let current = parse_version_tag(current_version).context("当前应用版本无效")?;

    if requested != trusted {
        anyhow::bail!("可用版本已变化，请重新检查更新后再安装");
    }
    if trusted <= current {
        anyhow::bail!("可信更新版本不高于当前版本，已拒绝重装或降级");
    }
    Ok(trusted_release)
}

pub fn release_from_latest_json_payload(payload: &Value) -> anyhow::Result<Release> {
    let version = payload
        .get("version")
        .or_else(|| payload.get("tag_name"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("latest.json missing version"))?
        .to_string();
    // Reject foreign/upstream channels before selecting assets.
    let parsed_version = parse_version_tag(&version)?;
    let minimum_supported_version = match payload.get("minimum_supported_version") {
        None => None,
        Some(Value::String(value)) => {
            let parsed = parse_version_tag(value)
                .map_err(|error| anyhow::anyhow!("invalid minimum_supported_version: {error}"))?;
            if parsed > parsed_version {
                anyhow::bail!(
                    "minimum_supported_version {parsed} must not exceed latest version {parsed_version}"
                );
            }
            Some(parsed.to_string())
        }
        Some(_) => anyhow::bail!("minimum_supported_version must be a string"),
    };
    let expected_installer_names = [
        format!(
            "{}-{}-windows-x64-setup.exe",
            crate::branding::ARTIFACT_PREFIX,
            parsed_version
        ),
        format!(
            "{}-{}-macos-x64.dmg",
            crate::branding::ARTIFACT_PREFIX,
            parsed_version
        ),
        format!(
            "{}-{}-macos-arm64.dmg",
            crate::branding::ARTIFACT_PREFIX,
            parsed_version
        ),
    ];

    let mut assets = Vec::new();
    let raw_assets = payload
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("latest.json missing assets"))?;
    for asset in raw_assets {
        let name = asset
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("latest.json asset missing name"))?
            .to_string();
        if is_chimera_installer_asset(&name)
            && !expected_installer_names
                .iter()
                .any(|expected| expected == &name)
        {
            anyhow::bail!(
                "latest.json installer filename does not exactly match manifest version {parsed_version}: {name}"
            );
        }
        let url = asset
            .get("url")
            .or_else(|| asset.get("browser_download_url"))
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("latest.json asset missing url"))?
            .to_string();
        validate_manifest_asset_url(&url, &name, &parsed_version)?;
        let sha256 = asset
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("latest.json asset missing sha256"))?
            .trim()
            .to_ascii_lowercase();
        if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            anyhow::bail!("latest.json asset sha256 must be 64 hexadecimal characters");
        }
        let size = asset
            .get("size")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("latest.json asset missing size"))?;
        if size == 0 || size > MAX_UPDATE_ASSET_BYTES {
            anyhow::bail!(
                "latest.json asset size must be between 1 and {MAX_UPDATE_ASSET_BYTES} bytes"
            );
        }
        assets.push(ReleaseAsset {
            name,
            browser_download_url: url,
            sha256,
            size,
        });
    }

    let selected = select_update_asset(&assets);
    Ok(Release {
        version,
        minimum_supported_version,
        url: payload
            .get("url")
            .or_else(|| payload.get("html_url"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        body: payload
            .get("body")
            .or_else(|| payload.get("release_summary"))
            .or_else(|| payload.get("notes"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        asset_name: selected.as_ref().map(|asset| asset.name.clone()),
        asset_url: selected
            .as_ref()
            .map(|asset| asset.browser_download_url.clone()),
        asset_sha256: selected.as_ref().map(|asset| asset.sha256.clone()),
        asset_size: selected.as_ref().map(|asset| asset.size),
    })
}

fn validate_manifest_asset_url(
    asset_url: &str,
    asset_name: &str,
    version: &semver::Version,
) -> anyhow::Result<()> {
    let url = reqwest::Url::parse(asset_url)
        .map_err(|error| anyhow::anyhow!("latest.json asset url is invalid: {error}"))?;
    let Some((owner, repository)) = crate::branding::REPOSITORY.split_once('/') else {
        anyhow::bail!("branding repository must use owner/name form");
    };
    if owner.is_empty() || repository.is_empty() || repository.contains('/') {
        anyhow::bail!("branding repository must use owner/name form");
    }

    let expected_tag = format!("v{version}");
    let expected_path = [
        owner,
        repository,
        "releases",
        "download",
        expected_tag.as_str(),
        asset_name,
    ];
    let path_matches = url
        .path_segments()
        .is_some_and(|segments| segments.eq(expected_path));
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str() != Some("github.com")
        || url.query().is_some()
        || url.fragment().is_some()
        || !path_matches
    {
        anyhow::bail!(
            "latest.json asset url must be an HTTPS GitHub Release download from {} for {asset_name}",
            crate::branding::REPOSITORY
        );
    }
    Ok(())
}

pub fn select_update_asset(assets: &[ReleaseAsset]) -> Option<ReleaseAsset> {
    select_update_asset_for_target(assets, std::env::consts::OS, std::env::consts::ARCH)
}

pub fn select_update_asset_for_target(
    assets: &[ReleaseAsset],
    target_os: &str,
    target_arch: &str,
) -> Option<ReleaseAsset> {
    let named = assets.iter().filter(|asset| {
        !asset.name.trim().is_empty()
            && !asset.browser_download_url.trim().is_empty()
            && !asset.sha256.trim().is_empty()
    });
    let mut best: Option<(u8, &ReleaseAsset)> = None;
    for asset in named {
        let rank = platform_asset_rank(&asset.name, target_os, target_arch);
        if rank >= 2 {
            continue;
        }
        if best.map_or(true, |(r, _)| rank < r) {
            best = Some((rank, asset));
        }
    }
    best.map(|(_, asset)| asset.clone())
}

pub async fn fetch_latest_release(latest_json_url: &str) -> anyhow::Result<Release> {
    fetch_latest_release_with_timeout(latest_json_url, LATEST_JSON_TIMEOUT).await
}

pub async fn fetch_latest_release_with_client(
    latest_json_url: &str,
    client: &reqwest::Client,
) -> anyhow::Result<Release> {
    fetch_latest_release_with_client_and_timeout(latest_json_url, client, LATEST_JSON_TIMEOUT).await
}

pub async fn fetch_latest_release_with_timeout(
    latest_json_url: &str,
    timeout: std::time::Duration,
) -> anyhow::Result<Release> {
    let client = crate::http_client::proxied_client(&crate::http_client::branded_user_agent(""))?;
    fetch_latest_release_with_client_and_timeout(latest_json_url, &client, timeout).await
}

pub async fn fetch_latest_release_with_client_and_timeout(
    latest_json_url: &str,
    client: &reqwest::Client,
    timeout: std::time::Duration,
) -> anyhow::Result<Release> {
    let response = client
        .get(latest_json_url)
        .timeout(timeout)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|error| http_request_error("latest.json", error))?
        .error_for_status()?;
    let bytes = read_response_limited(response, MAX_LATEST_JSON_BYTES, "latest.json").await?;
    let payload =
        serde_json::from_slice::<Value>(&bytes).context("failed to decode latest.json")?;
    release_from_latest_json_payload(&payload)
}

async fn read_response_limited(
    response: reqwest::Response,
    maximum: u64,
    label: &str,
) -> anyhow::Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum)
    {
        anyhow::bail!("{label} response size exceeds {maximum} bytes");
    }

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| http_request_error(label, error))?;
        let next_len = (bytes.len() as u64)
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| anyhow::anyhow!("{label} response size overflow"))?;
        if next_len > maximum {
            anyhow::bail!("{label} response size exceeds {maximum} bytes");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub async fn check_for_update(current_version: &str) -> anyhow::Result<UpdateCheck> {
    let release = fetch_latest_release(DEFAULT_LATEST_JSON_URL).await?;
    update_check_from_release(current_version, release)
}

pub async fn startup_update_status(current_version: &str) -> anyhow::Result<StartupUpdateStatus> {
    let store = UpdateStateStore::default();
    let cached = store.load_trusted_floor()?;
    match fetch_latest_release_with_timeout(DEFAULT_LATEST_JSON_URL, STARTUP_LATEST_JSON_TIMEOUT)
        .await
    {
        Ok(release) => {
            if release.minimum_supported_version.is_some()
                && !release_has_complete_native_asset(&release)
            {
                return startup_update_status_from_parts(current_version, None, cached);
            }
            let effective = if let Some(floor) = release.minimum_supported_version.as_deref() {
                store.record_trusted_floor(floor)?
            } else {
                cached
            };
            startup_update_status_from_parts(current_version, Some(release), effective)
        }
        Err(_) => startup_update_status_from_parts(current_version, None, cached),
    }
}

pub fn startup_update_status_from_parts(
    current_version: &str,
    release: Option<Release>,
    trusted: TrustedFloorSnapshot,
) -> anyhow::Result<StartupUpdateStatus> {
    let manifest_rolls_back_below_floor = release.as_ref().is_some_and(|release| {
        validate_release_against_trusted_floor(
            release,
            trusted.minimum_supported_version.as_deref(),
        )
        .is_err()
    });
    let usable_release = if manifest_rolls_back_below_floor {
        None
    } else {
        release
    };
    let decision = decide_update(
        current_version,
        usable_release.as_ref(),
        trusted.minimum_supported_version.as_deref(),
    )?;
    let manifest_available = usable_release.is_some();
    let mut check = usable_release
        .map(|release| update_check_from_release(current_version, release))
        .transpose()?;
    if let Some(check) = check.as_mut() {
        check.minimum_supported_version = trusted.minimum_supported_version;
        check.mandatory_update = decision.action == UpdateAction::Mandatory;
    }
    Ok(StartupUpdateStatus {
        decision,
        check,
        cache_status: trusted.status,
        manifest_available,
    })
}

pub fn update_check_from_release(
    current_version: &str,
    release: Release,
) -> anyhow::Result<UpdateCheck> {
    let mandatory_update = release
        .minimum_supported_version
        .as_deref()
        .map(|floor| is_newer_version(floor, current_version))
        .transpose()?
        .unwrap_or(false);
    let has_native_asset = release_has_complete_native_asset(&release);
    let update_available = has_native_asset && is_newer_version(&release.version, current_version)?;
    Ok(UpdateCheck {
        current_version: current_version.to_string(),
        latest_version: Some(release.version),
        minimum_supported_version: release.minimum_supported_version,
        release_summary: release.body,
        asset_name: release.asset_name,
        asset_url: release.asset_url,
        asset_sha256: release.asset_sha256,
        asset_size: release.asset_size,
        update_available,
        mandatory_update,
    })
}

pub async fn perform_update(
    release: &Release,
    download_dir: &Path,
    optional_continuation_current_version: Option<&str>,
) -> anyhow::Result<UpdateInstall> {
    validate_release_for_install(release)?;
    let installer_path = download_release_asset(release, download_dir).await?;
    let store = UpdateStateStore::default();
    let continuation_token = if let Some(current_version) = optional_continuation_current_version {
        let latest = store.load_trusted_floor()?;
        if decide_update(
            current_version,
            None,
            latest.minimum_supported_version.as_deref(),
        )?
        .action
            == UpdateAction::Mandatory
        {
            None
        } else {
            Some(
                UpdateContinuationStore::default()
                    .issue(current_version, latest.minimum_supported_version.as_deref())?,
            )
        }
    } else {
        None
    };
    let mut launch_guard = open_validated_installer_for_launch(release, &installer_path)?;
    let policy = store.authorize_release_install(release, || {
        launch_installer(
            &installer_path,
            &mut launch_guard,
            continuation_token.as_deref(),
        )
    })?;
    drop(launch_guard);
    Ok(UpdateInstall {
        release: release.clone(),
        installer_path,
        launched: true,
        exit_current_process: policy.exit_current_process,
        requires_user_confirmation: policy.requires_user_confirmation,
    })
}

pub async fn download_release_asset(
    release: &Release,
    download_dir: &Path,
) -> anyhow::Result<PathBuf> {
    download_release_asset_with_timeouts(
        release,
        download_dir,
        UPDATE_DOWNLOAD_TIMEOUT,
        UPDATE_LOCK_TIMEOUT,
    )
    .await
}

pub async fn download_release_asset_with_client(
    release: &Release,
    download_dir: &Path,
    client: &reqwest::Client,
) -> anyhow::Result<PathBuf> {
    download_release_asset_with_client_and_timeouts(
        release,
        download_dir,
        client,
        UPDATE_DOWNLOAD_TIMEOUT,
        UPDATE_LOCK_TIMEOUT,
    )
    .await
}

pub async fn download_release_asset_with_timeouts(
    release: &Release,
    download_dir: &Path,
    request_timeout: std::time::Duration,
    lock_timeout: std::time::Duration,
) -> anyhow::Result<PathBuf> {
    let client = crate::http_client::proxied_client(&crate::http_client::branded_user_agent(""))?;
    download_release_asset_with_client_and_timeouts(
        release,
        download_dir,
        &client,
        request_timeout,
        lock_timeout,
    )
    .await
}

pub async fn download_release_asset_with_client_and_timeouts(
    release: &Release,
    download_dir: &Path,
    client: &reqwest::Client,
    request_timeout: std::time::Duration,
    lock_timeout: std::time::Duration,
) -> anyhow::Result<PathBuf> {
    let (safe, url, expected_sha256, expected_size) = release_asset_metadata(release)?;
    std::fs::create_dir_all(download_dir)?;
    let _lock = lock_download_directory_async(download_dir, lock_timeout).await?;
    cleanup_stale_download_parts(download_dir)?;
    if let Some(existing) =
        find_verified_existing_asset(download_dir, &safe, &expected_sha256, expected_size)?
    {
        enforce_update_asset_retention(download_dir, &existing)?;
        return Ok(existing);
    }

    let response = client
        .get(&url)
        .timeout(request_timeout)
        .send()
        .await
        .map_err(|error| http_request_error("update asset", error))?
        .error_for_status()?;
    if let Some(length) = response.content_length()
        && length != expected_size
    {
        anyhow::bail!(
            "downloaded asset size mismatch: expected {expected_size}, got Content-Length {length}"
        );
    }

    let (temp_path, mut temp_file) = create_download_temp(download_dir, &safe)?;
    let download_result = async {
        let mut stream = response.bytes_stream();
        let mut digest = Sha256::new();
        let mut written = 0_u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| http_request_error("update asset", error))?;
            written = written
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| anyhow::anyhow!("downloaded asset size overflow"))?;
            if written > expected_size {
                anyhow::bail!(
                    "downloaded asset size mismatch: expected {expected_size}, exceeded while streaming"
                );
            }
            temp_file.write_all(&chunk)?;
            digest.update(&chunk);
        }
        if written != expected_size {
            anyhow::bail!(
                "downloaded asset size mismatch: expected {expected_size}, got {written}"
            );
        }
        let actual = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if actual != expected_sha256 {
            anyhow::bail!("downloaded asset sha256 mismatch");
        }
        temp_file.sync_all()?;
        drop(temp_file);
        verify_downloaded_file(&temp_path, &expected_sha256, expected_size)?;
        publish_verified_temp(
            &temp_path,
            download_dir,
            &safe,
            &expected_sha256,
            expected_size,
        )
    }
    .await;
    let published = finish_temp_download(&temp_path, download_result)?;
    enforce_update_asset_retention(download_dir, &published)?;
    Ok(published)
}

pub fn verify_downloaded_bytes(
    bytes: &[u8],
    expected_sha256: &str,
    expected_size: u64,
) -> anyhow::Result<()> {
    if bytes.len() as u64 != expected_size {
        anyhow::bail!(
            "downloaded asset size mismatch: expected {expected_size}, got {}",
            bytes.len()
        );
    }
    let actual = sha256_hex(bytes);
    let expected = expected_sha256.trim().to_ascii_lowercase();
    if actual != expected {
        anyhow::bail!("downloaded asset sha256 mismatch");
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn download_asset_to(
    release: &Release,
    bytes: &[u8],
    download_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let (safe, _url, expected_sha256, expected_size) = release_asset_metadata(release)?;
    std::fs::create_dir_all(download_dir)?;
    let _lock = lock_download_directory(download_dir)?;
    cleanup_stale_download_parts(download_dir)?;
    if let Some(existing) =
        find_verified_existing_asset(download_dir, &safe, &expected_sha256, expected_size)?
    {
        enforce_update_asset_retention(download_dir, &existing)?;
        return Ok(existing);
    }
    verify_downloaded_bytes(bytes, &expected_sha256, expected_size)?;

    let (temp_path, mut temp_file) = create_download_temp(download_dir, &safe)?;
    let publish_result = (|| -> anyhow::Result<PathBuf> {
        temp_file.write_all(bytes)?;
        temp_file.sync_all()?;
        drop(temp_file);
        verify_downloaded_file(&temp_path, &expected_sha256, expected_size)?;
        publish_verified_temp(
            &temp_path,
            download_dir,
            &safe,
            &expected_sha256,
            expected_size,
        )
    })();
    let published = finish_temp_download(&temp_path, publish_result)?;
    enforce_update_asset_retention(download_dir, &published)?;
    Ok(published)
}

fn release_asset_metadata(release: &Release) -> anyhow::Result<(String, String, String, u64)> {
    let name = release
        .asset_name
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("没有可下载的 Release asset"))?;
    let url = release
        .asset_url
        .as_ref()
        .filter(|url| !url.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Release asset missing url"))?
        .to_string();
    let expected_sha256 = release
        .asset_sha256
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase())
        .ok_or_else(|| anyhow::anyhow!("Release asset missing sha256"))?;
    if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        anyhow::bail!("Release asset sha256 must be 64 hexadecimal characters");
    }
    let expected_size = release
        .asset_size
        .ok_or_else(|| anyhow::anyhow!("Release asset missing size"))?;
    if expected_size == 0 || expected_size > MAX_UPDATE_ASSET_BYTES {
        anyhow::bail!("Release asset size must be between 1 and {MAX_UPDATE_ASSET_BYTES} bytes");
    }
    Ok((safe_asset_name(name)?, url, expected_sha256, expected_size))
}

fn lock_download_directory(download_dir: &Path) -> anyhow::Result<File> {
    let lock_path = download_dir.join(".chimera-update.lock");
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)?;
    lock_file
        .lock_exclusive()
        .map_err(|error| anyhow::anyhow!("failed to lock update directory: {error}"))?;
    Ok(lock_file)
}

async fn lock_download_directory_async(
    download_dir: &Path,
    timeout: std::time::Duration,
) -> anyhow::Result<File> {
    let lock_path = download_dir.join(".chimera-update.lock");
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)?;
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match FileExt::try_lock_exclusive(&lock_file) {
            Ok(()) => return Ok(lock_file),
            Err(error) if is_lock_contention(&error) => {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    anyhow::bail!("update directory lock timeout");
                }
                tokio::time::sleep(remaining.min(std::time::Duration::from_millis(25))).await;
            }
            Err(error) => {
                return Err(anyhow::anyhow!("failed to lock update directory: {error}"));
            }
        }
    }
}

fn is_lock_contention(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::PermissionDenied
    ) || (cfg!(windows) && error.raw_os_error() == Some(33))
}

fn http_request_error(label: &str, error: reqwest::Error) -> anyhow::Error {
    if error.is_timeout() {
        anyhow::anyhow!("{label} request timeout")
    } else {
        anyhow::anyhow!("{label} request failed: {error}")
    }
}

fn find_verified_existing_asset(
    download_dir: &Path,
    safe_name: &str,
    expected_sha256: &str,
    expected_size: u64,
) -> anyhow::Result<Option<PathBuf>> {
    for index in 0..=10_000 {
        let candidate = indexed_asset_path(download_dir, safe_name, index)?;
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_file() => {
                if verify_downloaded_file(&candidate, expected_sha256, expected_size).is_ok() {
                    return Ok(Some(candidate));
                }
            }
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(None)
}

fn verify_downloaded_file(
    path: &Path,
    expected_sha256: &str,
    expected_size: u64,
) -> anyhow::Result<()> {
    let mut file = File::open(path)?;
    verify_open_file(&mut file, expected_sha256, expected_size)
}

fn verify_open_file(
    file: &mut File,
    expected_sha256: &str,
    expected_size: u64,
) -> anyhow::Result<()> {
    if file.metadata()?.len() != expected_size {
        anyhow::bail!("downloaded asset size mismatch");
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let actual = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual != expected_sha256 {
        anyhow::bail!("downloaded asset sha256 mismatch");
    }
    Ok(())
}

pub fn validate_installer_for_launch(release: &Release, path: &Path) -> anyhow::Result<()> {
    drop(open_validated_installer_for_launch(release, path)?);
    Ok(())
}

fn open_validated_installer_for_launch(release: &Release, path: &Path) -> anyhow::Result<File> {
    let (safe_name, _url, expected_sha256, expected_size) = release_asset_metadata(release)?;
    if !path_matches_asset_name(path, &safe_name) {
        anyhow::bail!("installer path does not match trusted release asset name");
    }
    let path_metadata = std::fs::symlink_metadata(path)?;
    if !path_metadata.file_type().is_file() {
        anyhow::bail!("installer path is not a regular file");
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        const O_NOFOLLOW: i32 = 0x0000_0100;
        options.custom_flags(O_NOFOLLOW);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        const O_NOFOLLOW: i32 = 0x0002_0000;
        options.custom_flags(O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    if !file.metadata()?.file_type().is_file() {
        anyhow::bail!("installer handle is not a regular file");
    }
    verify_open_file(&mut file, &expected_sha256, expected_size)
        .context("installer changed before launch")?;
    Ok(file)
}

fn path_matches_asset_name(path: &Path, safe_name: &str) -> bool {
    let Some(actual_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    if actual_name == safe_name {
        return true;
    }
    let expected = Path::new(safe_name);
    let actual = Path::new(actual_name);
    if actual.extension() != expected.extension() {
        return false;
    }
    let Some(expected_stem) = expected.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    let Some(actual_stem) = actual.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    let Some(index) = actual_stem
        .strip_prefix(expected_stem)
        .and_then(|value| value.strip_prefix(" ("))
        .and_then(|value| value.strip_suffix(')'))
    else {
        return false;
    };
    !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
}

fn finish_temp_download(
    temp_path: &Path,
    publish_result: anyhow::Result<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let cleanup_result = std::fs::remove_file(&temp_path)
        .map_err(anyhow::Error::from)
        .context("failed to remove update temp file");
    match (publish_result, cleanup_result) {
        (Ok(path), Ok(())) => Ok(path),
        (Ok(_), Err(cleanup_error)) => Err(cleanup_error),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(error.context(cleanup_error.to_string())),
    }
}

fn cleanup_stale_download_parts(download_dir: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(download_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(owned) = name
            .strip_prefix(".chimera-update.")
            .and_then(|value| value.strip_suffix(".part"))
        else {
            continue;
        };
        let Some((safe_and_pid, sequence)) = owned.rsplit_once('.') else {
            continue;
        };
        let Some((safe_name, pid)) = safe_and_pid.rsplit_once('.') else {
            continue;
        };
        if safe_name.is_empty()
            || pid.is_empty()
            || sequence.is_empty()
            || !pid.bytes().all(|byte| byte.is_ascii_digit())
            || !sequence.bytes().all(|byte| byte.is_ascii_digit())
            || safe_asset_name(safe_name).is_err()
        {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_file() || file_type.is_symlink() {
            std::fs::remove_file(entry.path()).with_context(|| {
                format!(
                    "failed to remove stale update temp file {}",
                    entry.path().display()
                )
            })?;
        }
    }
    Ok(())
}

fn enforce_update_asset_retention(download_dir: &Path, preserve: &Path) -> anyhow::Result<()> {
    let mut owned = std::fs::read_dir(download_dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_file() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_owned_update_asset_name(&name) {
                return None;
            }
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            Some((modified, name, entry.path()))
        })
        .collect::<Vec<_>>();
    owned.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let remove_count = owned.len().saturating_sub(MAX_RETAINED_UPDATE_ASSETS);
    let mut removed = 0;
    for (_, _, path) in owned {
        if removed >= remove_count {
            break;
        }
        if path == preserve {
            continue;
        }
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove expired update asset {}", path.display()))?;
        removed += 1;
    }
    Ok(())
}

fn is_owned_update_asset_name(name: &str) -> bool {
    if is_chimera_installer_asset(name) {
        return true;
    }
    let path = Path::new(name);
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    let Some((base_stem, index)) = stem.rsplit_once(" (") else {
        return false;
    };
    let Some(index) = index.strip_suffix(')') else {
        return false;
    };
    if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let base = match path.extension().and_then(|value| value.to_str()) {
        Some(extension) => format!("{base_stem}.{extension}"),
        None => base_stem.to_string(),
    };
    is_chimera_installer_asset(&base)
}

fn publish_verified_temp(
    temp_path: &Path,
    download_dir: &Path,
    safe_name: &str,
    expected_sha256: &str,
    expected_size: u64,
) -> anyhow::Result<PathBuf> {
    publish_verified_temp_with_linker(
        temp_path,
        download_dir,
        safe_name,
        expected_sha256,
        expected_size,
        |source, destination| std::fs::hard_link(source, destination),
    )
}

fn publish_verified_temp_with_linker(
    temp_path: &Path,
    download_dir: &Path,
    safe_name: &str,
    expected_sha256: &str,
    expected_size: u64,
    link: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> anyhow::Result<PathBuf> {
    publish_verified_temp_with_io(
        temp_path,
        download_dir,
        safe_name,
        expected_sha256,
        expected_size,
        link,
        |source, destination| {
            let mut source = File::open(source)?;
            std::io::copy(&mut source, destination)?;
            Ok(())
        },
    )
}

fn publish_verified_temp_with_io(
    temp_path: &Path,
    download_dir: &Path,
    safe_name: &str,
    expected_sha256: &str,
    expected_size: u64,
    link: impl Fn(&Path, &Path) -> std::io::Result<()>,
    copy: impl Fn(&Path, &mut File) -> std::io::Result<()>,
) -> anyhow::Result<PathBuf> {
    for _ in 0..10_000 {
        let final_path = available_asset_path(download_dir, safe_name)?;
        match link(temp_path, &final_path) {
            Ok(()) => {
                return verify_newly_published_asset(&final_path, expected_sha256, expected_size);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(link_error) => {
                let (copy_path, mut copy_file) = create_download_temp(download_dir, safe_name)?;
                let copy_result = copy(temp_path, &mut copy_file)
                    .and_then(|()| copy_file.sync_all())
                    .map_err(anyhow::Error::from)
                    .and_then(|()| {
                        drop(copy_file);
                        verify_downloaded_file(&copy_path, expected_sha256, expected_size)
                    });
                if let Err(copy_error) = copy_result {
                    let cleanup = std::fs::remove_file(&copy_path);
                    return match cleanup {
                        Ok(()) => Err(anyhow::anyhow!(
                            "failed to publish update asset (hard-link: {link_error}; fallback copy: {copy_error:#})"
                        )),
                        Err(cleanup_error) => Err(anyhow::anyhow!(
                            "failed to publish update asset (hard-link: {link_error}; fallback copy: {copy_error:#}; cleanup: {cleanup_error})"
                        )),
                    };
                }
                match atomic_publish_noreplace(&copy_path, &final_path) {
                    Ok(()) => {
                        return verify_newly_published_asset(
                            &final_path,
                            expected_sha256,
                            expected_size,
                        );
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        std::fs::remove_file(&copy_path).with_context(|| {
                            format!(
                                "failed to remove fallback temp file {}",
                                copy_path.display()
                            )
                        })?;
                        continue;
                    }
                    Err(error) => {
                        let cleanup = std::fs::remove_file(&copy_path);
                        return match cleanup {
                            Ok(()) => Err(error.into()),
                            Err(cleanup_error) => Err(anyhow::anyhow!(
                                "fallback rename failed: {error}; cleanup failed: {cleanup_error}"
                            )),
                        };
                    }
                }
            }
        }
    }
    anyhow::bail!("无法为 Release asset 分配安全的下载文件名: {safe_name}")
}

#[cfg(windows)]
fn atomic_publish_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileW(existing: *const u16, new: *const u16) -> i32;
    }

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    if unsafe { MoveFileW(source.as_ptr(), destination.as_ptr()) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn atomic_publish_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt as _;

    unsafe extern "C" {
        fn renamex_np(
            from: *const std::ffi::c_char,
            to: *const std::ffi::c_char,
            flags: u32,
        ) -> std::ffi::c_int;
    }

    const RENAME_EXCL: u32 = 0x0000_0004;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    if unsafe { renamex_np(source.as_ptr(), destination.as_ptr(), RENAME_EXCL) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn atomic_publish_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt as _;

    unsafe extern "C" {
        fn renameat2(
            olddirfd: std::ffi::c_int,
            oldpath: *const std::ffi::c_char,
            newdirfd: std::ffi::c_int,
            newpath: *const std::ffi::c_char,
            flags: std::ffi::c_uint,
        ) -> std::ffi::c_int;
    }

    const AT_FDCWD: std::ffi::c_int = -100;
    const RENAME_NOREPLACE: std::ffi::c_uint = 1;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    if unsafe {
        renameat2(
            AT_FDCWD,
            source.as_ptr(),
            AT_FDCWD,
            destination.as_ptr(),
            RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
fn atomic_publish_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::hard_link(source, destination)?;
    std::fs::remove_file(source)
}

fn verify_newly_published_asset(
    final_path: &Path,
    expected_sha256: &str,
    expected_size: u64,
) -> anyhow::Result<PathBuf> {
    if let Err(error) = verify_downloaded_file(final_path, expected_sha256, expected_size) {
        let cleanup = std::fs::remove_file(final_path);
        return match cleanup {
            Ok(()) => Err(error.context("published asset failed final verification")),
            Err(cleanup_error) => Err(error.context(format!(
                "published asset failed final verification and cleanup failed: {cleanup_error}"
            ))),
        };
    }
    Ok(final_path.to_path_buf())
}

fn create_download_temp(download_dir: &Path, safe_name: &str) -> anyhow::Result<(PathBuf, File)> {
    for _ in 0..10_000 {
        let id = NEXT_DOWNLOAD_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let temp_name = format!(
            ".chimera-update.{safe_name}.{}.{}.part",
            std::process::id(),
            id
        );
        let temp_path = download_dir.join(temp_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("无法为 Release asset 创建独占临时文件: {safe_name}")
}

fn available_asset_path(download_dir: &Path, safe_name: &str) -> anyhow::Result<PathBuf> {
    for index in 0..=10_000 {
        let candidate = indexed_asset_path(download_dir, safe_name, index)?;
        match std::fs::symlink_metadata(&candidate) {
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("无法为 Release asset 分配安全的下载文件名: {safe_name}")
}

fn indexed_asset_path(download_dir: &Path, safe_name: &str, index: u32) -> anyhow::Result<PathBuf> {
    if index == 0 {
        return Ok(download_dir.join(safe_name));
    }
    let path = Path::new(safe_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("非法 Release asset 文件名: {safe_name}"))?;
    let extension = path.extension().and_then(|value| value.to_str());
    let candidate_name = match extension {
        Some(extension) => format!("{stem} ({index}).{extension}"),
        None => format!("{stem} ({index})"),
    };
    Ok(download_dir.join(candidate_name))
}

pub fn safe_asset_name(name: &str) -> anyhow::Result<String> {
    if name.trim().is_empty() {
        anyhow::bail!("非法 Release asset 文件名: {name}");
    }
    let path = Path::new(name);
    if path.components().count() != 1 {
        anyhow::bail!("非法 Release asset 文件名: {name}");
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("非法 Release asset 文件名: {name}"))?;
    if file_name == "." || file_name == ".." {
        anyhow::bail!("非法 Release asset 文件名: {name}");
    }
    if file_name.ends_with([' ', '.'])
        || file_name.chars().any(|character| {
            character.is_control() || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*')
        })
    {
        anyhow::bail!("非法 Release asset 文件名: {name}");
    }
    let reserved_stem = file_name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let numbered_device = {
        let mut characters = reserved_stem.chars();
        let prefix = characters.by_ref().take(3).collect::<String>();
        let suffix = characters.next();
        characters.next().is_none()
            && matches!(prefix.as_str(), "COM" | "LPT")
            && suffix.is_some_and(|digit| matches!(digit, '1'..='9' | '¹' | '²' | '³'))
    };
    if matches!(
        reserved_stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) || numbered_device
    {
        anyhow::bail!("非法 Release asset 文件名: {name}");
    }
    Ok(file_name.to_string())
}

fn platform_asset_rank(name: &str, target_os: &str, target_arch: &str) -> u8 {
    // 0 = exact platform and architecture, 2 = rejected.
    if target_os == "macos" {
        if !is_macos_installer_asset(name) {
            return 2;
        }
        if is_macos_native_arch_asset(name, target_arch) {
            return 0;
        }
        return 2;
    }
    if target_os == "windows" && target_arch == "x86_64" && is_windows_installer_asset(name) {
        return 0;
    }
    2
}

fn is_macos_native_arch_asset(name: &str, target_arch: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let native_arch_token = match target_arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => return false,
    };
    lower.ends_with(&format!("-macos-{native_arch_token}.dmg"))
}

fn is_windows_installer_asset(name: &str) -> bool {
    let prefix = format!("{}-", crate::branding::ARTIFACT_PREFIX);
    let lower = name.to_ascii_lowercase();
    name.starts_with(&prefix) && lower.ends_with("-windows-x64-setup.exe")
}

fn is_macos_installer_asset(name: &str) -> bool {
    let prefix = format!("{}-", crate::branding::ARTIFACT_PREFIX);
    let lower = name.to_ascii_lowercase();
    name.starts_with(&prefix)
        && (lower.ends_with("-macos-x64.dmg") || lower.ends_with("-macos-arm64.dmg"))
}

fn is_chimera_installer_asset(name: &str) -> bool {
    let prefix = format!("{}-", crate::branding::ARTIFACT_PREFIX);
    let lower = name.to_ascii_lowercase();
    name.starts_with(&prefix)
        && (lower.ends_with("-windows-x64-setup.exe")
            || lower.ends_with("-macos-x64.dmg")
            || lower.ends_with("-macos-arm64.dmg"))
}

fn launch_installer(
    path: &Path,
    launch_guard: &mut File,
    continuation_token: Option<&str>,
) -> anyhow::Result<InstallerLaunchPolicy> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = launch_guard;
        let mut policy = installer_launch_policy(UpdatePlatform::Windows);
        if let Some(token) = continuation_token {
            let parsed = uuid::Uuid::parse_str(token)
                .map_err(|_| anyhow::anyhow!("invalid update continuation token"))?;
            if parsed.to_string() != token {
                anyhow::bail!("invalid update continuation token");
            }
            policy
                .arguments
                .push(format!("/CONTINUATION_TOKEN={token}"));
        }
        std::process::Command::new(path)
            .args(&policy.arguments)
            .creation_flags(crate::windows_integration::CREATE_NO_WINDOW)
            .spawn()
            .map(|_| policy)
            .map_err(|error| anyhow::anyhow!("启动安装包失败：{error}"))
    }

    #[cfg(target_os = "macos")]
    {
        use std::io::{Seek as _, SeekFrom};
        use std::os::fd::AsRawFd as _;

        unsafe extern "C" {
            fn fcntl(fd: std::ffi::c_int, command: std::ffi::c_int, ...) -> std::ffi::c_int;
        }

        const F_GETFD: std::ffi::c_int = 1;
        const F_SETFD: std::ffi::c_int = 2;
        const FD_CLOEXEC: std::ffi::c_int = 1;
        let _ = (path, continuation_token);
        launch_guard.seek(SeekFrom::Start(0))?;
        let fd = launch_guard.as_raw_fd();
        let original_flags = unsafe { fcntl(fd, F_GETFD) };
        if original_flags < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if unsafe { fcntl(fd, F_SETFD, original_flags & !FD_CLOEXEC) } < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let fd_path = format!("/dev/fd/{fd}");
        let policy = installer_launch_policy(UpdatePlatform::Macos);
        let spawn_result = std::process::Command::new("/usr/bin/hdiutil")
            .args(["attach", "-autoopen"])
            .arg(fd_path)
            .status()
            .and_then(|status| {
                if status.success() {
                    Ok(policy.clone())
                } else {
                    Err(std::io::Error::other(format!(
                        "hdiutil exited with status {status}"
                    )))
                }
            })
            .map_err(|error| anyhow::anyhow!("打开 DMG 失败：{error}"));
        let restore_result = if unsafe { fcntl(fd, F_SETFD, original_flags) } < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        };
        match (spawn_result, restore_result) {
            (Ok(policy), Ok(())) => Ok(policy),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(installer_fd_restore_error(error)),
        }
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = (path, launch_guard, continuation_token);
        anyhow::bail!("当前平台不支持启动安装包")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_fd_restore_error_preserves_context_and_source() {
        let error = installer_fd_restore_error(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "restore denied",
        ));

        assert_eq!(error.to_string(), "failed to restore installer FD flags");
        assert!(format!("{error:#}").contains("restore denied"));
    }

    #[cfg(windows)]
    #[test]
    fn launch_guard_blocks_write_rename_and_delete_until_released() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"verified-launch-guard";
        let installer = dir.path().join("pkg.bin");
        std::fs::write(&installer, bytes).unwrap();
        let release = Release {
            version: "1.2.34-chimera.1".to_string(),
            minimum_supported_version: None,
            url: "https://example.test".to_string(),
            body: "fixes".to_string(),
            asset_name: Some("pkg.bin".to_string()),
            asset_url: Some("https://example.test/pkg.bin".to_string()),
            asset_sha256: Some(sha256_hex(bytes)),
            asset_size: Some(bytes.len() as u64),
        };

        let guard = open_validated_installer_for_launch(&release, &installer).unwrap();
        assert!(OpenOptions::new().write(true).open(&installer).is_err());
        assert!(std::fs::rename(&installer, dir.path().join("moved.bin")).is_err());
        assert!(std::fs::remove_file(&installer).is_err());
        drop(guard);

        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&installer)
            .unwrap();
    }

    #[test]
    fn publisher_falls_back_when_hard_links_are_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"verified-fallback";
        let temp = dir.path().join("temp.part");
        std::fs::write(&temp, bytes).unwrap();
        let hash = sha256_hex(bytes);

        let published = publish_verified_temp_with_linker(
            &temp,
            dir.path(),
            "pkg.bin",
            &hash,
            bytes.len() as u64,
            |_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "fixture does not support hard links",
                ))
            },
        )
        .unwrap();

        assert_eq!(std::fs::read(published).unwrap(), bytes);
    }

    #[test]
    fn publisher_removes_its_new_final_when_final_verification_fails() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"verified-before-publish";
        let temp = dir.path().join("temp.part");
        std::fs::write(&temp, bytes).unwrap();
        let hash = sha256_hex(bytes);

        let error = publish_verified_temp_with_linker(
            &temp,
            dir.path(),
            "pkg.bin",
            &hash,
            bytes.len() as u64,
            |_, destination| {
                std::fs::write(destination, b"corrupt-after-publish")?;
                Ok(())
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("final verification"));
        assert!(!dir.path().join("pkg.bin").exists());
    }

    #[test]
    fn fallback_publisher_never_clobbers_a_racing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"verified-fallback";
        let temp = dir.path().join("temp.part");
        std::fs::write(&temp, bytes).unwrap();
        let hash = sha256_hex(bytes);
        let injected = std::sync::atomic::AtomicBool::new(false);

        let published = publish_verified_temp_with_linker(
            &temp,
            dir.path(),
            "pkg.bin",
            &hash,
            bytes.len() as u64,
            |_, destination| {
                if !injected.swap(true, Ordering::SeqCst) {
                    std::fs::write(destination, b"external-winner")?;
                }
                Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "fixture does not support hard links",
                ))
            },
        )
        .unwrap();

        assert_eq!(
            std::fs::read(dir.path().join("pkg.bin")).unwrap(),
            b"external-winner"
        );
        assert_eq!(published, dir.path().join("pkg (1).bin"));
        assert_eq!(std::fs::read(published).unwrap(), bytes);
    }

    #[test]
    fn interrupted_fallback_copy_never_creates_a_final_asset() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"verified-before-copy";
        let temp = dir.path().join("temp.part");
        std::fs::write(&temp, bytes).unwrap();
        let hash = sha256_hex(bytes);

        let error = publish_verified_temp_with_io(
            &temp,
            dir.path(),
            "pkg.bin",
            &hash,
            bytes.len() as u64,
            |_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "fixture does not support hard links",
                ))
            },
            |_, destination| {
                destination.write_all(b"partial")?;
                Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "injected copy interruption",
                ))
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("copy"));
        assert!(!dir.path().join("pkg.bin").exists());
        assert!(
            std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| {
                    !entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".chimera-update.pkg.bin.")
                })
        );
    }
}
