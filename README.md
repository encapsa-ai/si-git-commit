# Super Intelligent Git Commits

<img width="3840" height="2160" alt="super-intelligent-git-commits-optimized" src="https://github.com/user-attachments/assets/2150f5c1-c665-4d41-98b4-93a784f8e9af" />

[![Crates.io](https://img.shields.io/crates/v/si-git-commit.svg)](https://crates.io/crates/si-git-commit)
[![Documentation](https://docs.rs/si-git-commit/badge.svg)](https://docs.rs/si-git-commit)
[![License](https://img.shields.io/crates/l/si-git-commit.svg)](LICENSE-MIT)

**AI-powered git commit messages and workflow automation.** The `ghsi` CLI
analyzes your staged changes with a current AI model and writes a
[Conventional Commits](https://www.conventionalcommits.org/) message —
then stages, commits, and pushes in one streamlined command.

`si-git-commit` is the maintained successor to the orphaned `ghit`/`helix`
tool. It keeps the same high-level workflow, fixes the breakage that killed
the original (deprecated model ids, legacy `max_tokens` / `temperature`
parameters that current models reject), and adds the quality-of-life
tooling the original never had — above all, changing your model with
`ghsi config model` instead of hunting down a dotfile to edit by hand.

## Features

- **One-command workflow**: `ghsi` stages everything, generates a message,
  asks for confirmation, commits, and pushes.
- **Modern model support**: OpenAI requests use `max_completion_tokens`
  and the model's default temperature, so GPT-5-class models
  (`gpt-5-mini`, `gpt-5.6-luna`, `gpt-5`) work out of the box — no
  `Unsupported parameter: 'max_tokens'` errors, no deprecated defaults.
- **Multiple providers**: OpenAI, Anthropic, and any OpenAI-compatible
  endpoint (OpenRouter, local servers, internal gateways).
- **Painless configuration**: `ghsi config model` shows your current
  model and lets you keep or change it interactively; `ghsi config path`
  always tells you where the settings TOML lives.
- **Streamlined CLI**: short commands replace the old shell-alias
  workarounds, while legacy `ghit`-style flags keep working.
- **Message levels**: `quiet` (one line), `normal` (subject + bullets),
  or `verbose` (detailed body).

## Installation

```bash
cargo install si-git-commit
```

This installs the `ghsi` binary (plus a `si-git-commit` alias binary).
Prebuilt binaries for Linux, macOS, and Windows are attached to each
[GitHub release](https://github.com/encapsa-ai/si-git-commit/releases).

Or build from source:

```bash
git clone https://github.com/encapsa-ai/si-git-commit.git
cd si-git-commit
cargo install --path .
```

## Quick start

```bash
# 1. Create your settings file (ghsi prints exactly where it lives)
ghsi config init

# 2. API key: if OPENAI_API_KEY is already exported (e.g. from ~/.zshrc),
#    ghsi finds it automatically — `ghsi config init` will show
#    "✓ OPENAI_API_KEY found" and there is nothing to do.
#    Otherwise export it, or set a ghsi-specific key that overrides
#    the environment with: ghsi config set api_key <your-key>
export OPENAI_API_KEY=<your-key>

# 3. Pick a model — interactively, or directly:
ghsi config model                 # shows current model, offers suggestions
ghsi config model gpt-5.6-luna    # switch in one command

# 4. Make some changes, then:
ghsi        # stage all → generate → confirm → commit → push
```

## Usage

| Command | What it does |
|---|---|
| `ghsi` | Stage all changes, generate, confirm, commit, push (default) |
| `ghsi -y` | Same as above without the confirmation prompt |
| `ghsi --dry-run` | Generate and print the message; commit nothing |
| `ghsi src/app.rs README.md` | Only stage/commit the listed files |
| `ghsi commit --no-push` | Commit locally without pushing |
| `ghsi commit --edit` | Edit the generated message in `$EDITOR` first |
| `ghsi stage` | Stage (all or listed files) + generate, no commit |
| `ghsi generate` | Generate a message for already-staged changes |
| `ghsi config` | Show all settings + settings file location |
| `ghsi config model [name]` | Show, pick, or set the AI model |
| `ghsi config path` | Print the settings TOML path |

Subcommand shortcuts: `ghsi c` (commit), `ghsi s` (stage),
`ghsi gen` / `ghsi g` (generate).

Useful flags on the commit workflow: `-b/--branch <name>`,
`--model <id>` and `--level quiet|normal|verbose` (per-run overrides),
`--no-push`, `--dry-run`, `-y/--yes`, `--edit`.

### Coming from `ghit`? Your aliases still work

The old workflow needed shell aliases because the flags were convoluted:

```bash
alias gp="ghit --stage-and-generate --auto"   # old
alias gs="ghit --stage-and-generate"          # old
```

With `ghsi` the defaults do what those aliases meant, so the aliases
collapse to:

```bash
alias gp="ghsi"         # stage all, generate, commit, push
alias gs="ghsi stage"   # stage + generate only
```

Legacy flags are also accepted directly: `ghsi --auto`,
`ghsi --generate`, and `ghsi --stage-and-generate` (`-a/-g/-s`)
behave exactly as they did in `ghit`.

## Configuration

Run `ghsi config path` to see where your settings file lives. By default
it is `<config dir>/si-git-commit/config.toml` (for example
`~/.config/si-git-commit/config.toml` on Linux or
`~/Library/Application Support/si-git-commit/config.toml` on macOS).
Set `$SI_GIT_COMMIT_CONFIG` to use a different path. A hand-created
`~/.si-git-commit.toml` is also discovered for convenience.

You rarely need to open the file: `ghsi config` shows everything in it,
`ghsi config model` changes the model, `ghsi config set <key> <value>`
changes anything else, and `ghsi config edit` opens it in `$EDITOR`
when you do want the file.

Example `config.toml`:

```toml
provider = "openai"            # openai | anthropic | openai-compatible
model = "gpt-5-mini"           # any current model id
api_key = ""                   # prefer OPENAI_API_KEY / ANTHROPIC_API_KEY env vars
message_level = "normal"       # quiet | normal | verbose
max_completion_tokens = 800
# base_url = "https://api.openai.com/v1"   # for openai-compatible endpoints
```

Environment overrides: `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`,
`SI_GIT_COMMIT_API_KEY`, `SI_GIT_COMMIT_MODEL`, `SI_GIT_COMMIT_PROVIDER`.

### Message levels

- **`quiet`**: one-line subject only — `fix: handle empty diff`
- **`normal`** (default): subject plus a short bullet body
- **`verbose`**: subject plus a detailed explanatory body

### A note on model parameters

Current OpenAI models reject the legacy `max_tokens` field and custom
`temperature` values. `ghsi` sends `max_completion_tokens` and lets each
model use its default temperature, which is accepted by both current
models and the late GPT-4.1 family. Anthropic requests use Anthropic's
current `max_tokens` parameter. Only current-generation models are
suggested by `ghsi config model --list`, so you will never be pointed
at a deprecated id.

## Library usage

The generation pipeline is also a library:

```rust
use si_git_commit::{Config, Workflow, WorkflowOptions};

#[tokio::main]
async fn main() -> si_git_commit::Result<()> {
    let workflow = Workflow::new(Config::load()?);
    workflow
        .commit_workflow(vec![], WorkflowOptions::default())
        .await
}
```

See `cargo run --example basic_usage` for a network-free tour of the
prompt building and response parsing helpers.

## Troubleshooting

- **"no config file found"** — run `ghsi config init`. The error always
  prints the expected settings file path; `ghsi config path` shows it too.
- **"no API key configured"** — export `OPENAI_API_KEY` (or
  `ANTHROPIC_API_KEY`), or set `api_key` via `ghsi config set api_key …`.
- **Model errors (deprecated / not found)** — your model id is gone or
  mistyped. Run `ghsi config model --list` and switch with
  `ghsi config model <name>`.
- **"No changes found to commit"** — check `git status`; `ghsi` stages
  everything (including untracked files) by default, so this usually
  means the tree is genuinely clean.

## License

Licensed under either of [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE) at your option.

