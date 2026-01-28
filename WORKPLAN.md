# WORKPLAN — workplan skill

## Plan Metadata
Approval pattern: ^Approved:[[:space:]]+yes$
Required sections: Intent,Goals,Non-Goals,Scope,Constraints,Plan,Commands,Validation,Approval
Validation policy: Guard scripts + deterministic cast/messaging audits
Plan Source: workplan
Plan Definition: plans/phase_plan.json
Phase progress file: execution/phase_progress.json

## Intent
Advance Hyperion's deterministic cast protocol by hardening queue telemetry, formalizing payload validation, bridging issue-to-merge flows, and documenting the RATATUI observability/governance story.

## Goals
- Instrument SqliteQueue leases, WAL telemetry, and Doctor diagnostics so cast health is visible and self-healing.
- Define a CastPayload schema plus validation hooks so every request includes approvals, TTLs, and guard outputs before hitting the queue.
- Build the issue-to-merge bridge, RATATUI telemetry dashboards, and governance docs so operators trust and audit every cast lifecycle.
- Integrate a dedicated AI agent harness that generates TaskRequest/ChangeRequest payloads and automates submission through the orchestrator while respecting guard suites.

## Non-Goals
- Add new database backends beyond SQLite/WAL.
- Bypass human approvals for merge slots.
- Introduce a separate GUI beyond the existing CLI/TUI surfaces.

## Scope
- Hyperion's queue wiring, change request schema, issue bridge, telemetry/doctor guard rails, and CLI/TUI surfaces.
- Cross-repo learnings from technocore queue resiliency plus Farcaster's cast/merge documentation and guards.

## Constraints
- Every cast must map to a deterministic JSON payload whose audit tokens, WAL anchors, and guard outputs are stored before enqueueing.
- Guard suites (cargo fmt/clippy/test stack plus the documented checks) must run before any merge or cast approval is recorded.
- Operators must continue seeing human approvals, telemetry, and logs inside the CLI/TUI so no cast bypasses observability.

## Plan
### Phase 0 — Queue Telemetry & Lease Hardening
Make leases, TTLs, dedup windows, and Doctor telemetry actionable so casts never linger unnoticed.
- [ ] T001: Audit SqliteQueue leases, dedupe, and Doctor instrumentation.
  Review queue/doctor sources, codify TTL/lease recovery rules, and add WAL audit
  points so lease histories stay observable.
  Commands:
  - rg -n "leased_until" src/queue.rs
  - rg -n "DEFAULT_DEDUP_WINDOW" hyperion/src/doctor.rs
  - rg -n "QueueMetrics" hyperion/src/models.rs
  Verification:
  - cargo fmt --check
  - rg -n "lease" hyperion/HYPERION.md
- [ ] T002: Add a lease reclamation watchdog tied to WAL logs.
  Implement or document a watcher that scans change_queue for expired leases,
  reclaims them, and logs the events as ChangeQueueLog entries for easier
  triaging.
  Commands:
  - rg -n "watcher" src
  - cat hyperion/src/watcher.rs
  Verification:
  - rg -n "lease reclaim" hyperion/execution/command_logs/command_*

### Phase 1 — Cast Schema & Validation
Ensure every cast payload carries structured headers, approvals, and guard outputs before the queue touches it.
- [ ] T101: Define the CastPayload schema and propagate it through models/requests.
  Add a typed CastPayload (task_id, agent_id, approvals, TTL, telemetry anchors,
  guard command outputs) in models.rs and ensure request types/serializers reflect
  it.
  Commands:
  - cat hyperion/src/models.rs
  - cat hyperion/src/request.rs
  Verification:
  - rg -n "CastPayload" hyperion/src
  - rg -n "Cast" hyperion/SCHEMAS.md
- [ ] T102: Enforce payload version/TTL/checksum during validation/enqueue.
  Extend validator.rs (or doctor.rs) to assert payload version, TTL, and checksum,
  then call those checks inside SqliteQueue::enqueue so only audited payloads
  enter the queue.
  Commands:
  - rg -n "validator" hyperion/src
  - rg -n "enqueue" hyperion/src/queue.rs
  Verification:
  - cargo test --workspace
  - rg -n "validator payload" hyperion/execution/command_logs/command_*

### Phase 2 — Issue → Merge Cast Flow
Ingest Farcaster-style casts, track guard outputs, and buffer them in a merge queue that waits for approvals + CI.
- [ ] T201: Build the issue bridge that enqueues casts with guard metadata.
  Wire orchestrator.rs/runner.rs to process Farcaster-style casts, record guard
  commands/approvals, and enqueue them while preserving the metadata.
  Commands:
  - cat hyperion/src/orchestrator.rs
  - rg -n "issue" hyperion/src/runner.rs
  Verification:
  - rg -n "merge queue" hyperion/src
  - rg -n "issue bridge" hyperion/HARDENING.md
- [ ] T202: Implement a merge queue buffer + release stub tied to guard suites.
  Add a buffer in exporter.rs/runner.rs that holds casts until cargo
  fmt/clippy/test pass plus human approval, logging CI outputs inside
  ChangeQueueLog for auditability.
  Commands:
  - rg -n "export" hyperion/src
  - rg -n "cargo fmt" hyperion/execution/commands_from_plan.txt
  Verification:
  - rg -n "guard run" hyperion/execution/exit_codes.json

### Phase 3 — RATATUI Visibility & Observability
Surface queue/guard telemetry inside the TUI and emit structured reports for dashboards.
- [ ] T301: Surface queue metrics + guard history in the RATATUI panels.
  Update tui.rs/fs_watch.rs to display queue depth, cast latency, WAL progress,
  guard command results, approvals, and ChangeQueueLog insights.
  Commands:
  - rg -n "ratatui" hyperion/src
  - cat hyperion/src/tui.rs
  Verification:
  - echo 'TUI snapshot references cast latency, queue depth, guard outcomes'
- [ ] T302: Emit structured telemetry per run for dashboards to trend.
  Write queue depth, WAL checkpoint stats, dedup hits, and guard results into
  execution/verification_report.json on every run so dashboards ingest
  deterministic telemetry.
  Commands:
  - cat hyperion/src/exporter.rs
  - rg -n "verification_report" execution
  Verification:
  - rg -n "queue depth" hyperion/execution/verification_report.json

### Phase 4 — Testing, Governance & Playbooks
Create cast replay fixtures, capture WAL audits, and document governance playbooks for operators.
- [ ] T401: Expand CI/tests with cast replay fixtures and WAL audits.
  Export sample payloads from request/models, build fixtures that replay casts,
  and verify WAL retention plus guard logging during cargo test runs.
  Commands:
  - cat hyperion/src/request.rs
  - cat hyperion/src/models.rs
  Verification:
  - cargo test --workspace
  - rg -n "cast fixture" hyperion/tests
- [ ] T402: Update docs/playbooks to explain the deterministic cast lifecycle.
  Refresh README.md, HYPERION.md, references/ASI_FRAMEWORK.md, and
  references/QUESTIONS.md with the issue bridge story, guard requirements,
  telemetry expectations, and lingering decisions.
  Commands:
  - cat hyperion/README.md
  - rg -n "cast" hyperion/references/QUESTIONS.md
  Verification:
  - rg -n "deterministic" hyperion/references/QUESTIONS.md
  - rg -n "cast work" hyperion/HYPERION.md

### Phase 5 — AI Agent Integration
Introduce an AI agent harness that produces TaskRequest/ChangeRequest payloads and ensures every cast is approved before enqueue.
- [ ] T501: Define agent harness responsibilities and validation hooks.
  Clarify how the AI agent should gather intent, generate TaskRequest payloads,
  validate against the schemas, and capture approval metadata before hitting the
  orchestrator.
  Commands:
  - rg -n "AgentHarness" hyperion/src
  - cat hyperion/src/agent.rs
  Verification:
  - rg -n "agent harness" hyperion/HYPERION.md
- [ ] T502: Wire the agent into the orchestrator/request path.
  Update orchestrator.rs/request.rs to accept AI-produced TaskRequest JSON,
  translate it into ChangeRequests, and submit them via cargo run -- enqueue while
  recording guard outputs.
  Commands:
  - cat hyperion/src/request.rs
  - cat hyperion/src/orchestrator.rs
  Verification:
  - rg -n "TaskRequest" hyperion/src
  - rg -n "change request" hyperion/execution/command_logs/command_*

### Phase 6 — Agent Monitoring & Feedback
Track agent outcomes, approvals, and telemetry so operators trust the automation.
- [ ] T601: Emit telemetry that correlates agent requests with guard runs.
  Push agent-generated metrics (requests/sec, approval latency, guard success
  rate) into execution/verification_report.json and mirror them in the TUI metrics
  panel.
  Commands:
  - rg -n "QueueMetrics" hyperion/src/models.rs
  - cat hyperion/execution/verification_report.json
  Verification:
  - rg -n "agent" hyperion/execution/verification_report.json
- [ ] T602: Document the agent workflow plus outstanding decisions.
  Refresh README.md, HYPERION.md, references/ASI_FRAMEWORK.md, and
  references/QUESTIONS.md with the agent automation story, guard requirements, and
  any unresolved governance questions.
  Commands:
  - cat hyperion/README.md
  - cat hyperion/HYPERION.md
  Verification:
  - rg -n "agent harness" hyperion/references/ASI_FRAMEWORK.md
  - rg -n "approval" hyperion/references/QUESTIONS.md

### Phase 7 — Cast Builder & Agent Export
Add a human-in-the-loop REPL that finalizes casts before Copilot ingestion and ensure exported skills reflect the structured guidance flow.
- [ ] T701: Design the cast-builder REPL/subcommand that negotiates intent + approvals and writes cast JSON for Copilot agents.
  Implement a `hyperion cast` (or exported-skill) REPL that records the
  conversation, collects AssignmentMetadata (intent, complexity, sample diff,
  telemetry anchors, approvals), and writes the approved phase/task JSON into
  `taskjson/` so Copilot agents ingest a deterministic payload.
  Commands:
  - cat src/request.rs
  - rg -n "HYPERION_AGENT" src/request.rs
  - rg -n "CopilotHarness" src/agent.rs
  Verification:
  - rg -n "cast-builder" README.md
  - cat execution/next_task_context.json | rg -n "intent"
- [ ] T702: Ship an exportable skill that orchestrates the REPL and cast submission into Copilot agents.
  Create a `skills/cast-builder` (or similar) directory with a SKILL manifest that
  mimics the workplan skill scaffolding so agentic operators know which scripts to
  run, how progress is tracked, and how to export the cast JSON into the queue.
  Commands:
  - ls skills/workplan
  - cat skills/workplan/SKILL.md
  Verification:
  - rg -n "cast-builder" skills/*/SKILL.md
  - rg -n "agent harness" README.md

### Phase 8 — TUI Visibility for Cast Builder
Show the new cast-builder lifecycle and export skill state inside the TUI so operators know when casts are finalized and exported.
- [ ] T801: Add a TUI panel that highlights the cast-builder export status and next-task context.
  Extend `src/tui.rs` to render the latest cast-builder assignment
  intent/complexity plus the export skill status using the
  `execution/next_task_context.json` data, ensuring operators can see when a cast
  is ready for Copilot agents.
  Commands:
  - rg -n "next_task_context" src/tui.rs
  - cat execution/next_task_context.json
  Verification:
  - Run `cargo run -- queue-metrics --format json` and confirm the new cast-builder fields appear
  - TUI screenshot or log entry referencing cast-builder status
- [ ] T802: Document the TUI changes plus how the exported skill surfaces status in the dashboard.
  Update README.md and HYPERION.md (or a dedicated docs file) so you know which
  TUI panes show cast-builder telemetry, what the statuses mean, and how to
  correlate them with the exported skill/plan tracker.
  Commands:
  - cat README.md
  - rg -n "cast-builder" HYPERION.md
  Verification:
  - rg -n "cast-builder" README.md
  - rg -n "TUI" HYPERION.md

### Phase 9 — Cast Skill Distribution
Package the cast-builder experience as a reusable skill and exporting bundle so other operator agents can reuse the workflow.
- [ ] T901: Document and test the `skills/cast-builder` manifest plus the cast-builder helper.
  Ensure `skills/cast-builder/SKILL.md` references `scripts/cast_builder.sh` and
  explains how to run the REPL so imported skills follow the same approvals +
  metadata flow.
  Commands:
  - ls skills/cast-builder
  - cat skills/cast-builder/SKILL.md
  - cat scripts/cast_builder.sh
  Verification:
  - rg -n "cast-builder" skills/cast-builder/SKILL.md
  - rg -n "Cast Builder" README.md
- [ ] T902: Build an export bundle for the cast-builder skill and documentation.
  Create a tarball or archive containing the skill manifest, helper script, and
  docs so downstream repos can adopt the workflow without guessing which files to
  copy.
  Commands:
  - pwd
  - tar -czf /tmp/cast-builder.tar.gz scripts/cast_builder.sh skills/cast-builder
  Verification:
  - ls /tmp | rg -n cast-builder
  - sha256sum /tmp/cast-builder.tar.gz

### Phase 10 — Guarded Delivery & Governance
Extend the governance story so the new cast-builder workflow, telemetry, and TUI visibility are audit-ready.
- [ ] T1001: Clarify the telemetry contract between the verification report and the new cast builder context.
  Describe what metrics the Cast Builder panel,
  `execution/verification_report.json`, and `execution/next_task_context.json`
  expose so reviewers know how guard runs tie back to exported casts.
  Commands:
  - cat execution/verification_report.json
  - cat execution/next_task_context.json
  - rg -n "cast-builder" README.md
  Verification:
  - rg -n "Cast Builder" hyperion/HYPERION.md
  - rg -n "telemetry" README.md
- [ ] T1002: Document the cast-builder + TUI story inside the governance references.
  Update `references/ASI_FRAMEWORK.md`, `references/QUESTIONS.md`, and
  `HYPERION.md` to include links to the new panels, scripts, and outstanding
  decisions so future reviewers see how the automation is audited.
  Commands:
  - cat references/ASI_FRAMEWORK.md
  - cat references/QUESTIONS.md
  - cat HYPERION.md
  Verification:
  - rg -n "Cast Builder" references/ASI_FRAMEWORK.md
  - rg -n "cast builder" references/QUESTIONS.md

## Commands
- cargo fmt --check
- cargo clippy --workspace --all-targets --all-features
- cargo test --workspace
- cargo build
- cargo run -- --help

## Validation
- ../developer/scripts/validate-plan.sh --plan ./WORKPLAN.md --required "Intent,Goals,Non-Goals,Scope,Constraints,Plan,Commands,Validation,Approval" --approval-pattern "^Approved:[[:space:]]+yes$"
- ../developer/scripts/check-workspace.sh --root . --fail-on-dirty
- ../developer/scripts/check-anti-patterns.sh --plan ./WORKPLAN.md --root .
- ../developer/scripts/librarian-discovery.sh --workspace . --db /Users/bsmith/thelibrary/hyperion-be573d94/librarian.db --plan WORKPLAN.md

## Approval
Approved: yes
Approved by: bsmith
Approved on: 2026-01-28
