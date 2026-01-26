use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::channel,
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use anyhow::Context;
use notify::{recommended_watcher, EventKind, RecursiveMode, Watcher};
use serde_json::json;

use crate::queue::SqliteQueue;

pub fn spawn_fs_monitor(
    root: PathBuf,
    queue: Arc<SqliteQueue>,
    modified_files: Arc<Mutex<VecDeque<String>>>,
    running: Arc<AtomicBool>,
) -> anyhow::Result<thread::JoinHandle<()>> {
    let (tx, rx) = channel();
    let mut watcher = recommended_watcher(tx).context("create fs watcher")?;
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .context("start fs watcher")?;

    let handle = thread::spawn(move || {
        while running.load(Ordering::SeqCst) {
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Ok(event)) => {
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    ) {
                        for path in event.paths {
                            if let Some(display) = path.to_str() {
                                let trimmed = display.trim();
                                let mut files = modified_files.lock().unwrap();
                                files.push_front(trimmed.to_string());
                                if files.len() > 10 {
                                    files.pop_back();
                                }
                                drop(files);
                                let event_kind = format!("{:?}", event.kind);
                                let details = json!({ "path": trimmed, "event": event_kind });
                                let _ = queue.log_event(
                                    0,
                                    "fsnotify",
                                    "info",
                                    "file modified",
                                    Some(&details),
                                );
                                let _ = queue.record_file_event(
                                    trimmed,
                                    &event_kind,
                                    "fsnotify",
                                    Some(&details),
                                );
                            }
                        }
                    }
                }
                Ok(Err(err)) => eprintln!("fs watcher error: {err}"),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(err) => {
                    eprintln!("fs watcher channel error: {err}");
                    break;
                }
            }
        }
    });
    Ok(handle)
}
