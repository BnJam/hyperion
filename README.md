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
- **Merge Queue/Buffer:** Applies changes via structured JSON patches.
- **Queue Storage:** SQLite with WAL enabled for durability and concurrent writers.
- **Agent Harness:** GitHub Copilot CLI (model `gpt-5-mini`) behind a trait for easy swapping.
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
- Validate a change request: `cargo run -- validate-change path/to/change.json`
- Orchestrate a task request: `cargo run -- orchestrate path/to/request.json --out assignments.json`
- Apply a change request: `cargo run -- apply path/to/change.json --run-checks`

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
