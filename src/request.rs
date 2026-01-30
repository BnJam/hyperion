use std::{
    env, fs,
    path::Path,
    sync::{mpsc, Arc, Mutex},
    thread,
};

use anyhow::Context;
use diffy::create_patch;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::agent::{AgentHarness, CopilotHarness};
use crate::models::{
    AssignmentMetadata, ChangeOperation, ChangeRequest, OperationKind, TaskAssignment, TaskRequest,
};
use crate::orchestrator;
use crate::queue::SqliteQueue;
use crate::telemetry;
use crate::validator;

pub const DEFAULT_MODEL: &str = "gpt-5-mini";
pub const FALLBACK_MODEL: &str = "gpt-4.1";
const FALLBACK_COMPLEXITY_THRESHOLD: u8 = 3;

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
    let session = if use_agents {
        queue.latest_agent_session()?
    } else {
        None
    };
    let session_for_threads = session.clone();

    for index in 0..agent_count {
        let receiver = Arc::clone(&receiver);
        let result_tx = result_tx.clone();
        let agent_name = format!("agent-{}", index + 1);
        let model_name = model.clone().unwrap_or_else(|| DEFAULT_MODEL.to_string());
        let session_ref = session_for_threads.clone();
        handles.push(thread::spawn(move || {
            let harness = if use_agents {
                Some(CopilotHarness::with_session(
                    model_name.clone(),
                    session_ref.as_ref(),
                ))
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
                            &model_name,
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
                let _ = queue.log_event(
                    id,
                    &request.task_id,
                    "info",
                    "agent_request",
                    Some(&json!({
                        "agent": request.agent,
                        "model": request
                            .metadata
                            .agent_model
                            .as_deref()
                            .unwrap_or(DEFAULT_MODEL),
                        "complexity": request.metadata.complexity,
                        "intent": request.metadata.intent,
                        "checks": request.checks,
                        "telemetry_anchors": request.metadata.telemetry_anchors,
                    })),
                );
                if let Err(err) = telemetry::write_verification_report(queue) {
                    eprintln!("telemetry update failed: {err}");
                }
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

    if use_agents {
        if let Some(session) = session {
            let _ = queue.touch_agent_session(session.id);
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
    model_name: &str,
) -> anyhow::Result<ChangeRequest> {
    let effective_model = select_model(model_name, &assignment.metadata);
    if let Some(harness) = harness {
        let prompt = build_prompt(assignment, agent_name, &effective_model);
        if let Ok(response) = harness.run(&prompt) {
            if let Ok(mut request) = serde_json::from_str::<ChangeRequest>(&response) {
                request.task_id = assignment.task_id.clone();
                request.agent = agent_name.to_string();
                // ensure the produced ChangeRequest records the originating phase and blocking flag
                request.metadata = merge_assignment_metadata(
                    request.metadata,
                    &assignment.metadata,
                    &effective_model,
                );
                // embed the phase_id and blocking flag in metadata for queue extraction
                // (metadata already has space for sample_diff / intent etc.)
                if request.metadata.sample_diff.is_none() {
                    request.metadata.sample_diff = assignment.metadata.sample_diff.clone();
                }
                // attach phase and blocking flag as additional top-level fields as well
                // so the queue can extract phase_id easily
                let mut payload_val: serde_json::Value = serde_json::from_str(&response).unwrap_or_else(|_| serde_json::json!({}));
                if let serde_json::Value::Object(ref mut map) = payload_val {
                    map.insert("phase_id".to_string(), serde_json::Value::String(assignment.phase_id.clone().unwrap_or_default()));
                    map.insert("blocking_on_failure".to_string(), serde_json::Value::Bool(assignment.blocking_on_failure));
                }
                let patched = serde_json::to_string(&payload_val).unwrap_or(response.clone());
                if let Ok(mut request) = serde_json::from_str::<ChangeRequest>(&patched) {
                    request.task_id = assignment.task_id.clone();
                    request.agent = agent_name.to_string();
                    request.metadata = merge_assignment_metadata(
                        request.metadata,
                        &assignment.metadata,
                        &effective_model,
                    );
                    for change in request.changes.iter_mut() {
                        if change.patch_hash.is_none() {
                            change.patch_hash = Some(compute_patch_hash(&change.patch));
                        }
                    }
                    return Ok(request);
                }
            } else {
                eprintln!("failed to parse agent response");
            }
        } else {
            eprintln!("agent execution failed with model {effective_model}");
        }
    }
    Ok(fallback_request(assignment, agent_name, &effective_model))
}

fn fallback_request(
    assignment: &TaskAssignment,
    agent_name: &str,
    model_name: &str,
) -> ChangeRequest {
    let change = build_change_operation(&assignment.file_targets[0], assignment, agent_name);
    ChangeRequest {
        task_id: assignment.task_id.clone(),
        agent: agent_name.to_string(),
        metadata: {
            let mut metadata = assignment.metadata.clone();
            metadata.agent_model = Some(model_name.to_string());
            metadata
        },
        changes: vec![change],
        checks: vec!["cargo fmt --check".to_string()],
    }
}

fn build_prompt(assignment: &TaskAssignment, agent_name: &str, model_name: &str) -> String {
    format!(
        "You are {agent_name} running {model_name}. Provide only JSON output, no prose.\n\n\
Task ID: {task_id}\n\
Summary: {summary}\n\
Intent: {intent}\n\
Complexity: {complexity}/10\n\
Sample Diff: {sample_diff}\n\
Telemetry Anchors: {anchors}\n\
Files: {files}\n\
Instructions:\n\
- {instructions}\n\n\
Return a single JSON object with keys: task_id, agent, metadata, changes, checks.\n\
metadata must contain: intent, complexity, sample_diff, telemetry_anchors, approvals, and agent_model.\n\
Each change entry must include path, operation (add/update/delete), patch, and patch_hash.\n\
Include at least one check in the checks array and describe the guard expectations for those checks.\n",
        task_id = assignment.task_id,
        summary = assignment.summary,
        files = assignment.file_targets.join(", "),
        instructions = assignment.instructions.join("\n- "),
        agent_name = agent_name,
        model_name = model_name,
        intent = assignment.metadata.intent,
        complexity = assignment.metadata.complexity,
        sample_diff = assignment
            .metadata
            .sample_diff
            .as_deref()
            .unwrap_or("sample diff placeholder"),
        anchors = if assignment.metadata.telemetry_anchors.is_empty() {
            "none".to_string()
        } else {
            assignment.metadata.telemetry_anchors.join(", ")
        }
    )
}

fn build_change_operation(
    path: &str,
    assignment: &TaskAssignment,
    agent_name: &str,
) -> ChangeOperation {
    let target = Path::new(path);
    let base_content = fs::read_to_string(target).unwrap_or_default();
    let addition = format!(
        "// Orchestrated update for {task_id} by {agent}\n",
        task_id = assignment.task_id,
        agent = agent_name
    );
    let mut updated_content = base_content.clone();
    if !updated_content.is_empty() && !updated_content.ends_with('\n') {
        updated_content.push('\n');
    }
    updated_content.push_str(&addition);
    let mut patch_body = create_patch(&base_content, &updated_content).to_string();
    if let Some(pos) = patch_body.find("@@") {
        patch_body = patch_body[pos..].to_string();
    }
    let patch = format!(
        "diff --git a/{path} b/{path}\n\
index 0000000..0000000 100644\n\
--- a/{path}\n\
+++ b/{path}\n\
{body}",
        path = path,
        body = patch_body
    );
    let patch_hash = compute_patch_hash(&patch);

    ChangeOperation {
        path: path.to_string(),
        operation: OperationKind::Update,
        patch,
        patch_hash: Some(patch_hash),
    }
}

fn compute_patch_hash(patch: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(patch.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn merge_assignment_metadata(
    agent_meta: AssignmentMetadata,
    assignment_meta: &AssignmentMetadata,
    model_name: &str,
) -> AssignmentMetadata {
    let mut metadata = assignment_meta.clone();
    if !agent_meta.intent.trim().is_empty() {
        metadata.intent = agent_meta.intent;
    }
    if agent_meta.complexity != 0 {
        metadata.complexity = agent_meta.complexity.clamp(1, 10);
    }
    if agent_meta.sample_diff.is_some() {
        metadata.sample_diff = agent_meta.sample_diff;
    }
    if !agent_meta.telemetry_anchors.is_empty() {
        metadata.telemetry_anchors = agent_meta.telemetry_anchors;
    }
    if !agent_meta.approvals.is_empty() {
        metadata.approvals = agent_meta.approvals;
    }
    metadata.agent_model = Some(model_name.to_string());
    metadata
}

fn select_model(requested: &str, metadata: &AssignmentMetadata) -> String {
    if metadata.complexity <= FALLBACK_COMPLEXITY_THRESHOLD {
        FALLBACK_MODEL.to_string()
    } else {
        requested.to_string()
    }
}
