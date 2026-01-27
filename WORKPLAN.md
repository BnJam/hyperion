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
- No rewrites that replace the in-repo SQLite-backed queue with an external database.
- No UX overhaul that adds a separate GUI beyond the existing CLI/TUI surfaces.
- No autonomous command execution not explicitly enumerated in the Commands section.

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
 - Ensure `hyperion request` is headless: it should process or fail fast and report enqueued change requests without launching the TUI.
 - - Wire the Copilot agent harness (with a JSON contract) into `hyperion request`, falling back to deterministic stubs when the model output cannot be parsed, and allow the harness to be opt-in (e.g., `HYPERION_AGENT=copilot`).
 - [x] Captured agent context persistence by storing `Copilot` session metadata (resume hash, allow-all flag) in SQLite and reusing it via `hyperion session init` / `session list`.
 - [x] Added CLI bootstrapping (`hyperion session init --resume=<token>`) so ongoing runs can reuse pre-provisioned sessions instead of relying solely on per-request prompts.
- [x] Added `testapp/orchestrate-request-003.json` to exercise the deterministic request path and to document the latest expectations for filesystem patches, fsnotify audits, and queue telemetry.

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
- Provide a deterministic/stubbed change-application path so the queue can be exercised without mutating source files during this integration phase.
- Execute queued change requests by invoking `git apply` on each patch and let up to three worker threads process the queue in parallel for merge-ready throughput.
- Migrate the worker patch step from `git apply` to direct filesystem writes and apply each change in parallel for faster throughput.
- [x] Switched the worker `apply_change_request` pipeline to `diffy`-aware parallel filesystem writes, eliminating the `git apply` dependency and allowing deterministic patching via `write_modification`.
- [x] Persisted worker telemetry (change_queue_logs) and fsnotify events (new `file_modifications` table) as JSON so the TUI can surface structured history without polluting the terminal.
- [x] Explored how to expand the orchestrator/runtime lifetime so agents/workers persist for the duration of the binary (system lifecycle), backing state in the queue/DB and surfacing this persistence story through the CLI/TUI experience.
- [x] Captured the runtime persistence story by wiring session data into SQLite and surfacing it in the CLI/TUI so the system lifecycle (binary execution) is the only persistence boundary.

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
- Persist worker telemetry, events, and failure details in `change_queue_logs` (JSON) so the TUI/history pane can read structured audit trails without scraping stdout, and route console tracing output to `sink` unless `HYPERION_LOG=1`.
- Add a task history pane that surfaces the last ~100 task requests and their statuses so operators can verify ingestion success.
- Capture filesystem modifications via `fsnotify`, record modified file paths into SQLite logs, and surface them in the TUI so historical audits can rely on the same data.
- [x] Added TUI controls/documentation that explain how to bootstrap and re-use Copilot sessions (including `--resume=<sessionhash>` plus `--allow-all-tools`) once the initial handshake is provisioned, clarifying that these interactions live only for the duration of the `hyperion` binary execution.
- [x] Added `hyperion export` so operators can seed other directories with the Hyperion skill catalog, export guide, and guidance on reusing the runtime/session persistence layer.
- Documented the new export command’s overwrite prompt/`--overwrite` flag so operators can refresh an existing export bundle without losing data inadvertently.
- [x] Reworked the ratatui dashboard into a multi-pane console that surfaces queue insights, up-to-100 task history entries, worker logs, and file modification audits sourced from the SQLite store so the screen stays clear of verbatim trace output.

**Deliverables**
- Metrics dashboards
- Incident response checklist
- Hardening & resiliency guide

### Phase 7: Queue Performance & Resilience
- [x] Replace the single `Mutex<Connection>` in `SqliteQueue` with per-worker (or pooled) connections, keep prepared statements warm, and limit lock spans so multiple workers can pull/dequeue/apply in parallel without contention.
- [x] Harden the dequeue/update sequence with an explicit `BEGIN IMMEDIATE`, a selective lease filter on `(status, leased_until)` plus a composite index covering `(status, leased_until, id)`, and a `lease_owner` audit column to avoid lost leases when threads overlap.
- [x] Surface dequeue latency, apply duration, and poll interval metrics in `change_queue_logs` and `file_modifications` so the TUI/dashboard can track throughput trends without external tracing.
- [x] Add TTL/archival controls for `change_queue` rows and dead-letter entries as part of workload-scaling experiments, balancing faster reads for active work with bounded history retention.

**Deliverables**
- Queue telemetry report (latency, contention, throughput) with new indexes/procedures documented.
- Updated queue schema migration summary covering lease owner, indexes, TTL strategy, and connection usage.
- Performance regression checks (benchmarks or smoke tests) that run `cargo test --workspace` after schema upgrades.

### Phase 8: CLI & TUI Experience
- [x] Add JSON/flags to CLI listing commands (`List`, `ListDeadLetters`, `History`, `Worker Logs`) so automation pipelines can consume structured records, while `--since`/`--limit` filters support narrower views (referencing `src/main.rs`’ simple prints).
- Teach `hyperion run`/`worker` to emit periodic progress status (applied/failed counts and dequeue deltas) so operators know whether the queue is in steady state before opening the TUI.
- Expand `run_dashboard_with_config` to allow interactive filtering (status/agent), adjustable refresh rates, a detail pane for the selected queue record/log entry, and optional event-sourcing toggles so operators can digest the new metrics without repaint noise.
- [x] Add CLI/TUI guidance that explains the new filtering controls and the expected throughput ranges, surfacing the `worker_count` and new metrics streamed from the queue logs to highlight the “live queue” status.

**Deliverables**
- CLI reference note describing `--format json`, `--since`, and progress output for the core queue commands.
- TUI help overlay or panel describing filters/refresh controls and showing current metrics (queue depth, apply latency, worker count).
- UX checklist covering key flows (request enqueue, worker progress, merge queue status, audit logs).


### Phase 9: Validation, Diagnostics & Testing
- [x] Strengthen `validator::validate_change_request` so it checks patch-target alignment, ensures `operation` matches the patch contents, and enforces per-change hashes/signatures before enqueueing to avoid drift (`src/validator.rs` currently only checks presence of fields).
- [x] Harden `apply::apply_change_request` to surface deterministic failure reasons (e.g., invalid patch format, permission errors) instead of silently writing the literal patch text when `diffy::apply` fails, and capture stdout/stderr into the queue log records for postmortem clarity.
- [x] Add unit/integration tests around `SqliteQueue` lease/mark transitions, worker retries, and CLI/TUI progress hooks plus `cargo fmt/clippy/test` automation to prove performance and safety improvements survive regression.
- [x] Build a diagnostics command (`hyperion doctor` or `hyperion inspect-queue`) that validates schema migrations, connection health, and WAL archival state before workers start, reusing the `SqliteQueue` APIs to check indexes/TTL columns added in earlier phases.
- [x] Document the new validation rules/checks in `SCHEMAS.md`, HARDENING.md, and README.md so contributors know what must pass before the queue applies changes.

**Deliverables**
- Improved validator module with extended patch/schema checks and signature hooks.
- Apply pipeline failure report format plus stored stdout/stderr for queue log auditing.
- Test matrix note (unit + integration + CLI) outlining commands to run after each schema change, plus new diagnostics command documentation.

### Phase 10: Performance, Metrics & UX Refinement
- [x] Add queue-side instrumentation that snapshots throughput/latency/contended-lease metrics, caches the structured telemetry, and surfaces it via CLI/TUI (flags, overlay, or dedicated command).
- [x] Teach `hyperion run`/`worker` to emit periodic progress summaries (applied/failed counts, lease deltas, queue depth) so headless runs report steady-state health before the TUI launches.
- [x] Extend the CLI/TUI guidance to describe refresh/filter controls, throughput expectations, and live worker counts/latency trends so operators have consistent UX cues when consuming the metrics.
- [x] Capture the new telemetry and progress behavior in README/HARDENING/SCHEMAS so downstream contributors know how to rerun or interpret the signals.
**Deliverables**
- [x] Queue telemetry API plus a CLI hook (e.g., `hyperion queue-metrics --format json`) exposing throughput, apply latency, and lease contention.
- [x] Periodic progress output for `hyperion run`/`worker` commands that reports counts and queue depth in a predictable, machine-readable format.
- [x] TUI guidance overlay/panel describing filters, refresh rates, metrics, and queue-health expectations.
- [x] Documentation updates in README/HARDENING/SCHEMAS describing the new telemetry and UX behavior.

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
Approved on: 2026-01-26
