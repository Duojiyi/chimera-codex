// Step 2.3 RED — Keychain port (secret management abstraction).
// Key 永远不进 DB、不进日志、不进 fixture。
// 通过 trait 注入 test double，真实 OS keychain 只在 integration 时用。
use chimera_provider::keychain::{KeychainPort, MemoryKeychain, SecretRef};

// ── 基本存取 ─────────────────────────────────────────────────────────────────

#[test]
fn store_and_retrieve_secret() {
    let kc = MemoryKeychain::new();
    let r = kc.store("chimera/test-provider", "sk-test-abc123").unwrap();
    let key = kc.retrieve(&r).unwrap().expect("must find stored secret");
    assert_eq!(key, "sk-test-abc123");
}

#[test]
fn overwrite_secret_returns_same_ref() {
    let kc = MemoryKeychain::new();
    let r1 = kc.store("chimera/prov", "key-v1").unwrap();
    let r2 = kc.store("chimera/prov", "key-v2").unwrap();
    // Same service name → same ref, updated value
    assert_eq!(r1, r2);
    let val = kc.retrieve(&r2).unwrap().unwrap();
    assert_eq!(val, "key-v2");
}

#[test]
fn delete_removes_secret() {
    let kc = MemoryKeychain::new();
    let r = kc.store("chimera/del", "secret").unwrap();
    kc.delete(&r).unwrap();
    let result = kc.retrieve(&r).unwrap();
    assert!(result.is_none(), "deleted secret must return None");
}

#[test]
fn retrieve_nonexistent_returns_none() {
    let kc = MemoryKeychain::new();
    let fake_ref = SecretRef::new("keychain://chimera/nonexistent");
    let result = kc.retrieve(&fake_ref).unwrap();
    assert!(result.is_none());
}

// ── API Key 绝不进日志 ────────────────────────────────────────────────────────

#[test]
fn secret_ref_debug_does_not_expose_key() {
    let kc = MemoryKeychain::new();
    let secret_ref = kc
        .store("chimera/sensitive", "sk-REAL-SECRET-KEY-HERE")
        .unwrap();
    // Debug output of SecretRef must NOT contain the actual key value
    let debug_str = format!("{:?}", secret_ref);
    assert!(
        !debug_str.contains("sk-REAL-SECRET-KEY-HERE"),
        "SecretRef debug must not expose the key: {debug_str}"
    );
}

#[test]
fn memory_keychain_is_isolated_per_instance() {
    let kc1 = MemoryKeychain::new();
    let kc2 = MemoryKeychain::new();
    kc1.store("chimera/shared", "key-in-kc1").unwrap();
    // kc2 must not see kc1's secrets
    let fake = SecretRef::new("keychain://chimera/shared");
    let result = kc2.retrieve(&fake).unwrap();
    assert!(result.is_none(), "keychain instances must be isolated");
}
