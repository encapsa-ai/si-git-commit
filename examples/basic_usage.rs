//! Library usage without any network or git calls:
//! inspect defaults, build the prompt that would be sent to the model,
//! and parse a model response back into subject/body.
//!
//! Run with: `cargo run --example basic_usage`

use si_git_commit::config::{Config, MessageLevel};
use si_git_commit::prompt::{build_user_prompt, parse_commit_message};

fn main() {
    let config = Config::default();
    println!("Default provider: {}", config.provider.as_str());
    println!("Default model:    {}", config.model);
    println!("Settings file:    {}", Config::settings_reference());
    println!("Change it with:   ghsi config model <name>");
    println!();

    let diff = "File: src/main.rs\n  Added: println!(\"hello\");";
    let prompt = build_user_prompt(diff, "src/main.rs | 1 +", MessageLevel::Normal);
    println!("Prompt sent to the model:\n{prompt}\n");

    let (subject, body) = parse_commit_message("feat: greet on startup\n\n- Print hello in main");
    println!("Parsed subject: {subject}");
    println!("Parsed body:    {}", body.as_deref().unwrap_or("(none)"));
}
