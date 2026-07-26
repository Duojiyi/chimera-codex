//! chimera-update — Chimera's own TUF-style updater, trust-isolated from the
//! Codex mirror (ADR-006, G8, G15).
//!
//! This crate verifies and decides; it does not fetch over a real network and
//! does not install anything itself. [`fetch::MetadataFetcher`] is a trait so
//! every test in this crate runs offline and deterministically, and the
//! decision this crate produces ([`app_target::UpdateDecision`]) is inert data
//! that a caller in `apps/chimera-desktop` must still act on explicitly.
//!
//! Module map:
//! - [`signature`] — Ed25519 primitive, deliberately re-implemented rather
//!   than shared with `services/mirror-contract` (see its doc comment for why).
//! - [`clock`] — injectable "now", because expiry/freeze-attack checks must
//!   never read the system clock directly.
//! - [`metadata`] — the TUF-style document shapes (root/timestamp/snapshot/
//!   targets) and the trust-domain tag that keeps this chain from ever being
//!   satisfied by a Codex mirror document.
//! - [`fetch`] — the network seam.
//! - [`cache`] — persisted trust state, namespaced so it can never collide
//!   with the mirror's own cache.
//! - [`bundled_root`] — the compiled-in trust seed a fresh install starts from.
//! - [`trust`] — the verification chain: root rotation, then timestamp,
//!   snapshot, targets, each checked for expiry, rollback and domain.
//! - [`app_target`] — `chimera-app-latest.json` as a pinned target, and the
//!   version/downgrade decision.
//! - [`atomic`] — crash-safe settings/ownership/transaction state (Step 9.2).
//! - [`redact`] — the pure secret-redaction function (Step 9.3).
//! - [`diagnostics`] — error classification, log rotation, and the
//!   twice-redacted diagnostics preview (Step 9.3).

pub mod app_target;
pub mod atomic;
pub mod bundled_root;
pub mod cache;
pub mod clock;
pub mod diagnostics;
pub mod fetch;
pub mod metadata;
pub mod redact;
pub mod signature;
pub mod trust;
