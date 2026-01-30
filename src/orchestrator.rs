use crate::models::{AssignmentMetadata, RequestedChange, TaskAssignment, TaskRequest};

pub fn decompose_request(request: &TaskRequest) -> Vec<TaskAssignment> {
    // honor provided phases by assigning phase_id into each TaskAssignment when present in requested_changes
    request
        .requested_changes
        .iter()
        .enumerate()
        .map(|(index, change)| build_assignment(request, change, index))
        .collect()
}

fn build_assignment(
    request: &TaskRequest,
    change: &RequestedChange,
    index: usize,
) -> TaskAssignment {
    let metadata = AssignmentMetadata {
        intent: format!("{} :: {}", request.summary, change.summary),
        complexity: compute_complexity(change),
        sample_diff: Some(sample_diff(change)),
        telemetry_anchors: vec![
            format!("cast:{}", request.request_id),
            format!("task:{}", change.summary.replace(' ', "_")),
        ],
        approvals: Vec::new(),
        agent_model: None,
        phase_id: change.phase_id.clone(),
        blocking_on_failure: change.blocking_on_failure,
    };

    TaskAssignment {
        task_id: format!("{}-{}", request.request_id, index + 1),
        parent_request_id: request.request_id.clone(),
        summary: change.summary.clone(),
        file_targets: vec![change.path.clone()],
        instructions: vec![
            "Keep changes isolated to the listed files.".to_string(),
            "Provide a structured JSON change request on completion.".to_string(),
        ],
        metadata,
        phase_id: change.phase_id.clone(),
        blocking_on_failure: change.blocking_on_failure.unwrap_or(true),
    }
}

fn compute_complexity(change: &RequestedChange) -> u8 {
    let length = change.summary.len() as u8;
    let path_factor = change.path.len() as u8;
    1 + ((length + path_factor) % 10)
}

fn sample_diff(change: &RequestedChange) -> String {
    format!(
        "diff --git a/{path} b/{path}\n@@ -0,0 +1 @@\n+// Update: {summary}",
        path = change.path,
        summary = change.summary
    )
}
