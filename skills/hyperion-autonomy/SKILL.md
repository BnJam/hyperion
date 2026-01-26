name: hyperion-autonomy
license: MIT
summary: Navigate the Hyperion agent lifecycle with Copilot session persistence and autonomous queue execution.
description: |
  Enhance the prior autonomy skill by adding lifecycle checks, verification of `agent_sessions`, and safety handoffs. The skill instructs the agent to bootstrap a Copilot session via `hyperion session init --resume=<hash> --model=<name> --allow-all-tools=<bool>` before touching `hyperion request`. It also encourages logging session metadata in SQLite so subsequent runs reuse it, and ensures `hyperion Tui` is refreshed to show queue and history data without log dumps. Run sequences must capture `change_queue_logs`/`file_modifications` entries when retries happen due to patch failures or registry outages.
instructions:
  - Confirm a Copilot session exists by calling `hyperion session list`; if empty, instruct the human to run `hyperion session init` with a recorded resume hash and optional allow-all flag.
  - When executing tasks, rely on the stored session metadata (model, resume ID) so agent invocations use `--resume` instead of repeating `-p`, preventing spoilers for the one-shot Cot path.
  - After enqueuing change requests, poll `hyperion Tui` to ensure the history/stats panes update and no worker logs remain on-screen; highlight that worker logs are read from `change_queue_logs` (structured JSON), not stdout.
  - If Cargo commands fail because `crates.io` cannot resolve, record the DNS error in `execution/verification_report.json` and abort with a note to retry later; do not retry automatically until the registry is reachable.
  - For any irreversible git operations (commits, resets), require explicit human approval; offer a dry-run summary before touching git to respect the repo's offline autonomy constraints.
assets: []
references: []
keywords: [autonomy, session, cli, safety]
