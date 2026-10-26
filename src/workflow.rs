use crate::config::Config;
use crate::error::{Result, SiError};
use crate::git::Git;
use crate::llm::Llm;
use crate::prompt::full_commit_text;
use std::io::{self, Write};

#[derive(Debug, Clone, Default)]
pub struct WorkflowOptions {
    /// Skip the confirmation prompt (`-y/--yes`).
    pub yes: bool,
    /// Generate the message but do not commit or push.
    pub dry_run: bool,
    /// Commit locally without pushing.
    pub no_push: bool,
    /// Branch to push to (defaults to the current branch).
    pub branch: Option<String>,
    /// Open the generated message in `$EDITOR` before committing.
    pub edit: bool,
}

pub struct Workflow {
    llm: Llm,
    #[allow(dead_code)]
    config: Config,
}

impl Workflow {
    pub fn new(config: Config) -> Self {
        let llm = Llm::new(config.clone());
        Self { llm, config }
    }

    /// Full workflow: stage → generate → confirm → commit → push.
    /// This is the default `ghsi` behavior (the old `gp` alias).
    pub async fn commit_workflow(&self, files: Vec<String>, opts: WorkflowOptions) -> Result<()> {
        Git::ensure_repo()?;
        println!("🔄 Starting automated commit workflow...");

        self.stage_files(&files).await?;

        if !Git::has_staged_changes()? {
            return Err(SiError::NoChanges);
        }

        let (subject, body) = self.generate_for_staged().await?;

        let (subject, body) = if opts.edit {
            self.edit_message(subject, body)?
        } else {
            (subject, body)
        };

        Self::print_generated(&subject, &body);

        if opts.dry_run {
            println!("\n🏜️  Dry run — no commit created.");
            Self::print_manual_command(&subject, &body);
            return Ok(());
        }

        if !opts.yes && !Self::confirm_commit()? {
            println!("❌ Commit cancelled by user");
            return Ok(());
        }

        println!("💾 Creating commit...");
        Git::commit(&subject, body.as_deref())?;

        if opts.no_push {
            println!("✅ Committed locally (--no-push). Push when ready: `git push`");
            return Ok(());
        }

        println!("🚀 Pushing to remote...");
        let target_branch = match opts.branch.clone() {
            Some(b) => b,
            None => Git::get_current_branch()?,
        };
        println!("Pushing to branch: {target_branch}");
        Git::push(Some(&target_branch))?;

        println!("✅ Workflow completed successfully!");
        Ok(())
    }

    /// Generate a message for already-staged changes. Does not commit.
    pub async fn generate_message_only(&self) -> Result<()> {
        Git::ensure_repo()?;
        if !Git::has_staged_changes()? {
            // Match the old tool's helpful behavior: stage everything
            // rather than failing when the user forgot `git add`.
            if Git::has_unstaged_changes()? {
                println!("📁 Auto-staging all modified files...");
                Git::add_all_changes()?;
            } else {
                return Err(SiError::NoChanges);
            }
        }

        let (subject, body) = self.generate_for_staged().await?;
        Self::print_generated(&subject, &body);
        println!("\nTo use this message:");
        Self::print_manual_command(&subject, &body);
        Ok(())
    }

    /// Stage files (or everything), then generate. Does not commit.
    /// This is the old `gs` alias (`ghit --stage-and-generate`).
    pub async fn stage_and_generate(&self, files: Vec<String>) -> Result<()> {
        Git::ensure_repo()?;
        self.stage_files(&files).await?;
        // Reuse generate path without double-staging surprises:
        // generate_for_staged only reads the staged diff.
        let (subject, body) = self.generate_for_staged().await?;
        Self::print_generated(&subject, &body);
        println!("\nTo use this message:");
        Self::print_manual_command(&subject, &body);
        Ok(())
    }

    async fn stage_files(&self, files: &[String]) -> Result<()> {
        if !files.is_empty() {
            println!("📁 Adding specified files to staging area...");
            Git::add_files(files)?;
        } else if Git::has_unstaged_changes()? {
            println!("📁 Auto-staging all modified files...");
            Git::add_all_changes()?;
        }
        Ok(())
    }

    async fn generate_for_staged(&self) -> Result<(String, Option<String>)> {
        println!("📊 Reading staged changes...");
        let diff = Git::get_staged_diff()?;
        let stat = Git::get_staged_stat().unwrap_or_default();
        println!(
            "🤖 Generating commit message with AI ({})...",
            self.llm.model()
        );
        self.llm
            .gen_commit_message(&diff, &stat)
            .await
            .map_err(|e| match e {
                SiError::Api { provider, message } => SiError::Api {
                    provider,
                    message: format!(
                        "{message}\nHint: check your model with `ghsi config model` \
                         (settings: {}). Deprecated model ids are the most common cause.",
                        Config::settings_reference()
                    ),
                },
                other => other,
            })
    }

    fn print_generated(subject: &str, body: &Option<String>) {
        println!("\n📝 Generated commit message:");
        println!("Subject: {subject}");
        if let Some(body_text) = body {
            println!("Body:\n{body_text}");
        }
    }

    fn print_manual_command(subject: &str, body: &Option<String>) {
        let escaped_subject = subject.replace('"', "\\\"");
        match body {
            Some(body_text) => {
                let escaped_body = body_text.replace('"', "\\\"");
                println!("git commit -m \"{escaped_subject}\" -m \"{escaped_body}\"");
            }
            None => println!("git commit -m \"{escaped_subject}\""),
        }
    }

    fn edit_message(
        &self,
        subject: String,
        body: Option<String>,
    ) -> Result<(String, Option<String>)> {
        let initial = full_commit_text(&subject, &body);
        let edited = edit_in_editor(&initial)?;
        Ok(crate::prompt::parse_commit_message(&edited))
    }

    fn confirm_commit() -> Result<bool> {
        print!("\n❓ Create this commit? [Y/n]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();
        Ok(input.is_empty() || input == "y" || input == "yes")
    }
}

/// Open `text` in `$EDITOR` (falling back to `vi` / `notepad`) and
/// return the edited contents.
pub fn edit_in_editor(text: &str) -> Result<String> {
    let editor = std::env::var("VISUAL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "vi".to_string()
            }
        });

    let path = std::env::temp_dir().join(format!("ghsi-commit-{}.txt", std::process::id()));
    std::fs::write(&path, text)?;

    let status = std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .map_err(|e| {
            SiError::Other(format!(
                "failed to launch editor `{editor}`: {e}. Set $EDITOR and retry."
            ))
        })?;

    if !status.success() {
        let _ = std::fs::remove_file(&path);
        return Err(SiError::Other(format!(
            "editor `{editor}` exited with {status}"
        )));
    }

    let edited = std::fs::read_to_string(&path)?;
    let _ = std::fs::remove_file(&path);
    Ok(edited)
}
