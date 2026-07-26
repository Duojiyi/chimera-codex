//! Chimera++ 2.0 — Mirror manifest schema and stable CAS logic.
//! The actual mirror deployment requires Release Gate R4 authorization.
//! This crate provides the contract layer: types, verification, CAS promotion.

pub mod capability;
pub mod cas;
pub mod manifest;
