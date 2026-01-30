use std::{
    error::Error as StdError,
    fmt, fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    thread,
};

use anyhow::{Context, Error};
use diffy::{apply, Patch};
use tracing::info;

use crate::models::{ChangeOperation, ChangeRequest, OperationKind};

#[derive(Debug)]
pub struct ApplyFailure {
    pub patch: String,
    pub stdout: String,
    pub stderr: String,
    source: anyhow::Error,
}

impl ApplyFailure {
    fn new<E>(source: E, patch: String, stdout: String, stderr: String) -> Self
    where
        E: Into<anyhow::Error>,
    {
        Self {
            source: source.into(),
            patch,
            stdout,
            stderr,
        }
    }
}

impl fmt::Display for ApplyFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "apply failure: {}", self.source)
    }
}

impl StdError for ApplyFailure {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.source.as_ref())
    }
}

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
                .map_err(|_| anyhow::anyhow!("worker thread panicked while applying changes"))??;
        }

        Ok(())
    })?;

    info!(task_id = %request.task_id, "change request applied");
    Ok(())
}

fn apply_change_operation(change: ChangeOperation) -> anyhow::Result<()> {
    // try git apply --check first; if it fails, attempt a couple of recoveries:
    //  - create parent directories when stderr indicates missing dirs and retry
    //  - fall back to applying the patch using the current filesystem contents via diffy
    let target = Path::new(&change.path);

    if let Err(err) = run_git_apply_check(&change) {
        // inspect error for missing file/dir and retry after creating parent dir
        let mut retried = false;
        if let Some(af) = err.downcast_ref::<ApplyFailure>() {
            let stderr = af.stderr.to_string();
            if stderr.contains("No such file or directory") {
                if let Some(parent) = target.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if run_git_apply_check(&change).is_ok() {
                    retried = true;
                }
            }
        }
        if !retried {
            // fallback: attempt to apply the patch against the current file contents using diffy
            let existing = fs::read_to_string(target).unwrap_or_default();
            if apply_patch_contents(&existing, &change.patch).is_err() {
                // original git check failure is authoritative
                return Err(err);
            }
            // else we can proceed to write the applied contents below
        }
    }

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
            } else {
                return Err(anyhow::anyhow!(
                    "delete failed: {} does not exist",
                    target.display()
                ));
            }
        }
    }

    Ok(())
}

fn run_git_apply_check(change: &ChangeOperation) -> anyhow::Result<()> {
    let patch = change.patch.clone();
    let mut child = Command::new("git")
        .arg("apply")
        .arg("--check")
        .arg("--whitespace=nowarn")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            Error::from(ApplyFailure::new(
                err,
                patch.clone(),
                String::new(),
                String::new(),
            ))
        })?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(patch.as_bytes()).map_err(|err| {
            Error::from(ApplyFailure::new(
                err,
                patch.clone(),
                String::new(),
                String::new(),
            ))
        })?;
    }
    let output = child.wait_with_output().map_err(|err| {
        Error::from(ApplyFailure::new(
            err,
            patch.clone(),
            String::new(),
            String::new(),
        ))
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return Err(Error::from(ApplyFailure::new(
            anyhow::anyhow!("git apply --check failed ({})", output.status),
            patch,
            stdout,
            stderr,
        )));
    }
    Ok(())
}

fn write_modification(target: &Path, base: &str, change: &ChangeOperation) -> anyhow::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).context("create target directories")?;
    }

    let content = apply_patch_contents(base, &change.patch)?;
    fs::write(target, content).context("write patched file")?;
    Ok(())
}

fn apply_patch_contents(base: &str, patch_text: &str) -> anyhow::Result<String> {
    let patch = Patch::from_str(patch_text).context("parse patch text")?;
    apply(base, &patch).context("apply patch contents")
}
