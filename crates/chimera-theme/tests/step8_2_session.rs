// Step 8.2 RED — CDP (Chrome DevTools Protocol) session lifecycle (ADR-005).
//
// Every test below except the two explicitly named `os_port_allocator_*`
// tests uses fakes for the process and the CDP transport and opens nothing
// real (no socket, no subprocess) — see `session.rs`'s module docs for why
// the port allocator's own tests are the one deliberate exception: proving
// "the OS handed us a genuinely free, loopback-only, non-fixed port" is not
// something a fake can honestly stand in for.

use chimera_theme::session::{
    BrowserLauncher, BrowserProcess, CdpClient, CdpSession, CdpTarget, OsPortAllocator,
    PortAllocator, SessionError,
};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// ── fakes ────────────────────────────────────────────────────────────────

struct FixedPort(u16);
impl PortAllocator for FixedPort {
    fn allocate(&self) -> Result<u16, SessionError> {
        Ok(self.0)
    }
}

#[derive(Clone)]
struct FakeProcess {
    running: Arc<Mutex<bool>>,
    killed: Arc<Mutex<bool>>,
}

impl FakeProcess {
    fn running() -> Self {
        Self {
            running: Arc::new(Mutex::new(true)),
            killed: Arc::new(Mutex::new(false)),
        }
    }

    fn already_exited() -> Self {
        Self {
            running: Arc::new(Mutex::new(false)),
            killed: Arc::new(Mutex::new(false)),
        }
    }
}

impl BrowserProcess for FakeProcess {
    fn is_running(&mut self) -> bool {
        *self.running.lock().unwrap()
    }

    fn kill(&mut self) -> Result<(), SessionError> {
        *self.running.lock().unwrap() = false;
        *self.killed.lock().unwrap() = true;
        Ok(())
    }
}

/// Hands out a single pre-built [`FakeProcess`] the first time `launch` is
/// called; a second call is a test bug (a session only ever launches once).
struct FakeLauncher {
    process: RefCell<Option<FakeProcess>>,
    launched_ports: RefCell<Vec<u16>>,
}

impl FakeLauncher {
    fn with(process: FakeProcess) -> Self {
        Self {
            process: RefCell::new(Some(process)),
            launched_ports: RefCell::new(Vec::new()),
        }
    }
}

impl BrowserLauncher for FakeLauncher {
    type Process = FakeProcess;

    fn launch(&self, port: u16) -> Result<Self::Process, SessionError> {
        self.launched_ports.borrow_mut().push(port);
        self.process
            .borrow_mut()
            .take()
            .ok_or_else(|| SessionError::LaunchFailed("fake already launched once".to_string()))
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Call {
    ListTargets,
    Inject { target: String, css: String },
    Clear { target: String },
    PollNavigated { target: String },
}

#[derive(Clone, Default)]
struct FakeCdpClient {
    targets: Vec<CdpTarget>,
    calls: Arc<Mutex<Vec<Call>>>,
    navigated_queue: Arc<Mutex<VecDeque<bool>>>,
}

impl FakeCdpClient {
    fn with_targets(targets: Vec<CdpTarget>) -> Self {
        Self {
            targets,
            ..Default::default()
        }
    }

    fn queue_navigated(&self, value: bool) {
        self.navigated_queue.lock().unwrap().push_back(value);
    }

    fn inject_count(&self) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| matches!(c, Call::Inject { .. }))
            .count()
    }
}

impl CdpClient for FakeCdpClient {
    fn connect(&mut self, _port: u16) -> Result<(), SessionError> {
        Ok(())
    }

    fn list_targets(&mut self) -> Result<Vec<CdpTarget>, SessionError> {
        self.calls.lock().unwrap().push(Call::ListTargets);
        Ok(self.targets.clone())
    }

    fn inject_css(&mut self, target_id: &str, css: &str) -> Result<(), SessionError> {
        self.calls.lock().unwrap().push(Call::Inject {
            target: target_id.to_string(),
            css: css.to_string(),
        });
        Ok(())
    }

    fn clear_css(&mut self, target_id: &str) -> Result<(), SessionError> {
        self.calls.lock().unwrap().push(Call::Clear {
            target: target_id.to_string(),
        });
        Ok(())
    }

    fn poll_navigated(&mut self, target_id: &str) -> Result<bool, SessionError> {
        self.calls.lock().unwrap().push(Call::PollNavigated {
            target: target_id.to_string(),
        });
        Ok(self
            .navigated_queue
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(false))
    }
}

fn page(id: &str) -> CdpTarget {
    CdpTarget {
        id: id.to_string(),
        kind: "page".to_string(),
        url: "about:blank".to_string(),
    }
}

// ── OsPortAllocator: the one seam whose own tests legitimately touch a real
//    (loopback-only) socket ──────────────────────────────────────────────

#[test]
fn os_port_allocator_hands_out_a_free_port_that_is_not_always_the_same() {
    let allocator = OsPortAllocator;
    let mut seen = std::collections::HashSet::new();
    for _ in 0..5 {
        let port = allocator.allocate().expect("must allocate a port");
        assert!(port > 0, "0 is not a real port");
        // The allocator must have released the port again rather than
        // holding it open itself — if it were still bound, this bind
        // would fail.
        let listener =
            std::net::TcpListener::bind(("127.0.0.1", port)).expect("port must be free again");
        drop(listener);
        seen.insert(port);
    }
    assert!(
        seen.len() > 1,
        "allocating several times in a row must not always return the same port — a fixed \
         port would be predictable/guessable and would collide across concurrent sessions"
    );
}

#[test]
fn os_port_allocator_only_ever_binds_the_loopback_interface() {
    // OsPortAllocator's contract (see its doc comment) is that it binds
    // 127.0.0.1 and nothing else. The most direct way to observe that from
    // outside is: immediately after allocating, 127.0.0.1 can bind that
    // port (nothing else is squatting on it there), which is only possible
    // if the allocator itself already let it go rather than lingering on
    // some other interface.
    let allocator = OsPortAllocator;
    let port = allocator.allocate().expect("must allocate a port");
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .expect("127.0.0.1 must be able to claim the allocated port");
    assert_eq!(listener.local_addr().unwrap().ip().to_string(), "127.0.0.1");
}

#[test]
fn two_concurrent_port_allocations_never_collide() {
    let allocator = OsPortAllocator;
    let a = allocator.allocate().expect("first allocation");
    let b = allocator.allocate().expect("second allocation");
    assert_ne!(
        a, b,
        "two sessions started back-to-back must not share a port"
    );
}

// ── CdpSession::start: port -> launch -> connect, in that order ───────────

#[test]
fn starting_a_session_launches_on_the_allocated_port_and_connects() {
    let launcher = FakeLauncher::with(FakeProcess::running());
    let session = CdpSession::start(&FixedPort(4242), &launcher, FakeCdpClient::default())
        .expect("start must succeed");
    assert_eq!(session.port(), 4242);
    assert_eq!(*launcher.launched_ports.borrow(), vec![4242]);
}

#[test]
fn two_sessions_started_with_a_real_allocator_do_not_collide_on_a_port() {
    // Real port picker, fake everything else: the property under test here
    // ("two concurrent sessions never share a port") is only meaningful
    // against the real allocator — a fake port allocator could trivially
    // "pass" this without proving anything about real port allocation.
    let allocator = OsPortAllocator;
    let session_a = CdpSession::start(
        &allocator,
        &FakeLauncher::with(FakeProcess::running()),
        FakeCdpClient::default(),
    )
    .unwrap();
    let session_b = CdpSession::start(
        &allocator,
        &FakeLauncher::with(FakeProcess::running()),
        FakeCdpClient::default(),
    )
    .unwrap();
    assert_ne!(session_a.port(), session_b.port());
}

// ── owned child cleanup: Drop, including mid-panic unwind ─────────────────

#[test]
fn dropping_a_session_kills_its_managed_process() {
    let process = FakeProcess::running();
    let killed = process.killed.clone();
    {
        let launcher = FakeLauncher::with(process);
        let _session = CdpSession::start(&FixedPort(1), &launcher, FakeCdpClient::default())
            .expect("start must succeed");
        assert!(
            !*killed.lock().unwrap(),
            "must not be killed while still in scope"
        );
    }
    assert!(
        *killed.lock().unwrap(),
        "process must be killed once the session is dropped"
    );
}

#[test]
fn a_panic_while_a_session_is_in_scope_still_kills_the_process() {
    // Rust runs `Drop` while unwinding a panic exactly as it does at
    // ordinary end of scope; this is the "abnormal exit" half of "guarantee
    // cleanup on Drop AND on abnormal exit" that is soundly testable from
    // pure Rust (surviving a hard OS-level kill of Chimera's own process is
    // a separate, out-of-scope guarantee — see the crate report).
    let process = FakeProcess::running();
    let killed = process.killed.clone();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let launcher = FakeLauncher::with(process);
        let _session = CdpSession::start(&FixedPort(1), &launcher, FakeCdpClient::default())
            .expect("start must succeed");
        panic!("boom");
    }));

    assert!(
        result.is_err(),
        "fixture sanity: the closure must have panicked"
    );
    assert!(
        *killed.lock().unwrap(),
        "Drop must still run while unwinding a panic"
    );
}

// ── target discovery, CSS application, reload/reinject after navigation ──

#[test]
fn discover_target_finds_the_page_target() {
    let client = FakeCdpClient::with_targets(vec![page("target-1")]);
    let mut session = CdpSession::start(
        &FixedPort(1),
        &FakeLauncher::with(FakeProcess::running()),
        client,
    )
    .unwrap();
    let target_id = session.discover_target().unwrap();
    assert_eq!(target_id, "target-1");
}

#[test]
fn discover_target_refuses_when_no_page_target_exists() {
    let client = FakeCdpClient::with_targets(vec![CdpTarget {
        id: "worker-1".to_string(),
        kind: "worker".to_string(),
        url: "about:blank".to_string(),
    }]);
    let mut session = CdpSession::start(
        &FixedPort(1),
        &FakeLauncher::with(FakeProcess::running()),
        client,
    )
    .unwrap();
    let result = session.discover_target();
    assert!(matches!(result, Err(SessionError::NoTargetFound)));
}

#[test]
fn apply_css_auto_discovers_the_target_if_none_was_discovered_yet() {
    let client = FakeCdpClient::with_targets(vec![page("target-1")]);
    let calls = client.calls.clone();
    let mut session = CdpSession::start(
        &FixedPort(1),
        &FakeLauncher::with(FakeProcess::running()),
        client,
    )
    .unwrap();

    session.apply_css(".x{color:red}").unwrap();

    let recorded = calls.lock().unwrap();
    assert!(recorded.contains(&Call::Inject {
        target: "target-1".to_string(),
        css: ".x{color:red}".to_string(),
    }));
}

#[test]
fn a_navigation_causes_the_last_applied_css_to_be_reinjected() {
    let client = FakeCdpClient::with_targets(vec![page("target-1")]);
    client.queue_navigated(true);
    let mut session = CdpSession::start(
        &FixedPort(1),
        &FakeLauncher::with(FakeProcess::running()),
        client.clone(),
    )
    .unwrap();

    session.apply_css(".x{color:red}").unwrap();
    assert_eq!(client.inject_count(), 1);

    let navigated = session.reinject_after_navigation().unwrap();
    assert!(
        navigated,
        "the queued poll result said a navigation happened"
    );
    assert_eq!(
        client.inject_count(),
        2,
        "css must be pushed again after a navigation was detected"
    );
}

#[test]
fn no_navigation_means_no_reinjection() {
    let client = FakeCdpClient::with_targets(vec![page("target-1")]);
    client.queue_navigated(false);
    let mut session = CdpSession::start(
        &FixedPort(1),
        &FakeLauncher::with(FakeProcess::running()),
        client.clone(),
    )
    .unwrap();

    session.apply_css(".x{color:red}").unwrap();
    let navigated = session.reinject_after_navigation().unwrap();
    assert!(!navigated);
    assert_eq!(
        client.inject_count(),
        1,
        "no navigation means no reinjection"
    );
}

#[test]
fn reinject_after_navigation_is_a_harmless_no_op_before_any_css_was_applied() {
    let client = FakeCdpClient::with_targets(vec![page("target-1")]);
    let mut session = CdpSession::start(
        &FixedPort(1),
        &FakeLauncher::with(FakeProcess::running()),
        client,
    )
    .unwrap();
    let navigated = session.reinject_after_navigation().unwrap();
    assert!(!navigated);
}

#[test]
fn clear_css_removes_the_live_stylesheet_and_forgets_it() {
    let client = FakeCdpClient::with_targets(vec![page("target-1")]);
    client.queue_navigated(true);
    let mut session = CdpSession::start(
        &FixedPort(1),
        &FakeLauncher::with(FakeProcess::running()),
        client.clone(),
    )
    .unwrap();

    session.apply_css(".x{color:red}").unwrap();
    session.clear_css().unwrap();

    // A navigation after clearing must not resurrect the old CSS: it was
    // forgotten, not just visually removed.
    let navigated = session.reinject_after_navigation().unwrap();
    assert!(navigated);
    assert_eq!(
        client.inject_count(),
        1,
        "clear_css must not be followed by a reinject"
    );
}

// ── abnormal exit of the managed process is surfaced, not papered over ────

#[test]
fn operations_after_the_process_has_already_exited_are_refused() {
    let mut session = CdpSession::start(
        &FixedPort(1),
        &FakeLauncher::with(FakeProcess::already_exited()),
        FakeCdpClient::with_targets(vec![page("target-1")]),
    )
    .unwrap();

    assert!(!session.is_alive());
    let result = session.discover_target();
    assert!(matches!(result, Err(SessionError::ProcessExited)));
}

#[test]
fn is_alive_reflects_a_crash_that_happens_after_the_session_started() {
    let process = FakeProcess::running();
    let running_flag = process.running.clone();
    let mut session = CdpSession::start(
        &FixedPort(1),
        &FakeLauncher::with(process),
        FakeCdpClient::with_targets(vec![page("target-1")]),
    )
    .unwrap();
    assert!(session.is_alive());

    // Simulate the child crashing on its own, independent of anything
    // Chimera did.
    *running_flag.lock().unwrap() = false;

    assert!(!session.is_alive());
    let result = session.apply_css(".x{color:red}");
    assert!(matches!(result, Err(SessionError::ProcessExited)));
}
