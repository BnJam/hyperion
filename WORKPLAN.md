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
