use si_git_commit::config::{Config, MessageLevel, Provider, DEFAULT_MODEL};
use si_git_commit::git::format_diff_for_llm;
use si_git_commit::prompt::{build_user_prompt, parse_commit_message};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn ghsi_bin() -> &'static str {
    env!("CARGO_BIN_EXE_ghsi")
}

fn temp_config(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("ghsi-it-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let config = dir.join("config.toml");
    (dir, config)
}

fn run_ghsi(config: &PathBuf, args: &[&str]) -> std::process::Output {
    Command::new(ghsi_bin())
        .args(args)
        .env("SI_GIT_COMMIT_CONFIG", config)
        .stdin(Stdio::null())
        .output()
        .expect("run ghsi")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn default_config_uses_a_modern_model() {
    let cfg = Config::default();
    assert_eq!(cfg.model, DEFAULT_MODEL);
    // The deprecated model from the orphaned tool must never come back.
    assert_ne!(cfg.model, "gpt-4.1-nano");
    assert_eq!(cfg.provider, Provider::Openai);
    assert_eq!(cfg.message_level, MessageLevel::Normal);
}

#[test]
fn prompt_and_parse_roundtrip() {
    let prompt = build_user_prompt(
        "File: src/lib.rs\n  Added: pub fn x() {}",
        "src/lib.rs | 1 +",
        MessageLevel::Verbose,
    );
    assert!(prompt.contains("src/lib.rs"));

    let (subject, body) = parse_commit_message("feat: add x\n\n- Implement x\n- Test x");
    assert_eq!(subject, "feat: add x");
    assert!(body.unwrap().contains("Implement x"));
}

#[test]
fn diff_formatting_handles_multiple_files() {
    let raw = "\
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -0,0 +1 @@
+hello
diff --git a/b.txt b/b.txt
--- a/b.txt
+++ b/b.txt
@@ -1 +0,0 @@
-bye
";
    let out = format_diff_for_llm(raw, 12_000);
    assert!(out.contains("File: a.txt"));
    assert!(out.contains("File: b.txt"));
    assert!(out.contains("Added: hello"));
    assert!(out.contains("Removed: bye"));
}

#[test]
fn cli_help_points_at_settings_file() {
    let (_dir, config) = temp_config("help");
    let out = run_ghsi(&config, &["--help"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("ghsi config"), "help was: {text}");
    assert!(text.contains("config model"), "help was: {text}");
}

#[test]
fn cli_version_matches_crate() {
    let (_dir, config) = temp_config("version");
    let out = run_ghsi(&config, &["--version"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("0.1.0"), "got: {}", stdout(&out));
}

#[test]
fn cli_config_path_prints_settings_location() {
    let (_dir, config) = temp_config("path");
    let out = run_ghsi(&config, &["config", "path"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains(&config.display().to_string()),
        "expected {} in: {}",
        config.display(),
        stdout(&out)
    );
}

#[test]
fn cli_config_init_model_and_show() {
    let (dir, config) = temp_config("init-model");

    let init = run_ghsi(&config, &["config", "init"]);
    assert!(init.status.success(), "stderr: {}", stderr(&init));
    assert!(config.exists(), "config file should be created");

    let set = run_ghsi(&config, &["config", "model", "gpt-5.6-luna"]);
    assert!(set.status.success(), "stderr: {}", stderr(&set));
    assert!(stdout(&set).contains("gpt-5.6-luna"));

    let show = run_ghsi(&config, &["config"]);
    assert!(show.status.success(), "stderr: {}", stderr(&show));
    let text = stdout(&show);
    assert!(text.contains("gpt-5.6-luna"), "show was: {text}");
    assert!(
        text.contains(&config.display().to_string()),
        "show must reference the settings file, was: {text}"
    );

    // Non-interactive `config model` reports the current model.
    let current = run_ghsi(&config, &["config", "model"]);
    assert!(current.status.success(), "stderr: {}", stderr(&current));
    assert!(stdout(&current).contains("gpt-5.6-luna"));

    let list = run_ghsi(&config, &["config", "model", "--list"]);
    assert!(list.status.success(), "stderr: {}", stderr(&list));
    assert!(stdout(&list).contains("gpt-5-mini"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cli_config_set_rejects_unknown_key() {
    let (_dir, config) = temp_config("bad-key");
    let out = run_ghsi(&config, &["config", "set", "bogus", "x"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("unknown config key"));
}

#[test]
fn cli_missing_config_error_references_settings() {
    let (_dir, config) = temp_config("missing");
    // No config init: a workflow command must fail pointing at the file.
    let out = run_ghsi(&config, &["generate"]);
    assert!(!out.status.success());
    let text = format!("{}{}", stdout(&out), stderr(&out));
    assert!(text.contains("config"), "error was: {text}");
    assert!(
        text.contains(&config.display().to_string()),
        "error must reference the settings file, was: {text}"
    );
}
