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
        let r = SecretRef::new(format!("keychain://chimera/{service_name}"));
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
