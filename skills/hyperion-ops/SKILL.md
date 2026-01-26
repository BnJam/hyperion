name: hyperion-ops
license: MIT
summary: Operationally manage Hyperion sessions, workers, and fsnotify audits through the CLI.
description: |
  Upgrade the ops skill to include explicit runbooks for provisioning Copilot sessions, handling fsnotify events, and controlling worker lifecycles. The skill reinforces CLI command sequences and documents when to escalate to human oversight. Audits captured in `change_queue_logs`/`file_modifications` feed the TUI’s File Modifications pane and can be exported to `execution/verification_report.json` for incident reviews.
instructions:
  - Start with `hyperion session list` to understand available Copilot resumes and advise humans to keep a vault of hashes for reuse; if none exist, run `hyperion session init` once the Copilot session hash is available.
  - Suggest operations like `hyperion session init`, `hyperion request <file>`, `hyperion run --workers=... --agents=...`, and `hyperion Tui` to orchestrate a full cycle without leaving the CLI.
  - When watching directories fails or fsnotify reports too few events, record the discrepancies via `queue.record_file_event` and instruct humans to verify the watcher path or increase its depth.
  - Avoid destructive git commands; if cleanup is required (e.g., editing files manually), prep a `hyperion request` that touches the file instead of running `git clean`.
assets: []
references: []
keywords: [ops, cli, audit]
