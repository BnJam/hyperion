use std::error::Error as StdError;
use std::fmt;
use std::process::{Command, Stdio};

use anyhow::Context;
use tracing::info;

#[derive(Debug)]
pub struct CheckFailure {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    source: anyhow::Error,
}

impl CheckFailure {
    fn new<E>(source: E, command: String, stdout: String, stderr: String) -> Self
    where
        E: Into<anyhow::Error>,
    {
        Self {
            source: source.into(),
            command,
            stdout,
            stderr,
        }
    }
}

impl fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "check failure: {}", self.source)
    }
}

impl StdError for CheckFailure {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.source.as_ref())
    }
}

pub fn run_checks(checks: &[String]) -> anyhow::Result<()> {
    for check in checks {
        info!(command = %check, "running check");
        let output = Command::new("sh")
            .arg("-c")
            .arg(check)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("run check: {check}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !output.status.success() {
            return Err(CheckFailure::new(
                anyhow::anyhow!("check failed: {check} (exit {})", output.status),
                check.clone(),
                stdout,
                stderr,
            )
            .into());
        }
    }
    Ok(())
}
