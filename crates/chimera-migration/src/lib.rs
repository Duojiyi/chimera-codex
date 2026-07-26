//! Chimera++ 2.0 — Task 7 (T47): read-only migration from Chimera++ 1.x and
//! CC Switch, plus coexistence protection against other managers.
//!
//! Layering (scripts/verify-v2-architecture.mjs): this is a layer-2 adapter
//! crate. It depends on `chimera-domain` (layer 0) and `chimera-platform`
//! (layer 1) only. It must NOT depend on `chimera-provider`, `chimera-runtime`
//! or `chimera-theme` — those are sibling adapters, and adapter-to-adapter
//! dependencies are banned (G1). Where this crate needs to write a provider
//! row or a keychain secret, it defines a narrow port in [`ports`] instead;
//! the desktop shell wires the real `chimera-provider` implementations
//! through those ports.
//!
//! Everything here that reads a 1.x/CC-Switch file is read-only by
//! construction — nothing in this crate ever opens a source path for
//! writing. Every filesystem or process fact this crate needs is passed in
//! explicitly (paths, marker presence, running-process names) so tests never
//! touch a real user profile.

pub mod ccswitch_source;
pub mod coexistence;
pub mod legacy_source;
pub mod migrate;
pub mod ports;
mod secret;

pub use secret::RedactedSecret;
