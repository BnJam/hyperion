use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRequest {
    pub task_id: String,
    pub agent: String,
    pub changes: Vec<ChangeOperation>,
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    pub request_id: String,
    pub summary: String,
    pub requested_changes: Vec<RequestedChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestedChange {
    pub path: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub task_id: String,
    pub parent_request_id: String,
    pub summary: String,
    pub file_targets: Vec<String>,
    pub instructions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeOperation {
    pub path: String,
    pub operation: OperationKind,
    pub patch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Add,
    Update,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueRecord {
    pub id: i64,
    pub status: QueueStatus,
    pub payload: ChangeRequest,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub leased_until: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterRecord {
    pub id: i64,
    pub queue_id: i64,
    pub task_id: String,
    pub agent: String,
    pub payload: ChangeRequest,
    pub error: Option<String>,
    pub failed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    Pending,
    InProgress,
    Applied,
    Failed,
}

impl QueueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueueStatus::Pending => "pending",
            QueueStatus::InProgress => "in_progress",
            QueueStatus::Applied => "applied",
            QueueStatus::Failed => "failed",
        }
    }
}
