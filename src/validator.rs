use std::path::Path;

use crate::models::{ChangeOperation, ChangeRequest, OperationKind, ValidationResult};
use sha2::{Digest, Sha256};

pub fn validate_change_request(request: &ChangeRequest) -> ValidationResult {
    let mut errors = Vec::new();
    if request.task_id.trim().is_empty() {
        errors.push("task_id is required".to_string());
    }
    if request.agent.trim().is_empty() {
        errors.push("agent is required".to_string());
    }
    if request.metadata.intent.trim().is_empty() {
        errors.push("metadata.intent is required".to_string());
    }
    if request.metadata.complexity == 0 || request.metadata.complexity > 10 {
        errors.push("metadata.complexity must be between 1 and 10".to_string());
    }
    if request
        .metadata
        .sample_diff
        .as_deref()
        .map(|text| text.trim().is_empty())
        .unwrap_or(true)
    {
        errors.push("metadata.sample_diff is required".to_string());
    }
    if request.metadata.telemetry_anchors.is_empty() {
        errors.push("metadata.telemetry_anchors must not be empty".to_string());
    }
    if request.changes.is_empty() {
        errors.push("changes must not be empty".to_string());
    }
    if request.checks.is_empty() {
        errors.push("checks must not be empty".to_string());
    }
    for (index, change) in request.changes.iter().enumerate() {
        validate_change_operation(index, change, &mut errors);
        validate_patch_alignment(index, change, &mut errors);
        validate_patch_hash(index, change, &mut errors);
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
    }
}

fn validate_change_operation(index: usize, change: &ChangeOperation, errors: &mut Vec<String>) {
    if change.path.trim().is_empty() {
        errors.push(format!("changes[{index}].path is required"));
    }
    if change.patch.trim().is_empty() {
        errors.push(format!("changes[{index}].patch is required"));
    }
    let path = Path::new(&change.path);
    if path.is_absolute() {
        errors.push(format!(
            "changes[{index}].path must be relative, got {}",
            change.path
        ));
    }
    if change.path.contains("..") {
        errors.push(format!(
            "changes[{index}].path must not contain '..', got {}",
            change.path
        ));
    }
}

fn validate_patch_alignment(index: usize, change: &ChangeOperation, errors: &mut Vec<String>) {
    if change.path.trim().is_empty() {
        return;
    }
    let normalized = change.path.replace('\\', "/");
    let add_marker = format!("+++ b/{normalized}");
    let remove_marker = format!("--- a/{normalized}");
    match change.operation {
        OperationKind::Add => {
            if !change.patch.contains(&add_marker) {
                errors.push(format!(
                    "changes[{index}]: add operation patch must mention {add_marker}"
                ));
            }
        }
        OperationKind::Update => {
            if !change.patch.contains(&add_marker) {
                errors.push(format!(
                    "changes[{index}]: update patch must mention {add_marker}"
                ));
            }
            if !change.patch.contains(&remove_marker) {
                errors.push(format!(
                    "changes[{index}]: update patch must mention {remove_marker}"
                ));
            }
        }
        OperationKind::Delete => {
            if !change.patch.contains(&remove_marker) {
                errors.push(format!(
                    "changes[{index}]: delete patch must mention {remove_marker}"
                ));
            }
        }
    }
}

fn validate_patch_hash(index: usize, change: &ChangeOperation, errors: &mut Vec<String>) {
    if change.patch.trim().is_empty() {
        return;
    }
    let calculated = patch_hash(&change.patch);
    match &change.patch_hash {
        Some(expected) => {
            if expected != &calculated {
                errors.push(format!(
                    "changes[{index}]: patch_hash mismatch (expected {calculated}, got {expected})"
                ));
            }
        }
        None => {
            errors.push(format!("changes[{index}].patch_hash is required"));
        }
    }
}

fn patch_hash(patch: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(patch.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AssignmentMetadata;
    use crate::models::OperationKind;

    #[test]
    fn validates_change_request_fields() {
        let request = ChangeRequest {
            task_id: "".to_string(),
            agent: "".to_string(),
            metadata: AssignmentMetadata {
                intent: "".to_string(),
                complexity: 0,
                sample_diff: None,
                telemetry_anchors: vec![],
                approvals: vec![],
                agent_model: None,
            },
            changes: vec![ChangeOperation {
                path: "".to_string(),
                operation: OperationKind::Update,
                patch: "".to_string(),
                patch_hash: Some(patch_hash("")),
            }],
            checks: vec![],
        };

        let result = validate_change_request(&request);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("task_id")));
        assert!(result.errors.iter().any(|e| e.contains("agent")));
        assert!(result.errors.iter().any(|e| e.contains("checks must")));
        assert!(result.errors.iter().any(|e| e.contains("changes[0].path")));
        assert!(result.errors.iter().any(|e| e.contains("changes[0].patch")));
    }

    #[test]
    fn accepts_valid_request() {
        let patch = "diff --git a/src/lib.rs b/src/lib.rs\n\
        --- a/src/lib.rs\n\
        +++ b/src/lib.rs\n\
        @@ -1,1 +1,1 @@\n\
        -old\n\
        +new"
            .replace("        ", "");
        let request = ChangeRequest {
            task_id: "TASK-1".to_string(),
            agent: "dev-1".to_string(),
            metadata: AssignmentMetadata {
                intent: "Clarify run".to_string(),
                complexity: 3,
                sample_diff: Some("diff --git a/foo b/foo".to_string()),
                telemetry_anchors: vec!["cast:REQ".to_string()],
                approvals: vec![],
                agent_model: Some("gpt-5-mini".to_string()),
            },
            changes: vec![ChangeOperation {
                path: "src/lib.rs".to_string(),
                operation: OperationKind::Update,
                patch: patch.clone(),
                patch_hash: Some(patch_hash(&patch)),
            }],
            checks: vec!["cargo test".to_string()],
        };

        let result = validate_change_request(&request);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }
}
