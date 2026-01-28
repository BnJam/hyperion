# Hyperion ASI Framework

## Intent
- Ground Hyperion improvements in the resilient queue/diagnostics story from TechnoCore and the issue-driven communication layers from Farcaster.
- Keep the human-in-the-loop approval, telemetry, and CLI/TUI observability that operators already trust.

## Observations
- TechnoCore writes to a WAL-backed SQLite queue, keeps prepared statements warm, and surfaces a Doctor/Ratatui dashboard with audit logs and fsnotify events. Reuse those patterns for Hyperion's `change_queue`, session metadata, and telemetry exports.
- Farcaster pairs GitHub issues with Jules-owned casts, gatekeeping through CI (fmt/clippy/test) and automated Jules auto-fixes. Mirror that overlay by adding issue ingestion, merge queue buffers, and structured notifications before running heavy validations.
- Both systems favor deterministic JSON payloads, per-task logging, and WAL/audit trails; Hyperion should continue those habits while expanding to new merge slots.

## Integrations
- Document the planned per-worker lease + WAL telemetry updates so Hyperion's queue follows TechnoCore's delightfully observable workflow.
- Describe the issue bridge and merge queue buffer so operators know how Farcaster-style casts translate into Hyperion change requests.
- Keep the CLI/TUI explicit: no hidden automation, just gating, telemetry, and human approvals borrowed from the two reference systems.
- Capture the agent harness metadata expectations (intent, complexity, sample diffs, telemetry anchors, agent_model) so the cast protocol remains deterministic and traceable from ingestion to apply.
- Surface the agent telemetry exports (requests/sec, guard success rate, approval latency) and note that `execution/verification_report.json` mirrors the TUI metrics for unattended dashboards.
