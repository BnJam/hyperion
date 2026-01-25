use std::process::Command;

use anyhow::Context;

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
        "applying patch"
    );
    let mut command = Command::new("git");
    let mut child = command
        .arg("apply")
        .arg("--whitespace=nowarn")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("spawn git apply")?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(change.patch.as_bytes())
            .context("write patch")?;
    }

    let status = child.wait().context("wait git apply")?;
    if !status.success() {
        return Err(anyhow::anyhow!("git apply failed for {}", change.path));
    }
    Ok(())
}
