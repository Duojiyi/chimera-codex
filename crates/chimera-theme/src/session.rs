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
// ── Real adapters ───────────────────────────────────────────────────────
//
// The production implementations of the three traits above live in
// `crate::cdp_transport`, not here.
//
// An earlier version of this module carried its own `pub mod real` that
// injected CSS by evaluating `document.createElement('style')` through
// `Runtime.evaluate`. It was careful about the CSS it embedded — JSON-encoded
// rather than concatenated — but careful about the wrong thing: G9 forbids
// arbitrary JavaScript outright, so the mechanism itself was the violation, not
// the payload. A skin engine that executes script to install a stylesheet
// breaks the rule it exists to enforce.
//
// `cdp_transport` uses CDP's CSS domain instead — `CSS.createStyleSheet` plus
// `CSS.setStyleSheetText` — which does the same job with no script execution at
// all. `tests/step8_2_no_javascript.rs` asserts no path back.
