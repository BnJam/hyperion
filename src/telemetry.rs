use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::queue::SqliteQueue;

pub fn write_verification_report(queue: &SqliteQueue) -> Result<()> {
    let metrics = queue.queue_metrics(Some(60))?;
    let payload = serde_json::to_string_pretty(&metrics).context("serialize queue metrics")?;
    let report_path = Path::new("execution").join("verification_report.json");
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create directory {}", parent.display()))?;
    }
    fs::write(&report_path, payload).with_context(|| format!("write {}", report_path.display()))?;
    Ok(())
}
