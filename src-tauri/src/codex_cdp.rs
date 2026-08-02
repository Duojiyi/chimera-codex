use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tungstenite::{client::client, Message, WebSocket};
use url::Url;

pub const CODEX_RENDERER_DEBUG_PORT: u16 = 9229;

const TARGET_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const TARGET_POLL_INTERVAL: Duration = Duration::from_millis(250);
const IO_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_HTTP_RESPONSE_BYTES: usize = 512 * 1024;
const MODEL_UNLOCK_SCRIPT: &str = include_str!("resources/codex_model_unlock.js");
const MODEL_UNLOCK_CONFIG_TOKEN: &str = "__CHIMERA_CODEX_MODEL_UNLOCK_CONFIG__";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexModelUnlockStatus {
    pub attempted: bool,
    pub injected: bool,
    pub model_count: usize,
    pub error: Option<String>,
}

impl CodexModelUnlockStatus {
    pub fn not_configured() -> Self {
        Self {
            attempted: false,
            injected: false,
            model_count: 0,
            error: None,
        }
    }

    fn failed(model_count: usize, error: impl Into<String>) -> Self {
        Self {
            attempted: true,
            injected: false,
            model_count,
            error: Some(error.into()),
        }
    }

    fn injected(model_count: usize) -> Self {
        Self {
            attempted: true,
            injected: true,
            model_count,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CodexRendererModel {
    model: String,
    display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CodexRendererModelUnlockConfig {
    default_model: String,
    models: Vec<CodexRendererModel>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct CdpTarget {
    #[serde(default)]
    id: String,
    #[serde(rename = "type")]
    target_type: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default, rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: Option<String>,
}

/// Inject the renderer-only model visibility patch into a newly launched
/// portable Codex instance. Failures are returned as diagnostics so a picker
/// compatibility patch never turns a successful Codex launch into a failure.
pub fn inject_codex_model_unlock(debug_port: u16) -> CodexModelUnlockStatus {
    let payload = match load_model_unlock_config() {
        Ok(Some(payload)) => payload,
        Ok(None) => return CodexModelUnlockStatus::not_configured(),
        Err(error) => return CodexModelUnlockStatus::failed(0, error),
    };
    let model_count = payload.models.len();
    let script = match build_model_unlock_script(&payload) {
        Ok(script) => script,
        Err(error) => return CodexModelUnlockStatus::failed(model_count, error),
    };

    let deadline = Instant::now() + TARGET_WAIT_TIMEOUT;
    let mut last_error = "Codex renderer 的 CDP 页面尚未出现".to_string();
    while Instant::now() < deadline {
        match list_targets(debug_port)
            .and_then(|targets| pick_codex_page_target(&targets))
            .and_then(|target| {
                let websocket_url = target
                    .web_socket_debugger_url
                    .as_deref()
                    .ok_or_else(|| "Codex renderer target 缺少 WebSocket 地址".to_string())?;
                validate_cdp_websocket_url(websocket_url, debug_port)?;
                inject_script(websocket_url, debug_port, &script)
            }) {
            Ok(()) => return CodexModelUnlockStatus::injected(model_count),
            Err(error) => last_error = error,
        }
        std::thread::sleep(TARGET_POLL_INTERVAL);
    }

    CodexModelUnlockStatus::failed(
        model_count,
        format!("Codex 已启动，但第三方模型注入未完成：{last_error}"),
    )
}

/// Produce a diagnostic for launch modes where Chimera++ cannot attach a local
/// renderer debugger. If there is no custom catalog, no warning is emitted.
pub fn unavailable_model_unlock(reason: impl Into<String>) -> CodexModelUnlockStatus {
    match load_model_unlock_config() {
        Ok(Some(payload)) => CodexModelUnlockStatus::failed(payload.models.len(), reason),
        Ok(None) => CodexModelUnlockStatus::not_configured(),
        Err(error) => CodexModelUnlockStatus::failed(0, error),
    }
}

fn load_model_unlock_config() -> Result<Option<CodexRendererModelUnlockConfig>, String> {
    let Some(catalog) = crate::codex_config::read_codex_model_catalog_simplified_from_live()
        .map_err(|error| format!("无法读取 Chimera 模型目录：{error}"))?
    else {
        return Ok(None);
    };
    build_model_unlock_config(
        &catalog,
        &crate::codex_config::read_codex_config_text().ok(),
    )
}

fn build_model_unlock_config(
    catalog: &Value,
    config_text: &Option<String>,
) -> Result<Option<CodexRendererModelUnlockConfig>, String> {
    let Some(entries) = catalog.get("models").and_then(Value::as_array) else {
        return Ok(None);
    };

    let mut seen = HashSet::new();
    let mut models = Vec::new();
    for entry in entries {
        let Some(model) = entry
            .get("model")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|model| !model.is_empty())
        else {
            continue;
        };
        if !seen.insert(model.to_string()) {
            continue;
        }
        let display_name = entry
            .get("displayName")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(model)
            .to_string();
        let description = entry
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let default_reasoning_effort = entry
            .get("defaultReasoningEffort")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        models.push(CodexRendererModel {
            model: model.to_string(),
            display_name,
            description,
            default_reasoning_effort,
        });
    }

    if models.is_empty() {
        return Ok(None);
    }

    let configured_default = config_text
        .as_deref()
        .and_then(|text| text.parse::<toml_edit::DocumentMut>().ok())
        .and_then(|document| {
            document
                .get("model")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|model| !model.is_empty())
                .map(ToOwned::to_owned)
        });
    let default_model = configured_default
        .filter(|candidate| models.iter().any(|entry| entry.model == *candidate))
        .unwrap_or_else(|| models[0].model.clone());

    Ok(Some(CodexRendererModelUnlockConfig {
        default_model,
        models,
    }))
}

fn build_model_unlock_script(payload: &CodexRendererModelUnlockConfig) -> Result<String, String> {
    let config_json = serde_json::to_string(payload)
        .map_err(|error| format!("无法序列化 renderer 模型目录：{error}"))?;
    if !MODEL_UNLOCK_SCRIPT.contains(MODEL_UNLOCK_CONFIG_TOKEN) {
        return Err("内置 Codex 模型注入脚本缺少配置占位符".to_string());
    }
    Ok(MODEL_UNLOCK_SCRIPT.replacen(MODEL_UNLOCK_CONFIG_TOKEN, &config_json, 1))
}

fn list_targets(debug_port: u16) -> Result<Vec<CdpTarget>, String> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), debug_port);
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(350))
        .map_err(|error| format!("无法连接 Codex CDP 端口 {debug_port}：{error}"))?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|error| format!("无法设置 CDP 读取超时：{error}"))?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|error| format!("无法设置 CDP 写入超时：{error}"))?;

    let request = format!(
        "GET /json HTTP/1.1\r\nHost: 127.0.0.1:{debug_port}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("查询 Codex CDP target 失败：{error}"))?;

    let mut response = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                response.extend_from_slice(&chunk[..read]);
                if response.len() > MAX_HTTP_RESPONSE_BYTES {
                    return Err("Codex CDP target 响应过大".to_string());
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(error) => return Err(format!("读取 Codex CDP target 失败：{error}")),
        }
    }
    parse_targets_http_response(&response)
}

fn parse_targets_http_response(response: &[u8]) -> Result<Vec<CdpTarget>, String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "Codex CDP 返回了不完整的 HTTP 响应".to_string())?;
    let headers = String::from_utf8_lossy(&response[..header_end]);
    let status_ok = headers
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("HTTP/") && line.contains(" 200 "));
    if !status_ok {
        return Err(format!(
            "Codex CDP target 查询失败：{}",
            headers.lines().next().unwrap_or("unknown HTTP status")
        ));
    }
    serde_json::from_slice(&response[header_end + 4..])
        .map_err(|error| format!("无法解析 Codex CDP target：{error}"))
}

fn pick_codex_page_target(targets: &[CdpTarget]) -> Result<CdpTarget, String> {
    targets
        .iter()
        .find(|target| is_primary_codex_page_target(target))
        .cloned()
        .ok_or_else(|| "未找到可注入的 Codex 主页面 target".to_string())
}

fn is_primary_codex_page_target(target: &CdpTarget) -> bool {
    if target.target_type != "page"
        || target
            .web_socket_debugger_url
            .as_deref()
            .is_none_or(str::is_empty)
    {
        return false;
    }
    let haystack = format!("{} {}", target.title, target.url).to_ascii_lowercase();
    let is_codex = haystack.contains("codex")
        || (target.title.trim().eq_ignore_ascii_case("chatgpt")
            && (target.url.starts_with("https://chatgpt.com")
                || target.url.starts_with("https://chat.openai.com")));
    is_codex && !is_auxiliary_codex_page(target)
}

fn is_auxiliary_codex_page(target: &CdpTarget) -> bool {
    let Ok(url) = Url::parse(target.url.trim()) else {
        return false;
    };
    let Some((_, route)) = url
        .query_pairs()
        .find(|(key, _)| key.eq_ignore_ascii_case("initialRoute"))
    else {
        return false;
    };
    let route = route.to_ascii_lowercase();
    route == "/avatar-overlay"
        || route == "/chatgpt/quick-chat"
        || route == "/chatgpt/quick-chat-prewarm"
        || route.starts_with("/chatgpt/quick-chat/")
}

fn validate_cdp_websocket_url(websocket_url: &str, expected_port: u16) -> Result<Url, String> {
    let parsed = Url::parse(websocket_url)
        .map_err(|error| format!("Codex CDP WebSocket 地址无效：{error}"))?;
    if parsed.scheme() != "ws" {
        return Err("Codex CDP WebSocket 必须使用本机 ws 协议".to_string());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "Codex CDP WebSocket 缺少主机".to_string())?;
    let address = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
        .map_err(|_| "Codex CDP WebSocket 主机必须是 loopback IP".to_string())?;
    if !address.is_loopback() {
        return Err("拒绝连接非 loopback 的 Codex CDP WebSocket".to_string());
    }
    if parsed.port() != Some(expected_port) {
        return Err(format!(
            "Codex CDP WebSocket 端口与预期端口 {expected_port} 不一致"
        ));
    }
    if !parsed.path().starts_with("/devtools/page/") {
        return Err("Codex CDP WebSocket 不是 renderer page target".to_string());
    }
    Ok(parsed)
}

fn inject_script(websocket_url: &str, debug_port: u16, script: &str) -> Result<(), String> {
    let parsed = validate_cdp_websocket_url(websocket_url, debug_port)?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "Codex CDP WebSocket 缺少主机".to_string())?;
    let address = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
        .map_err(|_| "Codex CDP WebSocket 主机必须是 loopback IP".to_string())?;
    let port = parsed
        .port()
        .ok_or_else(|| "Codex CDP WebSocket 缺少端口".to_string())?;
    let stream = TcpStream::connect_timeout(&SocketAddr::new(address, port), IO_TIMEOUT)
        .map_err(|error| format!("连接 Codex renderer WebSocket 失败：{error}"))?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|error| format!("无法设置 renderer 读取超时：{error}"))?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|error| format!("无法设置 renderer 写入超时：{error}"))?;
    let (mut socket, _) = client(websocket_url, stream)
        .map_err(|error| format!("Codex renderer WebSocket 握手失败：{error}"))?;

    send_cdp_command(&mut socket, 1, "Page.enable", json!({}))?;
    send_cdp_command(&mut socket, 2, "Runtime.enable", json!({}))?;
    send_cdp_command(
        &mut socket,
        3,
        "Page.addScriptToEvaluateOnNewDocument",
        json!({ "source": script }),
    )?;
    let evaluated = send_cdp_command(
        &mut socket,
        4,
        "Runtime.evaluate",
        json!({
            "expression": script,
            "awaitPromise": true,
            "returnByValue": true,
        }),
    )?;
    if evaluated.get("exceptionDetails").is_some() {
        return Err(format!("Codex renderer 执行注入脚本失败：{evaluated}"));
    }
    let installed = evaluated
        .pointer("/result/value/installed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !installed {
        return Err("Codex renderer 未确认模型注入脚本已安装".to_string());
    }
    let _ = socket.close(None);
    Ok(())
}

fn send_cdp_command(
    socket: &mut WebSocket<TcpStream>,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let payload = serde_json::to_string(&json!({
        "id": id,
        "method": method,
        "params": params,
    }))
    .map_err(|error| format!("无法序列化 CDP 命令 {method}：{error}"))?;
    socket
        .send(Message::Text(payload.into()))
        .map_err(|error| format!("发送 CDP 命令 {method} 失败：{error}"))?;

    loop {
        let message = socket
            .read()
            .map_err(|error| format!("等待 CDP 命令 {method} 响应失败：{error}"))?;
        match message {
            Message::Text(text) => {
                let value: Value = serde_json::from_str(text.as_ref())
                    .map_err(|error| format!("无法解析 CDP 响应：{error}"))?;
                if value.get("id").and_then(Value::as_u64) != Some(id) {
                    continue;
                }
                if let Some(error) = value.get("error") {
                    return Err(format!("CDP 命令 {method} 返回错误：{error}"));
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
            Message::Binary(bytes) => {
                let value: Value = serde_json::from_slice(&bytes)
                    .map_err(|error| format!("无法解析二进制 CDP 响应：{error}"))?;
                if value.get("id").and_then(Value::as_u64) != Some(id) {
                    continue;
                }
                if let Some(error) = value.get("error") {
                    return Err(format!("CDP 命令 {method} 返回错误：{error}"));
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
            Message::Ping(bytes) => socket
                .send(Message::Pong(bytes))
                .map_err(|error| format!("回复 Codex renderer 心跳失败：{error}"))?,
            Message::Close(frame) => {
                return Err(format!("Codex renderer 提前关闭 WebSocket：{frame:?}"));
            }
            Message::Pong(_) | Message::Frame(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_model_unlock_config, build_model_unlock_script, parse_targets_http_response,
        pick_codex_page_target, validate_cdp_websocket_url, CodexRendererModelUnlockConfig,
        CODEX_RENDERER_DEBUG_PORT,
    };
    use serde_json::json;

    #[test]
    fn payload_deduplicates_models_and_keeps_configured_default() {
        let catalog = json!({
            "models": [
                { "model": "claude-sonnet-5", "displayName": "Claude Sonnet 5" },
                { "model": "claude-opus-5", "displayName": "Claude Opus 5" },
                { "model": "claude-sonnet-5", "displayName": "Duplicate" }
            ]
        });
        let config = Some("model = \"claude-opus-5\"\n".to_string());
        let payload = build_model_unlock_config(&catalog, &config)
            .expect("payload builds")
            .expect("payload exists");
        assert_eq!(payload.default_model, "claude-opus-5");
        assert_eq!(payload.models.len(), 2);
        assert_eq!(payload.models[0].display_name, "Claude Sonnet 5");
    }

    #[test]
    fn target_picker_ignores_non_codex_and_quick_chat_pages() {
        let body = format!(
            r#"[
              {{"id":"chrome","type":"page","title":"New Tab","url":"chrome://newtab","webSocketDebuggerUrl":"ws://127.0.0.1:{0}/devtools/page/1"}},
              {{"id":"quick","type":"page","title":"Codex","url":"app://-/index.html?initialRoute=%2Fchatgpt%2Fquick-chat-prewarm","webSocketDebuggerUrl":"ws://127.0.0.1:{0}/devtools/page/2"}},
              {{"id":"codex","type":"page","title":"Codex","url":"app://-/index.html","webSocketDebuggerUrl":"ws://127.0.0.1:{0}/devtools/page/3"}}
            ]"#,
            CODEX_RENDERER_DEBUG_PORT
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let targets = parse_targets_http_response(response.as_bytes()).expect("targets parse");
        let selected = pick_codex_page_target(&targets).expect("Codex target selected");
        assert_eq!(selected.id, "codex");
    }

    #[test]
    fn websocket_validation_requires_loopback_expected_port_and_page_path() {
        assert!(validate_cdp_websocket_url(
            &format!("ws://127.0.0.1:{CODEX_RENDERER_DEBUG_PORT}/devtools/page/renderer"),
            CODEX_RENDERER_DEBUG_PORT
        )
        .is_ok());
        assert!(validate_cdp_websocket_url(
            &format!("ws://192.168.1.2:{CODEX_RENDERER_DEBUG_PORT}/devtools/page/renderer"),
            CODEX_RENDERER_DEBUG_PORT
        )
        .is_err());
        assert!(validate_cdp_websocket_url(
            "ws://127.0.0.1:9230/devtools/page/renderer",
            CODEX_RENDERER_DEBUG_PORT
        )
        .is_err());
    }

    #[test]
    fn injected_script_covers_model_paths_without_auth_mutation() {
        let payload = CodexRendererModelUnlockConfig {
            default_model: "claude-sonnet-5".to_string(),
            models: vec![super::CodexRendererModel {
                model: "claude-sonnet-5".to_string(),
                display_name: "Claude Sonnet 5".to_string(),
                description: None,
                default_reasoning_effort: None,
            }],
        };
        let script = build_model_unlock_script(&payload).expect("script builds");
        for expected in [
            "Response.prototype.json",
            "available_models",
            "includeHidden",
            "model/list",
            "modelPayloadLooksPatchable",
            "String(args[0]) === \"107580212\"",
            "claude-sonnet-5",
        ] {
            assert!(script.contains(expected), "missing {expected}");
        }
        assert!(!script.contains("auth.json"));
        assert!(!script.contains("OPENAI_API_KEY"));
        assert!(!script.contains("access_token"));
    }
}
