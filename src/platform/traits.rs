use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

use anyhow::Result;

use crate::mihomo::api::ApiClient;
use crate::platform::core::InstallProgress;

pub trait SystemProxy: Send + Sync {
    fn enable(&self, http_port: u16, socks_port: u16) -> Result<()>;
    fn disable(&self) -> Result<()>;
    fn is_enabled(&self) -> bool;
}

pub trait TunMode: Send + Sync {
    /// Platform-specific privilege prep (e.g. Linux setcap). No-op when unused.
    fn prepare(&self, core: &PathBuf) -> Result<()> {
        let _ = core;
        Ok(())
    }

    fn enable(&self, api: &ApiClient) -> Result<()>;
    fn disable(&self, api: &ApiClient) -> Result<()>;
    fn is_enabled(&self, api: &ApiClient) -> bool;

    /// When true, TUN requires the privileged service core (macOS).
    fn requires_privileged_core(&self) -> bool {
        false
    }
}

pub trait PrivilegedService: Send + Sync {
    fn supported(&self) -> bool;
    fn is_available(&self) -> bool;
    fn install(&self) -> Result<()>;
    fn uninstall(&self) -> Result<()>;
    fn start_core(&self, core: &PathBuf, profile: &PathBuf) -> Result<()>;
    fn stop_core(&self) -> Result<()>;
}

pub trait TrayAppearance: Send + Sync {
    fn uses_template_icon(&self) -> bool;
    fn tray_background_is_dark(&self) -> bool;
    fn theme_poll_interval(&self) -> Option<Duration>;

    /// Apply icon using the platform-appropriate tray-icon API.
    fn apply_tray_icon(&self, tray: &tray_icon::TrayIcon, icon: tray_icon::Icon);
}

pub trait CoreRunner: Send + Sync {
    fn start(&self, core: &PathBuf, profile: &PathBuf) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn restart(&self, core: &PathBuf, profile: &PathBuf) -> Result<()>;
}

pub trait CoreInstaller: Send + Sync {
    fn needs_install(&self) -> Result<bool>;
    fn ensure_installed(&self) -> Result<PathBuf>;
    fn ensure_installed_with_progress(
        &self,
        progress: Option<Sender<InstallProgress>>,
    ) -> Result<PathBuf>;
}

pub trait PathLayout: Send + Sync {
    fn config_dir(&self) -> Result<PathBuf>;
    // Available for backends / future callers; may be unused on some hosts.
    #[allow(dead_code)]
    fn bin_dir(&self) -> Result<PathBuf>;
    fn core_binary_path(&self) -> Result<PathBuf>;
    fn ensure_dirs(&self) -> Result<PathBuf>;
    fn open_config_folder(&self) -> Result<()>;
    fn list_profiles(&self) -> Result<Vec<PathBuf>>;
    fn set_active_profile(&self, path: &Path) -> Result<()>;
    #[allow(dead_code)]
    fn profile_display_name(&self, path: &Path) -> String;
}
