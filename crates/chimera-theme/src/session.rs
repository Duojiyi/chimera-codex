//! CDP (Chrome DevTools Protocol) session lifecycle — Step 8.2 (ADR-005, T48).
//!
//! Chimera talks CDP to the Codex window it itself launched, over a loopback
//! port it itself chose — never a fixed port (predictable across machines
//! and across runs is an attack surface for anything else on the box that
//! goes looking for it) and never any interface but `127.0.0.1` (binding
//! wider would let another host on the LAN drive Codex's UI). Three seams
//! keep the lifecycle logic below testable without a real socket or a real
//! browser (see `tests/step8_2_session.rs`):
//!   - [`PortAllocator`] picks the port. [`OsPortAllocator`] is the only
//!     real implementation, and the one piece of this module whose own
//!     tests legitimately open a real (loopback-only) socket — proving "the
//!     OS handed us a genuinely free, non-fixed, loopback-only port" is not
//!     something a fake can honestly stand in for. Every other test in this
//!     crate's session suite uses a fake and opens nothing real.
//!   - [`BrowserLauncher`] spawns the child process and returns an owned
//!     [`BrowserProcess`] handle.
//!   - [`CdpClient`] speaks the protocol itself: target discovery, CSS
//!     injection/clearing, navigation polling.
//! [`CdpSession`] composes the three without ever knowing whether it is
//! driving a real browser or a test double — that is the "narrow port"
//! `lib.rs`'s module docs refer to.

use std::net::{Ipv4Addr, TcpListener};
use thiserror::Error;

/// Why a CDP session could not be started, driven, or kept alive.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("could not allocate a loopback debug port: {0}")]
    PortAllocation(String),
    #[error("could not launch the managed browser process: {0}")]
    LaunchFailed(String),
    #[error("no page target found in the running session")]
    NoTargetFound,
    #[error("CDP transport error: {0}")]
    Transport(String),
    /// The managed process has already exited (crashed or was closed by the
    /// user). Surfaced rather than silently swallowed: a caller that keeps
    /// calling into a dead session would otherwise get a confusing
    /// transport error far from the actual cause.
    #[error("the managed browser process has already exited")]
    ProcessExited,
}

/// Picks a loopback debug port. [`OsPortAllocator`] is the only real
/// implementation; every caller in this module takes one as `&dyn
/// PortAllocator` so tests can supply a fixed, predictable value instead.
pub trait PortAllocator {
    /// Return a port that is, at the moment of the call, free on
    /// `127.0.0.1`. Implementations must never bind (even transiently) to
    /// any other interface.
    fn allocate(&self) -> Result<u16, SessionError>;
}

/// Real allocator: asks the OS for an ephemeral port by binding to port `0`
/// on `127.0.0.1`, reads back whichever port the OS chose, then immediately
/// releases it so the managed browser process can bind it in turn.
///
/// This release-then-hand-to-a-child pattern has a theoretical TOCTOU race
/// (something else on the machine could grab the port in the gap between
/// releasing it here and the child binding it) — the same race every "ask
/// the OS for a free port, then launch a subprocess with it" tool accepts,
/// because the alternative (holding the listener open and passing it as an
/// inherited handle) is not something a plain `--remote-debugging-port=N`
/// command-line flag can consume anyway. What this *does* guarantee, unlike
/// a fixed port: the number is never guessable in advance by anything not
/// already watching this process, and a second concurrent allocation gets a
/// different number (see `two_concurrent_port_allocations_never_collide`).
pub struct OsPortAllocator;

impl PortAllocator for OsPortAllocator {
    fn allocate(&self) -> Result<u16, SessionError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|e| SessionError::PortAllocation(e.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|e| SessionError::PortAllocation(e.to_string()))?
            .port();
        drop(listener);
        Ok(port)
    }
}

/// A child process Chimera owns end-to-end: it started it, and it alone
/// decides when it dies. `Send` because a session may be handed off to a
/// dedicated worker task (e.g. a Tauri async command) after construction.
pub trait BrowserProcess: Send {
    /// `true` while the process is still running. Must not block.
    fn is_running(&mut self) -> bool;
    /// Terminate the process. Idempotent: calling this on an
    /// already-exited process is not an error — the caller (notably
    /// [`CdpSession`]'s `Drop`) must be able to call it unconditionally.
    fn kill(&mut self) -> Result<(), SessionError>;
}

/// Starts the managed browser (Codex's own window) with remote debugging
/// enabled on `port`, and hands back an owned process handle.
pub trait BrowserLauncher {
    type Process: BrowserProcess;
    fn launch(&self, port: u16) -> Result<Self::Process, SessionError>;
}

/// One CDP target (a page/tab/worker/etc.), as reported by target discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdpTarget {
    pub id: String,
    /// CDP's own target `type` field (`"page"`, `"worker"`, ...).
    pub kind: String,
    pub url: String,
}

/// Speaks the protocol: discovery, CSS injection/clearing, navigation
/// polling. `Send` for the same reason as [`BrowserProcess`].
pub trait CdpClient: Send {
    /// Establish the transport-level connection to `port`. Called exactly
    /// once, before any other method.
    fn connect(&mut self, port: u16) -> Result<(), SessionError>;
    /// List the browser's current targets.
    fn list_targets(&mut self) -> Result<Vec<CdpTarget>, SessionError>;
    /// Push `css` into the page `target_id`, replacing whatever this
    /// session previously injected there.
    fn inject_css(&mut self, target_id: &str, css: &str) -> Result<(), SessionError>;
    /// Remove whatever CSS this session previously injected into
    /// `target_id`, restoring Codex's own default appearance there.
    fn clear_css(&mut self, target_id: &str) -> Result<(), SessionError>;
    /// `true` if `target_id` has navigated to a new top-level document
    /// since the last call (or since the target was first discovered, on
    /// the first call). A fresh document has no memory of a previously
    /// injected stylesheet, which is exactly what makes this signal
    /// necessary — see [`CdpSession::reinject_after_navigation`].
    fn poll_navigated(&mut self, target_id: &str) -> Result<bool, SessionError>;
}

/// A live CDP session against a single managed browser process.
///
/// Not `Clone`: exactly one value owns the managed process, and that
/// ownership is what makes `Drop` a reliable cleanup point rather than one
/// of several copies racing to decide who kills it.
pub struct CdpSession<P: BrowserProcess, C: CdpClient> {
    port: u16,
    process: P,
    client: C,
    target_id: Option<String>,
    last_css: Option<String>,
}

impl<P: BrowserProcess, C: CdpClient> CdpSession<P, C> {
    /// Allocate a port, launch the process, then connect — in that exact
    /// order, so a failure at any step leaves nothing dangling: launch
    /// never runs on an unallocated port, and connect never runs against a
    /// process that failed to start. If `launcher.launch` fails after the
    /// port was allocated, there is nothing left to clean up (the OS
    /// already reclaimed the port when [`OsPortAllocator`] released its
    /// probing listener, and no process was ever spawned to leak).
    pub fn start<L: BrowserLauncher<Process = P>>(
        allocator: &dyn PortAllocator,
        launcher: &L,
        mut client: C,
    ) -> Result<Self, SessionError> {
        let port = allocator.allocate()?;
        let process = launcher.launch(port)?;
        client.connect(port)?;
        Ok(Self {
            port,
            process,
            client,
            target_id: None,
            last_css: None,
        })
    }

    /// The loopback port this session's managed process is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// `true` while the managed process is still alive. A caller must check
    /// this (or handle [`SessionError::ProcessExited`] from the methods
    /// below) rather than assume a session stays usable forever — an
    /// abnormal exit of the *child* is a normal, expected event this type
    /// surfaces rather than papers over.
    pub fn is_alive(&mut self) -> bool {
        self.process.is_running()
    }

    fn ensure_alive(&mut self) -> Result<(), SessionError> {
        if self.process.is_running() {
            Ok(())
        } else {
            Err(SessionError::ProcessExited)
        }
    }

    /// Find and remember the first page target. Callers do not need to
    /// invoke this directly for the common case — [`Self::apply_css`] and
    /// [`Self::clear_css`] auto-discover on first use — but it is exposed
    /// so a caller can fail fast at attach time rather than at first paint.
    pub fn discover_target(&mut self) -> Result<&str, SessionError> {
        self.ensure_alive()?;
        let targets = self.client.list_targets()?;
        let page = targets
            .into_iter()
            .find(|t| t.kind == "page")
            .ok_or(SessionError::NoTargetFound)?;
        self.target_id = Some(page.id);
        Ok(self.target_id.as_deref().expect("just set"))
    }

    fn target(&mut self) -> Result<String, SessionError> {
        if self.target_id.is_none() {
            self.discover_target()?;
        }
        Ok(self
            .target_id
            .clone()
            .expect("discover_target populates target_id or returns Err"))
    }

    /// Inject `css` into the discovered target and remember it, so a later
    /// navigation can be repaired by [`Self::reinject_after_navigation`].
    pub fn apply_css(&mut self, css: &str) -> Result<(), SessionError> {
        self.ensure_alive()?;
        let target = self.target()?;
        self.client.inject_css(&target, css)?;
        self.last_css = Some(css.to_string());
        Ok(())
    }

    /// Remove the injected stylesheet and forget it — a later navigation
    /// must not resurrect CSS that was deliberately cleared.
    pub fn clear_css(&mut self) -> Result<(), SessionError> {
        self.ensure_alive()?;
        let target = self.target()?;
        self.client.clear_css(&target)?;
        self.last_css = None;
        Ok(())
    }

    /// Call periodically (or from a navigation-event callback). If the
    /// target navigated since CSS was last pushed, re-pushes the same CSS —
    /// a fresh document has no memory of the old one — and reports whether
    /// it did. A no-op (`Ok(false)`) before any target has been discovered.
    pub fn reinject_after_navigation(&mut self) -> Result<bool, SessionError> {
        self.ensure_alive()?;
        let Some(target) = self.target_id.clone() else {
            return Ok(false);
        };
        let navigated = self.client.poll_navigated(&target)?;
        if navigated {
            if let Some(css) = self.last_css.clone() {
                self.client.inject_css(&target, &css)?;
            }
        }
        Ok(navigated)
    }
}

impl<P: BrowserProcess, C: CdpClient> Drop for CdpSession<P, C> {
    /// Cleanup runs whenever this value is dropped — ordinary end of scope,
    /// an early `?` return after construction, or unwinding out of a panic —
    /// because Rust runs `Drop` on every one of those paths. What `Drop`
    /// cannot cover is the process surviving Chimera itself being killed
    /// out from under it (SIGKILL, power loss): that needs an OS-level
    /// mechanism (a Windows Job Object with `KILL_ON_JOB_CLOSE`) that this
    /// pass deliberately does not add — see the crate report for why.
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

// ── Real adapters ───────────────────────────────────────────────────────
//
// Nothing in this module is exercised directly by this crate's tests (see
// the module docs): `ChildProcess` would need to spawn a real browser and
// `TungsteniteCdpClient` would need a real one to talk CDP to, and both are
// explicitly off-limits in this crate's own test suite. They are still
// production code, not stubs — an integration layer (see the crate report's
// "Integration needed" section) wires these in, not fakes.
pub mod real {
    use super::{BrowserLauncher, BrowserProcess, CdpClient, CdpTarget, SessionError};
    use serde::Deserialize;
    use std::io::{Read, Write};
    use std::net::{Ipv4Addr, TcpStream};
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::time::Duration;

    /// Wraps an owned [`std::process::Child`].
    pub struct ChildProcess(Child);

    impl BrowserProcess for ChildProcess {
        fn is_running(&mut self) -> bool {
            matches!(self.0.try_wait(), Ok(None))
        }

        fn kill(&mut self) -> Result<(), SessionError> {
            // Racing an exit that just happened is not a real failure —
            // only report an error if the process is demonstrably still
            // there and still refused to die.
            match self.0.try_wait() {
                Ok(Some(_)) => Ok(()),
                _ => match self.0.kill() {
                    Ok(()) => Ok(()),
                    Err(e) if matches!(self.0.try_wait(), Ok(Some(_))) => {
                        let _ = e;
                        Ok(())
                    }
                    Err(e) => Err(SessionError::Transport(e.to_string())),
                },
            }
        }
    }

    /// Launches `executable` with remote debugging bound to
    /// `127.0.0.1:<port>` — never `0.0.0.0`, so the debug endpoint itself
    /// never listens beyond loopback even if the browser's own default
    /// would.
    pub struct CommandLauncher {
        pub executable: PathBuf,
        pub user_data_dir: PathBuf,
    }

    impl BrowserLauncher for CommandLauncher {
        type Process = ChildProcess;

        fn launch(&self, port: u16) -> Result<Self::Process, SessionError> {
            let child = Command::new(&self.executable)
                .arg(format!("--remote-debugging-port={port}"))
                .arg("--remote-debugging-address=127.0.0.1")
                .arg(format!("--user-data-dir={}", self.user_data_dir.display()))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| SessionError::LaunchFailed(e.to_string()))?;
            Ok(ChildProcess(child))
        }
    }

    #[derive(Deserialize)]
    struct RawTarget {
        id: String,
        #[serde(rename = "type")]
        kind: String,
        url: String,
        #[serde(rename = "webSocketDebuggerUrl")]
        web_socket_debugger_url: Option<String>,
    }

    /// Minimal blocking HTTP/1.1 GET over a loopback `TcpStream`.
    ///
    /// `Connection: close` means the CDP HTTP endpoint closes the socket
    /// once it has written its response, so `read_to_end` reliably reads
    /// the whole body without needing to parse `Content-Length` or chunked
    /// transfer-encoding — deliberately not a general HTTP client, only
    /// what talking to `/json/list` on a loopback debug port needs.
    fn http_get(port: u16, path: &str) -> Result<String, SessionError> {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .map_err(|e| SessionError::Transport(e.to_string()))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| SessionError::Transport(e.to_string()))?;
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .map_err(|e| SessionError::Transport(e.to_string()))?;
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|e| SessionError::Transport(e.to_string()))?;
        let text = String::from_utf8_lossy(&response);
        Ok(text
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .unwrap_or("")
            .to_string())
    }

    fn list_raw_targets(port: u16) -> Result<Vec<RawTarget>, SessionError> {
        let body = http_get(port, "/json/list")?;
        serde_json::from_str(&body)
            .map_err(|e| SessionError::Transport(format!("malformed target list: {e}")))
    }

    const STYLE_ELEMENT_ID: &str = "__chimera_skin__";

    /// Build a `Runtime.evaluate` expression that creates (or updates) a
    /// single `<style>` element carrying `css`.
    ///
    /// `css` is JSON-encoded into the expression rather than interpolated
    /// as a raw string: JSON string syntax is a valid JS string literal, so
    /// this is a safe, single escaping pass regardless of quotes, newlines,
    /// or backslashes inside the stylesheet — string concatenation here
    /// would reopen exactly the injection hazard `css_allowlist` exists to
    /// close one layer up.
    fn build_inject_expression(css: &str) -> String {
        let encoded_css = serde_json::to_string(css).unwrap_or_else(|_| "\"\"".to_string());
        format!(
            "(function(){{var s=document.getElementById('{STYLE_ELEMENT_ID}');\
             if(!s){{s=document.createElement('style');s.id='{STYLE_ELEMENT_ID}';\
             document.head.appendChild(s);}}s.textContent={encoded_css};}})()"
        )
    }

    fn build_clear_expression() -> String {
        format!(
            "(function(){{var s=document.getElementById('{STYLE_ELEMENT_ID}');\
             if(s){{s.remove();}}}})()"
        )
    }

    /// Real [`CdpClient`]: HTTP polling for target discovery/navigation
    /// (Chrome's own `/json/list` endpoint reports each page's current
    /// top-level `url`, so navigation is detectable without a persistent
    /// event subscription) and one-shot WebSocket round trips for CSS
    /// injection/clearing.
    pub struct TungsteniteCdpClient {
        runtime: tokio::runtime::Runtime,
        port: Option<u16>,
        next_id: u64,
        last_seen_url: std::collections::HashMap<String, String>,
    }

    impl TungsteniteCdpClient {
        pub fn new() -> Result<Self, SessionError> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| SessionError::Transport(e.to_string()))?;
            Ok(Self {
                runtime,
                port: None,
                next_id: 1,
                last_seen_url: std::collections::HashMap::new(),
            })
        }

        fn connected_port(&self) -> Result<u16, SessionError> {
            self.port
                .ok_or_else(|| SessionError::Transport("not connected".to_string()))
        }

        fn debugger_url_for(&self, target_id: &str) -> Result<String, SessionError> {
            let port = self.connected_port()?;
            let targets = list_raw_targets(port)?;
            targets
                .into_iter()
                .find(|t| t.id == target_id)
                .and_then(|t| t.web_socket_debugger_url)
                .ok_or(SessionError::NoTargetFound)
        }

        fn run_expression(
            &mut self,
            target_id: &str,
            expression: &str,
        ) -> Result<(), SessionError> {
            let ws_url = self.debugger_url_for(target_id)?;
            let id = self.next_id;
            self.next_id += 1;
            self.runtime.block_on(evaluate(&ws_url, id, expression))
        }
    }

    impl Default for TungsteniteCdpClient {
        /// Falls back to a runtime with no reactor drivers enabled if
        /// building the preferred one fails; every method that actually
        /// needs I/O will then surface a clear [`SessionError::Transport`]
        /// instead of this constructor needing to be fallible just for the
        /// vanishingly rare case tokio itself cannot start a runtime.
        fn default() -> Self {
            Self::new().unwrap_or_else(|_| Self {
                runtime: tokio::runtime::Builder::new_current_thread()
                    .build()
                    .expect("building a tokio runtime with no drivers enabled cannot fail"),
                port: None,
                next_id: 1,
                last_seen_url: std::collections::HashMap::new(),
            })
        }
    }

    impl CdpClient for TungsteniteCdpClient {
        fn connect(&mut self, port: u16) -> Result<(), SessionError> {
            // A liveness probe, not just bookkeeping: if the debug HTTP
            // endpoint is not answering yet, later calls would fail with a
            // transport error far from where the real problem (browser
            // took too long to open its debug port) actually is.
            list_raw_targets(port)?;
            self.port = Some(port);
            Ok(())
        }

        fn list_targets(&mut self) -> Result<Vec<CdpTarget>, SessionError> {
            let port = self.connected_port()?;
            let targets = list_raw_targets(port)?;
            Ok(targets
                .into_iter()
                .map(|t| CdpTarget {
                    id: t.id,
                    kind: t.kind,
                    url: t.url,
                })
                .collect())
        }

        fn inject_css(&mut self, target_id: &str, css: &str) -> Result<(), SessionError> {
            let expr = build_inject_expression(css);
            self.run_expression(target_id, &expr)
        }

        fn clear_css(&mut self, target_id: &str) -> Result<(), SessionError> {
            let expr = build_clear_expression();
            self.run_expression(target_id, &expr)
        }

        fn poll_navigated(&mut self, target_id: &str) -> Result<bool, SessionError> {
            let port = self.connected_port()?;
            let targets = list_raw_targets(port)?;
            let target = targets
                .into_iter()
                .find(|t| t.id == target_id)
                .ok_or(SessionError::NoTargetFound)?;
            let navigated = self
                .last_seen_url
                .get(target_id)
                .is_some_and(|prev| prev != &target.url);
            self.last_seen_url.insert(target_id.to_string(), target.url);
            Ok(navigated)
        }
    }

    async fn evaluate(ws_url: &str, id: u64, expression: &str) -> Result<(), SessionError> {
        use futures_util::{SinkExt, StreamExt};
        let (mut ws, _response) = tokio_tungstenite::connect_async(ws_url)
            .await
            .map_err(|e| SessionError::Transport(e.to_string()))?;

        let payload = serde_json::json!({
            "id": id,
            "method": "Runtime.evaluate",
            "params": { "expression": expression, "returnByValue": true },
        });
        let text =
            serde_json::to_string(&payload).map_err(|e| SessionError::Transport(e.to_string()))?;
        ws.send(tokio_tungstenite::tungstenite::Message::text(text))
            .await
            .map_err(|e| SessionError::Transport(e.to_string()))?;

        loop {
            match ws.next().await {
                Some(Ok(msg)) => {
                    let Ok(text) = msg.into_text() else {
                        continue;
                    };
                    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                        continue;
                    };
                    if value.get("id").and_then(serde_json::Value::as_u64) != Some(id) {
                        continue; // an unrelated event frame; keep waiting
                    }
                    return match value.get("error") {
                        Some(err) => Err(SessionError::Transport(err.to_string())),
                        None => Ok(()),
                    };
                }
                Some(Err(e)) => return Err(SessionError::Transport(e.to_string())),
                None => {
                    return Err(SessionError::Transport(
                        "connection closed before a response arrived".to_string(),
                    ));
                }
            }
        }
    }
}
