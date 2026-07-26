// Step 9.1 RED — the network seam.
//
// A trait, not a concrete HTTP client: every test in this crate must run
// offline and deterministically, and "offline" itself needs to be a case a
// test can construct without touching a socket.

use chimera_update::fetch::{FetchError, MetadataFetcher};
use chimera_update::metadata::{MetaSignature, SignedPayload};

struct AlwaysOffline;

impl MetadataFetcher for AlwaysOffline {
    fn fetch_root_next(&self, _after_version: u64) -> Result<Option<SignedPayload>, FetchError> {
        Err(FetchError::Offline)
    }
    fn fetch_timestamp(&self) -> Result<SignedPayload, FetchError> {
        Err(FetchError::Offline)
    }
    fn fetch_snapshot(&self) -> Result<SignedPayload, FetchError> {
        Err(FetchError::Offline)
    }
    fn fetch_targets(&self) -> Result<SignedPayload, FetchError> {
        Err(FetchError::Offline)
    }
    fn fetch_target_file(&self, _path: &str) -> Result<Vec<u8>, FetchError> {
        Err(FetchError::Offline)
    }
}

#[test]
fn a_fetcher_can_report_offline_without_any_real_network_access() {
    let f = AlwaysOffline;
    assert_eq!(f.fetch_timestamp().unwrap_err(), FetchError::Offline);
    assert_eq!(f.fetch_root_next(1).unwrap_err(), FetchError::Offline);
}

struct FixedFetcher;

impl MetadataFetcher for FixedFetcher {
    fn fetch_root_next(&self, after_version: u64) -> Result<Option<SignedPayload>, FetchError> {
        if after_version >= 1 {
            return Ok(None);
        }
        Ok(Some(SignedPayload {
            payload: "{}".to_string(),
            signatures: vec![MetaSignature {
                key_id: "k".to_string(),
                signature_hex: String::new(),
            }],
        }))
    }
    fn fetch_timestamp(&self) -> Result<SignedPayload, FetchError> {
        Ok(SignedPayload {
            payload: "{}".to_string(),
            signatures: vec![],
        })
    }
    fn fetch_snapshot(&self) -> Result<SignedPayload, FetchError> {
        Ok(SignedPayload {
            payload: "{}".to_string(),
            signatures: vec![],
        })
    }
    fn fetch_targets(&self) -> Result<SignedPayload, FetchError> {
        Ok(SignedPayload {
            payload: "{}".to_string(),
            signatures: vec![],
        })
    }
    fn fetch_target_file(&self, _path: &str) -> Result<Vec<u8>, FetchError> {
        Ok(b"hello".to_vec())
    }
}

#[test]
fn a_fetcher_reporting_no_next_root_signals_rotation_is_complete() {
    let f = FixedFetcher;
    assert!(f.fetch_root_next(0).unwrap().is_some());
    assert!(f.fetch_root_next(1).unwrap().is_none());
}
