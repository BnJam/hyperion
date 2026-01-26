# Multi-Agent Orchestration System

## Overview
This project defines a governance and execution system for multi-agent software delivery, implemented in Rust. The system accepts human task requests, routes them through an Engineer agent for clarification and scoping, then uses an Orchestrator to decompose work into isolated tasks for Developer agents. Developer outputs are submitted as structured change requests to a Merge Queue/Buffer that applies edits in parallel where safe and tracks outcomes.

## Governance Structure
1. **Human** submits task requests and provides approvals.
2. **Engineer** clarifies intent, defines acceptance criteria, and delegates.
3. **Orchestrator** decomposes tasks and assigns scoped work to Developers.
4. **Developer Agents** implement isolated changes and produce JSON change requests.
5. **Merge Queue/Buffer** validates, applies, and verifies changes at scale.

## Core Goals
- **Safety:** automated validation before applying changes.
- **Parallelism:** maximize throughput without conflicts.
- **Cost control:** Developer agents use smaller models with tight scopes.
- **Auditability:** every change is tied to a task ID and recorded in logs.

## Key Components
- **Task Intake:** Receives human requests and context.
- **Engineer Agent:** Clarifies and prioritizes tasks.
- **Orchestrator:** Splits tasks and enforces isolation rules.
- **Developer Agents:** Implement targeted changes.
- **Merge Queue/Buffer:** Applies changes via structured JSON patches and a parallel worker pool that invokes `git apply`.
- **Agent Harness:** Copilot CLI (model `gpt-5-mini`) behind a trait for easy swapping.
- **Queue Storage:** SQLite with WAL enabled for durability and concurrent writers.
- **Queue Logs:** Worker events are persisted as JSON in `change_queue_logs` so audit trails remain centralized.
- **Schema Catalog:** Documented JSON schemas in `SCHEMAS.md`.

## Change Application Model
Developer agents do not directly edit the repository. Instead they submit **JSON change requests** with:
- Target files and operations (add/update/delete)
- Patch fragments or replacements
- Validation steps and expected outcomes

The Merge Queue/Buffer:
- Validates JSON schema
- Applies compatible patches in parallel threads
- Detects conflicts and queues for manual review
- Runs tests and records results
- Uses file system notifications (fsnotify/`notify`) to ingest new requests quickly
- Leases dequeued work to allow retries on worker failure

## CLI Highlights
- Launch the integrated runtime: `cargo run` (default) or `cargo run -- run` starts the TUI dashboard plus worker pool for live monitoring.
- Enqueue a task request headlessly: `cargo run -- request path/to/request.json` (prints how many change requests were enqueued and does not open the TUI).
  - To use the real Copilot harness instead of deterministic stubs, set `HYPERION_AGENT=copilot` before invoking `hyperion request`.
- Validate change requests: `cargo run -- validate-change path/to/change.json`
- Apply a change request with checks: `cargo run -- apply path/to/change.json --run-checks`
- Operate the queue: `cargo run -- worker --run-checks --max-attempts 5`, `cargo run -- list-dead-letters`, `cargo run -- mark-applied <id>`
- Export the Hyperion skill bundle to another workspace: `cargo run -- export --dest /path/to/target` (writes the `skills/` catalog, `assets/templates/EXPORT_GUIDE.template.md`, and generates an `EXPORT_GUIDE.md` describing how to initialize `hyperion session init`, submit requests, and view the TUI).
  Add `--overwrite` to force replacing an existing export, or rerun without the flag to receive a prompt before overwriting the target directory’s `skills/` catalog.
 The TUI now shows a multi-pane view with queue stats, runtime telemetry, guidance, and the last 100 task requests, plus a Worker Logs panel that reads structured JSON events from SQLite so you can trace dequeue/validation/apply activity without flooding the terminal output (console logging remains suppressed unless `HYPERION_LOG=1`).

## Example JSON Change Request (Sketch)
```json
{
  "task_id": "ENG-214",
  "agent": "developer-3",
  "changes": [
    {
      "path": "src/module/file.ts",
      "operation": "update",
      "patch": "@@ -10,7 +10,8 @@\n- old\n+ new"
    }
  ],
  "checks": [
    "npm test",
    "npm run lint"
  ]
}
```

## Next Steps
- Review the detailed work plan in `WORKPLAN.md`.
- Track milestones and dependencies in `ROADMAP.md`.
- Review hardening guidance in `HARDENING.md`.
- Review JSON schemas in `SCHEMAS.md`.
- Run the Rust queue CLI (`cargo run -- init`) and load sample change requests.
- Exercise the agent harness (`cargo run -- agent \"Summarize this task\"`).

## Contributing
Contributions should align with the orchestration model and keep tasks scoped, isolated, and testable. For design changes, include a rationale and validation steps.
