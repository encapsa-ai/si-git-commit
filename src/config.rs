use crate::error::{Result, SiError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{env, fs};

/// AI provider used to generate commit messages.
///
/// Requests are built with each provider's *current* parameter names.
/// In particular, OpenAI requests use `max_completion_tokens` (never the
/// legacy `max_tokens`) and omit `temperature`, so current reasoning and
/// GPT-5-class models accept them without `unsupported_parameter` errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    Openai,
    Anthropic,
    /// Any OpenAI-compatible Chat Completions endpoint (custom `base_url`),
    /// e.g. OpenRouter, a local server, or an internal gateway.
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Openai => "openai",
            Provider::Anthropic => "anthropic",
            Provider::OpenAiCompatible => "openai-compatible",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Openai => "OpenAI",
            Provider::Anthropic => "Anthropic",
            Provider::OpenAiCompatible => "OpenAI-compatible",
        }
    }

    pub fn env_key_name(&self) -> &'static str {
        match self {
            Provider::Openai | Provider::OpenAiCompatible => "OPENAI_API_KEY",
            Provider::Anthropic => "ANTHROPIC_API_KEY",
        }
    }

    pub fn default_base_url(&self) -> &'static str {
        match self {
            Provider::Openai | Provider::OpenAiCompatible => "https://api.openai.com/v1",
            Provider::Anthropic => "https://api.anthropic.com/v1",
        }
    }
}

impl std::str::FromStr for Provider {
    type Err = SiError;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_lowercase().as_str() {
            "openai" => Ok(Provider::Openai),
            "anthropic" | "claude" => Ok(Provider::Anthropic),
            "openai-compatible" | "compatible" | "custom" => Ok(Provider::OpenAiCompatible),
            other => Err(SiError::Config(format!(
                "unknown provider `{other}` (expected: openai, anthropic, openai-compatible)"
            ))),
        }
    }
}

/// Commit message verbosity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MessageLevel {
    /// One-line subject only.
    Quiet,
    /// Subject + short body (default).
    #[default]
    Normal,
    /// Detailed subject + explanatory body.
    Verbose,
}

impl MessageLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageLevel::Quiet => "quiet",
            MessageLevel::Normal => "normal",
            MessageLevel::Verbose => "verbose",
        }
    }
}

impl std::str::FromStr for MessageLevel {
    type Err = SiError;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_lowercase().as_str() {
            "quiet" => Ok(MessageLevel::Quiet),
            "normal" => Ok(MessageLevel::Normal),
            "verbose" => Ok(MessageLevel::Verbose),
            other => Err(SiError::Config(format!(
                "unknown message_level `{other}` (expected: quiet, normal, verbose)"
            ))),
        }
    }
}

/// Modern default model. Small, fast, and cheap — ideal for commit messages.
/// Change it any time with `ghsi config model <name>`; no file hunting needed.
pub const DEFAULT_MODEL: &str = "gpt-5-mini";

/// Suggested current models shown by `ghsi config model --list`.
/// Only current-generation models are listed — deprecated ids are omitted
/// on purpose so new users never copy a model that is about to disappear.
pub fn suggested_models(provider: Provider) -> &'static [&'static str] {
    match provider {
        Provider::Openai | Provider::OpenAiCompatible => {
            &["gpt-5-mini", "gpt-5.6-luna", "gpt-5", "gpt-4.1-mini"]
        }
        Provider::Anthropic => &["claude-sonnet-4-6", "claude-haiku-4-5"],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub provider: Provider,

    #[serde(default = "default_model")]
    pub model: String,

    /// API key. Prefer the provider env var (`OPENAI_API_KEY` /
    /// `ANTHROPIC_API_KEY`) and leave this empty in the file.
    #[serde(default)]
    pub api_key: String,

    #[serde(default)]
    pub message_level: MessageLevel,

    /// Token budget for the generated message. Sent to OpenAI as
    /// `max_completion_tokens` and to Anthropic as `max_tokens`
    /// (each provider's current parameter name).
    #[serde(default = "default_max_completion_tokens")]
    pub max_completion_tokens: u32,

    /// Optional API base URL override (no trailing slash needed).
    /// Useful for OpenAI-compatible endpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}

fn default_max_completion_tokens() -> u32 {
    800
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: Provider::default(),
            model: default_model(),
            api_key: String::new(),
            message_level: MessageLevel::default(),
            max_completion_tokens: default_max_completion_tokens(),
            base_url: None,
        }
    }
}

impl Config {
    /// Canonical config file path (used for creation and display).
    ///
    /// Resolution: `$SI_GIT_COMMIT_CONFIG`, otherwise
    /// `<config_dir>/si-git-commit/config.toml` (e.g.
    /// `~/.config/si-git-commit/config.toml` on Linux,
    /// `~/Library/Application Support/si-git-commit/config.toml` on macOS).
    pub fn config_path() -> PathBuf {
        if let Ok(p) = env::var("SI_GIT_COMMIT_CONFIG") {
            if !p.trim().is_empty() {
                return PathBuf::from(p);
            }
        }
        let base = dirs::config_dir().unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        });
        base.join("si-git-commit").join("config.toml")
    }

    /// Candidate paths searched when loading, in priority order.
    /// Includes the legacy-style `~/.si-git-commit.toml` so a hand-created
    /// file in the home directory is still discovered.
    pub fn candidate_paths() -> Vec<PathBuf> {
        let mut paths = vec![Self::config_path()];
        if let Some(home) = dirs::home_dir() {
            let legacy = home.join(".si-git-commit.toml");
            if !paths.contains(&legacy) {
                paths.push(legacy);
            }
            let xdg_style = home
                .join(".config")
                .join("si-git-commit")
                .join("config.toml");
            if !paths.contains(&xdg_style) {
                paths.push(xdg_style);
            }
        }
        paths
    }

    pub fn find_existing_path() -> Option<PathBuf> {
        Self::candidate_paths().into_iter().find(|p| p.exists())
    }

    /// Human-readable reference to the settings file, for help and errors.
    pub fn settings_reference() -> String {
        match Self::find_existing_path() {
            Some(p) => p.display().to_string(),
            None => Self::config_path().display().to_string(),
        }
    }

    pub fn load() -> Result<Self> {
        let path = Self::find_existing_path().ok_or_else(|| {
            SiError::Config(format!(
                "no config file found. Expected settings TOML at: {}\n\
                 Run `ghsi config init` to create one, then `ghsi config model` to pick a model.\n\
                 Tip: run `ghsi config path` any time to see this location.",
                Self::config_path().display()
            ))
        })?;
        Self::load_from(&path)
    }

    /// Load if a config file exists, otherwise return defaults.
    /// Used by `ghsi config` display commands so they work before `init`.
    pub fn load_or_default() -> Self {
        match Self::find_existing_path() {
            Some(p) => Self::load_from(&p).unwrap_or_default(),
            None => Self::default(),
        }
    }

    pub fn load_from(path: &PathBuf) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .map_err(|e| SiError::Config(format!("failed to read {}: {e}", path.display())))?;
        let mut cfg: Config = toml::from_str(&raw)?;
        cfg.apply_env_overrides();
        Ok(cfg)
    }

    /// Environment overrides: `SI_GIT_COMMIT_MODEL`,
    /// `SI_GIT_COMMIT_PROVIDER`, and the provider API-key env vars.
    pub fn apply_env_overrides(&mut self) {
        if let Ok(m) = env::var("SI_GIT_COMMIT_MODEL") {
            if !m.trim().is_empty() {
                self.model = m.trim().to_string();
            }
        }
        if let Ok(p) = env::var("SI_GIT_COMMIT_PROVIDER") {
            if let Ok(parsed) = p.parse::<Provider>() {
                self.provider = parsed;
            }
        }
        if self.api_key.trim().is_empty() {
            if let Ok(k) = env::var("SI_GIT_COMMIT_API_KEY") {
                if !k.trim().is_empty() {
                    self.api_key = k;
                    return;
                }
            }
            if let Ok(k) = env::var(self.provider.env_key_name()) {
                if !k.trim().is_empty() {
                    self.api_key = k;
                }
            }
        }
    }

    /// Resolve the effective API key or return an error that points at
    /// both the settings file and the env var.
    pub fn require_api_key(&self) -> Result<String> {
        let key = self.api_key.trim().to_string();
        if !key.is_empty() {
            return Ok(key);
        }
        Err(SiError::Config(format!(
            "no API key configured for provider `{}`.\n\
             Settings file: {}\n\
             Set `api_key` in that file (see `ghsi config`) or export {}=<your-key>.\n\
             Change the model with `ghsi config model <name>` — no manual file hunting needed.",
            self.provider.as_str(),
            Self::settings_reference(),
            self.provider.env_key_name()
        )))
    }

    pub fn effective_base_url(&self) -> String {
        match &self.base_url {
            Some(u) if !u.trim().is_empty() => u.trim().trim_end_matches('/').to_string(),
            _ => self.provider.default_base_url().to_string(),
        }
    }

    pub fn save(&self) -> Result<PathBuf> {
        let path = Self::config_path();
        self.save_to(&path)?;
        Ok(path)
    }

    pub fn save_to(&self, path: &PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = toml::to_string_pretty(self)?;
        let header =
            "# si-git-commit (ghsi) settings — https://github.com/encapsa-ai/si-git-commit\n\
                      # Change the model without editing this file: `ghsi config model <name>`\n\
                      # Show this file's location any time: `ghsi config path`\n\n";
        fs::write(path, format!("{header}{body}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Create the config file with defaults if it does not exist.
    /// Returns `(path, created)`.
    pub fn init_if_missing() -> Result<(PathBuf, bool)> {
        if let Some(existing) = Self::find_existing_path() {
            return Ok((existing, false));
        }
        let cfg = Self::default();
        let path = cfg.save()?;
        Ok((path, true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_uses_modern_model() {
        let cfg = Config::default();
        assert_eq!(cfg.model, DEFAULT_MODEL);
        assert_eq!(cfg.provider, Provider::Openai);
        assert_eq!(cfg.max_completion_tokens, 800);
    }

    #[test]
    fn parses_minimal_toml_with_defaults() {
        let cfg: Config = toml::from_str("model = \"gpt-5.6-luna\"\napi_key = \"sk-test\"\n")
            .expect("toml should parse");
        assert_eq!(cfg.model, "gpt-5.6-luna");
        assert_eq!(cfg.provider, Provider::Openai);
        assert_eq!(cfg.message_level, MessageLevel::Normal);
    }

    #[test]
    fn parses_full_toml() {
        let raw = r#"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "sk-ant-test"
message_level = "verbose"
max_completion_tokens = 1200
"#;
        let cfg: Config = toml::from_str(raw).expect("toml should parse");
        assert_eq!(cfg.provider, Provider::Anthropic);
        assert_eq!(cfg.message_level, MessageLevel::Verbose);
        assert_eq!(cfg.max_completion_tokens, 1200);
    }

    #[test]
    fn provider_from_str_accepts_aliases() {
        assert_eq!("openai".parse::<Provider>().unwrap(), Provider::Openai);
        assert_eq!("claude".parse::<Provider>().unwrap(), Provider::Anthropic);
        assert!("nope".parse::<Provider>().is_err());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ghsi-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let cfg = Config {
            model: "gpt-5.6-luna".to_string(),
            ..Config::default()
        };
        cfg.save_to(&path).expect("save should succeed");
        let loaded = Config::load_from(&path).expect("load should succeed");
        assert_eq!(loaded.model, "gpt-5.6-luna");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
