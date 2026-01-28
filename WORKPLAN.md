# WORKPLAN — workplan skill

## Plan Metadata
Approval pattern: ^Approved:[[:space:]]+yes$
Required sections: Intent,Goals,Non-Goals,Scope,Constraints,Plan,Commands,Validation,Approval
Validation policy: explicit commands (guard scripts + scans)
Plan Source: workplan
Plan Definition: ../../.codex/skills/workplan/plans/phase_plan.json
Phase progress file: execution/phase_progress.json

## Intent
Describe how workplan authors dashboards and generators so developer runs follow the phased/task workflow.

## Goals
- Document the new generator + progress tracker flow for authors.
- Produce a phased WORKPLAN.md with commands and verification references.
- Ensure developer execution calls workplan for the next task and keeps the phase tracker updated.

## Non-Goals
- Automatically executing plan commands without approval.
- Replacing the developer skill’s receipt and verification pipeline.

## Scope
- This skill writes WORKPLAN.md/ROADMAP.md from the phase definition.
- It emits execution/phase_progress.json plus structured next-task context.
- It logs commands and artifacts required for downstream developer runs.

## Constraints
- Plans must include the required metadata block (Intent → Approval).
- Phase tracker updates stay in execution/phase_progress.json.
- All commands and validations reference deterministic scripts.

## Plan
### Phase 1 — Document the flow
Describe how authors use the generator, the phase tracker, and the helper script before running developer.
- [ ] T001: Update the README/SKILL references, command checklist, and reference docs to describe the generator + progress flow.
  Clarify that authors should run scripts/generate_plan.py, inspect
  execution/phase_progress.json, and refer to references/ASI_FRAMEWORK.md before
  invoking developer.
  Commands:
  - rg -n "workplan-helper" README.md
  - rg -n "phase_progress" references
  Verification:
  - python3 scripts/generate_plan.py --definition plans/phase_plan.json --output WORKPLAN.md --progress execution/phase_progress.json
- [ ] T002: Capture the deterministic surface and open questions in the new references/ASI_FRAMEWORK.md plus references/QUESTIONS.md entries.
  Document the ASI guidance in the top-level references so plan authors rely on
  visible docs.
  Commands:
  - cat references/ASI_FRAMEWORK.md
  - cat references/QUESTIONS.md
  Verification:
  - scripts/validate-plan.sh --plan ./WORKPLAN.md --required "Intent,Goals,Non-Goals,Scope,Constraints,Plan,Commands,Validation,Approval" --approval-pattern "^Approved:[[:space:]]+yes$"

### Phase 2 — Generate phased plans
Introduce scripts that render WORKPLAN.md from the definition and output execution/phase_progress.json.
- [ ] T003: Create scripts/generate_plan.py and scripts/update_phase_progress.py to keep phases/tasks structured and track status.
  Generate tasks with commands/listing plus a progress artifact that newbies can
  read.
  Commands:
  - python3 scripts/generate_plan.py --definition plans/phase_plan.json --output WORKPLAN.md --progress execution/phase_progress.json
  Verification:
  - cat execution/phase_progress.json
- [ ] T004: Ensure WORKPLAN.md is regenerated from the definition whenever the definition changes.
  The plan should always reflect the latest phases/tasks inside
  plans/phase_plan.json.
  Commands:
  - python3 scripts/generate_plan.py --definition plans/phase_plan.json --output WORKPLAN.md --progress execution/phase_progress.json
  Verification:
  - cat WORKPLAN.md

### Phase 3 — Track progress from developer
Hook developer's execute-plan loop into the workplan progress tracker and emit next-task context.
- [ ] T005: Add scripts/workplan-helper.sh and update execute-plan.sh so the developer skill always queries workplan for the next task.
  Each completed task triggers an update_phase_progress call and returns a
  structured JSON with the next phase/task context.
  Commands:
  - ../developer/scripts/workplan-helper.sh next-task --plan ./WORKPLAN.md --progress execution/phase_progress.json --task T005
  Verification:
  - python3 scripts/generate_plan.py --definition plans/phase_plan.json --output WORKPLAN.md --progress execution/phase_progress.json && cat execution/next_task_context.json
- [ ] T006: Document how developer reads execution/next_task_context.json and phase_progress before resuming work.
  Ensure plan authors see the structured message and understand how phases roll
  over.
  Commands:
  - cat execution/next_task_context.json
  Verification:
  - scripts/validate-plan.sh --plan ./WORKPLAN.md --required "Intent,Goals,Non-Goals,Scope,Constraints,Plan,Commands,Validation,Approval" --approval-pattern "^Approved:[[:space:]]+yes$"

## Commands
- python3 scripts/generate_plan.py --definition plans/phase_plan.json --output WORKPLAN.md --progress execution/phase_progress.json
- ../developer/scripts/workplan-helper.sh ensure-plan --plan ./WORKPLAN.md --progress execution/phase_progress.json

## Validation
- ./scripts/validate-plan.sh --plan ./WORKPLAN.md --required "Intent,Goals,Non-Goals,Scope,Constraints,Plan,Commands,Validation,Approval" --approval-pattern "^Approved:[[:space:]]+yes$"
- ./scripts/check-workspace.sh --root . --fail-on-dirty
- ./scripts/check-anti-patterns.sh --plan ./WORKPLAN.md --root .

## Approval
Approved: yes
Approved by: bsmith
Approved on: 2026-01-30
