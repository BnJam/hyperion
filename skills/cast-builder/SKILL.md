name: cast-builder
license: MIT
description: >
  Interactive REPL that lets you capture human intent, approvals, and metadata before exporting a cast JSON for Copilot agents.
metadata:
  references:
    - README.md
    - HYPERION.md
scripts:
  - scripts/cast_builder.sh
artifacts:
  - taskjson/
  - execution/next_task_context.json
keywords:
  - cast
  - builder
  - skill
---
## Instructions

1. Run `scripts/cast_builder.sh` (optionally pass a destination path) to open the REPL.
2. Follow the prompts to provide request ID, summary, intent, complexity, telemetry anchors, approvals, and requested-change details.
3. The command writes `taskjson/<REQUEST_ID>.json` and updates `execution/next_task_context.json` so the TUI can surface the export status.
4. Once satisfied, run `HYPERION_AGENT=copilot cargo run -- request taskjson/<REQUEST_ID>.json` to enqueue the deterministic payload.
