## Export Guide

Generated for `{{target_path}}` (detected as {{language}}). Git status at export time:
```
{{git_status}}
```

### Included Skills
- `skills/cast-builder`: REPL + export skill capturing intent, complexity (1–10), telemetry anchors, approvals, and sample diffs before producing deterministic JSON casts for Copilot agents. The exported directory mirrors the master manifest so downstream operators can run `scripts/cast_builder.sh` inside the target workspace and follow the same flow.
- The existing skill catalogs (`skills/hyperion-autonomy`, `skills/hyperion-improvement`, `skills/hyperion-ops`) are also copied so other agent guidance remains available.

### Cast Builder Instructions
1. Run `scripts/cast_builder.sh` to launch the prompts, supply request ID, intent, complexity, telemetry anchors, approvals, and desired changes.
2. Confirm the assignment metadata (intent, complexity rating, sample diff snippet, telemetry anchors, approvals, `agent_model`) so `execution/next_task_context.json` reflects the exported context.
3. Submit the deterministic payload with `HYPERION_AGENT=copilot cargo run -- request taskjson/<REQUEST_ID>.json` once the metadata and approvals match what you expect.

### Verification
- Produce a portable bundle with `tar -czf /tmp/cast-builder.tar.gz scripts/cast_builder.sh skills/cast-builder` and record `shasum -a 256 /tmp/cast-builder.tar.gz` (macOS) or `sha256sum` (Linux) so recipients can confirm the bundle before running Copilot workloads.
