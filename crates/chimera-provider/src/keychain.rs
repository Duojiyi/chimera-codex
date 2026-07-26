//! Keychain port — OS keychain abstraction with in-memory test double.
//! Key 永远不进 DB、不进日志、不进 fixture。
//! 生产代码通过 trait object 注入，测试用 MemoryKeychain 替身。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("keychain access failed: {0}")]
    Access(String),
}

/// OS keychain 引用字符串（不含 key 本身）。
/// Debug 不暴露 key。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SecretRef(String);

impl SecretRef {
    pub fn new(r: impl Into<String>) -> Self {
        Self(r.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Debug 只显示引用，不显示 key 值
impl std::fmt::Debug for SecretRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretRef({})", self.0)
    }
}

impl std::fmt::Display for SecretRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Keychain 操作 trait — 使 Provider Engine 可测试（注入 MemoryKeychain）。
pub trait KeychainPort: Send + Sync {
    /// 存储/更新 secret，返回 OS keychain 引用。
    fn store(&self, service_name: &str, secret: &str) -> Result<SecretRef, KeychainError>;
    /// 检索 secret；None = 不存在。
    fn retrieve(&self, r: &SecretRef) -> Result<Option<String>, KeychainError>;
    /// 删除 secret。
    fn delete(&self, r: &SecretRef) -> Result<(), KeychainError>;
}

/// 纯内存 test double — 永远不触碰 OS keychain。
#[derive(Clone, Default)]
pub struct MemoryKeychain {
    store: Arc<Mutex<HashMap<String, String>>>,
}

impl MemoryKeychain {
    pub fn new() -> Self {
        Self::default()
    }
}

impl KeychainPort for MemoryKeychain {
    fn store(&self, service_name: &str, secret: &str) -> Result<SecretRef, KeychainError> {
        let r = SecretRef::new(format!("{REF_PREFIX}{service_name}"));
        self.store
            .lock()
            .unwrap()
            .insert(r.as_str().to_string(), secret.to_string());
        Ok(r)
    }

    fn retrieve(&self, r: &SecretRef) -> Result<Option<String>, KeychainError> {
        Ok(self.store.lock().unwrap().get(r.as_str()).cloned())
    }

    fn delete(&self, r: &SecretRef) -> Result<(), KeychainError> {
        self.store.lock().unwrap().remove(r.as_str());
        Ok(())
    }
}

// ── Real OS keychain ─────────────────────────────────────────────────────────

/// Prefix shared by every Chimera secret reference. Kept identical to
/// `MemoryKeychain` so a `SecretRef` produced by either implementation is
/// interchangeable — the DB stores only this handle (G4).
const REF_PREFIX: &str = "keychain://chimera/";

/// Account name under the service entry. Chimera stores exactly one secret per
/// provider, so a fixed account keeps the entry addressable by service alone.
const KEYCHAIN_ACCOUNT: &str = "chimera";

/// Windows Credential Manager / macOS Keychain / Linux Secret Service, via the
/// `keyring` crate's native backends (ADR-007: one code path per platform).
///
/// The key itself never leaves this type: callers hold a [`SecretRef`], and the
/// value is only materialised at the moment it is projected into Codex config.
#[derive(Clone, Default)]
pub struct OsKeychain;

impl OsKeychain {
    pub fn new() -> Self {
        Self
    }

    /// Build the OS-level entry for a service name.
    fn entry(service_name: &str) -> Result<keyring::Entry, KeychainError> {
        // Namespace the OS entry so Chimera can never collide with, or read,
        // another application's credentials.
        let full = format!("Chimera++/{service_name}");
        keyring::Entry::new(&full, KEYCHAIN_ACCOUNT)
            .map_err(|e| KeychainError::Access(e.to_string()))
    }

    /// Recover the service name from a reference produced by `store`.
    /// Extract the service name from a ref, or `None` when the ref is not one
    /// this backend issued.
    ///
    /// Returns `Option` rather than `Result` so an unrecognised ref behaves the
    /// same as a missing credential. `MemoryKeychain` reports a ref it never
    /// stored as a hash miss (`Ok(None)`), and the two implementations must be
    /// substitutable behind `KeychainPort` — a backend-dependent Err/None split
    /// would mean tests passing against the double while production errors.
    ///
    /// Deliberately never echoes the ref: it can be attacker-influenced and the
    /// message reaches logs.
    fn service_of(r: &SecretRef) -> Option<&str> {
        r.as_str().strip_prefix(REF_PREFIX)
    }
}

impl KeychainPort for OsKeychain {
    fn store(&self, service_name: &str, secret: &str) -> Result<SecretRef, KeychainError> {
        Self::entry(service_name)?
            .set_password(secret)
            .map_err(|e| KeychainError::Access(e.to_string()))?;
        Ok(SecretRef::new(format!("{REF_PREFIX}{service_name}")))
    }

    fn retrieve(&self, r: &SecretRef) -> Result<Option<String>, KeychainError> {
        let Some(service) = Self::service_of(r) else {
            return Ok(None);
        };
        match Self::entry(service)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            // A missing entry is a normal state (user revoked the key, or the
            // DB row outlived the credential), not an error the UI should show.
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(KeychainError::Access(e.to_string())),
        }
    }

    fn delete(&self, r: &SecretRef) -> Result<(), KeychainError> {
        // A ref we did not mint cannot name a credential we own, so there is
        // nothing to remove. Deleting is idempotent, so this is success.
        let Some(service) = Self::service_of(r) else {
            return Ok(());
        };
        match Self::entry(service)?.delete_credential() {
            Ok(()) => Ok(()),
            // Idempotent: deleting an absent secret is success, so provider
            // removal never strands a row because cleanup already happened.
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(KeychainError::Access(e.to_string())),
        }
    }
}
