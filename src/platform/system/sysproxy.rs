use anyhow::{Context, Result};
use sysproxy::Sysproxy;

use super::bypass::BypassPolicy;
use crate::platform::traits::SystemProxy;

pub struct SysproxyBackend<B> {
    bypass: B,
}

impl<B: BypassPolicy> SysproxyBackend<B> {
    pub fn new(bypass: B) -> Self {
        Self { bypass }
    }
}

impl<B: BypassPolicy> SystemProxy for SysproxyBackend<B> {
    fn enable(&self, http_port: u16, _socks_port: u16) -> Result<()> {
        // sysproxy uses a single port field; prefer mixed/http port for HTTP(S).
        let proxy = Sysproxy {
            enable: true,
            host: "127.0.0.1".to_string(),
            port: http_port,
            bypass: self.bypass.when_enabled().to_string(),
        };
        proxy.set_system_proxy().context("set system proxy")
    }

    fn disable(&self) -> Result<()> {
        let proxy = Sysproxy {
            enable: false,
            host: "127.0.0.1".to_string(),
            port: 0,
            bypass: self.bypass.when_disabled().to_string(),
        };
        proxy.set_system_proxy().context("disable system proxy")
    }

    fn is_enabled(&self) -> bool {
        Sysproxy::get_system_proxy()
            .map(|p| p.enable)
            .unwrap_or(false)
    }
}
