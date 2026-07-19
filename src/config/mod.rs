use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::paths::{self, DEFAULT_PROFILE};

pub mod watch;
pub use watch::ActiveConfigWatcher;

const DEFAULT_CONFIG: &str = include_str!("default.yaml");

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProfileMeta {
    #[serde(rename = "mixed-port")]
    pub mixed_port: Option<u16>,
    pub port: Option<u16>,
    #[serde(rename = "socks-port")]
    pub socks_port: Option<u16>,
    /// Optional; tray talks to the core over a local socket, not this HTTP address.
    #[allow(dead_code)]
    #[serde(rename = "external-controller")]
    pub external_controller: Option<String>,
    pub secret: Option<String>,
}

fn nonzero_port(port: Option<u16>) -> Option<u16> {
    port.filter(|&p| p != 0)
}

impl ProfileMeta {
    pub fn http_port(&self) -> u16 {
        // Clash/mihomo often serialize disabled listeners as 0; ignore those.
        nonzero_port(self.mixed_port)
            .or_else(|| nonzero_port(self.port))
            .unwrap_or(7890)
    }

    pub fn socks_port(&self) -> u16 {
        nonzero_port(self.socks_port)
            .or_else(|| nonzero_port(self.mixed_port))
            .unwrap_or(7890)
    }

    pub fn secret(&self) -> Option<&str> {
        self.secret
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

pub fn ensure_default_profile() -> Result<PathBuf> {
    let dir = paths::ensure_dirs()?;
    let profiles = paths::list_profiles()?;
    if profiles.is_empty() {
        let path = dir.join(DEFAULT_PROFILE);
        fs::write(&path, DEFAULT_CONFIG)
            .with_context(|| format!("write default config {}", path.display()))?;
        paths::set_active_profile(&path)?;
        return Ok(path);
    }
    paths::read_active_profile()
}

pub fn parse_profile_meta(path: &Path) -> Result<ProfileMeta> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("read profile {}", path.display()))?;
    let meta: ProfileMeta = serde_yaml::from_str(&text)
        .with_context(|| format!("parse profile {}", path.display()))?;
    Ok(meta)
}

pub fn import_profile(source: &Path) -> Result<PathBuf> {
    if !source.is_file() {
        bail!("not a file: {}", source.display());
    }
    if !paths::is_profile_file(source) {
        bail!("profile must be .yaml or .yml");
    }
    let dir = paths::ensure_dirs()?;
    let name = source
        .file_name()
        .context("import source has no name")?
        .to_os_string();
    let dest = dir.join(&name);
    fs::copy(source, &dest)
        .with_context(|| format!("copy {} -> {}", source.display(), dest.display()))?;
    paths::set_active_profile(&dest)?;
    Ok(dest)
}

pub fn pick_and_import_profile() -> Result<Option<PathBuf>> {
    let file = rfd::FileDialog::new()
        .add_filter("YAML", &["yaml", "yml"])
        .set_title(rust_i18n::t!("dialog.import_profile").to_string())
        .pick_file();
    match file {
        Some(path) => Ok(Some(import_profile(&path)?)),
        None => Ok(None),
    }
}
