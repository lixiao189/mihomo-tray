use std::fs;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::i18n;
use crate::paths;

pub const SETTINGS_FILE: &str = "settings.json";

/// In-memory app preferences with automatic persistence to `settings.json`.
///
/// Prefer `set_*` / `set_switches` over mutating fields: every successful setter
/// updates memory and writes the full document to disk in one go.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// `zh-CN` or `en`.
    #[serde(default = "default_locale")]
    locale: String,
    #[serde(default)]
    system_proxy: bool,
    #[serde(default)]
    tun: bool,
}

fn default_locale() -> String {
    i18n::detect_locale()
}

impl Default for Settings {
    fn default() -> Self {
        Self::from_environment()
    }
}

impl Settings {
    /// Build initial settings from the current environment (OS language, etc.).
    pub fn from_environment() -> Self {
        Self {
            locale: i18n::detect_locale(),
            system_proxy: false,
            tun: false,
        }
    }

    /// Load `settings.json` if it exists; otherwise detect defaults and write the file.
    pub fn load_or_init() -> Self {
        match try_load() {
            Ok(Some(settings)) => {
                log::info!(
                    "loaded settings: locale={}, system_proxy={}, tun={}",
                    settings.locale,
                    settings.system_proxy,
                    settings.tun
                );
                settings
            }
            Ok(None) => {
                let settings = Self::from_environment();
                if let Err(e) = settings.persist() {
                    log::warn!("create initial settings failed: {e:#}");
                } else {
                    log::info!(
                        "created settings.json from environment: locale={}, system_proxy={}, tun={}",
                        settings.locale,
                        settings.system_proxy,
                        settings.tun
                    );
                }
                settings
            }
            Err(e) => {
                log::warn!("load settings failed ({e:#}), recreating from environment");
                let settings = Self::from_environment();
                let _ = settings.persist();
                settings
            }
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn system_proxy(&self) -> bool {
        self.system_proxy
    }

    pub fn tun(&self) -> bool {
        self.tun
    }

    pub fn set_locale(&mut self, locale: impl Into<String>) -> Result<()> {
        self.locale = locale.into();
        self.persist()
    }

    #[allow(dead_code)] // single-field API; App currently uses set_switches
    pub fn set_system_proxy(&mut self, enabled: bool) -> Result<()> {
        self.system_proxy = enabled;
        self.persist()
    }

    #[allow(dead_code)] // single-field API; App currently uses set_switches
    pub fn set_tun(&mut self, enabled: bool) -> Result<()> {
        self.tun = enabled;
        self.persist()
    }

    /// Update both switch flags in a single disk write.
    pub fn set_switches(&mut self, system_proxy: bool, tun: bool) -> Result<()> {
        self.system_proxy = system_proxy;
        self.tun = tun;
        self.persist()
    }

    fn persist(&self) -> Result<()> {
        paths::ensure_dirs()?;
        let path = settings_path()?;
        let raw = serde_json::to_string_pretty(self).context("serialize settings")?;
        fs::write(&path, raw).with_context(|| format!("write settings {}", path.display()))?;
        Ok(())
    }
}

pub fn settings_path() -> Result<std::path::PathBuf> {
    Ok(paths::config_dir()?.join(SETTINGS_FILE))
}

fn try_load() -> Result<Option<Settings>> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read settings {}", path.display()))?;
    let settings: Settings =
        serde_json::from_str(&raw).with_context(|| format!("parse settings {}", path.display()))?;
    Ok(Some(settings))
}
