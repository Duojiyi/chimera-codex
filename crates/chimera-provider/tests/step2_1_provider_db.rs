// Step 2.1 RED — Provider SQLite repository tests.
// Run: cargo test -p chimera-provider
use chimera_domain::{ProviderHealth, ProviderKind, ProviderProtocol};
use chimera_provider::db::{ProviderDb, ProviderRow};
use tempfile::tempdir;
use url::Url;
use uuid::Uuid;

fn test_db(dir: &std::path::Path) -> ProviderDb {
    ProviderDb::open(dir.join("test.db")).expect("open test db")
}

// ── CRUD ────────────────────────────────────────────────────────────────────

#[test]
fn insert_and_list_providers() {
    let tmp = tempdir().unwrap();
    let db = test_db(tmp.path());

    let id = Uuid::new_v4();
    db.insert(&ProviderRow {
        id,
        display_name: "ChimeraHub".into(),
        kind: ProviderKind::ChimeraHub,
        base_url: Url::parse("https://api.chimerahub.io/v1").unwrap(),
        protocol: ProviderProtocol::Responses,
        secret_ref: Some("keychain://chimera/chimerahub".into()),
        selected_model: Some("gpt-4o".into()),
        health: ProviderHealth::Unknown,
        sort_order: 0,
    })
    .expect("insert");

    let rows = db.list_all().expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].display_name, "ChimeraHub");
    assert_eq!(rows[0].kind, ProviderKind::ChimeraHub);
    assert!(rows[0].secret_ref.is_some());
}

#[test]
fn update_health_does_not_touch_secret_ref() {
    let tmp = tempdir().unwrap();
    let db = test_db(tmp.path());
    let id = Uuid::new_v4();
    db.insert(&ProviderRow {
        id,
        display_name: "MyAPI".into(),
        kind: ProviderKind::Custom,
        base_url: Url::parse("https://api.example.com/v1").unwrap(),
        protocol: ProviderProtocol::Responses,
        secret_ref: Some("keychain://chimera/myapi".into()),
        selected_model: None,
        health: ProviderHealth::Unknown,
        sort_order: 0,
    })
    .unwrap();

    db.update_health(id, ProviderHealth::Healthy)
        .expect("update health");

    let row = db.get_by_id(id).expect("get").expect("found");
    assert_eq!(row.health, ProviderHealth::Healthy);
    // secret_ref must survive a health update
    assert!(
        row.secret_ref.is_some(),
        "secret_ref must not be wiped by health update"
    );
}

#[test]
fn delete_removes_provider() {
    let tmp = tempdir().unwrap();
    let db = test_db(tmp.path());
    let id = Uuid::new_v4();
    db.insert(&ProviderRow {
        id,
        display_name: "Gone".into(),
        kind: ProviderKind::Custom,
        base_url: Url::parse("https://api.gone.io/v1").unwrap(),
        protocol: ProviderProtocol::Responses,
        secret_ref: None,
        selected_model: None,
        health: ProviderHealth::Unknown,
        sort_order: 0,
    })
    .unwrap();
    db.delete(id).expect("delete");
    assert!(
        db.get_by_id(id).unwrap().is_none(),
        "deleted provider must not be found"
    );
}

#[test]
fn chimera_hub_is_only_builtin_kind() {
    let tmp = tempdir().unwrap();
    let db = test_db(tmp.path());
    // Seed two providers
    for (name, kind) in [
        ("ChimeraHub", ProviderKind::ChimeraHub),
        ("Custom1", ProviderKind::Custom),
    ] {
        db.insert(&ProviderRow {
            id: Uuid::new_v4(),
            display_name: name.into(),
            kind,
            base_url: Url::parse("https://api.example.com/v1").unwrap(),
            protocol: ProviderProtocol::Responses,
            secret_ref: None,
            selected_model: None,
            health: ProviderHealth::Unknown,
            sort_order: 0,
        })
        .unwrap();
    }
    let hubs: Vec<_> = db
        .list_all()
        .unwrap()
        .into_iter()
        .filter(|r| r.kind == ProviderKind::ChimeraHub)
        .collect();
    assert_eq!(hubs.len(), 1, "Exactly one ChimeraHub row allowed");
}

// ── schema migration ─────────────────────────────────────────────────────────

#[test]
fn db_schema_version_is_recorded() {
    let tmp = tempdir().unwrap();
    let db = test_db(tmp.path());
    let ver = db.schema_version().expect("schema_version");
    assert!(ver >= 1, "schema version must be ≥1 after migration");
}

#[test]
fn reopening_db_applies_pending_migrations() {
    let tmp = tempdir().unwrap();
    {
        let _ = test_db(tmp.path()); // create + migrate
    }
    // Re-open; migrations must be idempotent
    let db2 = test_db(tmp.path());
    let ver2 = db2.schema_version().unwrap();
    assert!(ver2 >= 1);
}

// ── no plaintext key in db ───────────────────────────────────────────────────

#[test]
fn provider_row_has_no_api_key_field() {
    // Compile-time check: ProviderRow must NOT have an `api_key` field.
    // If this test compiles, the field does not exist.
    let row = ProviderRow {
        id: Uuid::new_v4(),
        display_name: "X".into(),
        kind: ProviderKind::Custom,
        base_url: Url::parse("https://a.example.com/v1").unwrap(),
        protocol: ProviderProtocol::Responses,
        secret_ref: None,
        selected_model: None,
        health: ProviderHealth::Unknown,
        sort_order: 0,
    };
    // Only secret_ref (keychain reference) is allowed — not the key itself.
    let _ = row.secret_ref;
}
