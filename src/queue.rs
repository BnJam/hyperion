use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;

use crate::models::{ChangeRequest, DeadLetterRecord, QueueRecord, QueueStatus};

pub struct SqliteQueue {
    conn: Mutex<Connection>,
}

impl SqliteQueue {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path).context("open sqlite queue")?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .context("enable WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .context("set synchronous")?;
        conn.pragma_update(None, "busy_timeout", 5000)
            .context("set busy_timeout")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS change_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                status TEXT NOT NULL,
                payload TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                leased_until INTEGER,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );",
        )
        .context("create queue table")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dead_letters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                queue_id INTEGER NOT NULL,
                task_id TEXT NOT NULL,
                agent TEXT NOT NULL,
                payload TEXT NOT NULL,
                error TEXT,
                failed_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );",
        )
        .context("create dead letter table")?;
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_dead_letters_queue_id ON dead_letters(queue_id);
             CREATE INDEX IF NOT EXISTS idx_dead_letters_failed_at ON dead_letters(failed_at);",
        )
        .context("create dead letter indices")?;
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_change_queue_status ON change_queue(status);
             CREATE INDEX IF NOT EXISTS idx_change_queue_lease ON change_queue(leased_until);",
        )
        .context("create queue indices")?;
        Self::try_add_column(&conn, "attempts INTEGER NOT NULL DEFAULT 0")?;
        Self::try_add_column(&conn, "last_error TEXT")?;
        Self::try_add_column(&conn, "leased_until INTEGER")?;
        Self::try_add_column(
            &conn,
            "updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn enqueue(&self, request: &ChangeRequest) -> anyhow::Result<i64> {
        let payload = serde_json::to_string(request).context("serialize change request")?;
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        conn.execute(
            "INSERT INTO change_queue (status, payload, updated_at) VALUES (?1, ?2, ?3)",
            params![QueueStatus::Pending.as_str(), payload, now_epoch()?],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn dequeue(&self, lease_duration: Duration) -> anyhow::Result<Option<QueueRecord>> {
        let now = now_epoch()?;
        let lease_until = now + lease_duration.as_secs() as i64;
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        let tx = conn.transaction()?;
        let row = tx
            .query_row(
                "SELECT id, status, payload, attempts, last_error, leased_until
                 FROM change_queue
                 WHERE status = ?1 OR (status = ?2 AND leased_until < ?3)
                 ORDER BY id
                 LIMIT 1",
                params![
                    QueueStatus::Pending.as_str(),
                    QueueStatus::InProgress.as_str(),
                    now
                ],
                |row| {
                    let id: i64 = row.get(0)?;
                    let status: String = row.get(1)?;
                    let payload: String = row.get(2)?;
                    let attempts: i64 = row.get(3)?;
                    let last_error: Option<String> = row.get(4)?;
                    let leased_until: Option<i64> = row.get(5)?;
                    Ok((id, status, payload, attempts, last_error, leased_until))
                },
            )
            .optional()?;

        let record = if let Some((id, _status, payload, attempts, last_error, _leased_until)) = row
        {
            tx.execute(
                "UPDATE change_queue SET status = ?1, attempts = ?2, leased_until = ?3, updated_at = ?4 WHERE id = ?5",
                params![
                    QueueStatus::InProgress.as_str(),
                    attempts + 1,
                    lease_until,
                    now,
                    id
                ],
            )?;
            let payload: ChangeRequest = serde_json::from_str(&payload)?;
            Some(QueueRecord {
                id,
                status: QueueStatus::InProgress,
                payload,
                attempts: attempts + 1,
                last_error,
                leased_until: Some(lease_until),
            })
        } else {
            None
        };
        tx.commit()?;
        Ok(record)
    }

    pub fn mark_failed(&self, id: i64, error: Option<String>) -> anyhow::Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        let payload: Option<String> = conn
            .query_row(
                "SELECT payload FROM change_queue WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        conn.execute(
            "UPDATE change_queue SET status = ?1, last_error = ?2, leased_until = NULL, updated_at = ?3 WHERE id = ?4",
            params![QueueStatus::Failed.as_str(), error, now_epoch()?, id],
        )?;
        if let Some(payload) = payload {
            if let Ok(request) = serde_json::from_str::<ChangeRequest>(&payload) {
                let _ = conn.execute(
                    "INSERT INTO dead_letters (queue_id, task_id, agent, payload, error, failed_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        id,
                        request.task_id,
                        request.agent,
                        payload,
                        error,
                        now_epoch()?
                    ],
                );
            }
        }
        Ok(())
    }

    pub fn mark_retry(&self, id: i64, error: Option<String>) -> anyhow::Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        conn.execute(
            "UPDATE change_queue SET status = ?1, last_error = ?2, leased_until = NULL, updated_at = ?3 WHERE id = ?4",
            params![QueueStatus::Pending.as_str(), error, now_epoch()?, id],
        )?;
        Ok(())
    }

    pub fn mark_applied(&self, id: i64) -> anyhow::Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        conn.execute(
            "UPDATE change_queue SET status = ?1, leased_until = NULL, updated_at = ?2 WHERE id = ?3",
            params![QueueStatus::Applied.as_str(), now_epoch()?, id],
        )?;
        Ok(())
    }

    pub fn list(&self, status: QueueStatus) -> anyhow::Result<Vec<QueueRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, status, payload, attempts, last_error, leased_until
             FROM change_queue
             WHERE status = ?1
             ORDER BY id",
        )?;
        let mut rows = stmt.query(params![status.as_str()])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let status: String = row.get(1)?;
            let payload: String = row.get(2)?;
            let attempts: i64 = row.get(3)?;
            let last_error: Option<String> = row.get(4)?;
            let leased_until: Option<i64> = row.get(5)?;
            let payload: ChangeRequest = serde_json::from_str(&payload)?;
            records.push(QueueRecord {
                id,
                status: QueueStatus::from_string(&status)?,
                payload,
                attempts,
                last_error,
                leased_until,
            });
        }
        Ok(records)
    }

    #[allow(dead_code)]
    pub fn dead_letter_count(&self) -> anyhow::Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM dead_letters", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn list_dead_letters(&self) -> anyhow::Result<Vec<DeadLetterRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, queue_id, task_id, agent, payload, error, failed_at
             FROM dead_letters
             ORDER BY failed_at DESC",
        )?;
        let mut rows = stmt.query([])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let queue_id: i64 = row.get(1)?;
            let task_id: String = row.get(2)?;
            let agent: String = row.get(3)?;
            let payload: String = row.get(4)?;
            let error: Option<String> = row.get(5)?;
            let failed_at: i64 = row.get(6)?;
            let payload: ChangeRequest = serde_json::from_str(&payload)?;
            records.push(DeadLetterRecord {
                id,
                queue_id,
                task_id,
                agent,
                payload,
                error,
                failed_at,
            });
        }
        Ok(records)
    }

    fn try_add_column(conn: &Connection, definition: &str) -> anyhow::Result<()> {
        let statement = format!("ALTER TABLE change_queue ADD COLUMN {definition}");
        let _ = conn.execute(&statement, []);
        Ok(())
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

fn now_epoch() -> anyhow::Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("time went backwards")?;
    Ok(now.as_secs() as i64)
}
