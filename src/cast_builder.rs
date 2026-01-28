use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::{ApprovalRecord, AssignmentMetadata, RequestedChange, TaskAssignment};

#[derive(Clone, Serialize, Deserialize)]
struct CastBundle {
    request_id: String,
    summary: String,
    requested_changes: Vec<RequestedChange>,
    assignments: Vec<TaskAssignment>,
}

pub fn run_cast_builder(out: Option<PathBuf>) -> Result<()> {
    println!("=== Hyperion Cast Builder ===");
    println!("This REPL captures the intent, complexity, approvals, and instructions you want Copilot to follow.");
    println!("You can exit any prompt by pressing Ctrl+C.");
    let context = load_next_task_context();
    if let Some(ctx) = context {
        println!("Current next-task context: {}", ctx);
    }

    let request_id = prompt("Request ID (e.g., REQ-1234): ")?;
    let summary = prompt("High-level summary: ")?;
    let intent = prompt("Intent / rationale: ")?;
    let complexity = prompt_complexity("Complexity rating (1-10): ")?;
    let sample_diff = prompt(
        "Sample diff snippet (single line, e.g., `diff --git a/src/lib.rs b/src/lib.rs`): ",
    )?;
    let telemetry_anchors = prompt_tags(
        "Telemetry anchors (comma separated, at least one): ",
        "cast_builder",
    )?;
    let approvals = prompt_approvals()?;

    let changes_count = prompt_number("How many requested changes (default 1): ", 1)?;
    let mut requested_changes = Vec::with_capacity(changes_count);
    for index in 0..changes_count {
        println!("--- Requested Change #{index} ---");
        let path = prompt("Path (relative to repo root): ")?;
        let summary = prompt("Change summary: ")?;
        requested_changes.push(RequestedChange { path, summary });
    }

    let instructions = prompt_instructions()?;
    let assignments = requested_changes
        .iter()
        .enumerate()
        .map(|(index, change)| TaskAssignment {
            task_id: format!("{}-{}", request_id, index + 1),
            parent_request_id: request_id.clone(),
            summary: change.summary.clone(),
            file_targets: vec![change.path.clone()],
            instructions: instructions.clone(),
            metadata: AssignmentMetadata {
                intent: intent.clone(),
                complexity,
                sample_diff: Some(sample_diff.clone()),
                telemetry_anchors: telemetry_anchors.clone(),
                approvals: approvals.clone(),
                agent_model: Some("gpt-5-mini".to_string()),
            },
        })
        .collect();

    let bundle = CastBundle {
        request_id: request_id.clone(),
        summary: summary.clone(),
        requested_changes,
        assignments,
    };

    let target =
        out.unwrap_or_else(|| PathBuf::from("taskjson").join(format!("{request_id}.json")));
    let target_dir = target
        .parent()
        .ok_or_else(|| anyhow!("failed to determine taskjson directory"))?;
    fs::create_dir_all(target_dir)
        .with_context(|| format!("create directory {}", target_dir.display()))?;
    let payload = serde_json::to_string_pretty(&bundle)?;
    fs::write(&target, payload)
        .with_context(|| format!("write task bundle to {}", target.display()))?;
    println!("Cast JSON written to {}", target.display());

    update_next_task_context(&bundle)?;
    Ok(())
}

fn prompt(message: &str) -> Result<String> {
    print!("{message}");
    io::stdout().flush()?;
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer)?;
    Ok(buffer.trim().to_string())
}

fn prompt_number(prompt_msg: &str, default: usize) -> Result<usize> {
    loop {
        let input = prompt(prompt_msg)?;
        if input.is_empty() {
            return Ok(default);
        }
        if let Ok(value) = input.parse::<usize>() {
            if value >= 1 {
                return Ok(value);
            }
        }
        println!("Please enter a positive integer.");
    }
}

fn prompt_complexity(prompt_msg: &str) -> Result<u8> {
    loop {
        let input = prompt(prompt_msg)?;
        if let Ok(value) = input.parse::<u8>() {
            if (1..=10).contains(&value) {
                return Ok(value);
            }
        }
        println!("Enter a number between 1 and 10.");
    }
}

fn prompt_tags(prompt_msg: &str, default: &str) -> Result<Vec<String>> {
    let input = prompt(prompt_msg)?;
    if input.trim().is_empty() {
        return Ok(vec![default.to_string()]);
    }
    Ok(input
        .split(',')
        .map(|piece| piece.trim().to_string())
        .filter(|val| !val.is_empty())
        .collect())
}

fn prompt_approvals() -> Result<Vec<ApprovalRecord>> {
    let mut approvals = Vec::new();
    loop {
        let add = prompt("Add an approval entry? (y/N): ")?;
        if add.to_lowercase().starts_with('y') {
            let approver = prompt("  Approver name: ")?;
            let note = prompt("  Note/details: ")?;
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|dur| dur.as_secs() as i64)
                .unwrap_or(0);
            approvals.push(ApprovalRecord {
                approver,
                note,
                timestamp: Some(timestamp),
            });
        } else {
            break;
        }
    }
    Ok(approvals)
}

fn prompt_instructions() -> Result<Vec<String>> {
    let input = prompt("Any custom instructions (semicolon separated)? ")?;
    let mut instructions = vec![
        "Keep changes isolated to the listed files.".to_string(),
        "Provide a structured JSON change request on completion.".to_string(),
    ];
    if !input.trim().is_empty() {
        instructions.extend(
            input
                .split(';')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .map(|item| item.to_string()),
        );
    }
    Ok(instructions)
}

fn load_next_task_context() -> Option<String> {
    let path = Path::new("execution/next_task_context.json");
    let contents = fs::read_to_string(path).ok()?;
    let parsed: Value = serde_json::from_str(&contents).ok()?;
    parsed
        .get("cast_builder")
        .and_then(|value| value.get("request_id"))
        .and_then(|value| value.as_str())
        .map(|request_id| format!("latest cast request is {request_id}"))
}

fn update_next_task_context(bundle: &CastBundle) -> Result<()> {
    let path = Path::new("execution").join("next_task_context.json");
    let mut context = if path.exists() {
        let text = fs::read_to_string(&path)?;
        serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
    if let Value::Object(map) = &mut context {
        map.insert(
            "cast_builder".to_string(),
            json!({
                "request_id": bundle.request_id,
                "summary": bundle.summary,
                "intent": bundle.assignments.first().map(|assignment| assignment.metadata.intent.clone()).unwrap_or_default(),
                "complexity": bundle.assignments.first().map(|assignment| assignment.metadata.complexity).unwrap_or(0),
                "telemetry_anchors": bundle.assignments.first().map(|assignment| assignment.metadata.telemetry_anchors.clone()).unwrap_or_default(),
                "approvals": bundle.assignments.first().map(|assignment| assignment.metadata.approvals.clone()).unwrap_or_default(),
                "exported_at": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|dur| dur.as_secs())
                    .unwrap_or(0),
            }),
        );
    }
    let serialized = serde_json::to_string_pretty(&context)?;
    fs::write(&path, serialized).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
