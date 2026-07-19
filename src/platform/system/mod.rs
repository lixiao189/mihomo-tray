mod fallback;
#[cfg(target_os = "macos")]
mod macos_elevated;
mod sysproxy;

pub use fallback::FallbackProxy;
#[cfg(target_os = "macos")]
pub use macos_elevated::MacosElevatedBackend;
pub use sysproxy::SysproxyBackend;

use std::sync::Arc;

use super::traits::SystemProxy;

pub fn create_system_proxy() -> Arc<dyn SystemProxy> {
    let primary = SysproxyBackend;
    #[cfg(target_os = "macos")]
    {
        Arc::new(FallbackProxy::new(primary, MacosElevatedBackend))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Arc::new(primary)
    }
}
