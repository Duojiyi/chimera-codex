use crate::settings::{RelayMode, RelayProfile, RelayProtocol, SettingsStore};
use anyhow::Context;
use fs2::FileExt as _;
use sha2::{Digest, Sha256};
use std::io::Read as _;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderImportRequest {
    #[serde(default)]
    pub request_id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default = "default_wire_api")]
    pub wire_api: String,
    #[serde(default = "default_relay_mode")]
    pub relay_mode: String,
    #[serde(default)]
    pub config_contents: String,
    #[serde(default)]
    pub auth_contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderImportResult {
    pub imported: bool,
    pub profile_id: String,
    pub profile_name: String,
}

pub fn import_provider_from_url(url: &str) -> anyhow::Result<ProviderImportResult> {
    let request = request_from_url(url)?;
    import_provider(request)
}

pub fn save_pending_provider_import_from_url(url: &str) -> anyhow::Result<ProviderImportRequest> {
    let request = prepare_pending_provider_import(&request_from_url(url)?);
    save_pending_provider_import(&request)?;
    Ok(request)
}

pub fn save_pending_provider_import(request: &ProviderImportRequest) -> anyhow::Result<()> {
    save_pending_provider_import_at(
        &crate::paths::default_pending_provider_import_path(),
        request,
    )
}

pub fn load_pending_provider_import() -> anyhow::Result<Option<ProviderImportRequest>> {
    load_pending_provider_import_at(&crate::paths::default_pending_provider_import_path())
}

pub fn clear_pending_provider_import(expected_request_id: &str) -> anyhow::Result<()> {
    clear_pending_provider_import_if_matches_at(
        &crate::paths::default_pending_provider_import_path(),
        expected_request_id,
    )
}

pub fn confirm_pending_provider_import(
    expected_request_id: &str,
) -> anyhow::Result<Option<ProviderImportResult>> {
    let path = crate::paths::default_pending_provider_import_path();
    if !path.exists() {
        return Ok(None);
    }
    confirm_pending_provider_import_at(&path, expected_request_id, SettingsStore::default())
        .map(Some)
}

pub fn save_pending_provider_import_at(
    path: &Path,
    request: &ProviderImportRequest,
) -> anyhow::Result<()> {
    with_pending_provider_import_lock(path, || {
        save_pending_provider_import_unlocked(path, request)
    })
}

fn save_pending_provider_import_unlocked(
    path: &Path,
    request: &ProviderImportRequest,
) -> anyhow::Result<()> {
    let request = prepare_pending_provider_import(request);
    let mut contents = serde_json::to_vec_pretty(&request)?;
    contents.push(b'\n');
    crate::settings::atomic_write(path, &contents)
        .with_context(|| format!("安全写入待确认供应商导入失败：{}", path.to_string_lossy()))
}

pub fn load_pending_provider_import_at(
    path: &Path,
) -> anyhow::Result<Option<ProviderImportRequest>> {
    with_pending_provider_import_lock(path, || load_pending_provider_import_unlocked(path))
}

fn load_pending_provider_import_unlocked(
    path: &Path,
) -> anyhow::Result<Option<ProviderImportRequest>> {
    let Some(mut file) = open_pending_file_nofollow(path)
        .with_context(|| format!("安全打开待确认供应商导入失败：{}", path.to_string_lossy()))?
    else {
        return Ok(None);
    };
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .with_context(|| format!("读取待确认供应商导入失败：{}", path.to_string_lossy()))?;
    let request: ProviderImportRequest =
        serde_json::from_str(&contents).context("待确认供应商导入内容无效")?;
    let prepared = prepare_pending_provider_import(&request);
    if prepared != request {
        save_pending_provider_import_unlocked(path, &prepared)?;
    }
    Ok(Some(prepared))
}

pub fn clear_pending_provider_import_at(path: &Path) -> anyhow::Result<()> {
    with_pending_provider_import_lock(path, || clear_pending_provider_import_unlocked(path))
}

fn clear_pending_provider_import_unlocked(path: &Path) -> anyhow::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("清理待确认供应商导入失败：{}", path.to_string_lossy())),
    }
}

pub fn clear_pending_provider_import_if_matches_at(
    path: &Path,
    expected_request_id: &str,
) -> anyhow::Result<()> {
    with_pending_provider_import_lock(path, || {
        let request =
            load_pending_provider_import_unlocked(path)?.context("没有待确认的供应商导入")?;
        ensure_pending_request_matches(&request, expected_request_id)?;
        clear_pending_provider_import_unlocked(path)
    })
}

pub fn confirm_pending_provider_import_at(
    path: &Path,
    expected_request_id: &str,
    store: SettingsStore,
) -> anyhow::Result<ProviderImportResult> {
    with_pending_provider_import_lock(path, || {
        let request =
            load_pending_provider_import_unlocked(path)?.context("没有待确认的供应商导入")?;
        ensure_pending_request_matches(&request, expected_request_id)?;
        let result = import_provider_with_store(request, store)?;
        clear_pending_provider_import_unlocked(path)?;
        Ok(result)
    })
}

fn prepare_pending_provider_import(request: &ProviderImportRequest) -> ProviderImportRequest {
    let mut request = request.clone();
    // 未展示的文件正文不进入待确认边界，确认后仅按可见 URL 与 Key 重新生成。
    request.config_contents.clear();
    request.auth_contents.clear();
    request.request_id = pending_provider_import_content_id(&request);
    request
}

fn ensure_pending_request_matches(
    request: &ProviderImportRequest,
    expected_request_id: &str,
) -> anyhow::Result<()> {
    let content_id = pending_provider_import_content_id(request);
    if expected_request_id.trim().is_empty()
        || request.request_id != expected_request_id
        || request.request_id != content_id
    {
        anyhow::bail!("待确认供应商导入已变化，请重新检查后再确认")
    }
    Ok(())
}

fn pending_provider_import_content_id(request: &ProviderImportRequest) -> String {
    let mut digest = Sha256::new();
    digest.update(b"chimera-provider-import-v1\0");
    for value in [
        request.name.as_bytes(),
        request.base_url.as_bytes(),
        request.api_key.as_bytes(),
        request.wire_api.as_bytes(),
        request.relay_mode.as_bytes(),
    ] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value);
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn pending_provider_import_lock_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| "pending-provider-import.json".into());
    path.with_file_name(format!("{file_name}.lock"))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const O_NOFOLLOW: i32 = 0x20000;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
const O_NOFOLLOW: i32 = 0x0100;
#[cfg(any(target_os = "linux", target_os = "android"))]
const O_NONBLOCK: i32 = 0x0800;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
const O_NONBLOCK: i32 = 0x0004;
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

fn configure_pending_nofollow(options: &mut std::fs::OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(O_NOFOLLOW | O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
}

fn ensure_opened_pending_file_safe(file: &std::fs::File, path: &Path) -> anyhow::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        anyhow::bail!("待确认供应商导入路径不是普通文件：{}", path.display());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            anyhow::bail!("待确认供应商导入不能是 reparse point：{}", path.display());
        }
    }
    Ok(())
}

fn open_pending_file_nofollow(path: &Path) -> anyhow::Result<Option<std::fs::File>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    configure_pending_nofollow(&mut options);
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure_opened_pending_file_safe(&file, path)?;
    Ok(Some(file))
}

fn open_pending_lock_nofollow(path: &Path) -> anyhow::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).read(true).write(true);
    configure_pending_nofollow(&mut options);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    ensure_opened_pending_file_safe(&file, path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingFileIdentity {
    volume: u64,
    index: u64,
    links: u64,
}

#[cfg(unix)]
fn pending_open_file_identity(file: &std::fs::File) -> std::io::Result<PendingFileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata()?;
    Ok(PendingFileIdentity {
        volume: metadata.dev(),
        index: metadata.ino(),
        links: metadata.nlink(),
    })
}

#[cfg(windows)]
fn pending_open_file_identity(file: &std::fs::File) -> std::io::Result<PendingFileIdentity> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut info = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let handle = HANDLE(file.as_raw_handle());
    unsafe { GetFileInformationByHandle(handle, info.as_mut_ptr()) }
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let info = unsafe { info.assume_init() };
    Ok(PendingFileIdentity {
        volume: u64::from(info.dwVolumeSerialNumber),
        index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        links: u64::from(info.nNumberOfLinks),
    })
}

fn ensure_pending_lock_path_matches(
    locked_file: &std::fs::File,
    lock_path: &Path,
) -> anyhow::Result<()> {
    let current_file = open_pending_file_nofollow(lock_path)?
        .with_context(|| format!("待确认供应商导入锁路径已消失：{}", lock_path.display()))?;
    let locked_identity = pending_open_file_identity(locked_file)?;
    let current_identity = pending_open_file_identity(&current_file)?;
    if locked_identity.links != 1
        || current_identity.links != 1
        || locked_identity.volume != current_identity.volume
        || locked_identity.index != current_identity.index
    {
        anyhow::bail!("待确认供应商导入锁路径已被替换：{}", lock_path.display());
    }
    Ok(())
}

fn with_pending_provider_import_lock<T>(
    path: &Path,
    operation: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    with_pending_provider_import_lock_inner(path, |_| {}, operation)
}

fn with_pending_provider_import_lock_inner<T>(
    path: &Path,
    after_lock_open: impl FnOnce(&Path),
    operation: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // 固定锁文件必须跨操作保留；删除后重建会让等待进程锁住不同 inode，破坏互斥。
    let lock_path = pending_provider_import_lock_path(path);
    let lock_file = open_pending_lock_nofollow(&lock_path).with_context(|| {
        format!(
            "打开待确认供应商导入锁失败：{}",
            lock_path.to_string_lossy()
        )
    })?;
    after_lock_open(&lock_path);
    lock_file
        .lock_exclusive()
        .with_context(|| format!("锁定待确认供应商导入失败：{}", lock_path.to_string_lossy()))?;
    // 文件锁协调正常应用进程；此复核同时拒绝 open 到加锁期间发生的路径换 inode。
    ensure_pending_lock_path_matches(&lock_file, &lock_path)?;
    lock_file.set_len(0).with_context(|| {
        format!(
            "清空待确认供应商导入锁失败：{}",
            lock_path.to_string_lossy()
        )
    })?;
    lock_file.sync_all().with_context(|| {
        format!(
            "同步待确认供应商导入锁失败：{}",
            lock_path.to_string_lossy()
        )
    })?;
    let result = operation();
    let unlock_result = fs2::FileExt::unlock(&lock_file)
        .with_context(|| format!("解锁待确认供应商导入失败：{}", lock_path.to_string_lossy()));
    match (result, unlock_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(unlock_error)) => Err(unlock_error),
        (Err(error), Err(unlock_error)) => {
            Err(error.context(format!("同时无法释放待确认供应商导入锁：{unlock_error:#}")))
        }
    }
}

pub fn import_provider(request: ProviderImportRequest) -> anyhow::Result<ProviderImportResult> {
    import_provider_with_store(request, SettingsStore::default())
}

pub fn import_provider_with_store(
    request: ProviderImportRequest,
    store: SettingsStore,
) -> anyhow::Result<ProviderImportResult> {
    let request = normalize_request(request)?;
    let mut settings = store.load().unwrap_or_default();
    let identity = provider_identity(&request.name, &request.base_url);
    if let Some(existing) = settings
        .relay_profiles
        .iter()
        .find(|profile| provider_identity(&profile.name, &profile.upstream_base_url) == identity)
    {
        return Ok(ProviderImportResult {
            imported: false,
            profile_id: existing.id.clone(),
            profile_name: existing.name.clone(),
        });
    }

    let existing_ids = settings
        .relay_profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect::<Vec<_>>();
    let profile = relay_profile_from_request(&request, &existing_ids);
    let result = ProviderImportResult {
        imported: true,
        profile_id: profile.id.clone(),
        profile_name: profile.name.clone(),
    };
    settings.relay_profiles.push(profile);
    settings.active_relay_id = result.profile_id.clone();
    store.save(&settings)?;
    Ok(result)
}

pub fn request_from_url(url: &str) -> anyhow::Result<ProviderImportRequest> {
    let (_, query) = url.split_once('?').context("导入链接缺少查询参数")?;
    let mut values = std::collections::BTreeMap::<String, String>::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        values.insert(percent_decode(key), percent_decode(value));
    }
    Ok(ProviderImportRequest {
        request_id: String::new(),
        name: required_value(&values, "name")?,
        base_url: required_value(&values, "baseUrl")?,
        api_key: required_value(&values, "apiKey")?,
        wire_api: values
            .get("wireApi")
            .cloned()
            .unwrap_or_else(default_wire_api),
        relay_mode: values
            .get("relayMode")
            .cloned()
            .unwrap_or_else(default_relay_mode),
        // 供应商链接中的隐藏文件内容不能覆盖确认框展示的 URL 与 Key。
        // 导入时仅依据可见字段生成规范文件。
        config_contents: String::new(),
        auth_contents: String::new(),
    })
}

fn relay_profile_from_request(
    request: &ProviderImportRequest,
    existing_ids: &[String],
) -> RelayProfile {
    RelayProfile {
        id: unique_profile_id(
            &format!("import-{}", sanitize_id(&request.name)),
            existing_ids,
        ),
        name: request.name.clone(),
        model: String::new(),
        base_url: request.base_url.clone(),
        upstream_base_url: request.base_url.clone(),
        api_key: request.api_key.clone(),
        protocol: relay_protocol(&request.wire_api),
        relay_mode: relay_mode(&request.relay_mode),
        official_mix_api_key: false,
        test_model: String::new(),
        config_contents: request.config_contents.clone(),
        auth_contents: request.auth_contents.clone(),
        use_common_config: true,
        context_selection: crate::settings::RelayContextSelection::default(),
        context_selection_initialized: false,
        context_window: String::new(),
        auto_compact_limit: String::new(),
        model_insert_mode: Default::default(),
        model_list: String::new(),
        model_windows: String::new(),
        user_agent: String::new(),
    }
}

fn normalize_request(mut request: ProviderImportRequest) -> anyhow::Result<ProviderImportRequest> {
    request.name = request.name.trim().to_string();
    request.base_url = request.base_url.trim().trim_end_matches('/').to_string();
    request.api_key = request.api_key.trim().to_string();
    request.wire_api = request.wire_api.trim().to_ascii_lowercase();
    request.relay_mode = request.relay_mode.trim().to_ascii_lowercase();
    if request.name.is_empty() {
        anyhow::bail!("供应商名称为空");
    }
    if request.base_url.is_empty() {
        anyhow::bail!("Base URL 为空");
    }
    if request.api_key.is_empty() {
        anyhow::bail!("API Key 为空");
    }
    request.config_contents = build_config_toml(
        &request.base_url,
        &request.api_key,
        relay_protocol(&request.wire_api),
    );
    request.auth_contents = build_auth_json(&request.api_key);
    Ok(request)
}

fn relay_protocol(value: &str) -> RelayProtocol {
    match value.trim().to_ascii_lowercase().as_str() {
        "chat" | "chat_completions" | "chat-completions" => RelayProtocol::ChatCompletions,
        _ => RelayProtocol::Responses,
    }
}

fn relay_mode(value: &str) -> RelayMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "official" => RelayMode::Official,
        "mixedapi" | "mixed-api" | "mixed_api" => RelayMode::MixedApi,
        "aggregate" => RelayMode::Aggregate,
        _ => RelayMode::PureApi,
    }
}

fn build_config_toml(base_url: &str, api_key: &str, protocol: RelayProtocol) -> String {
    let wire_api = match protocol {
        RelayProtocol::Responses => "responses",
        RelayProtocol::ChatCompletions => "chat",
    };
    [
        "model_provider = \"CodexPlusPlus\"".to_string(),
        String::new(),
        "[model_providers.CodexPlusPlus]".to_string(),
        "name = \"CodexPlusPlus\"".to_string(),
        format!("wire_api = \"{wire_api}\""),
        "requires_openai_auth = true".to_string(),
        format!("base_url = \"{}\"", toml_string(base_url)),
        format!("experimental_bearer_token = \"{}\"", toml_string(api_key)),
        String::new(),
    ]
    .join("\n")
}

fn build_auth_json(api_key: &str) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(&serde_json::json!({ "OPENAI_API_KEY": api_key }))
            .unwrap_or_else(|_| "{\"OPENAI_API_KEY\":\"\"}".to_string())
    )
}

fn required_value(
    values: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> anyhow::Result<String> {
    values
        .get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .with_context(|| format!("导入链接缺少 {key}"))
}

fn percent_decode(value: &str) -> String {
    let value = value.replace('+', " ");
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&value[index + 1..index + 3], 16) {
                output.push(hex);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).to_string()
}

fn provider_identity(name: &str, base_url: &str) -> String {
    format!(
        "{}\n{}",
        name.trim().to_ascii_lowercase(),
        base_url.trim().trim_end_matches('/').to_ascii_lowercase()
    )
}

fn sanitize_id(value: &str) -> String {
    let mut result = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
        } else if !result.ends_with('-') {
            result.push('-');
        }
    }
    let result = result.trim_matches('-').to_string();
    if result.is_empty() {
        "provider".to_string()
    } else {
        result
    }
}

fn unique_profile_id(base: &str, existing_ids: &[String]) -> String {
    if !existing_ids.iter().any(|id| id == base) {
        return base.to_string();
    }
    let mut index = 2;
    loop {
        let candidate = format!("{base}-{index}");
        if !existing_ids.iter().any(|id| id == &candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn default_wire_api() -> String {
    "responses".to_string()
}

fn default_relay_mode() -> String {
    "pureApi".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codexplusplus_provider_url() {
        let url = "codexplusplus://v1/import/provider?resource=provider&name=JOJO%20Code&baseUrl=https%3A%2F%2Fjojocode.com%2Fv1&apiKey=sk-test&wireApi=responses&relayMode=pureApi&configContents=bW9kZWxfcHJvdmlkZXIgPSAiQ29kZXhQbHVzUGx1cyIK&authContents=eyJPUEVOQUlfQVBJX0tFWSI6InNrLXRlc3QifQo%3D";

        let request = request_from_url(url).unwrap();

        assert_eq!(request.name, "JOJO Code");
        assert_eq!(request.base_url, "https://jojocode.com/v1");
        assert_eq!(request.api_key, "sk-test");
        assert_eq!(request.wire_api, "responses");
        assert_eq!(request.relay_mode, "pureApi");
        assert!(
            request.config_contents.is_empty(),
            "hidden configContents must not override the reviewed URL and key"
        );
        assert!(
            request.auth_contents.is_empty(),
            "hidden authContents must not override the reviewed key"
        );
    }

    #[test]
    fn imports_provider_once_and_selects_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        let request = ProviderImportRequest {
            request_id: String::new(),
            name: "JOJO Code".to_string(),
            base_url: "https://jojocode.com/v1/".to_string(),
            api_key: "sk-test".to_string(),
            wire_api: "responses".to_string(),
            relay_mode: "pureApi".to_string(),
            config_contents: String::new(),
            auth_contents: String::new(),
        };

        let first = import_provider_with_store(request.clone(), store.clone()).unwrap();
        let second = import_provider_with_store(request, store.clone()).unwrap();
        let settings = store.load().unwrap();

        assert!(first.imported);
        assert!(!second.imported);
        assert_eq!(first.profile_id, second.profile_id);
        assert_eq!(settings.active_relay_id, first.profile_id);
        assert_eq!(settings.relay_profiles.len(), 2);
        assert_eq!(
            settings.relay_profiles[1].protocol,
            RelayProtocol::Responses
        );
        assert_eq!(settings.relay_profiles[1].relay_mode, RelayMode::PureApi);
        assert_eq!(
            settings.relay_profiles[1].upstream_base_url,
            "https://jojocode.com/v1"
        );
    }

    #[test]
    fn hidden_provider_files_cannot_override_reviewed_url_and_key() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        let request = ProviderImportRequest {
            request_id: String::new(),
            name: "Reviewed Provider".to_string(),
            base_url: "https://reviewed.example/v1".to_string(),
            api_key: "reviewed-key".to_string(),
            wire_api: "responses".to_string(),
            relay_mode: "pureApi".to_string(),
            config_contents:
                "base_url = \"https://hidden-attacker.example/v1\"\nexperimental_bearer_token = \"hidden-key\"\n"
                    .to_string(),
            auth_contents: "{\"OPENAI_API_KEY\":\"hidden-key\"}\n".to_string(),
        };

        import_provider_with_store(request, store.clone()).unwrap();
        let settings = store.load().unwrap();
        let profile = settings
            .relay_profiles
            .iter()
            .find(|profile| profile.name == "Reviewed Provider")
            .unwrap();

        assert_eq!(profile.upstream_base_url, "https://reviewed.example/v1");
        assert!(
            profile
                .config_contents
                .contains("https://reviewed.example/v1")
        );
        assert!(!profile.config_contents.contains("hidden-attacker"));
        assert!(!profile.config_contents.contains("hidden-key"));
        assert!(profile.auth_contents.contains("reviewed-key"));
        assert!(!profile.auth_contents.contains("hidden-key"));
    }

    #[test]
    fn pending_provider_import_round_trips_and_clears() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pending-provider-import.json");
        let lock_path = pending_provider_import_lock_path(&path);
        std::fs::write(&lock_path, b"legacy-lock-bytes").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&lock_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        let request = ProviderImportRequest {
            request_id: String::new(),
            name: "JOJO Code".to_string(),
            base_url: "https://jojocode.com/v1".to_string(),
            api_key: "sk-test".to_string(),
            wire_api: "responses".to_string(),
            relay_mode: "pureApi".to_string(),
            config_contents: String::new(),
            auth_contents: String::new(),
        };

        save_pending_provider_import_at(&path, &request).unwrap();
        let source = include_str!("provider_import.rs");
        let save_helper = source
            .split("pub fn save_pending_provider_import_at")
            .nth(1)
            .and_then(|value| {
                value
                    .split("\npub fn load_pending_provider_import_at")
                    .next()
            })
            .expect("pending provider import writer");
        assert!(save_helper.contains("crate::settings::atomic_write"));
        assert!(!save_helper.contains("std::fs::write"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt, symlink};

            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            save_pending_provider_import_at(&path, &request).unwrap();
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );

            let outside = dir.path().join("outside-secret");
            std::fs::write(&outside, b"outside-must-stay").unwrap();
            std::fs::remove_file(&path).unwrap();
            symlink(&outside, &path).unwrap();
            save_pending_provider_import_at(&path, &request).unwrap();
            assert_eq!(std::fs::read(&outside).unwrap(), b"outside-must-stay");
            assert!(
                !std::fs::symlink_metadata(&path)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
        }

        let pending = load_pending_provider_import_at(&path).unwrap().unwrap();
        clear_pending_provider_import_at(&path).unwrap();

        assert!(!pending.request_id.is_empty());
        assert_eq!(pending.name, "JOJO Code");
        assert_eq!(pending.base_url, "https://jojocode.com/v1");
        assert!(pending.config_contents.is_empty());
        assert!(pending.auth_contents.is_empty());
        assert!(source.contains("lock_exclusive()"));
        assert!(load_pending_provider_import_at(&path).unwrap().is_none());
        let lock_metadata = std::fs::metadata(&lock_path).unwrap();
        assert!(lock_metadata.is_file());
        assert_eq!(lock_metadata.len(), 0, "持久锁文件不得保存 pending 内容");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(lock_metadata.permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn pending_reader_and_lock_use_open_time_nofollow_guards() {
        let source = include_str!("provider_import.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        assert!(production.contains("open_pending_file_nofollow"));
        assert!(production.contains("open_pending_lock_nofollow"));
        assert!(production.contains("FILE_FLAG_OPEN_REPARSE_POINT"));
        assert!(production.contains("O_NOFOLLOW"));
        assert!(production.contains("O_NONBLOCK"));
        assert!(!production.contains("std::fs::read_to_string(path)"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let dir = tempfile::tempdir().unwrap();
            let outside = dir.path().join("outside.json");
            let pending_path = dir.path().join("pending-provider-import.json");
            std::fs::write(
                &outside,
                r#"{"requestId":"reviewed","name":"Outside","baseUrl":"https://outside.example/v1","apiKey":"sk-outside"}"#,
            )
            .unwrap();
            symlink(&outside, &pending_path).unwrap();

            assert!(load_pending_provider_import_at(&pending_path).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn pending_reader_rejects_fifo_without_blocking() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        unsafe extern "C" {
            fn mkfifo(pathname: *const std::ffi::c_char, mode: u32) -> i32;
        }

        let dir = tempfile::tempdir().unwrap();
        let pending_path = dir.path().join("pending-provider-import.json");
        let c_path = CString::new(pending_path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { mkfifo(c_path.as_ptr(), 0o600) }, 0);

        let started = std::time::Instant::now();
        assert!(load_pending_provider_import_at(&pending_path).is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn pending_lock_rejects_a_path_replaced_after_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pending-provider-import.json");
        let displaced_lock = dir.path().join("displaced.lock");
        let operation_ran = std::cell::Cell::new(false);

        let result = with_pending_provider_import_lock_inner(
            &path,
            |lock_path| {
                std::fs::rename(lock_path, &displaced_lock).unwrap();
                std::fs::write(lock_path, b"replacement-lock").unwrap();
            },
            || {
                operation_ran.set(true);
                Ok(())
            },
        );

        assert!(result.is_err());
        assert!(!operation_ran.get());
        assert_eq!(std::fs::read(displaced_lock).unwrap(), b"");
    }

    #[test]
    fn confirms_pending_provider_import_and_removes_pending_file() {
        let dir = tempfile::tempdir().unwrap();
        let pending_path = dir.path().join("pending-provider-import.json");
        let settings_path = dir.path().join("settings.json");
        let store = SettingsStore::new(settings_path.clone());
        save_pending_provider_import_at(
            &pending_path,
            &ProviderImportRequest {
                request_id: String::new(),
                name: "JOJO Code".to_string(),
                base_url: "https://jojocode.com/v1".to_string(),
                api_key: "sk-test".to_string(),
                wire_api: "responses".to_string(),
                relay_mode: "pureApi".to_string(),
                config_contents: String::new(),
                auth_contents: String::new(),
            },
        )
        .unwrap();
        let pending = load_pending_provider_import_at(&pending_path)
            .unwrap()
            .unwrap();

        let result =
            confirm_pending_provider_import_at(&pending_path, &pending.request_id, store.clone())
                .unwrap();
        let settings = store.load().unwrap();

        assert!(result.imported);
        assert_eq!(settings.relay_profiles.len(), 2);
        assert!(
            load_pending_provider_import_at(&pending_path)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn confirmation_never_imports_a_request_that_replaced_the_reviewed_one() {
        let dir = tempfile::tempdir().unwrap();
        let pending_path = dir.path().join("pending-provider-import.json");
        let store = SettingsStore::new(dir.path().join("settings.json"));
        let reviewed = ProviderImportRequest {
            request_id: String::new(),
            name: "Reviewed A".to_string(),
            base_url: "https://reviewed-a.example/v1".to_string(),
            api_key: "reviewed-a-key".to_string(),
            wire_api: "responses".to_string(),
            relay_mode: "pureApi".to_string(),
            config_contents: String::new(),
            auth_contents: String::new(),
        };
        let replacement = ProviderImportRequest {
            request_id: String::new(),
            name: "Replacement B".to_string(),
            base_url: "https://replacement-b.example/v1".to_string(),
            api_key: "replacement-b-key".to_string(),
            wire_api: "responses".to_string(),
            relay_mode: "pureApi".to_string(),
            config_contents: String::new(),
            auth_contents: String::new(),
        };

        save_pending_provider_import_at(&pending_path, &reviewed).unwrap();
        let displayed = load_pending_provider_import_at(&pending_path)
            .unwrap()
            .unwrap();
        save_pending_provider_import_at(&pending_path, &replacement).unwrap();

        let error =
            confirm_pending_provider_import_at(&pending_path, &displayed.request_id, store.clone())
                .unwrap_err();
        let pending = load_pending_provider_import_at(&pending_path)
            .unwrap()
            .unwrap();
        let settings = store.load().unwrap();

        assert!(error.to_string().contains("已变化"));
        assert_eq!(pending.name, "Replacement B");
        assert!(
            settings
                .relay_profiles
                .iter()
                .all(|profile| profile.name != "Reviewed A" && profile.name != "Replacement B")
        );
    }

    #[test]
    fn stale_dismiss_never_clears_a_replacement_request() {
        let dir = tempfile::tempdir().unwrap();
        let pending_path = dir.path().join("pending-provider-import.json");
        let reviewed = ProviderImportRequest {
            request_id: String::new(),
            name: "Reviewed A".to_string(),
            base_url: "https://reviewed-a.example/v1".to_string(),
            api_key: "reviewed-a-key".to_string(),
            wire_api: "responses".to_string(),
            relay_mode: "pureApi".to_string(),
            config_contents: String::new(),
            auth_contents: String::new(),
        };
        let replacement = ProviderImportRequest {
            request_id: String::new(),
            name: "Replacement B".to_string(),
            base_url: "https://replacement-b.example/v1".to_string(),
            api_key: "replacement-b-key".to_string(),
            wire_api: "responses".to_string(),
            relay_mode: "pureApi".to_string(),
            config_contents: String::new(),
            auth_contents: String::new(),
        };

        save_pending_provider_import_at(&pending_path, &reviewed).unwrap();
        let displayed = load_pending_provider_import_at(&pending_path)
            .unwrap()
            .unwrap();
        save_pending_provider_import_at(&pending_path, &replacement).unwrap();

        let error =
            clear_pending_provider_import_if_matches_at(&pending_path, &displayed.request_id)
                .unwrap_err();
        let pending = load_pending_provider_import_at(&pending_path)
            .unwrap()
            .unwrap();

        assert!(error.to_string().contains("已变化"));
        assert_eq!(pending.name, "Replacement B");
    }

    #[test]
    fn confirmation_rejects_changed_fields_that_reuse_the_reviewed_request_id() {
        let dir = tempfile::tempdir().unwrap();
        let pending_path = dir.path().join("pending-provider-import.json");
        let settings_path = dir.path().join("settings.json");
        let store = SettingsStore::new(settings_path.clone());
        save_pending_provider_import_at(
            &pending_path,
            &ProviderImportRequest {
                request_id: String::new(),
                name: "Reviewed A".to_string(),
                base_url: "https://reviewed-a.example/v1".to_string(),
                api_key: "sk-reviewed-a".to_string(),
                wire_api: "responses".to_string(),
                relay_mode: "pureApi".to_string(),
                config_contents: String::new(),
                auth_contents: String::new(),
            },
        )
        .unwrap();
        let displayed = load_pending_provider_import_at(&pending_path)
            .unwrap()
            .unwrap();
        let replacement = ProviderImportRequest {
            request_id: displayed.request_id.clone(),
            name: "Attacker B".to_string(),
            base_url: "https://attacker-b.example/v1".to_string(),
            api_key: "sk-attacker-b".to_string(),
            wire_api: "responses".to_string(),
            relay_mode: "pureApi".to_string(),
            config_contents: String::new(),
            auth_contents: String::new(),
        };
        let mut bytes = serde_json::to_vec_pretty(&replacement).unwrap();
        bytes.push(b'\n');
        crate::settings::atomic_write(&pending_path, &bytes).unwrap();

        assert!(
            confirm_pending_provider_import_at(&pending_path, &displayed.request_id, store.clone())
                .is_err()
        );
        assert!(!settings_path.exists());
    }
}
