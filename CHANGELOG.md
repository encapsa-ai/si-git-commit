# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dynamic API-key instructions: `ghsi config init` and `ghsi config`
  now detect a system-wide `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` (or
  `SI_GIT_COMMIT_API_KEY`) and report "✓ … found in environment — used
  automatically", with an optional `ghsi config set api_key` override
  note. Without an environment key they show the export instruction.

### Fixed

- Changing the model or other settings no longer copies an
  environment-sourced API key into the settings TOML; only an explicit
  `ghsi config set api_key <key>` writes a key to the file.

## [0.1.0] - 2026-10-26

Initial release. A modern replacement for the orphaned `ghit`/`helix`
AI commit tool, rebuilt and maintained by Encapsa AI.

### Added

- `ghsi` CLI (GitHub Super Intelligence) with a streamlined workflow:
  `ghsi` stages everything, generates a message, confirms, commits, and
  pushes. `ghsi stage` covers the old `gs` alias; `ghsi generate` covers
  already-staged changes. Legacy `ghit` flags (`--auto`, `--generate`,
  `--stage-and-generate`, `-a/-g/-s`) keep working.
- Modern provider parameters: OpenAI requests send
  `max_completion_tokens` and omit `temperature`, so current models
  (GPT-5 family, including `gpt-5.6-luna`) no longer fail with
  `Unsupported parameter: 'max_tokens'`. Anthropic uses its current
  `max_tokens` parameter. No deprecated model ids anywhere.
- Providers: OpenAI, Anthropic, and any OpenAI-compatible endpoint
  (`provider = "openai-compatible"` + `base_url`).
- `ghsi config model [name]` — view the current model, pick from
  suggestions interactively, or set a new model in one command instead
  of hunting down and hand-editing the settings TOML.
- Settings discoverability: `ghsi config` shows every setting plus the
  settings file location, `ghsi config path` prints the path,
  `ghsi config init` creates the file, and `ghsi config edit` opens it
  in `$EDITOR`. Every config error references the file path.
- Workflow extras: `-y/--yes`, `--dry-run`, `--no-push`, `--edit`,
  `-b/--branch`, per-run `--model` / `--level` overrides.
- Library API (`Config`, `Workflow`, `Llm`, prompt/diff helpers) so the
  commit-generation pipeline can be reused outside the CLI.

### Fixed (relative to the orphaned tool)

- Untracked files are now detected and staged (`git status --porcelain`
  instead of `git diff --quiet`, which missed untracked files).
- Diff truncation is UTF-8 boundary safe (previously sliced at a byte
  offset and could panic on multi-byte characters) and uses a larger
  12k budget with file stats included in the prompt.
