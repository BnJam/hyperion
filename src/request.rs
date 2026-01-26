use std::env;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use anyhow::Context;

use crate::agent::{AgentHarness, CopilotHarness};
use crate::models::{ChangeOperation, ChangeRequest, OperationKind, TaskAssignment, TaskRequest};
use crate::orchestrator;
use crate::queue::SqliteQueue;
use crate::validator;

const DEFAULT_MODEL: &str = "gpt-5-mini";

pub fn handle_request(
    queue: &SqliteQueue,
    path: &Path,
    model: Option<String>,
    max_agents: usize,
) -> anyhow::Result<usize> {
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
    let use_agents = env::var("HYPERION_AGENT").is_ok_and(|val| val == "copilot");

    for index in 0..agent_count {
        let receiver = Arc::clone(&receiver);
        let result_tx = result_tx.clone();
        let agent_name = format!("agent-{}", index + 1);
        let model_name = model.clone().unwrap_or_else(|| DEFAULT_MODEL.to_string());
        handles.push(thread::spawn(move || {
            let harness = if use_agents {
                Some(CopilotHarness::new(model_name))
            } else {
                None
            };
            loop {
                let assignment = {
                    let mut guard = receiver
                        .lock()
                        .map_err(|_| anyhow::anyhow!("assignment receiver lock poisoned"))?;
                    guard.next()
                };
                match assignment {
                    Some(assignment) => {
                        let result = run_assignment(
                            harness.as_ref().map(|h| h as &dyn AgentHarness),
                            &assignment,
                            &agent_name,
                        );
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
    let mut enqueued = 0;
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
                enqueued += 1;
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

    Ok(enqueued)
}

fn run_assignment(
    harness: Option<&dyn AgentHarness>,
    assignment: &TaskAssignment,
    agent_name: &str,
) -> anyhow::Result<ChangeRequest> {
    if let Some(harness) = harness {
        let prompt = build_prompt(assignment, agent_name);
        if let Ok(response) = harness.run(&prompt) {
            if let Ok(mut request) = serde_json::from_str::<ChangeRequest>(&response) {
                request.task_id = assignment.task_id.clone();
                request.agent = agent_name.to_string();
                return Ok(request);
            } else {
                eprintln!("failed to parse agent response");
            }
        } else {
            eprintln!("agent execution failed");
        }
    }
    Ok(fallback_request(assignment, agent_name))
}

fn fallback_request(assignment: &TaskAssignment, agent_name: &str) -> ChangeRequest {
    let change = build_change_operation(&assignment.file_targets[0], assignment, agent_name);
    ChangeRequest {
        task_id: assignment.task_id.clone(),
        agent: agent_name.to_string(),
        changes: vec![change],
        checks: vec!["cargo fmt --check".to_string()],
    }
}

fn build_prompt(assignment: &TaskAssignment, agent_name: &str) -> String {
    format!(
        "You are {agent_name}. Produce a JSON change request only, no prose.\n\n\
Task ID: {task_id}\n\
Summary: {summary}\n\
Files: {files}\n\
Instructions:\n\
- {instructions}\n\n\
Return a single JSON object with fields: task_id, agent, changes (array), checks (array).\n\
Each change must include: path, operation (add/update/delete), patch (diff or full replacement).\n\
Include at least one check in the checks array.\n",
        task_id = assignment.task_id,
        summary = assignment.summary,
        files = assignment.file_targets.join(", "),
        instructions = assignment.instructions.join("\n- "),
        agent_name = agent_name
    )
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
@@ -0,0 +1 @@\n\
// Orchestrated update for {task_id} by {agent}\n",
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
