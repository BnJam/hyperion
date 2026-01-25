use crate::models::{ChangeOperation, ChangeRequest, ValidationResult};

pub fn validate_change_request(request: &ChangeRequest) -> ValidationResult {
    let mut errors = Vec::new();
    if request.task_id.trim().is_empty() {
        errors.push("task_id is required".to_string());
    }
    if request.agent.trim().is_empty() {
        errors.push("agent is required".to_string());
    }
    if request.changes.is_empty() {
        errors.push("changes must not be empty".to_string());
    }
    if request.checks.is_empty() {
        errors.push("checks must not be empty".to_string());
    }
    for (index, change) in request.changes.iter().enumerate() {
        validate_change_operation(index, change, &mut errors);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::OperationKind;

    #[test]
    fn validates_change_request_fields() {
        let request = ChangeRequest {
            task_id: "".to_string(),
            agent: "".to_string(),
            changes: vec![ChangeOperation {
                path: "".to_string(),
                operation: OperationKind::Update,
                patch: "".to_string(),
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
        let request = ChangeRequest {
            task_id: "TASK-1".to_string(),
            agent: "dev-1".to_string(),
            changes: vec![ChangeOperation {
                path: "src/lib.rs".to_string(),
                operation: OperationKind::Update,
                patch: "@@ -1,1 +1,1 @@\n-old\n+new".to_string(),
            }],
            checks: vec!["cargo test".to_string()],
        };

        let result = validate_change_request(&request);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }
}
