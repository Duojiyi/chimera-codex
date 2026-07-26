// Step 2.6 RED — real OS keychain backend.
//
// G4: the API key lives in the OS credential store and is referenced only by an
// opaque handle. MemoryKeychain proved the port; this proves the real backend.
//
// These tests touch the developer's actual credential store, so they are gated
// behind CHIMERA_TEST_OS_KEYCHAIN=1 and always clean up after themselves.
// Run with:  CHIMERA_TEST_OS_KEYCHAIN=1 cargo test -p chimera-provider --test step2_6_os_keychain
use chimera_provider::keychain::{KeychainPort, OsKeychain, SecretRef};

/// Unique per run so a crashed earlier run can never collide with this one.
fn unique_service() -> String {
    format!(
        "chimera-test/{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

fn enabled() -> bool {
    std::env::var("CHIMERA_TEST_OS_KEYCHAIN").as_deref() == Ok("1")
}

#[test]
fn store_then_retrieve_returns_the_same_secret() {
    if !enabled() {
        eprintln!("skipped: set CHIMERA_TEST_OS_KEYCHAIN=1 to run against the real store");
        return;
    }
    let kc = OsKeychain::new();
    let service = unique_service();

    let r = kc.store(&service, "sk-real-secret-value").expect("store");
    let got = kc.retrieve(&r).expect("retrieve");
    kc.delete(&r).ok(); // cleanup before asserting so a failure still cleans up

    assert_eq!(got.as_deref(), Some("sk-real-secret-value"));
}

#[test]
fn retrieve_after_delete_returns_none() {
    if !enabled() {
        return;
    }
    let kc = OsKeychain::new();
    let service = unique_service();

    let r = kc.store(&service, "to-be-deleted").expect("store");
    kc.delete(&r).expect("delete");
    let got = kc.retrieve(&r).expect("retrieve after delete");

    assert_eq!(got, None, "a deleted secret must not be retrievable");
}

#[test]
fn retrieve_unknown_reference_returns_none_not_error() {
    if !enabled() {
        return;
    }
    let kc = OsKeychain::new();
    // Never stored — the port contract says absent is None, not Err.
    let got = kc
        .retrieve(&SecretRef::new(unique_service()))
        .expect("absent secret must not be an error");
    assert_eq!(got, None);
}

#[test]
fn delete_is_idempotent() {
    if !enabled() {
        return;
    }
    let kc = OsKeychain::new();
    let service = unique_service();

    let r = kc.store(&service, "x").expect("store");
    kc.delete(&r).expect("first delete");
    // Deleting again must succeed: recovery paths call delete without checking.
    kc.delete(&r).expect("second delete must be idempotent");
}

#[test]
fn overwriting_a_secret_keeps_one_entry_with_the_new_value() {
    if !enabled() {
        return;
    }
    let kc = OsKeychain::new();
    let service = unique_service();

    let r1 = kc.store(&service, "old-value").expect("first store");
    let r2 = kc.store(&service, "new-value").expect("overwrite");
    let got = kc.retrieve(&r2).expect("retrieve");
    kc.delete(&r2).ok();

    assert_eq!(
        r1.as_str(),
        r2.as_str(),
        "same service must map to same ref"
    );
    assert_eq!(got.as_deref(), Some("new-value"));
}

// ── Contract tests that need no real store ───────────────────────────────────

#[test]
fn os_keychain_debug_never_prints_secret_material() {
    // Runs unconditionally: a Debug leak is a code defect, not an environment one.
    let r = SecretRef::new("chimera/provider-abc");
    let shown = format!("{r:?}");
    assert!(
        shown.contains("chimera/provider-abc"),
        "ref should be visible"
    );
    assert!(!shown.contains("sk-"), "no key material in Debug output");
}

#[test]
fn os_keychain_satisfies_the_keychain_port_trait() {
    // Compile-time proof the real backend is substitutable for the test double,
    // so callers depend on the port and never on a concrete store.
    fn takes_port(_: &dyn KeychainPort) {}
    let kc = OsKeychain::new();
    takes_port(&kc);
}
