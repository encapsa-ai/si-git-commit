use thiserror::Error;

pub type Result<T> = std::result::Result<T, SiError>;

#[derive(Error, Debug)]
pub enum SiError {
    #[error("config error: {0}")]
    Config(String),

    #[error("git error: {0}")]
    Git(String),

    #[error("{provider} API error: {message}")]
    Api { provider: String, message: String },

    #[error("no staged changes found. Stage files first (e.g. `ghsi stage` or `git add`)")]
    NoStagedChanges,

    #[error("no changes found to commit. Make sure you have modified files in your repository")]
    NoChanges,

    #[error("not inside a git repository")]
    NotARepo,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse config: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("failed to serialize config: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for SiError {
    fn from(err: anyhow::Error) -> Self {
        SiError::Other(err.to_string())
    }
}
