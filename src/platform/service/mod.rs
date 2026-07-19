use std::sync::Arc;

use super::traits::PrivilegedService;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;

#[cfg(target_os = "macos")]
pub use macos::MacosLaunchDaemonService;
#[cfg(not(target_os = "macos"))]
pub use unsupported::UnsupportedService;

pub fn create_service() -> Arc<dyn PrivilegedService> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(MacosLaunchDaemonService)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Arc::new(UnsupportedService)
    }
}
