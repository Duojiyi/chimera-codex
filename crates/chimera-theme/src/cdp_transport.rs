//! Step 8.2 — the real transport behind [`crate::session`]'s traits.
//!
//! `session.rs` owns the lifecycle and knows nothing about sockets. This module
//! is the opposite: it knows only how to spawn Codex with a debug port, list
//! targets over HTTP, and speak CDP over a WebSocket.
//!
//! **CSS is injected through CDP's CSS domain, never through JavaScript.**
//! The obvious implementation — `Runtime.evaluate` with a script that appends a
//! `<style>` element — is off the table: G9 forbids arbitrary JavaScript, and a
//! rule that the skin engine itself breaks is not a rule. `CSS.createStyleSheet`
//! plus `CSS.setStyleSheetText` does the same job with no script execution at
//! all, which is the whole reason G9 is satisfiable rather than merely stated.
//!
//! Everything that can be decided without a socket is a pure function here, and
//! is tested. What remains — that a real Codex build answers these exact
//! messages — is not something a unit test can honestly assert, and the module
//! says so rather than wrapping a fake in enough indirection to look covered.

use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::session::{BrowserLauncher, BrowserProcess, CdpClient, CdpTarget, SessionError};

/// How long to wait for the debug endpoint to come up after launch.
const READY_TIMEOUT: Duration = Duration::from_secs(20);

/// How long a single CDP call may take before it is treated as a dead session.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);

// ── Pure protocol helpers ──────────────────────────────────────────────────

/// CDP's target-list endpoint.
///
/// Always `127.0.0.1`, never `localhost`: `localhost` can resolve to `::1`
/// first on a dual-stack machine, and the browser was told to bind the IPv4
/// loopback. Hard-coding the literal removes a resolver from the path
/// entirely.
pub fn targets_endpoint(port: u16) -> String {
    format!("http://127.0.0.1:{port}/json/list")
}

/// Parse CDP's `/json/list` response.
///
/// Unknown fields are ignored and a malformed entry is dropped rather than
/// failing the whole listing: a browser that reports one odd target should not
/// make the skin engine unusable.
pub fn parse_targets(body: &str) -> Result<Vec<CdpTarget>, SessionError> {
    let raw: Value =
        serde_json::from_str(body).map_err(|e| SessionError::Transport(e.to_string()))?;
    let items = raw
        .as_array()
        .ok_or_else(|| SessionError::Transport("target list was not a JSON array".to_string()))?;

    Ok(items
        .iter()
        .filter_map(|t| {
            Some(CdpTarget {
                id: t.get("id")?.as_str()?.to_string(),
                kind: t.get("type")?.as_str()?.to_string(),
                url: t
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect())
}

/// The per-target WebSocket URL CDP publishes.
///
/// Built rather than read from the target's own `webSocketDebuggerUrl` field so
/// the host can never be anything but loopback. Trusting the browser's reported
/// URL would mean a compromised or spoofed `/json/list` response could point
/// the session at another host — which is precisely the property the random
/// loopback port exists to protect.
pub fn target_socket_url(port: u16, target_id: &str) -> String {
    format!("ws://127.0.0.1:{port}/devtools/page/{target_id}")
}

/// One JSON-RPC request frame.
pub fn build_command(id: u64, method: &str, params: Value) -> String {
    json!({ "id": id, "method": method, "params": params }).to_string()
}

/// Is this frame the response to `id`? `None` when it is an event or another
/// call's reply.
pub fn response_for(frame: &str, id: u64) -> Option<Result<Value, SessionError>> {
    let v: Value = serde_json::from_str(frame).ok()?;
    if v.get("id")?.as_u64()? != id {
        return None;
    }
    // A CDP error carries a message that can name a target or a URL, so only
    // its code is surfaced. The caller cannot act on the text either way.
    if let Some(err) = v.get("error") {
        let code = err.get("code").and_then(Value::as_i64).unwrap_or(0);
        return Some(Err(SessionError::Transport(format!(
            "the browser rejected a stylesheet command (code {code})"
        ))));
    }
    Some(Ok(v.get("result").cloned().unwrap_or(Value::Null)))
}

/// Does this frame announce that the top-level document was replaced?
///
/// A subframe navigation does not clear the page's stylesheets, so treating one
/// as a reason to reinject would push the skin again on every iframe load.
pub fn is_top_level_navigation(frame: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(frame) else {
        return false;
    };
    if v.get("method").and_then(Value::as_str) != Some("Page.frameNavigated") {
        return false;
    }
    // A top-level frame has no parent.
    v.get("params")
        .and_then(|p| p.get("frame"))
        .map(|f| f.get("parentId").is_none())
        .unwrap_or(false)
}

/// The arguments Codex is launched with.
///
/// `--remote-debugging-address` is passed explicitly rather than relying on the
/// default: some Chromium builds bind all interfaces when only the port is
/// given, which would put the debug endpoint on the LAN.
pub fn launch_args(port: u16, user_data_dir: &str) -> Vec<String> {
    vec![
        format!("--remote-debugging-port={port}"),
        "--remote-debugging-address=127.0.0.1".to_string(),
        // Its own profile directory, so enabling remote debugging never touches
        // the user's real Codex profile — and so a skin session cannot outlive
        // itself by leaving state behind in one.
        format!("--user-data-dir={user_data_dir}"),
    ]
}

// ── Owned process ──────────────────────────────────────────────────────────

/// A Codex process Chimera started and alone decides when to end.
pub struct OwnedBrowserProcess {
    child: Child,
}

impl BrowserProcess for OwnedBrowserProcess {
    fn is_running(&mut self) -> bool {
        // `try_wait` is the non-blocking form the trait requires; an error
        // reading the status means we can no longer vouch for the process, and
        // reporting it as gone is the safe direction — a caller then stops
        // sending it commands rather than blocking on a dead socket.
        matches!(self.child.try_wait(), Ok(None))
    }

    fn kill(&mut self) -> Result<(), SessionError> {
        // Idempotent by the trait's contract, which `CdpSession::drop` relies
        // on: an already-exited process is success, not an error.
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

/// Launches the managed Codex executable with remote debugging enabled.
pub struct CodexLauncher {
    exe: PathBuf,
    user_data_dir: PathBuf,
}

impl CodexLauncher {
    /// `exe` must already have been verified as owned by the managed runtime
    /// (`chimera_runtime::health::is_process_owned_by_runtime`). This type does
    /// not re-check it — it has no notion of a runtime root — so a caller that
    /// skips that check would be launching an arbitrary binary with remote
    /// debugging enabled.
    pub fn new(exe: impl Into<PathBuf>, user_data_dir: impl Into<PathBuf>) -> Self {
        Self {
            exe: exe.into(),
            user_data_dir: user_data_dir.into(),
        }
    }
}

impl BrowserLauncher for CodexLauncher {
    type Process = OwnedBrowserProcess;

    fn launch(&self, port: u16) -> Result<Self::Process, SessionError> {
        std::fs::create_dir_all(&self.user_data_dir)
            .map_err(|e| SessionError::LaunchFailed(e.kind().to_string()))?;

        let child = Command::new(&self.exe)
            .args(launch_args(port, &self.user_data_dir.to_string_lossy()))
            // No inherited stdio: Chimera exiting must not close a pipe Codex
            // is writing to.
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| SessionError::LaunchFailed(e.kind().to_string()))?;

        Ok(OwnedBrowserProcess { child })
    }
}

// ── WebSocket client ───────────────────────────────────────────────────────

type Socket = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>;

/// Speaks CDP to one page target.
pub struct WebSocketCdpClient {
    port: u16,
    socket: Option<Socket>,
    connected_target: Option<String>,
    stylesheet_id: Option<String>,
    next_id: u64,
    http: reqwest::blocking::Client,
}

impl Default for WebSocketCdpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSocketCdpClient {
    pub fn new() -> Self {
        Self {
            port: 0,
            socket: None,
            connected_target: None,
            stylesheet_id: None,
            next_id: 1,
            // Short timeout: everything here is loopback, so a slow answer
            // means the process is wedged, not that the network is far away.
            http: reqwest::blocking::Client::builder()
                .timeout(CALL_TIMEOUT)
                .build()
                .unwrap_or_default(),
        }
    }

    /// Open (or reuse) the socket for `target_id`.
    fn socket_for(&mut self, target_id: &str) -> Result<&mut Socket, SessionError> {
        if self.connected_target.as_deref() != Some(target_id) {
            let url = target_socket_url(self.port, target_id);
            let (socket, _) = tungstenite::connect(&url)
                .map_err(|e| SessionError::Transport(short_ws_error(&e)))?;
            self.socket = Some(socket);
            self.connected_target = Some(target_id.to_string());
            // A new socket means a new session: whatever stylesheet id the old
            // one held is meaningless to this one.
            self.stylesheet_id = None;
        }
        self.socket
            .as_mut()
            .ok_or_else(|| SessionError::Transport("no open session".to_string()))
    }

    /// Send one command and wait for its reply, discarding events that arrive
    /// in between rather than letting them desynchronise the stream.
    fn call(
        &mut self,
        target_id: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, SessionError> {
        let id = self.next_id;
        self.next_id += 1;
        let frame = build_command(id, method, params);

        let socket = self.socket_for(target_id)?;
        socket
            .send(tungstenite::Message::Text(frame.into()))
            .map_err(|e| SessionError::Transport(short_ws_error(&e)))?;

        let deadline = Instant::now() + CALL_TIMEOUT;
        while Instant::now() < deadline {
            let msg = socket
                .read()
                .map_err(|e| SessionError::Transport(short_ws_error(&e)))?;
            let tungstenite::Message::Text(text) = msg else {
                continue;
            };
            if let Some(result) = response_for(&text, id) {
                return result;
            }
            // Not ours: an event, or another call's reply. Dropped on purpose —
            // navigation is polled separately and does not need buffering here.
        }
        Err(SessionError::Transport(
            "the browser did not answer in time".to_string(),
        ))
    }

    /// Create the one stylesheet this session owns, if it has not already.
    fn ensure_stylesheet(&mut self, target_id: &str) -> Result<String, SessionError> {
        if let Some(id) = &self.stylesheet_id {
            return Ok(id.clone());
        }
        // DOM must be enabled before CSS will answer.
        self.call(target_id, "DOM.enable", json!({}))?;
        self.call(target_id, "CSS.enable", json!({}))?;
        self.call(target_id, "Page.enable", json!({}))?;

        let tree = self.call(target_id, "Page.getFrameTree", json!({}))?;
        let frame_id = tree
            .get("frameTree")
            .and_then(|t| t.get("frame"))
            .and_then(|f| f.get("id"))
            .and_then(Value::as_str)
            .ok_or_else(|| SessionError::Transport("the page reported no main frame".to_string()))?
            .to_string();

        let created = self.call(
            target_id,
            "CSS.createStyleSheet",
            json!({ "frameId": frame_id }),
        )?;
        let sheet = created
            .get("styleSheetId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                SessionError::Transport("the browser created no stylesheet".to_string())
            })?
            .to_string();

        self.stylesheet_id = Some(sheet.clone());
        Ok(sheet)
    }
}

/// Reduce a WebSocket error to something safe and actionable.
///
/// Its `Display` can contain the full URL, which carries the debug port — a
/// value worth not putting in a log or a screenshot.
fn short_ws_error(e: &tungstenite::Error) -> String {
    match e {
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {
            "the Codex window closed".to_string()
        }
        tungstenite::Error::Io(io) => io.kind().to_string(),
        _ => "the debug connection failed".to_string(),
    }
}

impl CdpClient for WebSocketCdpClient {
    fn connect(&mut self, port: u16) -> Result<(), SessionError> {
        self.port = port;

        // Poll the HTTP endpoint until the browser is actually listening. The
        // process being alive is not the same as its debug server being up, and
        // connecting too early fails in a way that reads like "no targets".
        let deadline = Instant::now() + READY_TIMEOUT;
        let endpoint = targets_endpoint(port);
        while Instant::now() < deadline {
            if self.http.get(&endpoint).send().is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        Err(SessionError::Transport(
            "the Codex window did not open its debug port in time".to_string(),
        ))
    }

    fn list_targets(&mut self) -> Result<Vec<CdpTarget>, SessionError> {
        let body = self
            .http
            .get(targets_endpoint(self.port))
            .send()
            .and_then(|r| r.text())
            .map_err(|_| SessionError::Transport("could not read the target list".to_string()))?;
        parse_targets(&body)
    }

    fn inject_css(&mut self, target_id: &str, css: &str) -> Result<(), SessionError> {
        let sheet = self.ensure_stylesheet(target_id)?;
        // Replaces the whole sheet rather than appending, so repeated applies
        // cannot stack: the session owns exactly one stylesheet and its text is
        // always the current skin.
        self.call(
            target_id,
            "CSS.setStyleSheetText",
            json!({ "styleSheetId": sheet, "text": css }),
        )?;
        Ok(())
    }

    fn clear_css(&mut self, target_id: &str) -> Result<(), SessionError> {
        // Only if we ever created one. Clearing before injecting is a no-op,
        // not an error — `restore_default` is allowed to run unconditionally.
        let Some(sheet) = self.stylesheet_id.clone() else {
            return Ok(());
        };
        self.call(
            target_id,
            "CSS.setStyleSheetText",
            json!({ "styleSheetId": sheet, "text": "" }),
        )?;
        Ok(())
    }

    fn poll_navigated(&mut self, target_id: &str) -> Result<bool, SessionError> {
        let Some(socket) = self.socket.as_mut() else {
            return Ok(false);
        };
        if self.connected_target.as_deref() != Some(target_id) {
            return Ok(false);
        }

        // Drain whatever has already arrived without blocking. A navigation we
        // have not been told about yet is simply not one we act on this tick.
        let mut navigated = false;
        while let Ok(msg) = socket.read() {
            if let tungstenite::Message::Text(text) = msg {
                if is_top_level_navigation(&text) {
                    navigated = true;
                }
            }
            break;
        }
        if navigated {
            // The new document has no stylesheets, so the id we held is stale.
            self.stylesheet_id = None;
        }
        Ok(navigated)
    }
}
