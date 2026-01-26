use std::{
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
    if skills_dir.exists() && !overwrite {
        if !confirm_overwrite(&skills_dir)? {
            anyhow::bail!("export aborted by user");
        }
    }
    copy_dir(Path::new("skills"), &target.join("skills"))?;
    copy_dir(
        Path::new("assets/templates"),
        &target.join("assets/templates"),
    )?;
    let template_path = Path::new("assets/templates").join("EXPORT_GUIDE.template.md");
    let template = fs::read_to_string(&template_path)
        .context("read export guide template from assets/templates")?;
    let guide = render_export_guide(&template, &target)?;
    let guide_path = target.join("EXPORT_GUIDE.md");
    fs::write(&guide_path, guide).with_context(|| format!("write {}", guide_path.display()))?;
    println!("Exported Hyperion skill bundle to {}", target.display());
    Ok(())
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
    Ok(matches!(
        buffer.trim().to_lowercase().as_str(),
        "y" | "yes"
    ))
}
