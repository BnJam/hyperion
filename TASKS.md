# Task Backlog: System Improvements

These tasks are derived from a codebase review focused on resiliency, safety, performance, and operational robustness.

## Queue + Storage Reliability
- Make dequeue atomic with `BEGIN IMMEDIATE` or a single UPDATE ... RETURNING to prevent double-lease races.
- Add a `lease_owner` column (worker ID) to support lease extension and traceability.
- Add lease extension/heartbeat support for long-running apply/check steps.
- Enforce a max-lease and auto-requeue sweep for stalled in-progress items.
- Add a `created_at` and `updated_at` trigger instead of manual updates to avoid drift.
- Add a queue-level dedupe key (task_id + agent + payload hash) to prevent duplicate enqueues.
- Add a retention policy for applied records and dead letters (TTL + archival).
- Add explicit dead-letter states for validation failures vs apply/check failures.
- Add a command to requeue dead letters with reason tracking.
- Add transactional mark-applied/failed that validates current status + lease owner.

## Change Safety & Validation
- Validate that `changes[*].path` matches the file paths touched by the patch.
- Reject patches that touch files outside the declared `file_targets`.
- Block path traversal in patch paths (`..`, absolute paths, symlinks).
- Validate patch format (unified diff) and minimum context to reduce apply drift.
- Validate `operation` matches the patch semantics (add/delete/update).
- Add schema validation against JSON Schemas in `SCHEMAS.md` before enqueue/apply.
- Add strict validation for `checks` (allowlist or policy-based runner).
- Enforce max patch size and max number of files per request.
- Require per-change content hash for integrity and audit.
- Add optional signature verification for change requests (agent auth).

## Apply Pipeline Hardening
- Run `git apply --check` before applying and report a structured failure reason.
- Add a dry-run mode for `apply` and `worker` to support safety previews.
- Add a clean-worktree guard (refuse to apply if git status is dirty).
- Support `--3way` apply fallback and emit a conflict report artifact.
- Add per-change timeout and cancellation handling.
- Capture and persist stdout/stderr from apply and checks in the queue record.
- Add a rollback command that reverts the last applied patch batch.
- Ensure `apply` honors `operation` when generating/validating patches.

## Worker Robustness
- Add jittered backoff between failed retries.
- Add a watchdog for long-running checks with configurable timeouts.
- Support multiple worker instances with unique IDs and lease ownership.
- Add a "drain" mode to finish in-flight items before shutdown.
- Record worker version/build info in applied records for audit.
- Add structured error codes to distinguish failure classes.

## Orchestrator Improvements
- Split assignments by file ownership boundaries and repo domains.
- Add max token/context sizing per assignment with automatic split.
- Add instruction templates based on change type (feature/bugfix/refactor/docs).
- Add dependency detection between assignments to sequence safely.
- Add task priority and SLA metadata in assignments.

## Agent Harness Safety
- Capture and store full agent responses with timestamps for audit.
- Add prompt redaction rules to prevent secret leakage.
- Add rate limiting and request budgeting to prevent runaway use.
- Add retries with exponential backoff for transient CLI failures.
- Add support for multiple harness providers with interface tests.

## Observability & Diagnostics
- Emit structured logs for enqueue/dequeue/apply/check lifecycle events.
- Add metrics counters for throughput, failures, retries, and latency.
- Add tracing spans across worker steps with task_id correlation.
- Add a `hyperion doctor` command to run diagnostics (DB health, schema, queue stats).
- Add a "queue inspect" command for deep per-record debugging.

## Security & Governance
- Add access control for enqueue/apply/worker commands (token or OS user policy).
- Add approval gates by risk tier (e.g., file path or command patterns).
- Add an audit log table that records state transitions with actor metadata.
- Add configurable policy rules (YAML/JSON) for validation and safety checks.
- Add secrets scanning on patches before apply.

## Performance & Scalability
- Add batched dequeue and apply for throughput testing.
- Add configurable batch size for worker processing.
- Add indexing for hot query paths (status+leased_until composite).
- Add optional pooling for SQLite connections to support multi-threaded mode.
- Add a benchmark suite for enqueue/dequeue/apply/check workloads.

## UX / CLI
- Add `--format json` output for list/dequeue commands.
- Add `--since`/`--limit` filtering for list and dead-letter commands.
- Add `hyperion init` to create a default config + db path.
- Add `hyperion config` to validate and print effective settings.
- Add progress output for long-running apply/check steps.

## TUI Enhancements
- Add live stream of recent events/errors.
- Add filtering by status and agent.
- Add a detail pane to inspect a record's payload and errors.
- Add a refresh rate setting and a non-blocking data poller.

## Testing & CI
- Add unit tests for queue lease behavior and retry thresholds.
- Add integration tests for enqueue → worker → apply → mark_applied.
- Add tests for watch ingestion and invalid JSON handling.
- Add tests for validation of patches vs declared paths.
- Add a CI job that runs `cargo test --workspace` and `cargo clippy`.

## Documentation & Samples
- Add sample JSON files for task requests and change requests.
- Add a "failure handling" guide with sample dead letters.
- Add a config reference (DB path, lease, retry policy, timeouts).
- Add a security model doc (trust boundaries, auth, validation).

