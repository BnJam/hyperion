# Schemas

## TaskRequest
Represents a human-originated request broken into requested changes.

```json
{
  "request_id": "REQ-1001",
  "summary": "Add API rate limit",
  "requested_changes": [
    {
      "path": "src/api/limits.rs",
      "summary": "Implement token bucket"
    }
  ]
}
```

## TaskAssignment
Represents a unit of work for a Developer agent.

```json
{
  "task_id": "REQ-1001-1",
  "parent_request_id": "REQ-1001",
  "summary": "Implement token bucket",
  "file_targets": ["src/api/limits.rs"],
  "instructions": [
    "Keep changes isolated to the listed files.",
    "Provide a structured JSON change request on completion."
  ]
}
```

## ChangeRequest
Represents a Developer-submitted change request.

```json
{
  "task_id": "REQ-1001-1",
  "agent": "developer-2",
  "changes": [
    {
      "path": "src/api/limits.rs",
      "operation": "update",
      "patch": "@@ -10,7 +10,8 @@\n- old\n+ new",
      "patch_hash": "3a7bd3e2360a3d5f5ef2efc6c0c13213cf21e3d25b5c6e4f3f2a0bd7ec9ec6b5"
    }
  ],
  "checks": [
    "cargo test",
    "cargo clippy"
  ]
}
```

When constructing a change request:
- `path` must stay relative and avoid traversal (`..`) to prevent directory escapes.
- `patch` needs to mention the computed `+++ b/{path}` or `--- a/{path}` lines so the queue can detect file alignment.
- `patch_hash` is the SHA-256 digest of the `patch` contents; the validator rejects requests whose hash does not match, ensuring integrity before apply.

## ValidationResult
Describes validation outcomes for a change request.

```json
{
  "valid": true,
  "errors": []
}
```

## QueueMetrics
Provides a telemetry snapshot (`hyperion queue-metrics --format json`) that mirrors the `[progress]` lines emitted by `hyperion run`/`worker`.

```json
{
  "window_seconds": 60,
  "status_counts": {
    "pending": 5,
    "in_progress": 2,
    "applied": 18,
    "failed": 1
  },
  "avg_dequeue_latency_ms": 12.5,
  "avg_apply_duration_ms": 38.2,
  "avg_poll_interval_ms": 500.0,
  "throughput_per_minute": 18.0,
  "lease_contention_events": 0,
  "timestamp": 1700000000,
  "stale_applied_rows": 3,
  "stale_dead_letter_rows": 1,
  "dedup_hits": 2,
  "last_cleanup_timestamp": 1700000500,
  "wal_checkpoint_stats": {
    "checkpointed": 123,
    "log": 456,
    "wal": 8
  },
  "timestamp_skew_secs": 5
}
```

- `window_seconds` is the look-back window (default 60s) used to compute the averages.
- `status_counts` reflects the current queue depth per status.
- Latency/progress fields are optional and `null` when no samples exist.
- `throughput_per_minute` normalizes the number of applied change requests into a per-minute rate.
- `lease_contention_events` counts dequeue metrics where `dequeue_latency_ms` exceeded `poll_interval_ms`, indicating workers were waiting for a lease.
- `stale_applied_rows` and `stale_dead_letter_rows` describe how many applied or dead-letter entries breached their TTLs.
- `dedup_hits` reports how many duplicate `task_id` + payload hash combinations were rejected during the sliding dedup window, and `last_cleanup_timestamp` records when the cleanup sweep most recently ran.
- `wal_checkpoint_stats` mirrors `PRAGMA wal_checkpoint(PASSIVE)` (checkpointed/log/wal pages) so operations can detect WAL pressure without peeking at the file.
- `timestamp_skew_secs` equals `now - MAX(updated_at)` and highlights when queue updates stopped progressing.
