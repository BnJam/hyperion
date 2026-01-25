use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod models;
mod queue;
mod tui;
mod watcher;

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
    Enqueue { file: PathBuf },
    Dequeue,
    List { status: Option<QueueStatus> },
    MarkApplied { id: i64 },
    MarkFailed { id: i64 },
    Watch { directory: PathBuf },
    Tui,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
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
        Commands::Dequeue => {
            if let Some(record) = queue.dequeue()? {
                println!("Dequeued {} from {}", record.id, record.payload.task_id);
            } else {
                println!("No pending change requests");
            }
        }
        Commands::List { status } => {
            let status = status.unwrap_or(QueueStatus::Pending);
            let records = queue.list(status)?;
            for record in records {
                println!(
                    "{} {} {}",
                    record.id,
                    record.status.as_str(),
                    record.payload.task_id
                );
            }
        }
        Commands::MarkApplied { id } => {
            queue.mark_applied(id)?;
            println!("Marked {id} as applied");
        }
        Commands::MarkFailed { id } => {
            queue.mark_failed(id)?;
            println!("Marked {id} as failed");
        }
        Commands::Watch { directory } => {
            watcher::watch_directory(&queue, &directory)?;
        }
        Commands::Tui => {
            tui::run_dashboard(&queue)?;
        }
    }

    Ok(())
}
