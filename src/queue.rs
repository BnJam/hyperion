use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;
use std::sync::Mutex;

use crate::models::{ChangeRequest, DeadLetterRecord, QueueRecord, QueueStatus};

pub struct SqliteQueue {
    conn: Mutex<Connection>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ChangeQueueLog {
    pub id: i64,
    pub queue_id: i64,
    pub task_id: String,
    pub level: String,
    pub message: String,
    pub details: Option<Value>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct FileModification {
    pub id: i64,
    pub path: String,
    pub event: String,
    pub source: String,
    pub details: Option<Value>,
    pub created_at: i64,
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
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS change_queue_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                queue_id INTEGER NOT NULL,
                task_id TEXT NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                details JSON,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            CREATE INDEX IF NOT EXISTS idx_change_queue_logs_queue_id ON change_queue_logs(queue_id);",
        )
        .context("create change queue logs table")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                resume_id TEXT NOT NULL UNIQUE,
                model TEXT NOT NULL,
                allow_all_tools INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                last_used INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            CREATE INDEX IF NOT EXISTS idx_agent_sessions_last_used ON agent_sessions(last_used);",
        )
        .context("create agent sessions table")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS file_modifications (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL,
                event TEXT NOT NULL,
                source TEXT NOT NULL,
                details JSON,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            CREATE INDEX IF NOT EXISTS idx_file_modifications_created_at ON file_modifications(created_at);",
        )
        .context("create file modifications table")?;
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

    pub fn log_event(
        &self,
        queue_id: i64,
        task_id: &str,
        level: &str,
        message: &str,
        details: Option<&serde_json::Value>,
    ) -> anyhow::Result<()> {
        let details_json = details.map(|value| value.to_string());
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        conn.execute(
            "INSERT INTO change_queue_logs (queue_id, task_id, level, message, details, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                queue_id,
                task_id,
                level,
                message,
                details_json,
                now_epoch()?
            ],
        )?;
        Ok(())
    }

    pub fn record_file_event(
        &self,
        path: &str,
        event: &str,
        source: &str,
        details: Option<&serde_json::Value>,
    ) -> anyhow::Result<()> {
        let details_json = details.map(|value| value.to_string());
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        conn.execute(
            "INSERT INTO file_modifications (path, event, source, details, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![path, event, source, details_json, now_epoch()?],
        )?;
        Ok(())
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

    pub fn recent_logs(&self, limit: usize) -> anyhow::Result<Vec<ChangeQueueLog>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, queue_id, task_id, level, message, details, created_at
             FROM change_queue_logs
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let mut rows = stmt.query(params![limit as i64])?;
        let mut logs = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let queue_id: i64 = row.get(1)?;
            let task_id: String = row.get(2)?;
            let level: String = row.get(3)?;
            let message: String = row.get(4)?;
            let details_str: Option<String> = row.get(5)?;
            let created_at: i64 = row.get(6)?;
            let details = details_str.and_then(|text| serde_json::from_str::<Value>(&text).ok());
            logs.push(ChangeQueueLog {
                id,
                queue_id,
                task_id,
                level,
                message,
                details,
                created_at,
            });
        }
        Ok(logs)
    }

    pub fn recent_file_events(&self, limit: usize) -> anyhow::Result<Vec<FileModification>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, path, event, source, details, created_at
             FROM file_modifications
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let mut rows = stmt.query(params![limit as i64])?;
        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let path: String = row.get(1)?;
            let event: String = row.get(2)?;
            let source: String = row.get(3)?;
            let details_str: Option<String> = row.get(4)?;
            let created_at: i64 = row.get(5)?;
            let details = details_str.and_then(|text| serde_json::from_str::<Value>(&text).ok());
            events.push(FileModification {
                id,
                path,
                event,
                source,
                details,
                created_at,
            });
        }
        Ok(events)
    }

    pub fn upsert_agent_session(
        &self,
        resume_id: &str,
        model: &str,
        allow_all_tools: bool,
    ) -> anyhow::Result<AgentSession> {
        let now = now_epoch()?;
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        conn.execute(
            "INSERT INTO agent_sessions (resume_id, model, allow_all_tools, created_at, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(resume_id) DO UPDATE SET
               model = excluded.model,
               allow_all_tools = excluded.allow_all_tools,
               last_used = excluded.last_used",
            params![
                resume_id,
                model,
                if allow_all_tools { 1 } else { 0 },
                now,
                now
            ],
        )?;
        let mut stmt = conn.prepare(
            "SELECT id, resume_id, model, allow_all_tools, created_at, last_used
             FROM agent_sessions
             WHERE resume_id = ?1",
        )?;
        let session =
            stmt.query_row(params![resume_id], |row| Self::agent_session_from_row(row))?;
        Ok(session)
    }

    pub fn latest_agent_session(&self) -> anyhow::Result<Option<AgentSession>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, resume_id, model, allow_all_tools, created_at, last_used
             FROM agent_sessions
             ORDER BY last_used DESC
             LIMIT 1",
        )?;
        let row = stmt
            .query_row([], |row| Self::agent_session_from_row(row))
            .optional()?;
        Ok(row)
    }

    pub fn list_agent_sessions(&self) -> anyhow::Result<Vec<AgentSession>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, resume_id, model, allow_all_tools, created_at, last_used
             FROM agent_sessions
             ORDER BY created_at DESC",
        )?;
        let mut rows = stmt.query([])?;
        let mut sessions = Vec::new();
        while let Some(row) = rows.next()? {
            sessions.push(Self::agent_session_from_row(row)?);
        }
        Ok(sessions)
    }

    pub fn touch_agent_session(&self, id: i64) -> anyhow::Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        conn.execute(
            "UPDATE agent_sessions SET last_used = ?1 WHERE id = ?2",
            params![now_epoch()?, id],
        )?;
        Ok(())
    }

    fn agent_session_from_row(row: &Row) -> anyhow::Result<AgentSession> {
        Ok(AgentSession {
            id: row.get(0)?,
            resume_id: row.get(1)?,
            model: row.get(2)?,
            allow_all_tools: row.get::<_, i64>(3)? != 0,
            created_at: row.get(4)?,
            last_used: row.get(5)?,
        })
    }

    pub fn recent_records(&self, limit: usize) -> anyhow::Result<Vec<QueueRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("queue lock poisoned"))?;
        let mut stmt = conn.prepare(
            "SELECT id, status, payload, attempts, last_error, leased_until
             FROM change_queue
             ORDER BY id DESC
             LIMIT ?1",
        )?;
        let mut rows = stmt.query(params![limit as i64])?;
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
