//! Provider SQLite repository.
//! Key不存 DB：只存 secret_ref（OS keychain 引用字符串）。
use chimera_domain::{ProviderHealth, ProviderKind, ProviderProtocol};
use rusqlite::{Connection, params};
use std::path::Path;
use url::Url;
use uuid::Uuid;

const CURRENT_SCHEMA_VERSION: i64 = 1;

/// DB 中一行供应商（无明文 API Key）。
#[derive(Debug, Clone)]
pub struct ProviderRow {
    pub id: Uuid,
    pub display_name: String,
    pub kind: ProviderKind,
    pub base_url: Url,
    pub protocol: ProviderProtocol,
    /// OS keychain 引用，非 key 本身。None = Official 系统模式。
    pub secret_ref: Option<String>,
    pub selected_model: Option<String>,
    pub health: ProviderHealth,
    pub sort_order: i64,
}

pub struct ProviderDb {
    conn: Connection,
}

impl ProviderDb {
    pub fn open<P: AsRef<Path>>(path: P) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _schema_version (version INTEGER NOT NULL);",
        )?;
        let ver: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version),0) FROM _schema_version",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if ver < CURRENT_SCHEMA_VERSION {
            self.conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS providers (
                    id           TEXT    NOT NULL PRIMARY KEY,
                    display_name TEXT    NOT NULL,
                    kind         TEXT    NOT NULL,
                    base_url     TEXT    NOT NULL,
                    protocol     TEXT    NOT NULL DEFAULT 'responses',
                    secret_ref   TEXT,
                    selected_model TEXT,
                    health       TEXT    NOT NULL DEFAULT 'unknown',
                    sort_order   INTEGER NOT NULL DEFAULT 0
                );",
            )?;
            self.conn.execute(
                "INSERT INTO _schema_version(version) VALUES(?1)",
                [CURRENT_SCHEMA_VERSION],
            )?;
        }
        Ok(())
    }

    pub fn schema_version(&self) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COALESCE(MAX(version),0) FROM _schema_version",
            [],
            |r| r.get(0),
        )
    }

    pub fn insert(&self, row: &ProviderRow) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO providers
             (id, display_name, kind, base_url, protocol, secret_ref, selected_model, health, sort_order)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                row.id.to_string(),
                row.display_name,
                kind_to_str(&row.kind),
                row.base_url.to_string(),
                protocol_to_str(&row.protocol),
                row.secret_ref,
                row.selected_model,
                health_to_str(&row.health),
                row.sort_order,
            ],
        )?;
        Ok(())
    }

    pub fn list_all(&self) -> rusqlite::Result<Vec<ProviderRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,display_name,kind,base_url,protocol,secret_ref,selected_model,health,sort_order
             FROM providers ORDER BY sort_order ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ProviderRow {
                id: Uuid::parse_str(&r.get::<_, String>(0)?).unwrap(),
                display_name: r.get(1)?,
                kind: str_to_kind(&r.get::<_, String>(2)?),
                base_url: r.get::<_, String>(3)?.parse().unwrap(),
                protocol: str_to_protocol(&r.get::<_, String>(4)?),
                secret_ref: r.get(5)?,
                selected_model: r.get(6)?,
                health: str_to_health(&r.get::<_, String>(7)?),
                sort_order: r.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_by_id(&self, id: Uuid) -> rusqlite::Result<Option<ProviderRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,display_name,kind,base_url,protocol,secret_ref,selected_model,health,sort_order
             FROM providers WHERE id=?1",
        )?;
        let mut rows = stmt.query_map([id.to_string()], |r| {
            Ok(ProviderRow {
                id: Uuid::parse_str(&r.get::<_, String>(0)?).unwrap(),
                display_name: r.get(1)?,
                kind: str_to_kind(&r.get::<_, String>(2)?),
                base_url: r.get::<_, String>(3)?.parse().unwrap(),
                protocol: str_to_protocol(&r.get::<_, String>(4)?),
                secret_ref: r.get(5)?,
                selected_model: r.get(6)?,
                health: str_to_health(&r.get::<_, String>(7)?),
                sort_order: r.get(8)?,
            })
        })?;
        rows.next().transpose()
    }

    pub fn update_health(&self, id: Uuid, health: ProviderHealth) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE providers SET health=?1 WHERE id=?2",
            params![health_to_str(&health), id.to_string()],
        )?;
        Ok(())
    }

    /// Put one provider first in the stable list so the active projection can
    /// be recovered after a restart without storing a credential in settings.
    pub fn mark_active(&self, id: Uuid) -> rusqlite::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("UPDATE providers SET sort_order = sort_order + 1", [])?;
        tx.execute(
            "UPDATE providers SET sort_order = 0 WHERE id = ?1",
            [id.to_string()],
        )?;
        tx.commit()
    }

    /// Remove the active marker while preserving deterministic provider order.
    pub fn clear_active(&self) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE providers SET sort_order = sort_order + 1", [])?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM providers WHERE id=?1", [id.to_string()])?;
        Ok(())
    }
}

// ── serde helpers ─────────────────────────────────────────────────────────────

fn kind_to_str(k: &ProviderKind) -> &'static str {
    match k {
        ProviderKind::ChimeraHub => "chimera_hub",
        ProviderKind::Custom => "custom",
    }
}
fn str_to_kind(s: &str) -> ProviderKind {
    match s {
        "chimera_hub" => ProviderKind::ChimeraHub,
        _ => ProviderKind::Custom,
    }
}
fn protocol_to_str(p: &ProviderProtocol) -> &'static str {
    match p {
        ProviderProtocol::Responses => "responses",
    }
}
fn str_to_protocol(_s: &str) -> ProviderProtocol {
    ProviderProtocol::Responses
}
fn health_to_str(h: &ProviderHealth) -> &'static str {
    match h {
        ProviderHealth::Unknown => "unknown",
        ProviderHealth::Healthy => "healthy",
        ProviderHealth::AuthFailed => "auth_failed",
        ProviderHealth::Incompatible => "incompatible",
        ProviderHealth::Unreachable => "unreachable",
    }
}
fn str_to_health(s: &str) -> ProviderHealth {
    match s {
        "healthy" => ProviderHealth::Healthy,
        "auth_failed" => ProviderHealth::AuthFailed,
        "incompatible" => ProviderHealth::Incompatible,
        "unreachable" => ProviderHealth::Unreachable,
        _ => ProviderHealth::Unknown,
    }
}
