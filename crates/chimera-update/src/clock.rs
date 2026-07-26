//! Injectable "now" for trust-chain verification.
//!
//! Every expiry/rollback check in [`crate::trust`] takes a `now: i64`
//! produced through this trait rather than calling `SystemTime::now()`
//! itself. That is the only way a freeze-attack test (valid signature, only
//! the expiry has passed) or a rollback test can exist without a test suite
//! that sleeps for real — and it is the only way production code can be
//! trusted to have made the same choice, since [`SystemClock`] is the single
//! place that reads the OS clock at all.

use std::time::{SystemTime, UNIX_EPOCH};

/// A source of "now", expressed as Unix seconds.
pub trait Clock {
    fn now(&self) -> i64;
}

/// The real clock. Used exactly once in this crate's non-test code path —
/// everywhere else takes `now` as a parameter.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            // A clock set before 1970 tells verification "everything has
            // expired" rather than panicking — fail closed even here.
            .unwrap_or(0)
    }
}

/// A clock that always reports the value it was constructed with.
///
/// Not `#[cfg(test)]`-gated: dry-run tooling outside this crate benefits from
/// deterministic time too, and gating it would force every test in this crate
/// into `tests/` integration files instead of unit tests colocated with the
/// code they check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now(&self) -> i64 {
        self.0
    }
}
