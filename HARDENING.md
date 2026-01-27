# Hardening & Resiliency

## Queue Durability
- **SQLite WAL** for concurrent writers and crash-safe durability.
- **Busy timeouts** to handle contention gracefully.
- **Synchronous NORMAL** to balance safety and throughput.

## Leases & Retries
- **Leased dequeue** prevents duplicate work on worker crashes.
- **Lease expiry** allows reprocessing when a worker dies.
- **Attempt counters** enable retry policies and escalation.
- **Error capture** stores last failure for diagnosis.
- **Retry caps** stop flapping requests from looping indefinitely.

## Operational Safety
- **Idempotency:** change requests must be safe to apply multiple times.
- **Auditability:** queue records capture state transitions and last errors.
- **Quarantine:** failed requests remain visible and reviewable.
- **Validation gates:** reject invalid change requests before apply.
- **Graceful shutdown:** worker responds to SIGINT and avoids partial state writes.
- **Dead letter handling:** failed requests are archived for triage.
- **Patch integrity:** `validator` enforces that paths stay relative, patches mention the targeted file, and the supplied `patch_hash` (SHA-256) matches the diff text so workers cannot drift silently.

## Observability
- **Structured logs** for enqueue/dequeue/apply outcomes.
- **Metrics** for throughput, retries, conflicts, and latency.
- **Telemetry CLI** (`hyperion queue-metrics --format json --since 60`) and `[progress]` lines from workers expose throughput/minute, average queue latencies, and lease contention counts so automation and ops scripts can consume the same signals as the TUI.
- **TUI dashboard** for real-time queue visibility (ratatui).
- **CLI validation** for schema enforcement during intake.
- **Diagnostics command:** `hyperion doctor` checkpoints the WAL, verifies schema/index health, and surfaces how many applied/dead-letter rows exceed retention bounds.

## Security & Governance
- **Schema validation** before enqueue.
- **Access control** for agents that submit change requests.
- **Approval gates** for high-risk changes.
- **Retention policy:** TTL/archival helpers (`purge_applied`, `purge_dead_letters`) keep the queue lean and track how many records cross retention thresholds for compliance.

## Disaster Recovery
- **Backup strategy** for SQLite DB.
- **Replay plan** using queued change requests.
- **Rollback procedures** for applied patches.
