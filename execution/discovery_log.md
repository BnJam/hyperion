# Discovery Log

## Summary

- Goal: Enhance TUI visibility with multi-pane layout and ensure `hyperion request` completes via deterministic change requests.
- Date (UTC): 2026-01-26
- Repo/Path: /Users/bsmith/bnjam/hyperion

## Grep-first Discovery (grepit/grape)

- Query 1: `rg -n "run_dashboard" src/tui.rs`
- Query 2: `rg -n "handle_request" -n src/request.rs`
- Narrowing steps: read `src/tui.rs`, `src/request.rs`, and related modules to understand current UI and request handling.
- Candidate files: `src/main.rs`, `src/request.rs`, `src/tui.rs`, `src/apply.rs`

## Targeted Extraction (code-optimizer)

- No auxiliary extraction scripts needed; edits were focused and limited in scope.

## Full-file Reads (if any)

- Files read: `src/tui.rs`, `src/request.rs`, `src/apply.rs`, `src/main.rs`
- Justification: Needed to rework the dashboard layout, fix request ingestion, and simplify apply logic.

## Recent Work

- Split the dashboard into separate queue/running panes and added a task history view showing the last 100 queue entries.
- Made `hyperion request` report success and avoid launching the integrated TUI, so the command can be used in automation without taking over the terminal.
- Documented the new runtime/request behavior and multi-pane dashboard history in `README.md`.
- Wired the agent harness into `hyperion request` (opt-in via `HYPERION_AGENT=copilot`) and backed the queue with actual `git apply` patch processing plus the multi-worker runtime, including a new deterministic patch format so workers can apply changes without failing the queue, storing worker events as JSON inside SQLite logs and surfacing them inside the TUI log panel.
- Added `change_queue_logs` table + retrieval API and taught the TUI to show a Worker Logs pane fed from the latest JSON events instead of printing noise to stdout; logging to the console is now suppressed unless `HYPERION_LOG=1`, `git apply` stderr/stdout is captured to keep errors off-screen, and filesystem changes are tracked via fsnotify and surfaced in the UI.

## Plan Updates

- Removed outdated task checklist entries and documented the current multi-pane TUI + deterministic request pipeline workstreams in `WORKPLAN.md`.
