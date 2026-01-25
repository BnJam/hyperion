use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::Context;
use notify::{recommended_watcher, RecursiveMode, Watcher};

use crate::models::ChangeRequest;
use crate::queue::SqliteQueue;

pub fn watch_directory(queue: &SqliteQueue, path: &Path) -> anyhow::Result<()> {
    let (tx, rx) = channel();
    let mut watcher = recommended_watcher(tx).context("create file watcher")?;
    watcher.watch(path, RecursiveMode::NonRecursive)?;

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => {
                for path in event.paths {
                    if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                        if let Err(err) = ingest_change_request(queue, &path) {
                            eprintln!("failed to ingest {path:?}: {err}");
                        }
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(err) => return Err(err.into()),
        }
    }
}

fn ingest_change_request(queue: &SqliteQueue, path: &PathBuf) -> anyhow::Result<()> {
    let contents = std::fs::read_to_string(path).context("read change request")?;
    let request: ChangeRequest = serde_json::from_str(&contents).context("parse change request")?;
    queue.enqueue(&request)?;
    Ok(())
}
