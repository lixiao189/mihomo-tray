use std::sync::Arc;

use super::traits::TrayAppearance;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod linux;

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
mod noop;

pub fn create_appearance() -> Arc<dyn TrayAppearance> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::MacosAppearance)
    }
    #[cfg(target_os = "windows")]
    {
        Arc::new(windows::WindowsAppearance)
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        Arc::new(linux::LinuxAppearance)
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    {
        Arc::new(noop::NoopAppearance)
    }
}
