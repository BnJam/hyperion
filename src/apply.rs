use std::io::Write;
use std::process::{Command, Stdio};

use crate::models::{ChangeOperation, ChangeRequest};
use anyhow::Context;
use tracing::{error, info};

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
    let mut child = Command::new("git")
        .arg("apply")
        .arg("--whitespace=nowarn")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn git apply")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(change.patch.as_bytes())
            .context("write patch")?;
    }

    let output = child.wait_with_output().context("wait git apply")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let mut message = format!("git apply failed for {}", change.path);
        if !stderr.is_empty() {
            message.push_str(&format!(" stderr: {}", stderr));
        }
        if !stdout.is_empty() {
            message.push_str(&format!(" stdout: {}", stdout));
        }
        error!("{}", message);
        return Err(anyhow::anyhow!(message));
    }
    Ok(())
}
