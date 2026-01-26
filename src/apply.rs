use std::{fs, path::Path, thread};

use anyhow::{anyhow, Context};
use diffy::{apply, Patch};
use tracing::info;

use crate::models::{ChangeOperation, ChangeRequest, OperationKind};

pub fn apply_change_request(request: &ChangeRequest) -> anyhow::Result<()> {
    info!(
        task_id = %request.task_id,
        agent = %request.agent,
        change_count = request.changes.len(),
        "applying change request via filesystem"
    );

    let operations = request.changes.clone();
    thread::scope(|scope| -> anyhow::Result<()> {
        let mut handles = Vec::with_capacity(operations.len());
        for change in operations {
            handles.push(scope.spawn(move || apply_change_operation(change)));
        }

        for handle in handles {
            handle
                .join()
                .map_err(|_| anyhow!("worker thread panicked while applying changes"))??;
        }

        Ok(())
    })?;

    info!(task_id = %request.task_id, "change request applied");
    Ok(())
}

fn apply_change_operation(change: ChangeOperation) -> anyhow::Result<()> {
    let target = Path::new(&change.path);
    info!(
        path = %change.path,
        operation = ?change.operation,
        "applying filesystem change"
    );

    match change.operation {
        OperationKind::Add => {
            write_modification(target, "", &change)?;
        }
        OperationKind::Update => {
            let existing = fs::read_to_string(target).unwrap_or_default();
            write_modification(target, &existing, &change)?;
        }
        OperationKind::Delete => {
            if target.exists() {
                fs::remove_file(target).context("delete target file")?;
            }
        }
    }

    Ok(())
}

fn write_modification(target: &Path, base: &str, change: &ChangeOperation) -> anyhow::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).context("create target directories")?;
    }

    let content = apply_patch_contents(base, &change.patch);
    fs::write(target, content).context("write patched file")?;
    Ok(())
}

fn apply_patch_contents(base: &str, patch_text: &str) -> String {
    if let Ok(patch) = Patch::from_str(patch_text) {
        if let Ok(applied) = apply(base, &patch) {
            return applied;
        }
    }

    patch_text.to_string()
}
