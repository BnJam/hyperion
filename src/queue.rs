use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use rusqlite::{
    params, types::Type, Connection, Error, OptionalExtension, Row, TransactionBehavior,
};
use serde_json::Value;

use crate::models::{
    AgentSession, ChangeQueueLog, ChangeRequest, DeadLetterRecord, FileModification, QueueMetrics,
    QueueRecord, QueueStatus, StatusCounts,
};

pub struct SqliteQueue {
    path: PathBuf,
}

impl SqliteQueue {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let queue = SqliteQueue {
            path: path.to_path_buf(),
        };
        queue.initialize_schema()?;
        Ok(queue)
    }

    fn initialize_schema(&self) -> anyhow::Result<()> {
        let conn = Connection::open(&self.path).context("open sqlite queue")?;
        Self::configure_pragmas(&conn)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS change_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                status TEXT NOT NULL,
                payload TEXT NOT NULL,
                attempts INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                leased_until INTEGER,
                lease_owner TEXT,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            CREATE TABLE IF NOT EXISTS dead_letters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                queue_id INTEGER NOT NULL,
                task_id TEXT NOT NULL,
                agent TEXT NOT NULL,
                payload TEXT NOT NULL,
                error TEXT,
                failed_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            CREATE TABLE IF NOT EXISTS change_queue_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                queue_id INTEGER NOT NULL,
                task_id TEXT NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                details JSON,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            CREATE TABLE IF NOT EXISTS agent_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                resume_id TEXT NOT NULL UNIQUE,
                model TEXT NOT NULL,
                allow_all_tools INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
                last_used INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );
            CREATE TABLE IF NOT EXISTS file_modifications (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL,
                event TEXT NOT NULL,
                source TEXT NOT NULL,
                details JSON,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            );",
        )
        .context("create base tables")?;
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_dead_letters_queue_id ON dead_letters(queue_id);
             CREATE INDEX IF NOT EXISTS idx_dead_letters_failed_at ON dead_letters(failed_at);
             CREATE INDEX IF NOT EXISTS idx_change_queue_status ON change_queue(status);
             CREATE INDEX IF NOT EXISTS idx_change_queue_status_lease_id ON change_queue(status, leased_until, id);
             CREATE INDEX IF NOT EXISTS idx_change_queue_lease ON change_queue(leased_until);
             CREATE INDEX IF NOT EXISTS idx_change_queue_logs_queue_id ON change_queue_logs(queue_id);
             CREATE INDEX IF NOT EXISTS idx_agent_sessions_last_used ON agent_sessions(last_used);
             CREATE INDEX IF NOT EXISTS idx_file_modifications_created_at ON file_modifications(created_at);",
        )
        .context("create indexes")?;
        Self::try_add_column(&conn, "lease_owner TEXT")?;
        Self::try_add_column(
            &conn,
            "updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))",
        )?;
        Ok(())
    }

    fn configure_pragmas(conn: &Connection) -> anyhow::Result<()> {
        conn.pragma_update(None, "journal_mode", "WAL")
            .context("enable WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .context("set synchronous")?;
        conn.pragma_update(None, "busy_timeout", 5000)
            .context("set busy_timeout")?;
        Ok(())
    }

    fn connection(&self) -> anyhow::Result<Connection> {
        let conn = Connection::open(&self.path).context("open sqlite queue")?;
        Self::configure_pragmas(&conn)?;
        Ok(conn)
    }

    pub fn enqueue(&self, request: &ChangeRequest) -> anyhow::Result<i64> {
        let payload = serde_json::to_string(request).context("serialize change request")?;
        let conn = self.connection()?;
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
        let conn = self.connection()?;
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
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO file_modifications (path, event, source, details, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![path, event, source, details_json, now_epoch()?],
        )?;
        Ok(())
    }

    pub fn dequeue(
        &self,
        lease_duration: Duration,
        lease_owner: &str,
    ) -> anyhow::Result<Option<QueueRecord>> {
        let now = now_epoch()?;
        let lease_until = now + lease_duration.as_secs() as i64;
        let mut conn = self.connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = tx
                .query_row(
                    "SELECT id, status, payload, attempts, last_error, leased_until, lease_owner, created_at
                 FROM change_queue
                 WHERE status = ?1 OR (status = ?2 AND leased_until < ?3)
                 ORDER BY id
                 LIMIT 1",
                    params![
                        QueueStatus::Pending.as_str(),
                        QueueStatus::InProgress.as_str(),
                        now
                    ],
                    Self::queue_record_from_row,
                )
                .optional()?;

        if let Some(mut record) = row {
            tx.execute(
                "UPDATE change_queue SET status = ?1, attempts = ?2, leased_until = ?3, lease_owner = ?4, updated_at = ?5 WHERE id = ?6",
                params![
                    QueueStatus::InProgress.as_str(),
                    record.attempts + 1,
                    lease_until,
                    lease_owner,
                    now,
                    record.id
                ],
            )?;
            record.status = QueueStatus::InProgress;
            record.attempts += 1;
            record.leased_until = Some(lease_until);
            record.lease_owner = Some(lease_owner.to_string());
            tx.commit()?;
            Ok(Some(record))
        } else {
            tx.commit()?;
            Ok(None)
        }
    }

    pub fn mark_failed(&self, id: i64, error: Option<String>) -> anyhow::Result<()> {
        let conn = self.connection()?;
        let payload: Option<String> = conn
            .query_row(
                "SELECT payload FROM change_queue WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        conn.execute(
            "UPDATE change_queue SET status = ?1, last_error = ?2, leased_until = NULL, lease_owner = NULL, updated_at = ?3 WHERE id = ?4",
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
        let conn = self.connection()?;
        conn.execute(
            "UPDATE change_queue SET status = ?1, last_error = ?2, leased_until = NULL, lease_owner = NULL, updated_at = ?3 WHERE id = ?4",
            params![QueueStatus::Pending.as_str(), error, now_epoch()?, id],
        )?;
        Ok(())
    }

    pub fn mark_applied(&self, id: i64) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE change_queue SET status = ?1, leased_until = NULL, lease_owner = NULL, updated_at = ?2 WHERE id = ?3",
            params![QueueStatus::Applied.as_str(), now_epoch()?, id],
        )?;
        Ok(())
    }

    pub fn list(&self, status: QueueStatus) -> anyhow::Result<Vec<QueueRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, status, payload, attempts, last_error, leased_until, lease_owner, created_at
             FROM change_queue
             WHERE status = ?1
             ORDER BY id",
        )?;
        let mut rows = stmt.query(params![status.as_str()])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(Self::queue_record_from_row(row)?);
        }
        Ok(records)
    }

    pub fn recent_logs(&self, limit: usize) -> anyhow::Result<Vec<ChangeQueueLog>> {
        let conn = self.connection()?;
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
        let conn = self.connection()?;
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
        let conn = self.connection()?;
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
        let session = stmt.query_row(params![resume_id], Self::agent_session_from_row)?;
        Ok(session)
    }

    pub fn latest_agent_session(&self) -> anyhow::Result<Option<AgentSession>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, resume_id, model, allow_all_tools, created_at, last_used
             FROM agent_sessions
             ORDER BY last_used DESC
             LIMIT 1",
        )?;
        let row = stmt
            .query_row([], Self::agent_session_from_row)
            .optional()?;
        Ok(row)
    }

    pub fn list_agent_sessions(&self) -> anyhow::Result<Vec<AgentSession>> {
        let conn = self.connection()?;
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
        let conn = self.connection()?;
        conn.execute(
            "UPDATE agent_sessions SET last_used = ?1 WHERE id = ?2",
            params![now_epoch()?, id],
        )?;
        Ok(())
    }

    pub fn recent_records(&self, limit: usize) -> anyhow::Result<Vec<QueueRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, status, payload, attempts, last_error, leased_until, lease_owner, created_at
             FROM change_queue
             ORDER BY id DESC
             LIMIT ?1",
        )?;
        let mut rows = stmt.query(params![limit as i64])?;
        let mut records = Vec::new();
        while let Some(row) = rows.next()? {
            records.push(Self::queue_record_from_row(row)?);
        }
        Ok(records)
    }

    pub fn dead_letter_count(&self) -> anyhow::Result<i64> {
        let conn = self.connection()?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM dead_letters", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn list_dead_letters(&self) -> anyhow::Result<Vec<DeadLetterRecord>> {
        let conn = self.connection()?;
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

    pub fn queue_metrics(&self, window_seconds: Option<i64>) -> anyhow::Result<QueueMetrics> {
        let window = window_seconds.unwrap_or(60).max(1);
        let now = now_epoch()?;
        let since = now.saturating_sub(window);
        let conn = self.connection()?;
        let pending = self.list(QueueStatus::Pending)?.len();
        let in_progress = self.list(QueueStatus::InProgress)?.len();
        let applied = self.list(QueueStatus::Applied)?.len();
        let failed = self.list(QueueStatus::Failed)?.len();

        let mut stmt = conn.prepare(
            "SELECT message, details
             FROM change_queue_logs
             WHERE created_at >= ?1
             ORDER BY created_at DESC",
        )?;
        let mut rows = stmt.query(params![since])?;
        let mut dequeue_latency_total = 0.0;
        let mut dequeue_samples = 0usize;
        let mut poll_total = 0.0;
        let mut poll_samples = 0usize;
        let mut apply_duration_total = 0.0;
        let mut apply_samples = 0usize;
        let mut applied_count = 0usize;
        let mut lease_contention_events = 0usize;
        while let Some(row) = rows.next()? {
            let message: String = row.get(0)?;
            let details_str: Option<String> = row.get(1)?;
            let details = details_str
                .as_deref()
                .and_then(|text| serde_json::from_str::<Value>(text).ok());
            match message.as_str() {
                "dequeue_metrics" => {
                    if let Some(details) = &details {
                        if let Some(latency) =
                            details.get("dequeue_latency_ms").and_then(Value::as_f64)
                        {
                            dequeue_latency_total += latency;
                            dequeue_samples += 1;
                            if let Some(poll) =
                                details.get("poll_interval_ms").and_then(Value::as_f64)
                            {
                                poll_total += poll;
                                poll_samples += 1;
                                if latency > poll {
                                    lease_contention_events += 1;
                                }
                            }
                        } else if let Some(poll) =
                            details.get("poll_interval_ms").and_then(Value::as_f64)
                        {
                            poll_total += poll;
                            poll_samples += 1;
                        }
                    }
                }
                "applied" => {
                    applied_count += 1;
                    if let Some(details) = &details {
                        if let Some(apply_duration) =
                            details.get("apply_duration_ms").and_then(Value::as_f64)
                        {
                            apply_duration_total += apply_duration;
                            apply_samples += 1;
                        }
                    }
                }
                _ => {}
            }
        }

        let avg_dequeue_latency = if dequeue_samples > 0 {
            Some(dequeue_latency_total / dequeue_samples as f64)
        } else {
            None
        };
        let avg_poll_interval = if poll_samples > 0 {
            Some(poll_total / poll_samples as f64)
        } else {
            None
        };
        let avg_apply_duration = if apply_samples > 0 {
            Some(apply_duration_total / apply_samples as f64)
        } else {
            None
        };

        let throughput_per_minute = if applied_count > 0 {
            Some(applied_count as f64 * 60.0 / window as f64)
        } else {
            None
        };

        Ok(QueueMetrics {
            window_seconds: window,
            status_counts: StatusCounts {
                pending,
                in_progress,
                applied,
                failed,
            },
            avg_dequeue_latency_ms: avg_dequeue_latency,
            avg_apply_duration_ms: avg_apply_duration,
            avg_poll_interval_ms: avg_poll_interval,
            throughput_per_minute,
            lease_contention_events,
            timestamp: now,
        })
    }

    #[allow(dead_code)]
    pub fn purge_applied(&self, ttl_secs: i64) -> anyhow::Result<usize> {
        let threshold = now_epoch()? - ttl_secs;
        let conn = self.connection()?;
        let deleted = conn.execute(
            "DELETE FROM change_queue WHERE status = ?1 AND updated_at < ?2",
            params![QueueStatus::Applied.as_str(), threshold],
        )?;
        Ok(deleted)
    }

    #[allow(dead_code)]
    pub fn purge_dead_letters(&self, ttl_secs: i64) -> anyhow::Result<usize> {
        let threshold = now_epoch()? - ttl_secs;
        let conn = self.connection()?;
        let deleted = conn.execute(
            "DELETE FROM dead_letters WHERE failed_at < ?1",
            params![threshold],
        )?;
        Ok(deleted)
    }

    pub fn count_applied_older_than(&self, ttl_secs: i64) -> anyhow::Result<i64> {
        let threshold = now_epoch()? - ttl_secs;
        let conn = self.connection()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM change_queue WHERE status = ?1 AND updated_at < ?2",
            params![QueueStatus::Applied.as_str(), threshold],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn count_dead_letters_older_than(&self, ttl_secs: i64) -> anyhow::Result<i64> {
        let threshold = now_epoch()? - ttl_secs;
        let conn = self.connection()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM dead_letters WHERE failed_at < ?1",
            params![threshold],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn wal_checkpoint(&self) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .context("checkpoint wal")?;
        Ok(())
    }

    pub fn verify_schema(&self) -> anyhow::Result<()> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare("PRAGMA table_info(change_queue)")?;
        let mut columns = HashSet::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            columns.insert(name);
        }
        let required = [
            "id",
            "status",
            "payload",
            "attempts",
            "leased_until",
            "lease_owner",
            "updated_at",
        ];
        for column in required {
            if !columns.contains(column) {
                anyhow::bail!("missing column {column} in change_queue");
            }
        }

        let mut stmt = conn.prepare("PRAGMA index_list('change_queue')")?;
        let mut indexes = HashSet::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            indexes.insert(name);
        }
        if !indexes.contains("idx_change_queue_status") {
            anyhow::bail!("missing idx_change_queue_status");
        }
        if !indexes.contains("idx_change_queue_status_lease_id") {
            anyhow::bail!("missing idx_change_queue_status_lease_id");
        }

        let mut stmt = conn.prepare("PRAGMA table_info(dead_letters)")?;
        let mut rows = stmt.query([])?;
        let mut dead_columns = HashSet::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            dead_columns.insert(name);
        }
        if !dead_columns.contains("failed_at") {
            anyhow::bail!("missing failed_at in dead_letters");
        }

        Ok(())
    }

    fn agent_session_from_row(row: &Row) -> rusqlite::Result<AgentSession> {
        Ok(AgentSession {
            id: row.get(0)?,
            resume_id: row.get(1)?,
            model: row.get(2)?,
            allow_all_tools: row.get::<_, i64>(3)? != 0,
            created_at: row.get(4)?,
            last_used: row.get(5)?,
        })
    }

    fn queue_record_from_row(row: &Row) -> rusqlite::Result<QueueRecord> {
        let id: i64 = row.get(0)?;
        let status: String = row.get(1)?;
        let payload: String = row.get(2)?;
        let attempts: i64 = row.get(3)?;
        let last_error: Option<String> = row.get(4)?;
        let leased_until: Option<i64> = row.get(5)?;
        let lease_owner: Option<String> = row.get(6)?;
        let created_at: i64 = row.get(7)?;
        let payload: ChangeRequest = serde_json::from_str(&payload)
            .map_err(|err| Error::FromSqlConversionFailure(2, Type::Text, Box::new(err)))?;
        Ok(QueueRecord {
            id,
            status: QueueStatus::from_string(&status)?,
            payload,
            attempts,
            last_error,
            leased_until,
            lease_owner,
            created_at,
        })
    }

    fn try_add_column(conn: &Connection, definition: &str) -> anyhow::Result<()> {
        let statement = format!("ALTER TABLE change_queue ADD COLUMN {definition}");
        let _ = conn.execute(&statement, []);
        Ok(())
    }
}

impl QueueStatus {
    fn from_string(value: &str) -> rusqlite::Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "in_progress" => Ok(Self::InProgress),
            "applied" => Ok(Self::Applied),
            "failed" => Ok(Self::Failed),
            _ => Err(Error::InvalidColumnName(format!(
                "unknown queue status: {value}"
            ))),
        }
    }
}

fn now_epoch() -> anyhow::Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("time went backwards")?;
    Ok(now.as_secs() as i64)
}
