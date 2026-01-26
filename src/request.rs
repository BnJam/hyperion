use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use anyhow::Context;

use crate::models::{ChangeOperation, ChangeRequest, OperationKind, TaskAssignment, TaskRequest};
use crate::orchestrator;
use crate::queue::SqliteQueue;
use crate::validator;

pub fn handle_request(
    queue: &SqliteQueue,
    path: &Path,
    _model: Option<String>,
    max_agents: usize,
) -> anyhow::Result<()> {
    let contents = std::fs::read_to_string(path).context("read task request")?;
    let request: TaskRequest = serde_json::from_str(&contents).context("parse task request")?;
    let assignments = orchestrator::decompose_request(&request);
    if assignments.is_empty() {
        return Err(anyhow::anyhow!("task request produced no assignments"));
    }

    let agent_count = max_agents.clamp(1, 3);
    let receiver = Arc::new(Mutex::new(assignments.into_iter()));
    let (result_tx, result_rx) = mpsc::channel();
    let mut handles = Vec::new();

    for index in 0..agent_count {
        let receiver = Arc::clone(&receiver);
        let result_tx = result_tx.clone();
        let agent_name = format!("agent-{}", index + 1);
        handles.push(thread::spawn(move || {
            loop {
                let assignment = {
                    let mut guard = receiver
                        .lock()
                        .map_err(|_| anyhow::anyhow!("assignment receiver lock poisoned"))?;
                    guard.next()
                };
                match assignment {
                    Some(assignment) => {
                        let result = run_assignment(&assignment, &agent_name);
                        if result_tx.send(result).is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            Ok::<_, anyhow::Error>(())
        }));
    }

    drop(result_tx);

    let mut failures = 0;
    for result in result_rx {
        match result {
            Ok(request) => {
                let validation = validator::validate_change_request(&request);
                if !validation.valid {
                    failures += 1;
                    eprintln!(
                        "invalid change request for {}: {:?}",
                        request.task_id, validation.errors
                    );
                    continue;
                }
                let id = queue.enqueue(&request)?;
                println!("Enqueued change request {id} for {}", request.task_id);
            }
            Err(err) => {
                failures += 1;
                eprintln!("agent failure: {err}");
            }
        }
    }

    for handle in handles {
        if let Err(err) = handle.join() {
            failures += 1;
            eprintln!("agent thread panicked: {err:?}");
        }
    }

    if failures > 0 {
        return Err(anyhow::anyhow!("{} assignment(s) failed", failures));
    }

    Ok(())
}

fn run_assignment(assignment: &TaskAssignment, agent_name: &str) -> anyhow::Result<ChangeRequest> {
    let changes = assignment
        .file_targets
        .iter()
        .map(|path| build_change_operation(path, assignment, agent_name))
        .collect::<Vec<_>>();
    Ok(ChangeRequest {
        task_id: assignment.task_id.clone(),
        agent: agent_name.to_string(),
        changes,
        checks: vec!["cargo fmt --check".to_string()],
    })
}

fn build_change_operation(
    path: &str,
    assignment: &TaskAssignment,
    agent_name: &str,
) -> ChangeOperation {
    let patch = format!(
        "diff --git a/{path} b/{path}\n\
index 0000000..0000000 100644\n\
--- a/{path}\n\
+++ b/{path}\n\
@@\n\
- // Orchestrated update for {task_id} by {agent}\n",
        path = path,
        task_id = assignment.task_id,
        agent = agent_name
    );

    ChangeOperation {
        path: path.to_string(),
        operation: OperationKind::Update,
        patch,
    }
}
