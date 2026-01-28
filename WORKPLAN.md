# WORKPLAN — workplan skill

## Plan Metadata
Approval pattern: ^Approved:[[:space:]]+yes$
Required sections: Intent,Goals,Non-Goals,Scope,Constraints,Plan,Commands,Validation,Approval
Validation policy: Guard scripts + deterministic cast/messaging audits
Plan Source: workplan
Plan Definition: plans/phase_plan.json
Phase progress file: execution/phase_progress.json

## Intent
Design and verify a deterministic cast-based messaging protocol that merges TechnoCore's resilient queue, WAL telemetry, and Doctor diagnostics with Farcaster's issue->cast communication so Hyperion can audit every task, approval, and merge attempt.

## Goals
- Document the TechnoCore queue resilience story plus Farcaster's cast workflow so Hyperion can borrow proven telemetry, packet tracing, and human-in-the-loop gating.
- Define a verifiable JSON cast schema, handshake, and audit tokens that capture task IDs, payload digests, approvals, and WAL anchors.
- Deliver phased work that imprints the cast protocol into Hyperion's change_queue, CLI/TUI messaging surfaces, and operator-facing docs so every cast remains deterministic and replayable.

## Non-Goals
- Introduce new database backends beyond SQLite/WAL.
- Automate merges without explicit human approval or documented guard checks.
- Replace the existing CLI/TUI surfaces with a separate GUI.

## Scope
- Hyperion's queue, change request schema, cast messaging protocol, telemetry, and CLI/TUI orchestration agents.
- Cross-repo learnings from technocore (queue, Doctor, RATATUI diagnostics) and farcaster (issue -> cast -> merge buffer, Jules auto-fix).

## Constraints
- Every cast must be scoped to a deterministic JSON payload that includes task_id, agent_id, approvals, and WAL audit anchors stored in change_queue.
- Guard scripts (cargo fmt/clippy/test stack) must run before any merge or cast approval, and their outputs are recorded with each cast message.
- Human approvals, logging, and telemetry remain visible via the CLI/TUI so operators trust the protocol and no cast bypasses observable channels.

## Plan
### Phase 0 — Cast Foundations & Inspirations
Ground Hyperion's cast work structure in TechnoCore's resilient queue story and Farcaster's issue-to-cast communication.
- [ ] T001: Harvest TechnoCore queue, WAL, and Doctor diagnostics to frame the cast work guardrails.
  Review technocore/README.md plus the Rust queue/agent crates to understand per-
  worker leases, WAL telemetry, RATATUI/Doctor diagnostics, and session
  persistence before encoding them as cast checks.
  Commands:
  - cat technocore/README.md
  - rg -n "SqliteQueue" technocore/crates -g '*.rs'
  Verification:
  - rg -n "TechnoCore queue" hyperion/references/ASI_FRAMEWORK.md
  - rg -n "cast" hyperion/references/ASI_FRAMEWORK.md
- [ ] T002: Decode Farcaster's cast workflow and merge queue guardrails.
  Read farcaster/README.md and the workflows under .github/workflows to catalog
  how issues become casts, how Jules intervenes, and how CI/merge queues stay
  synchronous.
  Commands:
  - cat farcaster/README.md
  - ls farcaster/.github/workflows
  Verification:
  - rg -n "Farcaster cast" hyperion/references/ASI_FRAMEWORK.md

### Phase 1 — Cast Protocol Design
Define the deterministic JSON schema and messaging steps that make each cast auditable before it enters the queue.
- [ ] T101: Compose the cast payload schema and handshake narrative.
  Draft a JSON schema that captures cast headers (task_id, agent_id,
  payload_version, ttl, checksum), metadata (origin, approvals, telemetry
  anchors), and the handshake that ties submissions to WAL audit entries.
  Commands:
  - cat hyperion/SCHEMAS.md
  - rg -n "cast" hyperion/src
  Verification:
  - rg -n "cast schema" hyperion/HYPERION.md
- [ ] T102: Document the deterministic messaging stages for cast lifecycle events.
  Outline the stage transitions (submitted -> cast -> guard-suite -> apply ->
  archive) and WAL entries so operators can replay each message from submission
  through approval and delivery.
  Commands:
  - rg -n "change_queue" hyperion/src
  - cat technocore/README.md
  Verification:
  - rg -n "cast protocol" hyperion/HYPERION.md

### Phase 2 — Queue & Messaging Integration
Bring the cast protocol alive inside Hyperion's queue, WAL, and telemetry surfaces so every message is observable and deterministic.
- [ ] T201: Apply the cast schema to Hyperion's change_queue writer and WAL logging.
  Update the queue writer to enforce the cast schema, log the JSON payload,
  approvals, guard commands, and emit telemetry entries that mention the
  originating cast message.
  Commands:
  - rg -n "persist" hyperion/src
  - rg -n "change_queue" hyperion/src
  Verification:
  - rg -n "cast queue" hyperion/HYPERION.md
- [ ] T202: Surface cast telemetry inside the CLI/TUI guard panels.
  Plan RATATUI panes that display queue depth, cast latency, WAL progress, guard
  command results, and human approval comments so operators never lose sight of
  the messaging health.
  Commands:
  - rg -n "ratatui" hyperion/src
  - cat hyperion/HARDENING.md
  Verification:
  - rg -n "telemetry" hyperion/HYPERION.md

### Phase 3 — Governance, Testing & Delivery
Document the cast work structure, add deterministic guard tests, and highlight unresolved decisions so Hyperion operators trust the protocol.
- [ ] T301: Capture the cast messaging story inside Hyperion docs and references.
  Update README.md, HYPERION.md, and references/QUESTIONS.md to describe the cast
  work structure, guard expectations, telemetry knobs, and where human judgment is
  still required.
  Commands:
  - cat hyperion/README.md
  - cat hyperion/HYPERION.md
  Verification:
  - rg -n "cast work" hyperion/HYPERION.md
  - rg -n "deterministic" hyperion/references/QUESTIONS.md
- [ ] T302: Define verifiable tests and guard scripts that keep casts deterministic.
  Plan JSON fixtures, WAL audits, and CI guard checks that fetch cast logs, replay
  their states, and confirm audit fields before each merge slot is advanced.
  Commands:
  - cat hyperion/TASKS.md
  - rg -n "cast" hyperion/src
  Verification:
  - rg -n "cast" hyperion/references/QUESTIONS.md

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
