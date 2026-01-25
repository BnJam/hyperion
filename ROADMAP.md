# Roadmap: Multi-Agent Orchestration System

## Near Term (0–1 months)
- Finalize requirements and glossary.
- Draft task decomposition rules for the Orchestrator.
- Define JSON schemas for:
  - Task assignments
  - Change requests
  - Validation results
- Prototype Merge Queue/Buffer with basic validation.
- Implement agent harness trait with GitHub Copilot CLI and model `gpt-5-mini`.

## Short Term (1–3 months)
- Implement Engineer agent intake workflow.
- Build Orchestrator service with decomposition and assignment logic.
- Integrate Developer agents with scoped context windows.
- Implement conflict detection and quarantine logic in Merge Queue.
- Add a basic audit log and reporting.
- Add lease-based dequeue and retry counters in the queue.
- Add a worker loop for continuous processing and retries.

## Mid Term (3–6 months)
- Add automated rebase/retry flows in Merge Queue.
- Implement advanced conflict resolution policies.
- Expand validation to include static analysis and security checks.
- Add dashboards for throughput, conflict rate, and success rate.
- Ship a ratatui-based TUI dashboard for live visibility.
- Publish hardening and resiliency checklist.

## Long Term (6–12 months)
- Support multi-repo orchestration.
- Add adaptive task decomposition based on historical performance.
- Implement human approval gates for risk tiers.
- Add rollback workflows and change impact analysis.

## Dependencies
- Stable JSON schema definitions.
- CI/test environment compatible with parallel patching.
- Access controls and audit logging framework.
- SQLite queue storage with WAL enabled for durability.

## Risks & Contingencies
- **High conflict rate:** enforce file-level ownership and reduce parallelism.
- **Low Developer output quality:** strengthen validation and feedback loops.
- **Schema drift:** version schemas and enforce compatibility checks.
