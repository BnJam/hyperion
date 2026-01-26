name: hyperion-improvement
license: MIT
summary: Continuously assess Hyperion queue health and surface actionable improvements.
description: |
  This enhancement instructs agents to monitor `change_queue`, `change_queue_logs`, `file_modifications`, and session tables to find failure patterns (apply hits, DNS issues, stale sessions), then propose improvements. Observations are recorded in `execution/verification_report.json` with recommended human actions. The skill keeps a small backlog of improvement tasks (docs updates, telemetry gaps) tracked via `execution/task_status_summary.json`.
instructions:
  - Regularly call `queue.list(QueueStatus::Failed)` and `queue.recent_logs(50)` to summarize repeated failures; capture errors into the verification report with proposed remediations (e.g., adjust `max_attempts`, fix corrupted patches, or refresh Copilot sessions).
  - Use `queue.recent_file_events(20)` to check that fsnotify/audit logging is catching the files touched by agents; if the File Modifications pane misses expected paths, propose expanding the watcher scope.
  - When the improvement backlog grows, suggest human-reviewed change requests that update `WORKPLAN.md`, README guidance, or `AGENTS.md` to document repeated manual steps (session bootstrap, clippy reruns).
  - Annotate each report entry with metadata (`source`, `severity`, `recommendation`) and store it via `execution/verification_report.json` plus `execution/task_status_summary.json` for automation to see next actions.
assets: []
references: []
keywords: [monitoring, quality, audit]
