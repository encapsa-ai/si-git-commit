use crate::config::{suggested_models, Config, MessageLevel, Provider};
use crate::workflow::{Workflow, WorkflowOptions};
use clap::{Args, Parser, Subcommand};
use std::io::{IsTerminal, Write};

const AFTER_HELP: &str = "\
SETTINGS FILE:
  Your settings TOML lives at the path shown by `ghsi config path`
  (override with $SI_GIT_COMMIT_CONFIG). You never need to hunt for it:
    ghsi config          Show settings + file location
    ghsi config model    View or change your AI model interactively
    ghsi config init     Create the settings file with defaults

EXAMPLES:
  ghsi                       Stage all, generate, confirm, commit, push
  ghsi -y                    Same, without the confirmation prompt
  ghsi --dry-run             Generate the message, commit nothing
  ghsi src/main.rs           Only stage/commit the listed files
  ghsi stage                 Stage + generate only (old `gs` alias)
  ghsi generate              Generate for already-staged changes
  ghsi commit --no-push      Commit locally without pushing
  ghsi config model gpt-5.6-luna   Switch models in one command

SHELL ALIASES (optional — ghsi is already short):
  alias gp=\"ghsi\"
  alias gs=\"ghsi stage\"";

#[derive(Parser, Debug)]
#[command(
    name = "ghsi",
    version,
    about = "Super Intelligent Git Commit — AI-powered commit messages and workflow automation",
    long_about = None,
    after_help = AFTER_HELP
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Files to stage and commit (default workflow; empty = all changes)
    pub files: Vec<String>,

    /// Skip the confirmation prompt
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Generate the message without committing or pushing
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Commit locally without pushing
    #[arg(long = "no-push")]
    pub no_push: bool,

    /// Open the generated message in $EDITOR before committing
    #[arg(long)]
    pub edit: bool,

    /// Branch to push to (defaults to the current branch)
    #[arg(short, long)]
    pub branch: Option<String>,

    /// Override the configured model for this run
    #[arg(long)]
    pub model: Option<String>,

    /// Override message level for this run: quiet, normal, or verbose
    #[arg(long)]
    pub level: Option<String>,

    /// (Legacy ghit flag) Stage, generate, commit, and push — the default
    #[arg(short, long)]
    pub auto: bool,

    /// (Legacy ghit flag) Only generate a message for staged changes
    #[arg(short, long)]
    pub generate: bool,

    /// (Legacy ghit flag) Stage files and generate (don't commit)
    #[arg(short = 's', long = "stage-and-generate")]
    pub stage_and_generate: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Stage, generate, commit, and push (default workflow) [alias: c]
    #[command(visible_alias = "c")]
    Commit(CommitArgs),

    /// Generate a message for already-staged changes [aliases: gen, g]
    #[command(visible_aliases = ["gen", "g"])]
    Generate(ModelArgs),

    /// Stage files (or all changes), then generate — no commit [alias: s]
    #[command(visible_alias = "s")]
    Stage(StageArgs),

    /// View and edit your settings TOML (model, provider, keys)
    Config(ConfigArgs),
}

#[derive(Args, Debug)]
pub struct CommitArgs {
    /// Files to stage and commit (empty = all changes)
    pub files: Vec<String>,

    /// Skip the confirmation prompt
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Generate the message without committing or pushing
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Commit locally without pushing
    #[arg(long = "no-push")]
    pub no_push: bool,

    /// Open the generated message in $EDITOR before committing
    #[arg(long)]
    pub edit: bool,

    /// Branch to push to (defaults to the current branch)
    #[arg(short, long)]
    pub branch: Option<String>,

    #[command(flatten)]
    pub model_args: ModelArgs,
}

#[derive(Args, Debug)]
pub struct StageArgs {
    /// Files to stage (empty = all changes)
    pub files: Vec<String>,

    #[command(flatten)]
    pub model_args: ModelArgs,
}

#[derive(Args, Debug, Clone)]
pub struct ModelArgs {
    /// Override the configured model for this run
    #[arg(long)]
    pub model: Option<String>,

    /// Override message level for this run: quiet, normal, or verbose
    #[arg(long)]
    pub level: Option<String>,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: Option<ConfigCommands>,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current settings and the settings file location
    Show,

    /// Print the settings TOML path (so you never have to hunt for it)
    Path,

    /// Create the settings file with defaults if it does not exist
    Init,

    /// Open the settings file in $EDITOR
    Edit,

    /// Show or change the AI model (interactive when run in a terminal)
    Model {
        /// New model id, e.g. `ghsi config model gpt-5.6-luna`.
        /// Omit to see the current model and pick interactively.
        model: Option<String>,

        /// Also switch provider: openai, anthropic, or openai-compatible
        #[arg(long)]
        provider: Option<String>,

        /// List suggested current models for the provider
        #[arg(long)]
        list: bool,
    },

    /// Set any config key: model, provider, message_level, api_key, base_url, max_completion_tokens
    Set {
        /// Key to set
        key: String,
        /// New value
        value: String,
    },
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match run_cli(cli).await {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("Error: {e}");
            Err(e)
        }
    }
}

async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    if let Some(command) = cli.command {
        return match command {
            Commands::Config(args) => run_config(args),
            Commands::Generate(model_args) => {
                let workflow = load_workflow(&model_args)?;
                workflow.generate_message_only().await?;
                Ok(())
            }
            Commands::Stage(args) => {
                let workflow = load_workflow(&args.model_args)?;
                workflow.stage_and_generate(args.files).await?;
                Ok(())
            }
            Commands::Commit(args) => {
                let workflow = load_workflow(&args.model_args)?;
                let opts = WorkflowOptions {
                    yes: args.yes,
                    dry_run: args.dry_run,
                    no_push: args.no_push,
                    branch: args.branch,
                    edit: args.edit,
                };
                workflow.commit_workflow(args.files, opts).await?;
                Ok(())
            }
        };
    }

    // Legacy ghit flags keep working: --generate / --stage-and-generate.
    // --auto is the default workflow, so it needs no special handling.
    let model_args = ModelArgs {
        model: cli.model.clone(),
        level: cli.level.clone(),
    };

    if cli.generate {
        let workflow = load_workflow(&model_args)?;
        workflow.generate_message_only().await?;
        return Ok(());
    }

    if cli.stage_and_generate && !cli.auto {
        let workflow = load_workflow(&model_args)?;
        workflow.stage_and_generate(cli.files.clone()).await?;
        return Ok(());
    }

    let workflow = load_workflow(&model_args)?;
    let opts = WorkflowOptions {
        yes: cli.yes,
        dry_run: cli.dry_run,
        no_push: cli.no_push,
        branch: cli.branch.clone(),
        edit: cli.edit,
    };
    workflow.commit_workflow(cli.files.clone(), opts).await?;
    Ok(())
}

fn load_workflow(model_args: &ModelArgs) -> anyhow::Result<Workflow> {
    let mut config = Config::load().map_err(|e| {
        anyhow::anyhow!(
            "{e}\nSettings file: {}\nRun `ghsi config` to inspect your settings, \
             or `ghsi config init` to create the file.",
            Config::settings_reference()
        )
    })?;
    apply_model_overrides(&mut config, model_args)?;
    // Fail fast with a settings-pointing error before doing git work.
    config.require_api_key()?;
    Ok(Workflow::new(config))
}

fn apply_model_overrides(config: &mut Config, args: &ModelArgs) -> anyhow::Result<()> {
    if let Some(model) = &args.model {
        if model.trim().is_empty() {
            anyhow::bail!("--model must not be empty");
        }
        config.model = model.trim().to_string();
    }
    if let Some(level) = &args.level {
        config.message_level = level
            .parse::<MessageLevel>()
            .map_err(|e: crate::SiError| anyhow::anyhow!(e.to_string()))?;
    }
    Ok(())
}

fn save_config(config: &Config) -> anyhow::Result<std::path::PathBuf> {
    // Save back to the file the user already has, if any, so a
    // hand-created ~/.si-git-commit.toml is updated in place.
    // Preserve the key stored in the file: a key picked up from the
    // environment (e.g. system-wide OPENAI_API_KEY) must not be copied
    // into the settings file just because the model changed.
    let mut to_save = config.clone();
    to_save.api_key = Config::stored_api_key().unwrap_or_default();
    if let Some(existing) = Config::find_existing_path() {
        to_save.save_to(&existing)?;
        Ok(existing)
    } else {
        Ok(to_save.save()?)
    }
}

/// Save exactly what is in `config`, including its API key.
/// Used only when the user explicitly sets `api_key`.
fn save_config_verbatim(config: &Config) -> anyhow::Result<std::path::PathBuf> {
    if let Some(existing) = Config::find_existing_path() {
        config.save_to(&existing)?;
        Ok(existing)
    } else {
        Ok(config.save()?)
    }
}

fn run_config(args: ConfigArgs) -> anyhow::Result<()> {
    match args.command {
        None | Some(ConfigCommands::Show) => config_show(),
        Some(ConfigCommands::Path) => {
            println!("{}", Config::settings_reference());
            if Config::find_existing_path().is_none() {
                println!("(file does not exist yet — create it with `ghsi config init`)");
            }
            Ok(())
        }
        Some(ConfigCommands::Init) => config_init(),
        Some(ConfigCommands::Edit) => config_edit(),
        Some(ConfigCommands::Model {
            model,
            provider,
            list,
        }) => config_model(model, provider, list),
        Some(ConfigCommands::Set { key, value }) => config_set(&key, &value),
    }
}

fn key_tail(key: &str) -> String {
    let key = key.trim();
    if key.len() > 4 {
        format!("…{}", &key[key.len() - 4..])
    } else {
        "set".to_string()
    }
}

/// One-line, source-aware API key status for `ghsi config`.
/// A system-wide key (e.g. `OPENAI_API_KEY` exported from your shell
/// profile) is used automatically — say so, and explain the override.
fn api_key_line(config: &Config) -> String {
    let stored = Config::stored_api_key();
    let env_found = config.env_api_key();
    match (stored, env_found) {
        (Some(key), Some((name, _))) => format!(
            "set in settings file ({}) — overrides {name} from your environment",
            key_tail(&key)
        ),
        (Some(key), None) => format!("set in settings file ({})", key_tail(&key)),
        (None, Some((name, value))) => format!(
            "✓ {name} found in environment ({}) — used automatically; \
             to use a different key just for ghsi, run `ghsi config set api_key <key>`",
            key_tail(&value)
        ),
        (None, None) => format!(
            "not set — export {}=<your-key> or run `ghsi config set api_key <key>`",
            config.provider.env_key_name()
        ),
    }
}

fn config_show() -> anyhow::Result<()> {
    let config = Config::load_or_default();
    let existing = Config::find_existing_path();
    println!("⚙️  si-git-commit (ghsi) settings");
    println!("  Settings file: {}", Config::settings_reference());
    match &existing {
        Some(p) => println!("  Status:        found ({})", p.display()),
        None => println!("  Status:        not created yet — run `ghsi config init`"),
    }
    println!("  Provider:      {}", config.provider.as_str());
    println!("  Model:         {}", config.model);
    println!("  Message level: {}", config.message_level.as_str());
    println!(
        "  Token budget:  {} (sent as max_completion_tokens to OpenAI, max_tokens to Anthropic)",
        config.max_completion_tokens
    );
    println!(
        "  Base URL:      {}",
        config.base_url.as_deref().unwrap_or("(provider default)")
    );
    println!("  API key:       {}", api_key_line(&config));
    println!();
    println!("Change the model without editing this file:");
    println!("  ghsi config model <name>     e.g. ghsi config model gpt-5.6-luna");
    println!("  ghsi config model            interactive picker (in a terminal)");
    println!("  ghsi config model --list     suggested current models");
    println!("Open the file directly: ghsi config edit   (path: ghsi config path)");
    Ok(())
}

fn config_init() -> anyhow::Result<()> {
    let (path, created) = Config::init_if_missing()?;
    if created {
        println!("✓ Created settings file: {}", path.display());
    } else {
        println!("Settings file already exists: {}", path.display());
    }
    println!();
    println!("Next steps:");
    let config = Config::load_or_default();
    match config.env_api_key() {
        Some((name, _)) => {
            println!(
                "  1. ✓ {name} found in your environment — used automatically, \
                 no setup needed."
            );
            println!(
                "     (Optional: use a different key just for ghsi with \
                 `ghsi config set api_key <key>` — it overrides the environment.)"
            );
        }
        None => {
            println!(
                "  1. Add your API key: export {}=<your-key>  \
                 (or `ghsi config set api_key <key>`)",
                config.provider.env_key_name()
            );
        }
    }
    println!("  2. Pick a model:     ghsi config model");
    println!("  3. Commit with AI:   ghsi");
    Ok(())
}

fn config_edit() -> anyhow::Result<()> {
    let (path, _) = Config::init_if_missing()?;
    println!("Opening {} …", path.display());
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
    let status = std::process::Command::new(&editor).arg(&path).status()?;
    if !status.success() {
        anyhow::bail!("editor `{editor}` exited with {status}");
    }
    // Validate the edited file still parses.
    Config::load_from(&path)?;
    println!("✓ Settings saved and valid: {}", path.display());
    Ok(())
}

fn config_model(model: Option<String>, provider: Option<String>, list: bool) -> anyhow::Result<()> {
    let mut config = Config::load_or_default();

    if let Some(p) = &provider {
        config.provider = p
            .parse::<Provider>()
            .map_err(|e: crate::SiError| anyhow::anyhow!(e.to_string()))?;
    }

    if list {
        println!(
            "Suggested current models for provider `{}`:",
            config.provider.as_str()
        );
        for m in suggested_models(config.provider) {
            let marker = if *m == config.model { " (current)" } else { "" };
            println!("  - {m}{marker}");
        }
        println!();
        println!("Any current model id works — set it with: ghsi config model <name>");
        println!("Settings file: {}", Config::settings_reference());
        return Ok(());
    }

    if let Some(new_model) = model {
        let new_model = new_model.trim().to_string();
        if new_model.is_empty() {
            anyhow::bail!("model must not be empty");
        }
        let old = config.model.clone();
        config.model = new_model;
        let path = save_config(&config)?;
        println!("✓ Model changed: {old} → {}", config.model);
        println!("  Provider: {}", config.provider.as_str());
        println!("  Settings file: {}", path.display());
        return Ok(());
    }

    // No explicit model: show current, then offer to change interactively.
    println!("Current model:   {}", config.model);
    println!("Provider:        {}", config.provider.as_str());
    println!("Settings file:   {}", Config::settings_reference());
    println!();

    if !std::io::stdin().is_terminal() {
        println!("To change it: ghsi config model <name>   (e.g. ghsi config model gpt-5.6-luna)");
        println!("Suggestions:   ghsi config model --list");
        return Ok(());
    }

    let suggestions = suggested_models(config.provider);
    println!("Suggested current models:");
    for (i, m) in suggestions.iter().enumerate() {
        let marker = if *m == config.model { " (current)" } else { "" };
        println!("  {}. {m}{marker}", i + 1);
    }
    println!();
    print!(
        "Enter a number, a model id, or press Enter to keep \"{}\": ",
        config.model
    );
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        println!("Kept model: {}", config.model);
        return Ok(());
    }

    let chosen = if let Ok(n) = input.parse::<usize>() {
        if n >= 1 && n <= suggestions.len() {
            suggestions[n - 1].to_string()
        } else {
            anyhow::bail!("choice {n} is out of range (1-{})", suggestions.len());
        }
    } else {
        input.to_string()
    };

    if chosen == config.model && provider.is_none() {
        println!("Kept model: {}", config.model);
        return Ok(());
    }

    config.model = chosen;
    let path = save_config(&config)?;
    println!("✓ Model changed to: {}", config.model);
    println!("  Provider: {}", config.provider.as_str());
    println!("  Settings file: {}", path.display());
    Ok(())
}

fn config_set(key: &str, value: &str) -> anyhow::Result<()> {
    let mut config = Config::load_or_default();
    let value = value.trim().to_string();
    match key {
        "model" => {
            if value.is_empty() {
                anyhow::bail!("model must not be empty");
            }
            config.model = value;
        }
        "provider" => {
            config.provider = value
                .parse::<Provider>()
                .map_err(|e: crate::SiError| anyhow::anyhow!(e.to_string()))?;
        }
        "message_level" | "level" => {
            config.message_level = value
                .parse::<MessageLevel>()
                .map_err(|e: crate::SiError| anyhow::anyhow!(e.to_string()))?;
        }
        "api_key" => config.api_key = value,
        "base_url" => {
            config.base_url = if value.is_empty() { None } else { Some(value) };
        }
        "max_completion_tokens" | "max_tokens" => {
            config.max_completion_tokens = value
                .parse::<u32>()
                .map_err(|_| anyhow::anyhow!("max_completion_tokens must be a positive integer"))?;
            if config.max_completion_tokens == 0 {
                anyhow::bail!("max_completion_tokens must be greater than 0");
            }
        }
        other => {
            anyhow::bail!(
                "unknown config key `{other}`. Valid keys: model, provider, message_level, \
                 api_key, base_url, max_completion_tokens. Run `ghsi config` to see your settings \
                 (file: {}).",
                Config::settings_reference()
            );
        }
    }
    let path = if key == "api_key" {
        save_config_verbatim(&config)?
    } else {
        save_config(&config)?
    };
    println!("✓ Set {key} in {}", path.display());
    config_show()?;
    Ok(())
}
