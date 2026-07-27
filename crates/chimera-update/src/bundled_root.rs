//! The compiled-in production trust root for a fresh install (Step 9.1).
//!
//! The private root key was generated during an offline ceremony and is kept
//! outside the repository. Only the public root metadata and its self-
//! signature are embedded here. Role keys are intentionally independent so a
//! compromise of an online metadata signer cannot mint a new root.

use thiserror::Error;

use crate::metadata::{RootMetadata, SignedPayload};

const BUNDLED_ROOT_PAYLOAD: &str = r#"{"domain":"chimera-app-update.v1","version":1,"expires":2082758400,"keys":[{"key_id":"chimera-app-root-v1","public_key_hex":"68dd01e433a3d7e580c01ce8e7e7c48da7edacc3fb08e2f029f2159c8e766dc4"},{"key_id":"chimera-app-targets-v1","public_key_hex":"006761e12e85809e3103f1e3a8ff5756cc5b44661681fe798a3750161a43c241"},{"key_id":"chimera-app-snapshot-v1","public_key_hex":"5e1dd65242ef7be06c2736b052eb3c1a773fd816d86b29125b72a5969e31c0c4"},{"key_id":"chimera-app-timestamp-v1","public_key_hex":"166f3fdcf7425b96633ff24c5c66b799b04ec44ef87567cf197618cae72630aa"}],"root":{"key_ids":["chimera-app-root-v1"],"threshold":1},"targets":{"key_ids":["chimera-app-targets-v1"],"threshold":1},"snapshot":{"key_ids":["chimera-app-snapshot-v1"],"threshold":1},"timestamp":{"key_ids":["chimera-app-timestamp-v1"],"threshold":1}}"#;
const BUNDLED_ROOT_SIGNATURE_HEX: &str = "01776f2fe7f75ee2001b93647e7ae32da09ef1f584be2fcd218e298017a909a9c6173cea9160e6a2696d8509aea261236199876691a560d0fd8823526c9c8807";
const BUNDLED_ROOT_KEY_ID: &str = "chimera-app-root-v1";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BundledRootError {
    #[error("the compiled-in trust root is malformed")]
    Malformed,
}

/// Return the production root trusted by a fresh installation.
pub fn bundled_root() -> Result<SignedPayload, BundledRootError> {
    let root: RootMetadata =
        serde_json::from_str(BUNDLED_ROOT_PAYLOAD).map_err(|_| BundledRootError::Malformed)?;
    root.validate_shape()
        .map_err(|_| BundledRootError::Malformed)?;
    Ok(SignedPayload {
        payload: BUNDLED_ROOT_PAYLOAD.to_string(),
        signatures: vec![crate::metadata::MetaSignature {
            key_id: BUNDLED_ROOT_KEY_ID.to_string(),
            signature_hex: BUNDLED_ROOT_SIGNATURE_HEX.to_string(),
        }],
    })
}

/// Backward-compatible name for callers that used the pre-release helper.
#[deprecated(note = "use bundled_root; this is the production root")]
pub fn development_root() -> Result<SignedPayload, BundledRootError> {
    bundled_root()
}

/// Detect the old development placeholder if a stale cached document is ever
/// presented. A production root can never use this reserved identifier.
pub fn is_development_root(root: &RootMetadata) -> bool {
    root.keys.iter().any(|key| {
        key.key_id == "chimera-dev-insecure-DO-NOT-SHIP-root-1"
            || key.key_id.contains("dev-insecure")
    })
}
