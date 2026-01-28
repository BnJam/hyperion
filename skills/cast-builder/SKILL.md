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
2. Follow the prompts to provide the request ID, summary, intent, complexity rating (1–10), telemetry anchors, approvals, and requested-change details.
3. Each cast writes a deterministic JSON payload in `taskjson/<REQUEST_ID>.json` and updates `execution/next_task_context.json` so the TUI can surface the export status; every payload mirrors the assignment metadata (intent, complexity, sample diff snippet, telemetry anchors, approvals, agent_model) noted inside the orchestrator.
4. The harness passes `model` metadata inside each request, preferring `gpt-5-mini` and falling back to `gpt-4.1` for low-complexity casts (≤3) while still capturing approval latency and guard outputs.
5. Once satisfied, set `HYPERION_AGENT=copilot` and run `cargo run -- request taskjson/<REQUEST_ID>.json` to enqueue the approved cast; this ensures the exported skill, REPL, and guard telemetry stay in sync for downstream Copilot agents.
