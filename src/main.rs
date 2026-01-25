use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod agent;
mod apply;
mod models;
mod orchestrator;
mod queue;
mod runner;
mod tui;
mod validator;
mod watcher;
mod worker;

use models::QueueStatus;
use queue::SqliteQueue;

#[derive(Parser)]
#[command(name = "hyperion", version, about = "Multi-agent orchestration queue")]
struct Cli {
    #[arg(long, default_value = "hyperion.db")]
    db: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_tracing();
    let queue = SqliteQueue::open(&cli.db)?;

    match cli.command {
        Commands::Init => {
            println!("Initialized queue at {}", cli.db.display());
        }
        Commands::Enqueue { file } => {
            let contents = std::fs::read_to_string(file)?;
            let request: models::ChangeRequest = serde_json::from_str(&contents)?;
            let id = queue.enqueue(&request)?;
            println!("Enqueued change request {id}");
        }
        Commands::Dequeue { lease_seconds } => {
            if let Some(record) = queue.dequeue(std::time::Duration::from_secs(lease_seconds))? {
                println!(
                    "Dequeued {} from {} (attempt {})",
                    record.id, record.payload.task_id, record.attempts
                );
            } else {
                println!("No pending change requests");
            }
        }
        Commands::List { status } => {
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
        }
        Commands::MarkApplied { id } => {
            queue.mark_applied(id)?;
            println!("Marked {id} as applied");
        }
        Commands::MarkFailed { id, error } => {
            queue.mark_failed(id, error)?;
            println!("Marked {id} as failed");
        }
        Commands::Watch { directory } => {
            watcher::watch_directory(&queue, &directory)?;
        }
        Commands::Tui => {
            tui::run_dashboard(&queue)?;
        }
        Commands::Agent { model, prompt } => {
            let harness = agent::CopilotHarness::new(model.unwrap_or_else(|| "gpt-5-mini".into()));
            let response = harness.run(&prompt)?;
            println!("{response}");
        }
        Commands::ValidateChange { file } => {
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
        }
        Commands::Orchestrate { file, out } => {
            let contents = std::fs::read_to_string(file)?;
            let request: models::TaskRequest = serde_json::from_str(&contents)?;
            let assignments = orchestrator::decompose_request(&request);
            let payload = serde_json::to_string_pretty(&assignments)?;
            if let Some(out) = out {
                std::fs::write(out, payload)?;
            } else {
                println!("{payload}");
            }
        }
        Commands::Apply { file, run_checks } => {
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
        }
        Commands::Worker {
            lease_seconds,
            poll_interval_ms,
            run_checks,
        } => {
            worker::run_worker(
                &queue,
                worker::WorkerConfig {
                    lease_seconds,
                    poll_interval_ms,
                    run_checks,
                },
            )?;
        }
    }

    Ok(())
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("hyperion=info".parse().unwrap());
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
