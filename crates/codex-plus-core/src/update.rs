use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const DEFAULT_REPOSITORY: &str = crate::branding::REPOSITORY;
pub const DEFAULT_LATEST_JSON_URL: &str = crate::branding::LATEST_JSON_URL;

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
    pub release_summary: String,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub asset_sha256: Option<String>,
    pub asset_size: Option<u64>,
    pub update_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateInstall {
    pub release: Release,
    pub installer_path: PathBuf,
    pub launched: bool,
}

pub fn parse_version_tag(value: &str) -> anyhow::Result<semver::Version> {
    let normalized = value.trim().trim_start_matches(['v', 'V']);
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

pub fn release_from_latest_json_payload(payload: &Value) -> anyhow::Result<Release> {
    let version = payload
        .get("version")
        .or_else(|| payload.get("tag_name"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("latest.json missing version"))?
        .to_string();
    // Reject foreign/upstream channels before selecting assets.
    let _ = parse_version_tag(&version)?;

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
        let url = asset
            .get("url")
            .or_else(|| asset.get("browser_download_url"))
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("latest.json asset missing url"))?
            .to_string();
        let sha256 = asset
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("latest.json asset missing sha256"))?
            .trim()
            .to_ascii_lowercase();
        if sha256.is_empty() {
            anyhow::bail!("latest.json asset missing sha256");
        }
        let size = asset
            .get("size")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("latest.json asset missing size"))?;
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

pub fn select_update_asset(assets: &[ReleaseAsset]) -> Option<ReleaseAsset> {
    let named = assets.iter().filter(|asset| {
        !asset.name.trim().is_empty()
            && !asset.browser_download_url.trim().is_empty()
            && !asset.sha256.trim().is_empty()
    });
    let mut best: Option<(u8, &ReleaseAsset)> = None;
    for asset in named {
        let rank = platform_asset_rank(&asset.name);
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
    let client =
        crate::http_client::proxied_client(&format!("Codex++/{}", crate::version::VERSION))?;
    let payload = client
        .get(latest_json_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    release_from_latest_json_payload(&payload)
}

pub async fn check_for_update(current_version: &str) -> anyhow::Result<UpdateCheck> {
    let release = fetch_latest_release(DEFAULT_LATEST_JSON_URL).await?;
    let update_available = is_newer_version(&release.version, current_version)?;
    Ok(UpdateCheck {
        current_version: current_version.to_string(),
        latest_version: Some(release.version),
        release_summary: release.body,
        asset_name: release.asset_name,
        asset_url: release.asset_url,
        asset_sha256: release.asset_sha256,
        asset_size: release.asset_size,
        update_available,
    })
}

pub async fn perform_update(
    release: &Release,
    download_dir: &Path,
) -> anyhow::Result<UpdateInstall> {
    let url = release
        .asset_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("没有可下载的 Release asset"))?;
    let bytes =
        crate::http_client::proxied_client(&format!("Codex++/{}", crate::version::VERSION))?
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
    let installer_path = download_asset_to(release, &bytes, download_dir)?;
    launch_installer(&installer_path)?;
    Ok(UpdateInstall {
        release: release.clone(),
        installer_path,
        launched: true,
    })
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
    let name = release
        .asset_name
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("没有可下载的 Release asset"))?;
    let expected_sha256 = release
        .asset_sha256
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Release asset missing sha256"))?;
    let expected_size = release
        .asset_size
        .ok_or_else(|| anyhow::anyhow!("Release asset missing size"))?;

    let safe = safe_asset_name(name)?;
    std::fs::create_dir_all(download_dir)?;
    let final_path = download_dir.join(&safe);
    let temp_path = download_dir.join(format!("{safe}.part"));

    if let Err(error) = (|| -> anyhow::Result<()> {
        std::fs::write(&temp_path, bytes)?;
        verify_downloaded_bytes(bytes, expected_sha256, expected_size)?;
        std::fs::rename(&temp_path, &final_path)?;
        Ok(())
    })() {
        let _ = std::fs::remove_file(&temp_path);
        let _ = std::fs::remove_file(&final_path);
        return Err(error);
    }

    Ok(final_path)
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
    Ok(file_name.to_string())
}

fn platform_asset_rank(name: &str) -> u8 {
    // 0 = exact match (current OS + native arch)
    // 1 = same OS, other arch (acceptable fallback)
    // 2 = wrong platform / rejected shape
    if cfg!(target_os = "macos") {
        if !is_macos_installer_asset(name) {
            return 2;
        }
        if is_macos_native_arch_asset(name) {
            return 0;
        }
        return 1;
    }
    if cfg!(windows) && is_windows_installer_asset(name) {
        return 0;
    }
    2
}

fn is_macos_native_arch_asset(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let native_arch_token = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => return true,
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

pub fn launch_installer(path: &Path) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(path)
            .creation_flags(crate::windows_integration::CREATE_NO_WINDOW)
            .spawn()
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("启动安装包失败：{error}"))
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("打开 DMG 失败：{error}"))
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = path;
        anyhow::bail!("当前平台不支持启动安装包")
    }
}
