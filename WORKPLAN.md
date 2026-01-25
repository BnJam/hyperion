# Work Plan: Multi-Agent Orchestration System

## Purpose
Establish a clear, staged plan to design and implement a multi-agent orchestration system that:
- Accepts human task requests.
- Uses an Engineer agent to clarify scope and delegate work.
- Uses an Orchestrator to decompose tasks and assign to Developer agents.
- Employs a Merge Queue/Buffer to apply changes safely and at scale.

## Guiding Principles
- **Isolation:** Developer tasks must be small, scoped, and independent.
- **Traceability:** Every change request is tied to a task ID and JSON patch payload.
- **Parallelism with Safety:** Enable concurrent changes without merge conflicts.
- **Human-in-the-loop:** Preserve human visibility and approval gates.

## Phases & Milestones

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
- Publish hardening and resiliency checklist.

**Deliverables**
- Metrics dashboards
- Incident response checklist
- Hardening & resiliency guide

## Risks & Mitigations
- **Conflicting edits:** Use file-level ownership and apply order rules.
- **Overly broad tasks:** Enforce max scope and file count limits.
- **Low-quality patches:** Require validation and tests in schema.
- **Hidden dependency changes:** Enforce dependency impact analysis.

## Success Criteria
- 80%+ tasks can be decomposed into independent units.
- Merge Queue applies >95% change requests without manual intervention.
- Clear traceability for every change from human request to applied patch.
