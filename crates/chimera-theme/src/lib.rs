//! Chimera++ 2.0 — restricted skin engine (Task 8 / T48, ADR-005).
//!
//! `.codexskin` is a declarative, restricted package format: CSS + image/font
//! assets, no script, no executables, no remote resources. This crate parses
//! and safely extracts that package, applies it through a loopback-only CDP
//! session Chimera itself owns, and keeps a local skin-state transaction that
//! never touches the official Codex install.
//!
//! Layering (`scripts/verify-v2-architecture.mjs`, V15): this crate is an
//! adapter (L2) and may depend only on `chimera-domain` (L0) and
//! `chimera-platform` (L1) — never `chimera-runtime`/`chimera-provider`
//! (sibling adapters) and never `mirror-contract` (also L2). Anything this
//! crate needs from those layers is expressed as a narrow port (a plain
//! closure or trait parameter) that the caller supplies; see the module docs
//! on `session` and `fingerprint` for the specific seams.

pub mod apply;
pub mod cdp_transport;
pub mod css_allowlist;
pub mod fingerprint;
pub mod package;
pub mod schema;
pub mod session;
