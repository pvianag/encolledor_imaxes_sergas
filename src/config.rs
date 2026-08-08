use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE_NAME: &str = "imaxes_diag_shrinker.cfg";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub language: String,
    pub last_input_dir: String,
    pub ask_delete_original: bool,
    pub remember_delete_choice: bool,
    pub default_delete_original: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: "gl".to_string(),
            last_input_dir: String::new(),
            ask_delete_original: true,
            remember_delete_choice: false,
            default_delete_original: false,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        if let Some(dir) = dirs::config_dir() {
            return dir.join(CONFIG_FILE_NAME);
        }
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CONFIG_FILE_NAME)
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match Self::load_from(&path) {
            Ok(cfg) => cfg,
            Err(_) => Self::default(),
        }
    }

    fn load_from(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let cfg: Self = toml::from_str(&text).context("parse config")?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let text = toml::to_string_pretty(self).context("serialize config")?;
        fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }
}
