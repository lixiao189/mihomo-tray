/// Platform-specific proxy bypass domain policy.
///
/// Injected into system-proxy backends so they stay free of `#[cfg]` / OS details.
pub trait BypassPolicy: Copy + Send + Sync {
    /// Domains to ignore when the proxy is enabled.
    fn when_enabled(self) -> &'static str;
    /// Value that clears the bypass list when the proxy is disabled.
    fn when_disabled(self) -> &'static str;
}

const DEFAULT_BYPASS: &str = "localhost,127.0.0.1,*.local,10.*,172.16.*,172.17.*,172.18.*,172.19.*,172.20.*,172.21.*,172.22.*,172.23.*,172.24.*,172.25.*,172.26.*,172.27.*,172.28.*,172.29.*,172.30.*,172.31.*,192.168.*,<local>";

/// Windows / Linux: empty string clears the override / ignore-hosts list.
#[cfg(not(target_os = "macos"))]
#[derive(Clone, Copy, Default)]
pub struct DefaultBypassPolicy;

#[cfg(not(target_os = "macos"))]
impl BypassPolicy for DefaultBypassPolicy {
    fn when_enabled(self) -> &'static str {
        DEFAULT_BYPASS
    }

    fn when_disabled(self) -> &'static str {
        ""
    }
}

/// macOS: `networksetup -setproxybypassdomains` requires the literal `Empty`.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Default)]
pub struct MacosBypassPolicy;

#[cfg(target_os = "macos")]
impl BypassPolicy for MacosBypassPolicy {
    fn when_enabled(self) -> &'static str {
        DEFAULT_BYPASS
    }

    fn when_disabled(self) -> &'static str {
        "Empty"
    }
}
