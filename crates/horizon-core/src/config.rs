use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudioConfig {
    pub server: ServerConfig,
    pub paths: PathsConfig,
    pub i18n: I18nConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub cors_origin: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathsConfig {
    pub horizon_dir: PathBuf,
    pub locales_dir: PathBuf,
    pub extra_ysm_dir: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct I18nConfig {
    pub default_locale: String,
    pub supported: Vec<String>,
}

impl StudioConfig {
    pub fn load(path: &Path) -> CoreResult<Self> {
        let raw = fs::read_to_string(path).map_err(|source| CoreError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let cfg: Self = toml::from_str(&raw)?;
        Ok(cfg)
    }
}
