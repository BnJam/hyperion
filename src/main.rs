use std::collections::VecDeque;
use std::env;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;

use clap::{Parser, Subcommand};

mod agent;
mod apply;
mod cast_builder;
mod doctor;
mod exporter;
mod fs_watch;
mod models;
mod orchestrator;
mod queue;
mod request;
mod runner;
mod telemetry;
mod tui;
mod validator;
mod watcher;
mod worker;

use crate::agent::AgentHarness;
use crate::request::DEFAULT_MODEL;
use models::QueueStatus;
use queue::{SqliteQueue, DEFAULT_APPLIED_RETENTION_SECS};
use serde_json::to_string_pretty;

#[derive(Parser)]
#[command(name = "hyperion", version, about = "Multi-agent orchestration queue")]
struct Cli {
    #[arg(long, default_value = "hyperion.db")]
    db: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(long, default_value_t = 3)]
        workers: usize,
        #[arg(long, default_value_t = 3)]
        agents: usize,
    },
    Request {
        file: PathBuf,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, default_value_t = 3)]
        agents: usize,
        #[arg(long, default_value_t = 3)]
        workers: usize,
    },
    Cast {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Export {
        #[arg(long)]
        dest: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
    },
    Init,
    Enqueue {
        file: PathBuf,
    },
    Dequeue {
        #[arg(long, default_value = "300")]
        lease_seconds: u64,
    },
    List {
        status: Option<QueueStatus>,
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        since: Option<i64>,
        #[arg(long)]
        limit: Option<usize>,
    },
    ListDeadLetters {
        #[arg(long)]
        format: Option<String>,
        #[arg(long)]
        since: Option<i64>,
        #[arg(long)]
        limit: Option<usize>,
    },
    MarkApplied {
        id: i64,
    },
    MarkFailed {
        id: i64,
        #[arg(long)]
        error: Option<String>,
    },
    Watch {
        directory: PathBuf,
    },
    Tui,
    Agent {
        #[arg(long)]
        model: Option<String>,
        prompt: String,
    },
    ValidateChange {
        file: PathBuf,
    },
    Orchestrate {
        file: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Apply {
        file: PathBuf,
        #[arg(long)]
        run_checks: bool,
    },
    Worker {
        #[arg(long, default_value = "300")]
        lease_seconds: u64,
        #[arg(long, default_value = "500")]
        poll_interval_ms: u64,
        #[arg(long)]
        run_checks: bool,
        #[arg(long, default_value = "5")]
        max_attempts: i64,
        #[arg(long, default_value = "worker-cli")]
        worker_id: String,
    },
    SessionInit {
        #[arg(long)]
        resume_id: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, default_value_t = true)]
        allow_all_tools: bool,
    },
    SessionList,
    Doctor,
    QueueMetrics {
        #[arg(long)]
        since: Option<i64>,
        #[arg(long)]
        format: Option<String>,
    },
    Cleanup {
        #[arg(long)]
        ttl_seconds: Option<i64>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_tracing();
    let queue = Arc::new(SqliteQueue::open(&cli.db)?);

    match cli.command {
        None => run_integrated(queue.clone(), cli.db.clone(), 3, 3),
        Some(Commands::Run { workers, agents }) => {
            run_integrated(queue.clone(), cli.db.clone(), workers, agents)
        }
        Some(Commands::Request {
            file,
            model,
            agents,
            ..
        }) => {
            let agent_count = agents.clamp(1, 3);
            let enqueued = request::handle_request(queue.as_ref(), &file, model, agent_count)?;
            println!(
                "Processed request {} and enqueued {} change request(s)",
                file.display(),
                enqueued
            );
            Ok(())
        }
        Some(Commands::Cast { out }) => {
            cast_builder::run_cast_builder(out)?;
            Ok(())
        }
        Some(Commands::Export { dest, overwrite }) => {
            let target =
                dest.unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            exporter::export_skill(&target, overwrite)?;
            Ok(())
        }
        Some(Commands::Init) => {
            println!("Initialized queue at {}", cli.db.display());
            Ok(())
        }
        Some(Commands::Enqueue { file }) => {
            let contents = std::fs::read_to_string(file)?;
            let request: models::ChangeRequest = serde_json::from_str(&contents)?;
            let id = queue.enqueue(&request)?;
            println!("Enqueued change request {id}");
            Ok(())
        }
        Some(Commands::Dequeue { lease_seconds }) => {
            if let Some(record) =
                queue.dequeue(std::time::Duration::from_secs(lease_seconds), "cli")?
            {
                println!(
                    "Dequeued {} from {} (attempt {})",
                    record.id, record.payload.task_id, record.attempts
                );
            } else {
                println!("No pending change requests");
            }
            Ok(())
        }
        Some(Commands::List {
            status,
            format,
            since,
            limit,
        }) => {
            let status = status.unwrap_or(QueueStatus::Pending);
            let mut records = queue.list(status)?;
            if let Some(since) = since {
                records.retain(|record| record.created_at >= since);
            }
            if let Some(limit) = limit {
                records.truncate(limit);
            }
            if format.as_deref() == Some("json") {
                println!("{}", to_string_pretty(&records)?);
            } else {
                for record in records {
                    println!(
                        "{} {} {} attempts={} lease_until={:?}",
                        record.id,
                        record.status.as_str(),
                        record.payload.task_id,
                        record.attempts,
                        record.leased_until
                    );
                }
            }
            Ok(())
        }
        Some(Commands::ListDeadLetters {
            format,
            since,
            limit,
        }) => {
            let mut records = queue.list_dead_letters()?;
            if let Some(since) = since {
                records.retain(|record| record.failed_at >= since);
            }
            if let Some(limit) = limit {
                records.truncate(limit);
            }
            if format.as_deref() == Some("json") {
                println!("{}", to_string_pretty(&records)?);
            } else {
                for record in records {
                    println!(
                        "{} queue_id={} task_id={} agent={} failed_at={} error={:?}",
                        record.id,
                        record.queue_id,
                        record.task_id,
                        record.agent,
                        record.failed_at,
                        record.error
                    );
                }
            }
            Ok(())
        }
        Some(Commands::MarkApplied { id }) => {
            queue.mark_applied(id)?;
            println!("Marked {id} as applied");
            Ok(())
        }
        Some(Commands::MarkFailed { id, error }) => {
            queue.mark_failed(id, error)?;
            println!("Marked {id} as failed");
            Ok(())
        }
        Some(Commands::Watch { directory }) => {
            watcher::watch_directory(queue.as_ref(), &directory)?;
            Ok(())
        }
        Some(Commands::SessionInit {
            resume_id,
            model,
            allow_all_tools,
        }) => {
            let model = model.unwrap_or_else(|| DEFAULT_MODEL.to_string());
            let session = queue.upsert_agent_session(&resume_id, &model, allow_all_tools)?;
            println!(
                "Recorded agent session {} (resume={}, model={}, allow_all_tools={})",
                session.id, session.resume_id, session.model, session.allow_all_tools
            );
            Ok(())
        }
        Some(Commands::SessionList) => {
            let sessions = queue.list_agent_sessions()?;
            if sessions.is_empty() {
                println!("no agent sessions recorded");
            } else {
                for session in sessions {
                    println!(
                        "{} resume={} model={} allow_all_tools={} last_used={}",
                        session.id,
                        session.resume_id,
                        session.model,
                        session.allow_all_tools,
                        session.last_used
                    );
                }
            }
            Ok(())
        }
        Some(Commands::Tui) => {
            tui::run_dashboard(queue.as_ref())?;
            Ok(())
        }
        Some(Commands::Agent { model, prompt }) => {
            let harness = agent::CopilotHarness::new(model.unwrap_or_else(|| "gpt-5-mini".into()));
            let response = harness.run(&prompt)?;
            println!("{response}");
            Ok(())
        }
        Some(Commands::ValidateChange { file }) => {
            let contents = std::fs::read_to_string(file)?;
            let request: models::ChangeRequest = serde_json::from_str(&contents)?;
            let result = validator::validate_change_request(&request);
            if result.valid {
                println!("valid");
            } else {
                println!("invalid");
                for error in result.errors {
                    println!("- {error}");
                }
            }
            Ok(())
        }
        Some(Commands::Orchestrate { file, out }) => {
            let contents = std::fs::read_to_string(file)?;
            let request: models::TaskRequest = serde_json::from_str(&contents)?;
            let assignments = orchestrator::decompose_request(&request);
            let payload = serde_json::to_string_pretty(&assignments)?;
            if let Some(out) = out {
                std::fs::write(out, payload)?;
            } else {
                println!("{payload}");
            }
            Ok(())
        }
        Some(Commands::Apply { file, run_checks }) => {
            let contents = std::fs::read_to_string(file)?;
            let request: models::ChangeRequest = serde_json::from_str(&contents)?;
            let validation = validator::validate_change_request(&request);
            if !validation.valid {
                return Err(anyhow::anyhow!(
                    "invalid change request: {errors:?}",
                    errors = validation.errors
                ));
            }
            apply::apply_change_request(&request)?;
            if run_checks {
                runner::run_checks(&request.checks)?;
            }
            println!("applied");
            Ok(())
        }
        Some(Commands::Worker {
            lease_seconds,
            poll_interval_ms,
            run_checks,
            max_attempts,
            worker_id,
        }) => worker::run_worker(
            queue.as_ref(),
            worker::WorkerConfig {
                worker_id,
                lease_seconds,
                poll_interval_ms,
                run_checks,
                max_attempts,
            },
        ),
        Some(Commands::Doctor) => {
            doctor::run(queue.as_ref())?;
            Ok(())
        }
        Some(Commands::QueueMetrics { since, format }) => {
            let metrics = queue.queue_metrics(since)?;
            if format.as_deref() == Some("json") {
                println!("{}", to_string_pretty(&metrics)?);
            } else {
                let counts = metrics.status_counts;
                let formatted = |value: Option<f64>, suffix: &str| {
                    value
                        .map(|v| format!("{:.1}{}", v, suffix))
                        .unwrap_or_else(|| "n/a".to_string())
                };
                println!(
                    "Queue metrics ({}s window): pending={} in_progress={} applied={} failed={} throughput={} lease_contention_events={}",
                    metrics.window_seconds,
                    counts.pending,
                    counts.in_progress,
                    counts.applied,
                    counts.failed,
                    formatted(metrics.throughput_per_minute, "/min"),
                    metrics.lease_contention_events
                );
                println!(
                    "           avg_dequeue_latency={} avg_apply_duration={} avg_poll_interval={}",
                    formatted(metrics.avg_dequeue_latency_ms, "ms"),
                    formatted(metrics.avg_apply_duration_ms, "ms"),
                    formatted(metrics.avg_poll_interval_ms, "ms"),
                );
            }
            Ok(())
        }
        Some(Commands::Cleanup { ttl_seconds }) => {
            let ttl = ttl_seconds.unwrap_or(DEFAULT_APPLIED_RETENTION_SECS).max(1);
            let deleted = queue.cleanup_stale_records(ttl)?;
            println!(
                "Removed {deleted} applied/failed entries older than {ttl} seconds via cleanup."
            );
            Ok(())
        }
    }
}

fn init_tracing() {
    use std::io;
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("hyperion=info".parse().unwrap());
    let fmt = tracing_subscriber::fmt().with_env_filter(filter);
    if std::env::var("HYPERION_LOG").is_ok() {
        let _ = fmt.try_init();
    } else {
        let _ = fmt.with_writer(io::sink).try_init();
    }
}

fn run_integrated(
    queue: Arc<SqliteQueue>,
    db_path: PathBuf,
    worker_count: usize,
    agent_count: usize,
) -> anyhow::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let signal = running.clone();
    ctrlc::set_handler(move || {
        signal.store(false, Ordering::SeqCst);
    })?;

    let worker_count = worker_count.clamp(1, 3);
    let agent_count = agent_count.clamp(1, 3);

    let mut handles = Vec::new();
    for index in 0..worker_count {
        let queue = queue.clone();
        let running = running.clone();
        let worker_id = format!("worker-{}", index + 1);
        handles.push(thread::spawn(move || {
            worker::run_worker_with_signal(
                queue.as_ref(),
                worker::WorkerConfig {
                    worker_id: worker_id.clone(),
                    lease_seconds: 300,
                    poll_interval_ms: 500,
                    run_checks: true,
                    max_attempts: 5,
                },
                running,
            )
        }));
    }

    let modified_files = Arc::new(Mutex::new(VecDeque::new()));
    let fs_root = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let fs_handle = fs_watch::spawn_fs_monitor(
        fs_root,
        queue.clone(),
        modified_files.clone(),
        running.clone(),
    )?;

    let tui_config = tui::TuiConfig {
        db_path: db_path.display().to_string(),
        worker_count,
        agent_count,
        modified_files: modified_files.clone(),
    };
    let tui_result = tui::run_dashboard_with_config(queue.as_ref(), tui_config);
    running.store(false, Ordering::SeqCst);

    for handle in handles {
        if let Err(err) = handle.join() {
            eprintln!("worker thread failed: {err:?}");
        }
    }

    if let Err(err) = fs_handle.join() {
        eprintln!("fs monitor thread failed: {err:?}");
    }

    tui_result
}
// Orchestrated update for REQ-TEST-002-2 by agent-3
