mod bypass;
mod fallback;
#[cfg(target_os = "macos")]
mod macos_elevated;
mod sysproxy;

pub use fallback::FallbackProxy;
#[cfg(target_os = "macos")]
pub use macos_elevated::MacosElevatedBackend;
pub use sysproxy::SysproxyBackend;

use std::sync::Arc;

#[cfg(not(target_os = "macos"))]
use bypass::DefaultBypassPolicy;
#[cfg(target_os = "macos")]
use bypass::MacosBypassPolicy;
use super::traits::SystemProxy;

/// Composition root: pick the OS bypass policy and wire backends.
pub fn create_system_proxy() -> Arc<dyn SystemProxy> {
    #[cfg(target_os = "macos")]
    {
        let bypass = MacosBypassPolicy;
        Arc::new(FallbackProxy::new(
            SysproxyBackend::new(bypass),
            MacosElevatedBackend::new(bypass),
        ))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Arc::new(SysproxyBackend::new(DefaultBypassPolicy))
    }
}
