use crate::models::{ChangeOperation, ChangeRequest};
use tracing::info;

pub fn apply_change_request(request: &ChangeRequest) -> anyhow::Result<()> {
    info!(
        task_id = %request.task_id,
        agent = %request.agent,
        change_count = request.changes.len(),
        "applying change request"
    );
    for change in &request.changes {
        apply_change_operation(change)?;
    }
    info!(task_id = %request.task_id, "change request applied");
    Ok(())
}

fn apply_change_operation(change: &ChangeOperation) -> anyhow::Result<()> {
    info!(
        path = %change.path,
        operation = ?change.operation,
        "simulating patch application: {}",
        change.patch
    );
    Ok(())
}
