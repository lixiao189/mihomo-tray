use std::sync::Arc;

use super::traits::{PrivilegedService, TunMode};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod fallback;

pub fn create_tun(service: Arc<dyn PrivilegedService>) -> Arc<dyn TunMode> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::MacosTun::new(service))
    }
    #[cfg(target_os = "linux")]
    {
        let _ = service;
        Arc::new(linux::LinuxTun)
    }
    #[cfg(target_os = "windows")]
    {
        let _ = service;
        Arc::new(windows::WindowsTun)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = service;
        Arc::new(fallback::FallbackTun)
    }
}
