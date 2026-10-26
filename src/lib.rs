//! # si-git-commit — Super Intelligent Git Commit
//!
//! **AI-powered git commit messages and workflow automation** (`ghsi` CLI).
//!
//! Analyzes staged changes with a current AI model and writes
//! Conventional Commits messages, then stages, commits, and pushes in a
//! single streamlined command. Built with modern provider parameters
//! (`max_completion_tokens` for OpenAI) so current models keep working.
//!
//! ## CLI quick start
//!
//! ```bash
//! ghsi config init          # create the settings TOML (path shown for you)
//! ghsi config model         # view / change your model interactively
//! ghsi                      # stage all → generate → confirm → commit → push
//! ghsi stage                # stage + generate only (no commit)
//! ghsi generate             # generate for staged changes only
//! ```
//!
//! The settings file location is always discoverable with
//! `ghsi config path` — no hunting for dotfiles.
//!
//! ## Library usage
//!
//! ```no_run
//! use si_git_commit::{Config, Workflow, WorkflowOptions};
//!
//! # async fn run() -> si_git_commit::Result<()> {
//! let config = Config::load()?;
//! let workflow = Workflow::new(config);
//! workflow
//!     .commit_workflow(vec![], WorkflowOptions { yes: false, ..Default::default() })
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub mod cli;
pub mod config;
pub mod error;
pub mod git;
pub mod llm;
pub mod prompt;
pub mod workflow;

pub use config::{Config, MessageLevel, Provider};
pub use error::{Result, SiError};
pub use llm::Llm;
pub use workflow::{Workflow, WorkflowOptions};
