use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRequest {
    pub task_id: String,
    pub agent: String,
    #[serde(default)]
    pub metadata: AssignmentMetadata,
    pub changes: Vec<ChangeOperation>,
    pub checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    pub request_id: String,
    pub summary: String,
    pub requested_changes: Vec<RequestedChange>,
    #[serde(default)]
    pub phases: Option<Vec<PhaseSpec>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestedChange {
    pub path: String,
    pub summary: String,
    #[serde(default)]
    pub phase_id: Option<String>,
    #[serde(default)]
    pub blocking_on_failure: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub task_id: String,
    pub parent_request_id: String,
    pub summary: String,
    pub file_targets: Vec<String>,
    pub instructions: Vec<String>,
    #[serde(default)]
    pub metadata: AssignmentMetadata,
    #[serde(default)]
    pub phase_id: Option<String>,
    #[serde(default)]
    pub blocking_on_failure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentMetadata {
    pub intent: String,
    pub complexity: u8,
    pub sample_diff: Option<String>,
    pub telemetry_anchors: Vec<String>,
    pub approvals: Vec<ApprovalRecord>,
    pub agent_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseSpec {
    pub id: String,
    pub name: String,
    pub ordinal: u32,
    #[serde(default = "default_failure_policy")]
    pub failure_policy: String,
}

fn default_failure_policy() -> String { "block".to_string() }

impl Default for AssignmentMetadata {
    fn default() -> Self {
        Self {
            intent: String::new(),
            complexity: 1,
            sample_diff: None,
            telemetry_anchors: Vec::new(),
            approvals: Vec::new(),
            agent_model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub approver: String,
    pub note: String,
    pub timestamp: Option<i64>,
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
    pub stale_applied_rows: Option<i64>,
    pub stale_dead_letter_rows: Option<i64>,
    pub dedup_hits: Option<i64>,
    pub last_cleanup_timestamp: Option<i64>,
    pub wal_checkpoint_stats: Option<WalCheckpointStats>,
    pub timestamp_skew_secs: Option<i64>,
    pub agent_requests_per_second: Option<f64>,
    pub agent_average_complexity: Option<f64>,
    pub agent_guard_success_rate: Option<f64>,
    pub agent_guard_approval_latency_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalCheckpointStats {
    pub checkpointed: i64,
    pub log: i64,
    pub wal: i64,
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
            stale_applied_rows: None,
            stale_dead_letter_rows: None,
            dedup_hits: None,
            last_cleanup_timestamp: None,
            wal_checkpoint_stats: None,
            timestamp_skew_secs: None,
            agent_requests_per_second: None,
            agent_average_complexity: None,
            agent_guard_success_rate: None,
            agent_guard_approval_latency_ms: None,
        }
    }
}
