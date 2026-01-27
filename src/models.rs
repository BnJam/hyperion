use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    #[serde(default)]
    pub patch_hash: Option<String>,
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
    pub lease_owner: Option<String>,
    pub created_at: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: i64,
    pub resume_id: String,
    pub model: String,
    pub allow_all_tools: bool,
    pub created_at: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeQueueLog {
    pub id: i64,
    pub queue_id: i64,
    pub task_id: String,
    pub level: String,
    pub message: String,
    pub details: Option<Value>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileModification {
    pub id: i64,
    pub path: String,
    pub event: String,
    pub source: String,
    pub details: Option<Value>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatusCounts {
    pub pending: usize,
    pub in_progress: usize,
    pub applied: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMetrics {
    pub window_seconds: i64,
    pub status_counts: StatusCounts,
    pub avg_dequeue_latency_ms: Option<f64>,
    pub avg_apply_duration_ms: Option<f64>,
    pub avg_poll_interval_ms: Option<f64>,
    pub throughput_per_minute: Option<f64>,
    pub lease_contention_events: usize,
    pub timestamp: i64,
}

impl Default for QueueMetrics {
    fn default() -> Self {
        Self {
            window_seconds: 60,
            status_counts: StatusCounts::default(),
            avg_dequeue_latency_ms: None,
            avg_apply_duration_ms: None,
            avg_poll_interval_ms: None,
            throughput_per_minute: None,
            lease_contention_events: 0,
            timestamp: 0,
        }
    }
}
