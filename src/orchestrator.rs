use crate::models::{RequestedChange, TaskAssignment, TaskRequest};

pub fn decompose_request(request: &TaskRequest) -> Vec<TaskAssignment> {
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
    TaskAssignment {
        task_id: format!("{}-{}", request.request_id, index + 1),
        parent_request_id: request.request_id.clone(),
        summary: change.summary.clone(),
        file_targets: vec![change.path.clone()],
        instructions: vec![
            "Keep changes isolated to the listed files.".to_string(),
            "Provide a structured JSON change request on completion.".to_string(),
        ],
    }
}
