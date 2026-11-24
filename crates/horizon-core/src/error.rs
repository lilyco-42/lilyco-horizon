use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("validation error: {0}")]
    Validation(String),
}

pub type CoreResult<T> = Result<T, CoreError>;
