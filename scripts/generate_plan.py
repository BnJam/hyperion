#!/usr/bin/env python3
import argparse
import json
import os
import textwrap


def ensure_dir(path):
    directory = os.path.dirname(path)
    if directory:
        os.makedirs(directory, exist_ok=True)


def render_plan(metadata, phases, definition_path, output_path, progress_path):
    lines = []
    lines.append("# WORKPLAN — workplan skill")
    lines.append("")

    required_sections = metadata.get("required_sections", "Intent,Goals,Non-Goals,Scope,Constraints,Plan,Commands,Validation,Approval")
    approval_pattern = metadata.get("approval_pattern", "^Approved:[[:space:]]+yes$")
    validation_policy = metadata.get("validation_policy", "explicit commands (guard scripts + scans)")
    lines.append("## Plan Metadata")
    lines.append(f"Approval pattern: {approval_pattern}")
    lines.append(f"Required sections: {required_sections}")
    lines.append(f"Validation policy: {validation_policy}")
    lines.append(f"Plan Source: workplan")
    lines.append(f"Plan Definition: {os.path.relpath(definition_path, os.path.dirname(output_path))}")
    lines.append(f"Phase progress file: {os.path.relpath(progress_path, os.path.dirname(output_path))}")
    lines.append("")

    section_map = {
        "Intent": metadata.get("intent", ""),
        "Goals": metadata.get("goals", []),
        "Non-Goals": metadata.get("non_goals", []),
        "Scope": metadata.get("scope", []),
        "Constraints": metadata.get("constraints", []),
    }

    for heading, value in section_map.items():
        lines.append(f"## {heading}")
        if isinstance(value, list):
            if value:
                for item in value:
                    lines.append(f"- {item}")
            else:
                lines.append("- none")
        else:
            lines.append(value or "- none")
        lines.append("")

    lines.append("## Plan")
    for phase in phases:
        lines.append(f"### {phase.get('name')}")
        summary = phase.get("summary")
        if summary:
            lines.append(f"{summary}")
        for task in phase.get("tasks", []):
            task_line = f"- [ ] {task.get('id')}: {task.get('summary')}"
            lines.append(task_line)
            description = task.get("description")
            if description:
                for desc_line in textwrap.wrap(description, width=80):
                    lines.append(f"  {desc_line}")
            commands = task.get("commands", [])
            if commands:
                lines.append("  Commands:")
                for cmd in commands:
                    lines.append(f"  - {cmd}")
            verification = task.get("verification", [])
            if verification:
                lines.append("  Verification:")
                for ver in verification:
                    lines.append(f"  - {ver}")
        lines.append("")

    commands = metadata.get("commands", [])
    lines.append("## Commands")
    if commands:
        for cmd in commands:
            lines.append(f"- {cmd}")
    else:
        lines.append("- see phase-specific entries above")
    lines.append("")

    validation_steps = metadata.get("validation_steps", [])
    lines.append("## Validation")
    if validation_steps:
        for step in validation_steps:
            lines.append(f"- {step}")
    else:
        lines.append("- scripts/bootstrap.sh --check")
    lines.append("")

    approval = metadata.get("approval", {})
    lines.append("## Approval")
    if approval:
        for key, value in approval.items():
            lines.append(f"{key}: {value}")
    else:
        lines.append("Approved: yes")
        lines.append("Approved by: workflow")
        lines.append("Approved on: 2026-01-30")
    lines.append("")

    ensure_dir(output_path)
    with open(output_path, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines))


def build_progress_data(definition, output_path, progress_path):
    phases = []
    for phase in definition.get("phases", []):
        task_entries = []
        for task in phase.get("tasks", []):
            entry = {
                "id": task.get("id"),
                "summary": task.get("summary"),
                "description": task.get("description"),
                "commands": task.get("commands", []),
                "verification": task.get("verification", []),
                "status": "pending",
            }
            task_entries.append(entry)
        phases.append(
            {
                "name": phase.get("name"),
                "summary": phase.get("summary"),
                "tasks": task_entries,
            }
        )

    if phases and phases[0]["tasks"]:
        phases[0]["tasks"][0]["status"] = "current"
        current_phase = 0
        current_task = 0
    else:
        current_phase = -1
        current_task = -1

    progress = {
        "plan_definition": os.path.relpath(definition["path"], os.path.dirname(output_path)),
        "plan_path": os.path.abspath(output_path),
        "phase_index": current_phase,
        "task_index": current_task,
        "phases": phases,
    }

    ensure_dir(progress_path)
    with open(progress_path, "w", encoding="utf-8") as fh:
        json.dump(progress, fh, indent=2)


def main():
    parser = argparse.ArgumentParser(description="Generate phased WORKPLAN.md from a definition.")
    parser.add_argument("--definition", required=True, help="Path to the phase plan definition JSON.")
    parser.add_argument("--output", required=True, help="Path to write the generated WORKPLAN.md.")
    parser.add_argument("--progress", required=True, help="Path to write execution/phase_progress.json.")
    args = parser.parse_args()

    with open(args.definition, "r", encoding="utf-8") as fh:
        definition = json.load(fh)
    metadata = definition.get("metadata", {})
    phases = definition.get("phases", [])
    definition["path"] = os.path.abspath(args.definition)

    render_plan(metadata, phases, args.definition, args.output, args.progress)
    build_progress_data(definition, args.output, args.progress)


if __name__ == "__main__":
    main()
