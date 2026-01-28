use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use serde_json::json;

use crate::queue::{
    SqliteQueue, DEFAULT_APPLIED_RETENTION_SECS, DEFAULT_DEADLETTER_RETENTION_SECS,
    DEFAULT_DEDUP_WINDOW_SECS,
};

pub fn run(queue: &SqliteQueue) -> anyhow::Result<()> {
    queue.verify_schema()?;
    queue.wal_checkpoint()?;
    let stale_applied = queue.count_applied_older_than(DEFAULT_APPLIED_RETENTION_SECS)?;
    let stale_dead_letters =
        queue.count_dead_letters_older_than(DEFAULT_DEADLETTER_RETENTION_SECS)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("compute current timestamp")?
        .as_secs() as i64;
    let dedup_since = now.saturating_sub(DEFAULT_DEDUP_WINDOW_SECS);
    let dedup_hits = queue.count_dedup_hits_since(dedup_since)?;
    let last_cleanup = queue.last_cleanup_timestamp()?;
    let max_updated = queue.max_updated_timestamp()?;
    let timestamp_skew = max_updated.map(|value| now - value);
    let wal_stats = queue.wal_checkpoint_status()?;
    let _ = queue.log_event(
        0,
        "doctor",
        "info",
        "diagnostics passed",
        Some(&json!({
            "applied_retention_secs": DEFAULT_APPLIED_RETENTION_SECS,
            "dead_letter_retention_secs": DEFAULT_DEADLETTER_RETENTION_SECS,
            "stale_applied_rows": stale_applied,
            "stale_dead_letters": stale_dead_letters,
            "dedup_window_secs": DEFAULT_DEDUP_WINDOW_SECS,
            "dedup_hits": dedup_hits,
            "last_cleanup": last_cleanup,
            "timestamp_skew_secs": timestamp_skew,
            "wal_checkpoint": {
                "checkpointed": wal_stats.checkpointed,
                "log": wal_stats.log,
                "wal": wal_stats.wal
            }
        })),
    );
    println!("Queue diagnostics: schema OK");
    println!("- applied rows older than {DEFAULT_APPLIED_RETENTION_SECS}s: {stale_applied}");
    println!(
        "- dead letters older than {DEFAULT_DEADLETTER_RETENTION_SECS}s: {stale_dead_letters}"
    );
    println!("- dedup hits within {DEFAULT_DEDUP_WINDOW_SECS}s window: {dedup_hits}");
    if let Some(cleanup_ts) = last_cleanup {
        println!("- last cleanup sweep recorded at epoch {cleanup_ts}");
    } else {
        println!("- cleanup sweep not yet run");
    }
    if let Some(skew) = timestamp_skew {
        println!("- timestamp skew (now - latest update): {skew}s");
    }
    println!(
        "- WAL checkpoint (passive): checkpointed={}, log={}, wal={}",
        wal_stats.checkpointed, wal_stats.log, wal_stats.wal
    );
    Ok(())
}
