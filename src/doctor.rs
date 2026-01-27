use serde_json::json;

use crate::queue::SqliteQueue;

const APPLIED_RETENTION_SECS: i64 = 7 * 24 * 60 * 60;
const DEAD_LETTER_RETENTION_SECS: i64 = 30 * 24 * 60 * 60;

pub fn run(queue: &SqliteQueue) -> anyhow::Result<()> {
    queue.verify_schema()?;
    queue.wal_checkpoint()?;
    let stale_applied = queue.count_applied_older_than(APPLIED_RETENTION_SECS)?;
    let stale_dead_letters = queue.count_dead_letters_older_than(DEAD_LETTER_RETENTION_SECS)?;
    let _ = queue.log_event(
        0,
        "doctor",
        "info",
        "diagnostics passed",
        Some(&json!({
            "applied_retention_secs": APPLIED_RETENTION_SECS,
            "dead_letter_retention_secs": DEAD_LETTER_RETENTION_SECS,
            "stale_applied_rows": stale_applied,
            "stale_dead_letters": stale_dead_letters
        })),
    );
    println!("Queue diagnostics: schema OK");
    println!("- applied rows older than {APPLIED_RETENTION_SECS}s: {stale_applied}");
    println!("- dead letters older than {DEAD_LETTER_RETENTION_SECS}s: {stale_dead_letters}");
    Ok(())
}
