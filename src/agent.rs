use std::process::Command;

use anyhow::Context;

pub trait AgentHarness {
    fn run(&self, prompt: &str) -> anyhow::Result<String>;
}

pub struct CopilotHarness {
    pub binary: String,
    pub model: String,
}

impl CopilotHarness {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            binary: "copilot".to_string(),
            model: model.into(),
        }
    }

    fn build_command(&self, prompt: &str) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .arg("--model")
            .arg(&self.model)
            .arg("--silent")
            .arg("-p")
            .arg(prompt);
        command
    }
}

impl AgentHarness for CopilotHarness {
    fn run(&self, prompt: &str) -> anyhow::Result<String> {
        let output = self.build_command(prompt).output().context("run copilot")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut message = format!("copilot exited with {}", output.status);
            if !stderr.trim().is_empty() {
                message.push_str(&format!(", stderr: {}", stderr.trim()));
            }
            if !stdout.trim().is_empty() {
                message.push_str(&format!(", stdout: {}", stdout.trim()));
            }
            return Err(anyhow::anyhow!(message));
        }
        let stdout = String::from_utf8(output.stdout).context("decode copilot output")?;
        Ok(stdout)
    }
}
