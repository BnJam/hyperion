use std::path::Path;

use anyhow::Context;
use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{ChangeRequest, QueueRecord, QueueStatus};

pub struct SqliteQueue {
    conn: Connection,
}

impl SqliteQueue {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path).context("open sqlite queue")?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .context("enable WAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS change_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                status TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .context("create queue table")?;
        Ok(Self { conn })
    }

    pub fn enqueue(&self, request: &ChangeRequest) -> anyhow::Result<i64> {
        let payload = serde_json::to_string(request).context("serialize change request")?;
        self.conn.execute(
            "INSERT INTO change_queue (status, payload) VALUES (?1, ?2)",
            params![QueueStatus::Pending.as_str(), payload],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn dequeue(&self) -> anyhow::Result<Option<QueueRecord>> {
        let tx = self.conn.transaction()?;
        let row = tx
            .query_row(
                "SELECT id, status, payload FROM change_queue WHERE status = ?1 ORDER BY id LIMIT 1",
                params![QueueStatus::Pending.as_str()],
                |row| {
                    let id: i64 = row.get(0)?;
                    let status: String = row.get(1)?;
                    let payload: String = row.get(2)?;
                    Ok((id, status, payload))
                },
            )
            .optional()?;

        let record = if let Some((id, status, payload)) = row {
            tx.execute(
                "UPDATE change_queue SET status = ?1 WHERE id = ?2",
                params![QueueStatus::InProgress.as_str(), id],
            )?;
            let payload: ChangeRequest = serde_json::from_str(&payload)?;
            Some(QueueRecord {
                id,
                status: QueueStatus::InProgress,
                payload,
            })
        } else {
            None
        };
        tx.commit()?;
        Ok(record)
    }

    pub fn mark_failed(&self, id: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE change_queue SET status = ?1 WHERE id = ?2",
            params![QueueStatus::Failed.as_str(), id],
        )?;
        Ok(())
    }

    pub fn mark_applied(&self, id: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE change_queue SET status = ?1 WHERE id = ?2",
            params![QueueStatus::Applied.as_str(), id],
        )?;
        Ok(())
    }

    pub fn list(&self, status: QueueStatus) -> anyhow::Result<Vec<QueueRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, status, payload FROM change_queue WHERE status = ?1 ORDER BY id",
        )?;
        let mut rows = stmt.query(params![status.as_str()])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let status: String = row.get(1)?;
            let payload: String = row.get(2)?;
            let payload: ChangeRequest = serde_json::from_str(&payload)?;
            records.push(QueueRecord {
                id,
                status: QueueStatus::from_string(&status)?,
                payload,
            });
        }
        Ok(records)
    }
}

impl QueueStatus {
    fn from_string(value: &str) -> anyhow::Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "in_progress" => Ok(Self::InProgress),
            "applied" => Ok(Self::Applied),
            "failed" => Ok(Self::Failed),
            _ => Err(anyhow::anyhow!("unknown queue status: {value}")),
        }
    }
}
