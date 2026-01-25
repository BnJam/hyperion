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
            binary: "gh".to_string(),
            model: model.into(),
        }
    }

    fn build_command(&self, prompt: &str) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .arg("copilot")
            .arg("suggest")
            .arg("--model")
            .arg(&self.model)
            .arg("--prompt")
            .arg(prompt);
        command
    }
}

impl AgentHarness for CopilotHarness {
    fn run(&self, prompt: &str) -> anyhow::Result<String> {
        let output = self
            .build_command(prompt)
            .output()
            .context("run gh copilot")?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("gh copilot exited with {}", output.status));
        }
        let stdout = String::from_utf8(output.stdout).context("decode gh copilot output")?;
        Ok(stdout)
    }
}
