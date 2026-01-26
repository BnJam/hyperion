use std::process::Command;

use anyhow::Context;

use crate::models::AgentSession;

pub trait AgentHarness {
    fn run(&self, prompt: &str) -> anyhow::Result<String>;
}

pub struct CopilotHarness {
    pub binary: String,
    pub model: String,
    pub session: Option<String>,
    pub allow_all_tools: bool,
}

impl CopilotHarness {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            binary: "copilot".to_string(),
            model: model.into(),
            session: None,
            allow_all_tools: true,
        }
    }

    pub fn with_session(model: impl Into<String>, session: Option<&AgentSession>) -> Self {
        let default_model = model.into();
        let (model, session_id, allow_all_tools) = if let Some(session) = session {
            (
                session.model.clone(),
                Some(session.resume_id.clone()),
                session.allow_all_tools,
            )
        } else {
            (default_model, None, true)
        };
        Self {
            binary: "copilot".to_string(),
            model,
            session: session_id,
            allow_all_tools,
        }
    }

    fn build_command(&self, prompt: &str) -> Command {
        let mut command = Command::new(&self.binary);
        if let Some(session_id) = self.session.as_ref() {
            command.arg("--resume").arg(session_id);
            if self.allow_all_tools {
                command.arg("--allow-all-tools");
            }
        } else {
            command.arg("--model").arg(&self.model);
            if self.allow_all_tools {
                command.arg("--allow-all-tools");
            }
        }
        command.arg("--silent").arg("-p").arg(prompt);
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
