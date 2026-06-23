use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use youai_common::{NodeInfo, NODE_STALE_SECS};

#[derive(Debug, Clone)]
pub struct StoredNode {
    pub id: String,
    pub token: String,
    pub name: String,
    pub region: String,
    pub worker_url: String,
    pub model: String,
    pub last_heartbeat: i64,
    pub created_at: i64,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("open db {}", path.display()))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                token TEXT NOT NULL,
                name TEXT NOT NULL,
                region TEXT NOT NULL DEFAULT '',
                worker_url TEXT NOT NULL,
                model TEXT NOT NULL,
                last_heartbeat INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    pub fn upsert_node(&self, node: &StoredNode) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO nodes (id, token, name, region, worker_url, model, last_heartbeat, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
                token = excluded.token,
                name = excluded.name,
                region = excluded.region,
                worker_url = excluded.worker_url,
                model = excluded.model,
                last_heartbeat = excluded.last_heartbeat
            "#,
            params![
                node.id,
                node.token,
                node.name,
                node.region,
                node.worker_url,
                node.model,
                node.last_heartbeat,
                node.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn heartbeat(&self, node_id: &str, token: &str) -> Result<bool> {
        let now = Utc::now().timestamp();
        let updated = self.conn.execute(
            "UPDATE nodes SET last_heartbeat = ?1 WHERE id = ?2 AND token = ?3",
            params![now, node_id, token],
        )?;
        Ok(updated == 1)
    }

    #[allow(dead_code)]
    pub fn get_node(&self, node_id: &str) -> Result<Option<StoredNode>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, token, name, region, worker_url, model, last_heartbeat, created_at FROM nodes WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![node_id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(StoredNode {
                id: row.get(0)?,
                token: row.get(1)?,
                name: row.get(2)?,
                region: row.get(3)?,
                worker_url: row.get(4)?,
                model: row.get(5)?,
                last_heartbeat: row.get(6)?,
                created_at: row.get(7)?,
            }));
        }
        Ok(None)
    }

    pub fn list_nodes(&self) -> Result<Vec<NodeInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, region, worker_url, model, last_heartbeat FROM nodes ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let last_heartbeat: i64 = row.get(5)?;
            Ok(NodeInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                region: row.get(2)?,
                worker_url: row.get(3)?,
                model: row.get(4)?,
                last_heartbeat,
                online: is_online(last_heartbeat),
            })
        })?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row?);
        }
        Ok(nodes)
    }

    pub fn online_nodes(&self) -> Result<Vec<StoredNode>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, token, name, region, worker_url, model, last_heartbeat, created_at FROM nodes",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredNode {
                id: row.get(0)?,
                token: row.get(1)?,
                name: row.get(2)?,
                region: row.get(3)?,
                worker_url: row.get(4)?,
                model: row.get(5)?,
                last_heartbeat: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;

        let cutoff = Utc::now().timestamp() - NODE_STALE_SECS;
        let mut nodes = Vec::new();
        for row in rows {
            let node = row?;
            if node.last_heartbeat >= cutoff {
                nodes.push(node);
            }
        }
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(nodes)
    }
}

fn is_online(last_heartbeat: i64) -> bool {
    let cutoff = Utc::now().timestamp() - NODE_STALE_SECS;
    last_heartbeat >= cutoff
}