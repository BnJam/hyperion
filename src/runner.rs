use std::process::Command;

use anyhow::Context;
use tracing::info;

pub fn run_checks(checks: &[String]) -> anyhow::Result<()> {
    for check in checks {
        info!(command = %check, "running check");
        let status = Command::new("sh")
            .arg("-c")
            .arg(check)
            .status()
            .with_context(|| format!("run check: {check}"))?;
        if !status.success() {
            return Err(anyhow::anyhow!("check failed: {check}"));
        }
    }
    Ok(())
}
