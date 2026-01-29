use std::{
    env,
    fs,
    io::{self, Write},
    path::Path,
    process::Command,
};

use anyhow::{Context, Result};

pub fn export_skill(target: &Path, overwrite: bool) -> Result<()> {
    let target = if target.starts_with(".") {
        fs::canonicalize(target).unwrap_or_else(|_| target.to_path_buf())
    } else {
        target.to_path_buf()
    };
    fs::create_dir_all(&target)
        .with_context(|| format!("create target directory {}", target.display()))?;
    let skills_dir = target.join("skills");
    if skills_dir.exists() && !overwrite && !confirm_overwrite(&skills_dir)? {
        anyhow::bail!("export aborted by user");
    }

    // Copy skills and templates into the target bundle
    copy_dir(Path::new("skills"), &target.join("skills"))?;

    // Do not copy external agentskills; prompts should be curated from examples only.

    // Copy templates if available in repo; otherwise ensure target assets/templates exists (handled later)
    copy_dir(
        Path::new("assets/templates"),
        &target.join("assets/templates"),
    )?;

    // Render and write the export guide. If repo assets/templates missing, generate a default template into the target bundle.
    let source_template = Path::new("assets/templates").join("EXPORT_GUIDE.template.md");
    let template = if source_template.exists() {
        fs::read_to_string(&source_template)
            .context("read export guide template from assets/templates")?
    } else {
        // Create target assets/templates directory and write a default template there.
        let target_templates_dir = target.join("assets/templates");
        fs::create_dir_all(&target_templates_dir)
            .with_context(|| format!("create {}", target_templates_dir.display()))?;
        let default_template = r#"## Export Guide

Exported bundle path: {{target_path}}

Language: {{language}}

Git Status:
{{git_status}}

# Using the bundle

The 'skills/' directory contains skill implementations. Run 'hyperion run' or other CLI commands to operate.
"#;
        let target_template_path = target_templates_dir.join("EXPORT_GUIDE.template.md");
        fs::write(&target_template_path, default_template)
            .with_context(|| format!("write {}", target_template_path.display()))?;
        default_template.to_string()
    };
    let guide = render_export_guide(&template, &target)?;
    let guide_path = target.join("EXPORT_GUIDE.md");
    fs::write(&guide_path, guide).with_context(|| format!("write {}", guide_path.display()))?;

    // Generate per-skill prompt files in the bundle root (e.g. <skill>.prompt.md)
    if let Err(e) = generate_skill_prompts(&target.join("skills"), &target) {
        eprintln!("warning: failed to generate skill prompt files: {}", e);
    }

    println!("Exported Hyperion skill bundle to {}", target.display());
    Ok(())
}

fn generate_skill_prompts(skills_dir: &Path, target: &Path) -> Result<()> {
    // If there is no skills directory (nothing to export), nothing to do
    if !skills_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(skills_dir).with_context(|| format!("read {}", skills_dir.display()))? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.starts_with('.') {
            continue;
        }

        let path = entry.path();
        let skill_name = if path.is_file() {
            match Path::new(&file_name).file_stem().and_then(|s| s.to_str()) {
                Some(stem) => stem.to_string(),
                None => file_name.clone(),
            }
        } else {
            file_name.clone()
        };

        let prompt_path = target.join(format!("{}.prompt.md", skill_name));

        // If the skill is a directory and contains SKILL.md, use its contents as the prompt body
        let content = if path.is_dir() {
            let skill_md = path.join("SKILL.md");
            if skill_md.exists() {
                let md = fs::read_to_string(&skill_md).unwrap_or_default();
                format!("# {skill_name} skill prompt\n\n{md}\n\nCLI examples\n- hyperion agent --model gpt-5-mini \"{skill_name}: <describe the task>\"\n- hyperion request path/to/request.json --model gpt-5-mini --agents 1\n- hyperion orchestrate path/to/request.json\n- hyperion worker --worker-id {skill_name}-worker\n- hyperion run\n- hyperion apply path/to/changes.json\n\nInspect the skill implementation in skills/{skill_name} for more details.\n", skill_name=skill_name)
            } else {
                render_skill_prompt(&skill_name, skills_dir)?
            }
        } else {
            render_skill_prompt(&skill_name, skills_dir)?
        };

        fs::write(&prompt_path, content)
            .with_context(|| format!("write {}", prompt_path.display()))?;
    }
    Ok(())
}

fn render_skill_prompt(skill: &str, skills_dir: &Path) -> Result<String> {
    let skills_dir_display = skills_dir.display();
    let prompt = format!(r#"# {skill} skill prompt

This file contains instructions that an agent can use to interact with the Hyperion system to execute the `{skill}` skill.

Environment
- The `hyperion` binary must be available in PATH (build with `cargo build --release`), or run from the repository root using `cargo run --`.
- The exported bundle contains a `skills/` directory at `{skills_dir}` with the skill implementation.

Quick command examples
- Run an interactive agent with a short prompt:
  hyperion agent --model gpt-5-mini \"{skill}: <describe the task>\"

- Submit a task request file (JSON):
  hyperion request path/to/request.json --model gpt-5-mini --agents 1

- Decompose a request into assignments:
  hyperion orchestrate path/to/request.json

- Start a worker to process queued tasks:
  hyperion worker --worker-id {skill}-worker

- Run the integrated stack (workers + TUI):
  hyperion run

- Apply changes produced by agents:
  hyperion apply path/to/changes.json

Notes
- Inspect `skills/{skill}` inside the exported bundle for implementation details.
- Use these prompts and the CLI examples to automate or orchestrate the skill.
"#, skill=skill, skills_dir=skills_dir_display);
    Ok(prompt)
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst).with_context(|| format!("create {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("read {}", src.display()))? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)
                .with_context(|| format!("copy {} to {} failed", path.display(), dest.display()))?;
        }
    }
    Ok(())
}

fn render_export_guide(template: &str, target: &Path) -> Result<String> {
    let target_path = target.display().to_string();
    let language = detect_language(target);
    let git_status = describe_git_status(target);
    let output = template
        .replace("{{target_path}}", &target_path)
        .replace("{{language}}", &language)
        .replace("{{git_status}}", &git_status);
    Ok(output)
}

fn detect_language(target: &Path) -> String {
    if target.join("Cargo.toml").exists() {
        "Rust (Cargo)".into()
    } else if target.join("package.json").exists() {
        "Node.js (npm)".into()
    } else if target.join("pyproject.toml").exists() {
        "Python (poetry)".into()
    } else {
        "Unknown".into()
    }
}

fn describe_git_status(target: &Path) -> String {
    let status = Command::new("git")
        .arg("status")
        .arg("-sb")
        .current_dir(target)
        .output();
    match status {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            format!("git status failed: {}", stderr.trim())
        }
        Err(err) => format!("git status unavailable: {err}"),
    }
}

fn confirm_overwrite(path: &Path) -> Result<bool> {
    eprint!(
        "Target skills directory {} already exists. Overwrite? [y/N]: ",
        path.display()
    );
    io::stdout().flush().ok();
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer)?;
    Ok(matches!(buffer.trim().to_lowercase().as_str(), "y" | "yes"))
}
