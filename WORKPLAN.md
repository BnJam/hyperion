# WORKPLAN — Multi-Agent Orchestration System

## Plan Metadata
Approval pattern: ^Approved:[[:space:]]+yes$
Required sections: Intent, Goals, Non-Goals, Scope, Constraints, Plan, Commands, Validation, Approval
Validation policy: cargo fmt --check; cargo clippy --workspace --all-targets --all-features; cargo test --workspace

## Intent
Establish a clear, staged plan to design and implement a multi-agent orchestration system that accepts human task requests, uses an Engineer agent to clarify scope and delegate work, uses an Orchestrator to decompose tasks and assign to Developer agents, and employs a Merge Queue/Buffer to apply changes safely and at scale.

## Goals
- Accepts human task requests.
- Uses an Engineer agent to clarify scope and delegate work.
- Uses an Orchestrator to decompose tasks and assign to Developer agents.
- Employs a Merge Queue/Buffer to apply changes safely and at scale.

## Non-Goals
- Not specified in the source plan.

## Scope
- Multi-agent orchestration system for this repo (Engineer, Orchestrator, Developer, Merge Queue).

## Constraints
- **Isolation:** Developer tasks must be small, scoped, and independent.
- **Traceability:** Every change request is tied to a task ID and JSON patch payload.
- **Parallelism with Safety:** Enable concurrent changes without merge conflicts.
- **Human-in-the-loop:** Preserve human visibility and approval gates.

## Plan

### Approved Tasks
- [x] cargo fmt --check
- [x] cargo clippy --workspace --all-targets --all-features
- [x] cargo test --workspace
- [x] cargo build
- [x] cargo run
- [ ] Enable TUI by default in the build (default features include `tui`) and document the default.
- [ ] Fix `hyperion agent` to use the `copilot` CLI reliably and avoid exit status 1 on normal prompts.
- [ ] Add a lightweight smoke check for `hyperion agent` execution flow (mocked or guarded) to prevent regressions.

### Phase 0: Discovery & Requirements
- Collect user stories for request intake, delegation, and merge workflow.
- Define task classification (feature, bugfix, refactor, docs).
- Identify constraints: repo size, CI runtime, security concerns.

**Deliverables**
- Requirements brief
- System glossary

### Phase 1: Architecture & Interfaces
- Define core agents and responsibilities:
  - Human → Engineer → Orchestrator → Developers → Merge Queue
- Define standard JSON task schema for Developer assignments.
- Define JSON change request schema for Developer submissions.
- Define Queue/Buffer workflow, SQLite WAL storage, and conflict detection strategy.
- Establish agent harness trait with GitHub Copilot CLI implementation.

**Deliverables**
- Architecture diagram
- API schema draft
- Merge workflow spec
- Storage design (SQLite WAL + log shipping)
- Agent harness contract and CLI integration

### Phase 2: Orchestrator Task Decomposition
- Implement task splitter for work items.
- Ensure tasks are independent (file ownership, boundaries).
- Add constraints for model capacity (token and context limits).
- Add file-system notification integration for fast feedback loops.

**Deliverables**
- Task decomposition rules
- Orchestrator policy configuration

### Phase 3: Developer Execution & Change Submission
- Integrate Developer agents with scoped context.
- Validate output against JSON change request schema.
- Enforce lint/test execution instructions per task.
- Provide a deterministic `hyperion request <file>` path that ingests `TaskRequest` JSON, turns it into queued change requests, and keeps the TUI updated without hanging.

**Deliverables**
- Developer agent spec
- Change request validator

### Phase 4: Merge Queue/Buffer
- Implement queue that accepts change requests.
- Apply patches in parallel threads where possible.
- Detect and resolve conflicts, quarantine failures.
- Support auto-rebase/retry strategy.
- Maintain a WAL-backed audit trail for replay and rollback.
- Add lease-based dequeue with retry counters and error capture.
- Add worker loop to process queue entries with validation and checks.

**Deliverables**
- Merge Queue MVP
- Conflict resolution policy
- Audit log format

### Phase 5: Human Oversight & Governance
- Approval gates for high-risk changes.
- Provide summaries of applied changes and tests.
- Reporting and rollback procedures.

**Deliverables**
- Review workflow
- Governance handbook

### Phase 6: Hardening & Observability
- Metrics: throughput, conflicts, success rate, latency.
- Logging and alerts for failed patches.
- Continuous improvements from postmortems.
- TUI dashboard (ratatui) for live queue status and health indicators.
- Expand the dashboard to multi-pane views including runtime insights and actionable guidance so operators can track workers, agents, and queue entries at a glance.
- Publish hardening and resiliency checklist.

**Deliverables**
- Metrics dashboards
- Incident response checklist
- Hardening & resiliency guide

## Commands
cargo fmt --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
cargo build
cargo run

## Validation
cargo fmt --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace

## Approval
Approved: yes
Approved by: not specified
Approved on:
