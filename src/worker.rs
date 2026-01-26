use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use tracing::{info, warn};

use crate::apply;
use crate::queue::SqliteQueue;
use crate::runner;
use crate::validator;

pub struct WorkerConfig {
    pub lease_seconds: u64,
    pub poll_interval_ms: u64,
    pub run_checks: bool,
    pub max_attempts: i64,
}

pub fn run_worker(queue: &SqliteQueue, config: WorkerConfig) -> anyhow::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let signal = running.clone();
    ctrlc::set_handler(move || {
        signal.store(false, Ordering::SeqCst);
    })?;
    run_worker_with_signal(queue, config, running)
}

pub fn run_worker_with_signal(
    queue: &SqliteQueue,
    config: WorkerConfig,
    running: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    info!(
        lease_seconds = config.lease_seconds,
        poll_interval_ms = config.poll_interval_ms,
        run_checks = config.run_checks,
        "worker started"
    );

    while running.load(Ordering::SeqCst) {
        let record = queue.dequeue(Duration::from_secs(config.lease_seconds))?;
        if let Some(record) = record {
            if record.attempts > config.max_attempts {
                warn!(
                    task_id = %record.payload.task_id,
                    attempts = record.attempts,
                    max_attempts = config.max_attempts,
                    "max attempts reached"
                );
                queue.mark_failed(
                    record.id,
                    Some(format!(
                        "max attempts reached ({}/{})",
                        record.attempts, config.max_attempts
                    )),
                )?;
                continue;
            }

            let validation = validator::validate_change_request(&record.payload);
            if !validation.valid {
                warn!(
                    task_id = %record.payload.task_id,
                    errors = ?validation.errors,
                    "invalid change request"
                );
                queue.mark_failed(
                    record.id,
                    Some(format!("validation errors: {:?}", validation.errors)),
                )?;
                continue;
            }

            if let Err(err) = apply::apply_change_request(&record.payload) {
                warn!(task_id = %record.payload.task_id, error = %err, "apply failed");
                if record.attempts >= config.max_attempts {
                    queue.mark_failed(record.id, Some(err.to_string()))?;
                } else {
                    queue.mark_retry(record.id, Some(err.to_string()))?;
                }
                continue;
            }

            if config.run_checks {
                if let Err(err) = runner::run_checks(&record.payload.checks) {
                    warn!(task_id = %record.payload.task_id, error = %err, "checks failed");
                    if record.attempts >= config.max_attempts {
                        queue.mark_failed(record.id, Some(err.to_string()))?;
                    } else {
                        queue.mark_retry(record.id, Some(err.to_string()))?;
                    }
                    continue;
                }
            }

            queue.mark_applied(record.id)?;
            info!(task_id = %record.payload.task_id, "change request applied");
        } else {
            std::thread::sleep(Duration::from_millis(config.poll_interval_ms));
        }
    }

    info!("worker shutting down");
    Ok(())
}
