use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, Row};
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
    pub shard_group: String,
    pub shard_stage: u8,
    pub shard_total_stages: u8,
    pub rpc_url: String,
    pub gguf_shard_index: u16,
    pub gguf_shard_total: u16,
    pub pipeline_kind: String,
    pub cpu_percent: u8,
    pub ram_max_mb: u32,
    pub contributor_score: f64,
}

pub struct Database {
    conn: Connection,
}

const NODE_SELECT: &str =
    "SELECT id, token, name, region, worker_url, model, last_heartbeat, created_at,
    shard_group, shard_stage, shard_total_stages, rpc_url, gguf_shard_index, gguf_shard_total,
    pipeline_kind, cpu_percent, ram_max_mb, contributor_score FROM nodes";

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
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
                created_at INTEGER NOT NULL,
                shard_group TEXT NOT NULL DEFAULT '',
                shard_stage INTEGER NOT NULL DEFAULT 0,
                shard_total_stages INTEGER NOT NULL DEFAULT 1
            );
            "#,
        )?;
        let _ = self.conn.execute(
            "ALTER TABLE nodes ADD COLUMN shard_group TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE nodes ADD COLUMN shard_stage INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE nodes ADD COLUMN shard_total_stages INTEGER NOT NULL DEFAULT 1",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE nodes ADD COLUMN rpc_url TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE nodes ADD COLUMN gguf_shard_index INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE nodes ADD COLUMN gguf_shard_total INTEGER NOT NULL DEFAULT 1",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE nodes ADD COLUMN pipeline_kind TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE nodes ADD COLUMN cpu_percent INTEGER NOT NULL DEFAULT 30",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE nodes ADD COLUMN ram_max_mb INTEGER NOT NULL DEFAULT 2048",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE nodes ADD COLUMN contributor_score REAL NOT NULL DEFAULT 0",
            [],
        );
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS seen_nonces (
                nonce TEXT PRIMARY KEY,
                seen_at INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    pub fn node_token(&self, node_id: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT token FROM nodes WHERE id = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![node_id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row.get(0)?));
        }
        Ok(None)
    }

    pub fn node_id_for_token(&self, token: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM nodes WHERE token = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![token])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row.get(0)?));
        }
        Ok(None)
    }

    pub fn contributor_score_for_token(&self, token: &str) -> Result<f64> {
        let mut stmt = self
            .conn
            .prepare("SELECT contributor_score FROM nodes WHERE token = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![token])?;
        if let Some(row) = rows.next()? {
            return Ok(row.get(0)?);
        }
        Ok(0.0)
    }

    pub fn network_compute_units(&self, online: &[StoredNode]) -> u64 {
        use youai_common::compute::node_compute_units;
        online
            .iter()
            .map(|n| node_compute_units(n.cpu_percent, n.ram_max_mb))
            .sum()
    }

    pub fn count_pipeline_chains(&self, online: &[StoredNode]) -> u32 {
        if self.resolve_pipeline(online, None).is_some() {
            1
        } else {
            0
        }
    }

    pub fn upsert_node(&self, node: &StoredNode) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO nodes (
                id, token, name, region, worker_url, model, last_heartbeat, created_at,
                shard_group, shard_stage, shard_total_stages, rpc_url, gguf_shard_index, gguf_shard_total,
                pipeline_kind, cpu_percent, ram_max_mb, contributor_score
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
            ON CONFLICT(id) DO UPDATE SET
                token = excluded.token,
                name = excluded.name,
                region = excluded.region,
                worker_url = excluded.worker_url,
                model = excluded.model,
                last_heartbeat = excluded.last_heartbeat,
                shard_group = excluded.shard_group,
                shard_stage = excluded.shard_stage,
                shard_total_stages = excluded.shard_total_stages,
                rpc_url = excluded.rpc_url,
                gguf_shard_index = excluded.gguf_shard_index,
                gguf_shard_total = excluded.gguf_shard_total,
                pipeline_kind = excluded.pipeline_kind,
                cpu_percent = excluded.cpu_percent,
                ram_max_mb = excluded.ram_max_mb,
                contributor_score = excluded.contributor_score
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
                node.shard_group,
                node.shard_stage,
                node.shard_total_stages,
                node.rpc_url,
                node.gguf_shard_index,
                node.gguf_shard_total,
                node.pipeline_kind,
                node.cpu_percent,
                node.ram_max_mb,
                node.contributor_score,
            ],
        )?;
        Ok(())
    }

    pub fn heartbeat(&self, node_id: &str, token: &str) -> Result<bool> {
        let now = Utc::now().timestamp();
        let updated = self.conn.execute(
            "UPDATE nodes SET last_heartbeat = ?1,
             contributor_score = contributor_score + 1.0
             WHERE id = ?2 AND token = ?3",
            params![now, node_id, token],
        )?;
        Ok(updated == 1)
    }

    pub fn list_nodes(&self) -> Result<Vec<NodeInfo>> {
        let sql = format!("{NODE_SELECT} ORDER BY name ASC, shard_stage ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_node_info)?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(row?);
        }
        Ok(nodes)
    }

    pub fn find_node_by_identity(
        &self,
        name: &str,
        worker_url: &str,
    ) -> Result<Option<StoredNode>> {
        let sql = format!(
            "{NODE_SELECT} WHERE name = ?1 AND worker_url = ?2 ORDER BY last_heartbeat DESC LIMIT 1"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![name, worker_url])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(map_stored_node(row).map_err(anyhow::Error::from)?));
        }
        Ok(None)
    }

    pub fn delete_nodes_by_worker_url_except(
        &self,
        worker_url: &str,
        keep_id: &str,
    ) -> Result<u32> {
        let deleted = self.conn.execute(
            "DELETE FROM nodes WHERE worker_url = ?1 AND id != ?2",
            params![worker_url, keep_id],
        )?;
        Ok(deleted as u32)
    }

    pub fn prune_nodes(&self) -> Result<u32> {
        let cutoff = Utc::now().timestamp() - NODE_STALE_SECS;
        let mut total = 0u32;

        let stale = self.conn.execute(
            "DELETE FROM nodes WHERE last_heartbeat < ?1",
            params![cutoff],
        )?;
        total += stale as u32;

        let mut stmt = self
            .conn
            .prepare("SELECT id, worker_url, last_heartbeat FROM nodes")?;
        let rows: Vec<(String, String, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        use std::collections::HashMap;
        let mut best: HashMap<String, (String, i64)> = HashMap::new();
        for (id, worker_url, hb) in rows {
            match best.get_mut(&worker_url) {
                Some((best_id, best_hb)) if hb > *best_hb => {
                    *best_id = id;
                    *best_hb = hb;
                }
                None => {
                    best.insert(worker_url, (id, hb));
                }
                _ => {}
            }
        }

        for (worker_url, (keep_id, _)) in &best {
            total += self.delete_nodes_by_worker_url_except(worker_url, keep_id)?;
        }

        Ok(total)
    }

    pub fn online_nodes(&self) -> Result<Vec<StoredNode>> {
        let mut stmt = self.conn.prepare(NODE_SELECT)?;
        let rows = stmt.query_map([], map_stored_node)?;

        let cutoff = Utc::now().timestamp() - NODE_STALE_SECS;
        let mut nodes = Vec::new();
        for row in rows {
            let node = row?;
            if node.last_heartbeat >= cutoff {
                nodes.push(node);
            }
        }
        nodes.sort_by(|a, b| a.name.cmp(&b.name).then(a.shard_stage.cmp(&b.shard_stage)));
        Ok(nodes)
    }

    /// Returns ordered pipeline stages (0..n-1) when a complete online chain exists.
    pub fn resolve_pipeline(
        &self,
        online: &[StoredNode],
        preferred_group: Option<&str>,
    ) -> Option<Vec<StoredNode>> {
        use std::collections::HashMap;

        let mut groups: HashMap<String, Vec<&StoredNode>> = HashMap::new();
        for node in online {
            if node.shard_total_stages < 2 || node.shard_group.is_empty() {
                continue;
            }
            groups
                .entry(node.shard_group.clone())
                .or_default()
                .push(node);
        }

        let try_group = |group: &str| -> Option<Vec<StoredNode>> {
            let members = groups.get(group)?;
            if members.is_empty() {
                return None;
            }
            let total = members[0].shard_total_stages;
            if total < 2 {
                return None;
            }
            let model = &members[0].model;
            let mut stages: Vec<StoredNode> = Vec::new();
            for stage in 0..total {
                let found = members
                    .iter()
                    .find(|n| n.shard_stage == stage && n.model == *model)
                    .copied()?;
                stages.push(found.clone());
            }
            Some(stages)
        };

        if let Some(group) = preferred_group.filter(|g| !g.is_empty()) {
            if let Some(chain) = try_group(group) {
                return Some(chain);
            }
        }

        let mut candidates: Vec<String> = groups.keys().cloned().collect();
        candidates.sort();
        for group in candidates {
            if let Some(chain) = try_group(&group) {
                return Some(chain);
            }
        }
        None
    }
}

fn map_stored_node(row: &Row<'_>) -> rusqlite::Result<StoredNode> {
    Ok(StoredNode {
        id: row.get(0)?,
        token: row.get(1)?,
        name: row.get(2)?,
        region: row.get(3)?,
        worker_url: row.get(4)?,
        model: row.get(5)?,
        last_heartbeat: row.get(6)?,
        created_at: row.get(7)?,
        shard_group: row.get(8)?,
        shard_stage: row.get(9)?,
        shard_total_stages: row.get(10)?,
        rpc_url: row.get(11)?,
        gguf_shard_index: row.get(12)?,
        gguf_shard_total: row.get(13)?,
        pipeline_kind: row.get(14)?,
        cpu_percent: row.get(15)?,
        ram_max_mb: row.get(16)?,
        contributor_score: row.get(17)?,
    })
}

fn map_node_info(row: &Row<'_>) -> rusqlite::Result<NodeInfo> {
    let last_heartbeat: i64 = row.get(6)?;
    Ok(NodeInfo {
        id: row.get(0)?,
        name: row.get(2)?,
        region: row.get(3)?,
        worker_url: row.get(4)?,
        model: row.get(5)?,
        last_heartbeat,
        online: is_online(last_heartbeat),
        shard_group: row.get(8)?,
        shard_stage: row.get(9)?,
        shard_total_stages: row.get(10)?,
        rpc_url: row.get(11)?,
        gguf_shard_index: row.get(12)?,
        gguf_shard_total: row.get(13)?,
        pipeline_kind: row.get(14)?,
        cpu_percent: row.get(15)?,
        ram_max_mb: row.get(16)?,
        compute_units: youai_common::compute::node_compute_units(
            row.get::<_, u8>(15)?,
            row.get::<_, u32>(16)?,
        ),
        contributor_score: row.get(17)?,
    })
}

fn is_online(last_heartbeat: i64) -> bool {
    let cutoff = Utc::now().timestamp() - NODE_STALE_SECS;
    last_heartbeat >= cutoff
}
