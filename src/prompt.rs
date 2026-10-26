use crate::config::MessageLevel;

pub const SYSTEM_PROMPT: &str = "You are an expert engineer who writes clear, precise Git commit messages \
following Conventional Commits (feat:, fix:, docs:, style:, refactor:, perf:, test:, build:, ci:, chore:). \
Rules: keep the subject line at most 72 characters, imperative mood, no trailing period, \
no markdown fences, no extra commentary. Output only the commit message.";

pub fn build_user_prompt(diff: &str, stat: &str, level: MessageLevel) -> String {
    let style = match level {
        MessageLevel::Quiet => {
            "a single one-line commit subject only (no body). Format: `<type>: <short summary>`"
        }
        MessageLevel::Normal => {
            "a concise subject line, a blank line, then a short body with 2-5 bullet points \
             explaining what changed and why. Format the first line as `<type>: <summary>`"
        }
        MessageLevel::Verbose => {
            "a detailed subject line, a blank line, then a thorough body explaining what changed, \
             why it changed, and any impact or follow-ups. Format the first line as `<type>: <summary>`"
        }
    };

    if stat.trim().is_empty() {
        format!("Write {style} for these staged changes.\n\nChanges:\n{diff}")
    } else {
        format!(
            "Write {style} for these staged changes.\n\nFiles changed:\n{stat}\n\nChanges:\n{diff}"
        )
    }
}

/// Parse a model response into `(subject, Option<body>)`.
/// Strips markdown code fences models sometimes add despite instructions.
pub fn parse_commit_message(response: &str) -> (String, Option<String>) {
    let trimmed = response.trim();
    let cleaned = strip_code_fence(trimmed);

    if let Some((subject, body)) = cleaned.split_once("\n\n") {
        let subject = subject.lines().next().unwrap_or(subject).trim().to_string();
        let body = body.trim();
        if body.is_empty() {
            (subject, None)
        } else {
            (subject, Some(body.to_string()))
        }
    } else {
        let subject = cleaned.lines().next().unwrap_or("").trim().to_string();
        (subject, None)
    }
}

fn strip_code_fence(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```") {
        // Drop the opening fence line (``` or ```text) and closing fence.
        let after_first_line = rest.split_once('\n').map(|(_, b)| b).unwrap_or("");
        let inner = after_first_line
            .trim_end()
            .strip_suffix("```")
            .unwrap_or(after_first_line);
        return inner.trim().to_string();
    }
    t.to_string()
}

pub fn full_commit_text(subject: &str, body: &Option<String>) -> String {
    match body {
        Some(b) if !b.trim().is_empty() => format!("{subject}\n\n{b}"),
        _ => subject.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_subject_and_body() {
        let (subject, body) = parse_commit_message("feat: add login\n\n- Add JWT\n- Add tests");
        assert_eq!(subject, "feat: add login");
        assert_eq!(body.as_deref(), Some("- Add JWT\n- Add tests"));
    }

    #[test]
    fn parses_subject_only() {
        let (subject, body) = parse_commit_message("fix: typo");
        assert_eq!(subject, "fix: typo");
        assert!(body.is_none());
    }

    #[test]
    fn strips_code_fences() {
        let (subject, _) = parse_commit_message("```\nfeat: x\n\nbody\n```");
        assert_eq!(subject, "feat: x");
    }

    #[test]
    fn prompt_includes_stat_and_diff() {
        let p = build_user_prompt("File: a.rs", "a.rs | 1 +", MessageLevel::Normal);
        assert!(p.contains("a.rs | 1 +"));
        assert!(p.contains("File: a.rs"));
    }
}
