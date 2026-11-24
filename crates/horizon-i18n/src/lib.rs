use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use horizon_core::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocaleMeta {
    pub code: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocaleBundle {
    pub meta: LocaleMeta,
    pub gui: BTreeMap<String, String>,
    pub mods: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateModEntryRequest {
    pub locale: String,
    pub mod_id: String,
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncReport {
    pub active_locale: String,
    pub synced_mods: Vec<String>,
    pub written_files: u32,
}

pub struct I18nStore {
    locales_dir: PathBuf,
    supported: Vec<String>,
}

impl I18nStore {
    pub fn new(locales_dir: PathBuf, supported: Vec<String>) -> Self {
        Self {
            locales_dir,
            supported,
        }
    }

    pub fn supported_locales(&self) -> &[String] {
        &self.supported
    }

    pub fn load(&self, locale: &str) -> CoreResult<LocaleBundle> {
        self.validate_locale(locale)?;
        let path = self.locales_dir.join(format!("{locale}.toml"));
        let raw = fs::read_to_string(&path).map_err(|source| CoreError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let bundle: LocaleBundle = toml::from_str(&raw)?;
        Ok(bundle)
    }

    pub fn update_mod_entry(&self, req: &UpdateModEntryRequest) -> CoreResult<LocaleBundle> {
        let mut bundle = self.load(&req.locale)?;
        let table = bundle.mods.entry(req.mod_id.clone()).or_default();
        table.insert(req.key.clone(), req.value.clone());
        self.save(&req.locale, &bundle)?;
        Ok(bundle)
    }

    pub fn sync_to_horizon(&self, horizon_dir: &Path, active_locale: &str) -> CoreResult<SyncReport> {
        let active_bundle = self.load(active_locale)?;
        let mod_data = horizon_dir
            .join("horizon-mods")
            .join(".horizon-mod-data")
            .join("lilyco_i18n");
        fs::create_dir_all(&mod_data).map_err(|source| CoreError::Io {
            path: mod_data.display().to_string(),
            source,
        })?;

        let serialized = toml::to_string_pretty(&active_bundle)
            .map_err(|e| CoreError::Validation(e.to_string()))?;
        let locale_toml = mod_data.join("locale.toml");
        fs::write(&locale_toml, serialized).map_err(|source| CoreError::Io {
            path: locale_toml.display().to_string(),
            source,
        })?;
        let active_file = mod_data.join("active_locale");
        fs::write(&active_file, active_locale).map_err(|source| CoreError::Io {
            path: active_file.display().to_string(),
            source,
        })?;

        let mut written_files: u32 = 2;
        let mut synced_mods = Vec::new();
        for locale in &self.supported {
            let bundle = self.load(locale)?;
            let mc_lang = to_mc_lang_code(locale);
            for (mod_id, entries) in &bundle.mods {
                if !synced_mods.contains(mod_id) {
                    synced_mods.push(mod_id.clone());
                }
                let lang_dir = horizon_dir
                    .join("horizon-native-mods")
                    .join(mod_id)
                    .join("assets")
                    .join(mod_id)
                    .join("lang");
                fs::create_dir_all(&lang_dir).map_err(|source| CoreError::Io {
                    path: lang_dir.display().to_string(),
                    source,
                })?;
                let json_path = lang_dir.join(format!("{mc_lang}.json"));
                let body = serde_json::to_string_pretty(entries)?;
                fs::write(&json_path, body).map_err(|source| CoreError::Io {
                    path: json_path.display().to_string(),
                    source,
                })?;
                written_files += 1;
            }
        }
        Ok(SyncReport {
            active_locale: active_locale.to_string(),
            synced_mods,
            written_files,
        })
    }

    fn save(&self, locale: &str, bundle: &LocaleBundle) -> CoreResult<()> {
        self.validate_locale(locale)?;
        let path = self.locales_dir.join(format!("{locale}.toml"));
        let serialized = toml::to_string_pretty(bundle)
            .map_err(|e| CoreError::Validation(e.to_string()))?;
        fs::write(&path, serialized).map_err(|source| CoreError::Io {
            path: path.display().to_string(),
            source,
        })
    }

    fn validate_locale(&self, locale: &str) -> CoreResult<()> {
        if !self.supported.iter().any(|l| l == locale) {
            return Err(CoreError::Validation(format!("unsupported locale: {locale}")));
        }
        Ok(())
    }
}

fn to_mc_lang_code(locale: &str) -> String {
    locale.replace('-', "_").to_lowercase()
}
