use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use anyhow::Error;
use tracing::{info, warn};

use crate::apply;
use crate::queue::SqliteQueue;
use crate::runner;
use crate::validator;
use serde_json::json;

pub struct WorkerConfig {
    pub worker_id: String,
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

const PROGRESS_INTERVAL: Duration = Duration::from_secs(5);

pub fn run_worker_with_signal(
    queue: &SqliteQueue,
    config: WorkerConfig,
    running: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    info!(
        worker_id = %config.worker_id,
        lease_seconds = config.lease_seconds,
        poll_interval_ms = config.poll_interval_ms,
        run_checks = config.run_checks,
        "worker started"
    );

    let mut next_progress = Instant::now();
    while running.load(Ordering::SeqCst) {
        let now = Instant::now();
        if config.worker_id == "worker-cli" && now >= next_progress {
            if let Err(err) = report_progress(queue) {
                eprintln!("progress report failed: {err}");
            }
            next_progress = now + PROGRESS_INTERVAL;
        }
        let dequeue_start = Instant::now();
        let record = queue.dequeue(Duration::from_secs(config.lease_seconds), &config.worker_id)?;
        let dequeue_duration = dequeue_start.elapsed();
        if let Some(record) = record {
            let _ = queue.log_event(
                record.id,
                &record.payload.task_id,
                "info",
                "dequeued",
                Some(&json!({"attempt": record.attempts})),
            );
            let _ = queue.log_event(
                record.id,
                &record.payload.task_id,
                "info",
                "dequeue_metrics",
                Some(&json!({
                    "dequeue_latency_ms": dequeue_duration.as_millis(),
                    "poll_interval_ms": config.poll_interval_ms,
                    "worker_id": config.worker_id
                })),
            );
            if record.attempts > config.max_attempts {
                let _ = queue.log_event(
                    record.id,
                    &record.payload.task_id,
                    "warn",
                    "max attempts reached",
                    Some(&json!({"attempts": record.attempts, "max": config.max_attempts})),
                );
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
                let _ = queue.log_event(
                    record.id,
                    &record.payload.task_id,
                    "warn",
                    "validation failed",
                    Some(&json!({"errors": validation.errors})),
                );
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

            let apply_start = Instant::now();
            if let Err(err) = apply::apply_change_request(&record.payload) {
                let _ = queue.log_event(
                    record.id,
                    &record.payload.task_id,
                    "warn",
                    "apply failed",
                    Some(&failure_details(&err)),
                );
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
                    let _ = queue.log_event(
                        record.id,
                        &record.payload.task_id,
                        "warn",
                        "checks failed",
                        Some(&failure_details(&err)),
                    );
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
            let apply_duration = apply_start.elapsed();
            let _ = queue.log_event(
                record.id,
                &record.payload.task_id,
                "info",
                "applied",
                Some(&json!({"apply_duration_ms": apply_duration.as_millis()})),
            );
            info!(task_id = %record.payload.task_id, "change request applied");
        } else {
            let _ = queue.log_event(
                0,
                "worker",
                "info",
                "idle",
                Some(&json!({
                    "worker_id": config.worker_id,
                    "poll_interval_ms": config.poll_interval_ms
                })),
            );
            std::thread::sleep(Duration::from_millis(config.poll_interval_ms));
        }
    }

    info!("worker shutting down");
    Ok(())
}

fn report_progress(queue: &SqliteQueue) -> anyhow::Result<()> {
    let metrics = queue.queue_metrics(Some(60))?;
    let counts = metrics.status_counts;
    let fmt_opt = |value: Option<f64>, suffix: &str| {
        value
            .map(|v| format!("{:.1}{}", v, suffix))
            .unwrap_or_else(|| "n/a".to_string())
    };
    println!(
        "[progress] pending={} in_progress={} applied={} failed={} throughput={} avg_dequeue_latency={} avg_apply_duration={} lease_contention_events={}",
        counts.pending,
        counts.in_progress,
        counts.applied,
        counts.failed,
        fmt_opt(metrics.throughput_per_minute, "/min"),
        fmt_opt(metrics.avg_dequeue_latency_ms, "ms"),
        fmt_opt(metrics.avg_apply_duration_ms, "ms"),
        metrics.lease_contention_events,
    );
    Ok(())
}

fn failure_details(err: &Error) -> serde_json::Value {
    let mut details = json!({ "error": err.to_string() });
    if let Some(apply_failure) = err.downcast_ref::<apply::ApplyFailure>() {
        if let Some(map) = details.as_object_mut() {
            map.insert("apply_stdout".into(), json!(apply_failure.stdout));
            map.insert("apply_stderr".into(), json!(apply_failure.stderr));
            map.insert(
                "patch_preview".into(),
                json!(excerpt(&apply_failure.patch, 512)),
            );
        }
    }
    if let Some(check_failure) = err.downcast_ref::<runner::CheckFailure>() {
        if let Some(map) = details.as_object_mut() {
            map.insert("check_command".into(), json!(check_failure.command));
            map.insert("check_stdout".into(), json!(check_failure.stdout));
            map.insert("check_stderr".into(), json!(check_failure.stderr));
        }
    }
    details
}

fn excerpt(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}…", &text[..max_len])
    }
}
