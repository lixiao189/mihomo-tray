use anyhow::{Context, Result};
use sysproxy::Sysproxy;

use crate::platform::traits::SystemProxy;

const BYPASS: &str = "localhost,127.0.0.1,*.local,10.*,172.16.*,172.17.*,172.18.*,172.19.*,172.20.*,172.21.*,172.22.*,172.23.*,172.24.*,172.25.*,172.26.*,172.27.*,172.28.*,172.29.*,172.30.*,172.31.*,192.168.*,<local>";

pub struct SysproxyBackend;

impl SystemProxy for SysproxyBackend {
    fn enable(&self, http_port: u16, _socks_port: u16) -> Result<()> {
        // sysproxy uses a single port field; prefer mixed/http port for HTTP(S).
        let proxy = Sysproxy {
            enable: true,
            host: "127.0.0.1".to_string(),
            port: http_port,
            bypass: BYPASS.to_string(),
        };
        proxy.set_system_proxy().context("set system proxy")
    }

    fn disable(&self) -> Result<()> {
        let proxy = Sysproxy {
            enable: false,
            host: "127.0.0.1".to_string(),
            port: 0,
            bypass: BYPASS.to_string(),
        };
        proxy.set_system_proxy().context("disable system proxy")
    }

    fn is_enabled(&self) -> bool {
        Sysproxy::get_system_proxy()
            .map(|p| p.enable)
            .unwrap_or(false)
    }
}
