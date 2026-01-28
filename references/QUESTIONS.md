# Outstanding Questions

1. Should the Hyperion issue bridge mirror Farcaster by auto-invoking CI + auto-fix when a guard suite fails, or should it log the failure and wait for a human before rerunning?
2. How far should Hyperion's RATATUI dashboard go in exposing WAL telemetry and agent session hashes before the CLI/TUI becomes too noisy for operators?
3. Do we need to extend the change_queue schema with dedup keys/time-to-live entries before we start ingesting Farcaster-style casts, or can we defer that until later phases?
4. Should the assignment complexity threshold that triggers the gpt-4.1 fallback be configurable, or is the current ≤3 rule enough for deterministic rope-offs?
5. How tightly should we couple telemetry anchors with guard events before showing them in the TUI (for example, should we expose identical agent intent strings), and who owns the final approvals recorded in `metadata.approvals`?
