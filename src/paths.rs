use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub const APP_DIR_NAME: &str = "mihomo-tray";
pub const ACTIVE_MARKER: &str = ".active";
pub const DEFAULT_PROFILE: &str = "config.yaml";
pub const DELAY_TEST_URL: &str = "https://www.gstatic.com/generate_204";
pub const DELAY_TIMEOUT_MS: u32 = 5000;

pub fn config_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().context("cannot resolve data directory")?;
    Ok(base.join(APP_DIR_NAME))
}

pub fn bin_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("bin"))
}

pub fn core_binary_path() -> Result<PathBuf> {
    let name = if cfg!(windows) {
        "mihomo.exe"
    } else {
        "mihomo"
    };
    Ok(bin_dir()?.join(name))
}

pub fn active_marker_path() -> Result<PathBuf> {
    Ok(config_dir()?.join(ACTIVE_MARKER))
}

pub fn ensure_dirs() -> Result<PathBuf> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("create config dir {}", dir.display()))?;
    fs::create_dir_all(bin_dir()?).context("create bin dir")?;
    Ok(dir)
}

pub fn open_config_folder() -> Result<()> {
    let dir = ensure_dirs()?;
    open::that(&dir).with_context(|| format!("open {}", dir.display()))?;
    Ok(())
}

pub fn is_profile_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
}

pub fn list_profiles() -> Result<Vec<PathBuf>> {
    let dir = ensure_dirs()?;
    let mut profiles = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_profile_file(&path) {
            profiles.push(path);
        }
    }
    profiles.sort();
    Ok(profiles)
}

pub fn read_active_profile() -> Result<PathBuf> {
    let dir = ensure_dirs()?;
    let marker = active_marker_path()?;
    if marker.exists() {
        let name = fs::read_to_string(&marker)?.trim().to_string();
        if !name.is_empty() {
            let path = dir.join(&name);
            if path.exists() {
                return Ok(path);
            }
        }
    }
    let default = dir.join(DEFAULT_PROFILE);
    if default.exists() {
        set_active_profile(&default)?;
        return Ok(default);
    }
    let profiles = list_profiles()?;
    if let Some(first) = profiles.first() {
        set_active_profile(first)?;
        return Ok(first.clone());
    }
    bail!("no profile found in {}", dir.display());
}

pub fn set_active_profile(path: &Path) -> Result<()> {
    let dir = ensure_dirs()?;
    let name = path
        .file_name()
        .context("profile has no file name")?
        .to_string_lossy()
        .to_string();
    fs::write(active_marker_path()?, name.as_bytes())
        .with_context(|| format!("write active marker in {}", dir.display()))?;
    Ok(())
}

pub fn profile_display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}
