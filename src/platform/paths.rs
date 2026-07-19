use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;

use crate::paths;
use crate::platform::traits::PathLayout;

pub struct DefaultPathLayout;

impl PathLayout for DefaultPathLayout {
    fn config_dir(&self) -> Result<PathBuf> {
        paths::config_dir()
    }

    fn bin_dir(&self) -> Result<PathBuf> {
        paths::bin_dir()
    }

    fn core_binary_path(&self) -> Result<PathBuf> {
        paths::core_binary_path()
    }

    fn ensure_dirs(&self) -> Result<PathBuf> {
        paths::ensure_dirs()
    }

    fn open_config_folder(&self) -> Result<()> {
        paths::open_config_folder()
    }

    fn list_profiles(&self) -> Result<Vec<PathBuf>> {
        paths::list_profiles()
    }

    fn set_active_profile(&self, path: &Path) -> Result<()> {
        paths::set_active_profile(path)
    }

    fn profile_display_name(&self, path: &Path) -> String {
        paths::profile_display_name(path)
    }
}

pub fn create_path_layout() -> Arc<dyn PathLayout> {
    Arc::new(DefaultPathLayout)
}
