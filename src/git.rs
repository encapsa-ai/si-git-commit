use crate::error::{Result, SiError};
use std::process::Command;

pub struct Git;

fn run_git(args: &[&str]) -> Result<std::process::Output> {
    Command::new("git")
        .args(args)
        .output()
        .map_err(|e| SiError::Git(format!("failed to execute `git {}`: {e}", args.join(" "))))
}

fn ensure_success(args: &[&str], output: &std::process::Output) -> Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(SiError::Git(format!(
            "`git {}` failed: {stderr}",
            args.join(" ")
        )))
    }
}

impl Git {
    pub fn is_inside_work_tree() -> bool {
        run_git(&["rev-parse", "--is-inside-work-tree"])
            .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
            .unwrap_or(false)
    }

    pub fn ensure_repo() -> Result<()> {
        if Self::is_inside_work_tree() {
            Ok(())
        } else {
            Err(SiError::NotARepo)
        }
    }

    /// Stage specific files, or everything (`git add .`) when empty.
    pub fn add_files(files: &[String]) -> Result<()> {
        if files.is_empty() {
            return Self::add_all_changes();
        }
        let mut args: Vec<&str> = vec!["add", "--"];
        args.extend(files.iter().map(|s| s.as_str()));
        let output = run_git(&args)?;
        ensure_success(&args, &output)
    }

    pub fn add_all_changes() -> Result<()> {
        let output = run_git(&["add", "."])?;
        ensure_success(&["add", "."], &output)?;
        println!("📁 Staged all modified files");
        Ok(())
    }

    /// True when there are unstaged or untracked changes.
    /// Uses `git status --porcelain` so untracked files count
    /// (plain `git diff --quiet` misses them).
    pub fn has_unstaged_changes() -> Result<bool> {
        let output = run_git(&["status", "--porcelain"])?;
        ensure_success(&["status", "--porcelain"], &output)?;
        let text = String::from_utf8_lossy(&output.stdout);
        // Porcelain lines starting with a space or `?` in the worktree
        // column mean unstaged/untracked work exists. Any line at all
        // without staged content is also fine to treat via staged check;
        // here we report "worktree dirty" when any line has worktree state.
        Ok(text.lines().any(|line| {
            let bytes = line.as_bytes();
            bytes.len() >= 2 && (bytes[1] != b' ' || line.starts_with("??"))
        }))
    }

    pub fn has_staged_changes() -> Result<bool> {
        let output = run_git(&["diff", "--cached", "--quiet"])?;
        Ok(!output.status.success())
    }

    pub fn get_staged_stat() -> Result<String> {
        let output = run_git(&["diff", "--cached", "--stat"])?;
        ensure_success(&["diff", "--cached", "--stat"], &output)?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Raw staged diff (`git diff --cached`).
    pub fn get_staged_diff_raw() -> Result<String> {
        let output = run_git(&["diff", "--cached", "--no-color"])?;
        ensure_success(&["diff", "--cached", "--no-color"], &output)?;
        let diff = String::from_utf8(output.stdout)
            .map_err(|e| SiError::Git(format!("invalid UTF-8 in git diff output: {e}")))?;
        if diff.trim().is_empty() {
            return Err(SiError::NoStagedChanges);
        }
        Ok(diff)
    }

    pub fn get_staged_diff() -> Result<String> {
        let raw = Self::get_staged_diff_raw()?;
        Ok(format_diff_for_llm(&raw, DEFAULT_DIFF_LIMIT))
    }

    pub fn commit(subject: &str, body: Option<&str>) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("commit").arg("-m").arg(subject);
        if let Some(body_text) = body {
            if !body_text.trim().is_empty() {
                cmd.arg("-m").arg(body_text);
            }
        }
        let output = cmd
            .output()
            .map_err(|e| SiError::Git(format!("failed to execute `git commit`: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SiError::Git(format!("git commit failed: {stderr}")));
        }
        println!("✓ Commit created successfully");
        Ok(())
    }

    pub fn push(branch: Option<&str>) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("push");
        if let Some(branch_name) = branch {
            cmd.arg("origin").arg(branch_name);
        }
        let output = cmd
            .output()
            .map_err(|e| SiError::Git(format!("failed to execute `git push`: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SiError::Git(format!("git push failed: {stderr}")));
        }
        println!("✓ Changes pushed to remote successfully");
        Ok(())
    }

    pub fn get_current_branch() -> Result<String> {
        let output = run_git(&["branch", "--show-current"])?;
        ensure_success(&["branch", "--show-current"], &output)?;
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() {
            return Err(SiError::Git(
                "could not determine current branch (detached HEAD?). Specify one with --branch"
                    .to_string(),
            ));
        }
        Ok(branch)
    }
}

pub const DEFAULT_DIFF_LIMIT: usize = 12_000;

/// Format a raw unified diff into a compact, LLM-friendly form.
/// Truncation is UTF-8 boundary safe (the original tool sliced at a byte
/// offset and could panic on multi-byte characters).
pub fn format_diff_for_llm(raw_diff: &str, limit: usize) -> String {
    let mut formatted = String::new();
    let mut current_file = String::new();
    let mut changes: Vec<String> = Vec::new();

    let flush = |formatted: &mut String, file: &str, changes: &Vec<String>| {
        if !file.is_empty() && !changes.is_empty() {
            formatted.push_str(&format!("File: {file}\n"));
            formatted.push_str(&changes.join("\n"));
            formatted.push_str("\n\n");
        }
    };

    for line in raw_diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            flush(&mut formatted, &current_file, &changes);
            // `diff --git a/path b/path` — take the b/ side.
            current_file = rest
                .split_whitespace()
                .nth(1)
                .unwrap_or("unknown")
                .trim_start_matches("b/")
                .to_string();
            changes.clear();
        } else if line.starts_with("+++") || line.starts_with("---") {
            continue;
        } else if line.starts_with("@@") {
            changes.push(format!("Changes at {line}"));
        } else if let Some(content) = line.strip_prefix('+') {
            changes.push(format!("  Added: {content}"));
        } else if let Some(content) = line.strip_prefix('-') {
            changes.push(format!("  Removed: {content}"));
        } else if let Some(content) = line.strip_prefix(' ') {
            if !content.trim().is_empty() && changes.len() < 40 {
                changes.push(format!("  Context: {content}"));
            }
        }
    }
    flush(&mut formatted, &current_file, &changes);

    let formatted = formatted.trim_end().to_string();
    truncate_str(&formatted, limit)
}

/// Truncate to at most `limit` bytes without splitting a UTF-8 character.
pub fn truncate_str(s: &str, limit: usize) -> String {
    if s.len() <= limit {
        return s.to_string();
    }
    let mut end = limit;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n\n[... diff truncated for brevity ...]", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_added_and_removed_lines() {
        let raw = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,2 +1,2 @@\n-old line\n+new line\n context\n";
        let out = format_diff_for_llm(raw, 12_000);
        assert!(out.contains("File: src/main.rs"));
        assert!(out.contains("Added: new line"));
        assert!(out.contains("Removed: old line"));
    }

    #[test]
    fn truncation_is_utf8_safe() {
        let s = "héllo wörld — ünïcode text that keeps going and going";
        let out = truncate_str(s, 10);
        assert!(out.contains("truncated"));
        // No panic and valid UTF-8 by construction.
        assert!(out.len() < s.len() + 60);
    }

    #[test]
    fn short_diff_is_not_truncated() {
        let raw = "diff --git a/a.txt b/a.txt\n@@ -0,0 +1 @@\n+hello\n";
        let out = format_diff_for_llm(raw, 12_000);
        assert!(!out.contains("truncated"));
    }
}
