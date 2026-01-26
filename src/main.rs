use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;

use clap::{Parser, Subcommand};

mod agent;
mod apply;
mod models;
mod orchestrator;
mod queue;
mod request;
mod runner;
mod tui;
mod validator;
mod watcher;
mod worker;

use crate::agent::AgentHarness;
use models::QueueStatus;
use queue::SqliteQueue;

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
    },
    ListDeadLetters,
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
            workers,
        }) => {
            let agent_count = agents.clamp(1, 3);
            let worker_count = workers.clamp(1, 3);
            request::handle_request(queue.as_ref(), &file, model, agent_count)?;
            run_integrated(queue.clone(), cli.db.clone(), worker_count, agent_count)
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
            if let Some(record) = queue.dequeue(std::time::Duration::from_secs(lease_seconds))? {
                println!(
                    "Dequeued {} from {} (attempt {})",
                    record.id, record.payload.task_id, record.attempts
                );
            } else {
                println!("No pending change requests");
            }
            Ok(())
        }
        Some(Commands::List { status }) => {
            let status = status.unwrap_or(QueueStatus::Pending);
            let records = queue.list(status)?;
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
            Ok(())
        }
        Some(Commands::ListDeadLetters) => {
            let records = queue.list_dead_letters()?;
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
        }) => worker::run_worker(
            queue.as_ref(),
            worker::WorkerConfig {
                lease_seconds,
                poll_interval_ms,
                run_checks,
                max_attempts,
            },
        ),
    }
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("hyperion=info".parse().unwrap());
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
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
    for _ in 0..worker_count {
        let queue = queue.clone();
        let running = running.clone();
        handles.push(thread::spawn(move || {
            worker::run_worker_with_signal(
                queue.as_ref(),
                worker::WorkerConfig {
                    lease_seconds: 300,
                    poll_interval_ms: 500,
                    run_checks: true,
                    max_attempts: 5,
                },
                running,
            )
        }));
    }

    let tui_config = tui::TuiConfig {
        db_path: db_path.display().to_string(),
        worker_count,
        agent_count,
    };
    let tui_result = tui::run_dashboard_with_config(queue.as_ref(), tui_config);
    running.store(false, Ordering::SeqCst);

    for handle in handles {
        if let Err(err) = handle.join() {
            eprintln!("worker thread failed: {err:?}");
        }
    }

    tui_result
}
