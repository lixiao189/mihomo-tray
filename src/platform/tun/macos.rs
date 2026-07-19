use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use crate::mihomo::api::ApiClient;
use crate::platform::system::{restore_public_dns, set_public_dns};
use crate::platform::traits::{PrivilegedService, TunMode};

const TUN_DNS: &str = "223.5.5.5";

pub struct MacosTun {
    service: Arc<dyn PrivilegedService>,
}

impl MacosTun {
    pub fn new(service: Arc<dyn PrivilegedService>) -> Self {
        Self { service }
    }
}

impl TunMode for MacosTun {
    fn enable(&self, api: &ApiClient) -> Result<()> {
        if !self.service.is_available() {
            bail!("privileged service is not available; install it first");
        }
        api.set_tun_enabled(true)
            .context("enable tun via API")?;
        // Clear any stale override, then set public DNS for TUN (macOS quirk).
        if let Err(e) = restore_public_dns() {
            log::warn!("restore system dns before tun: {e:#}");
        }
        if let Err(e) = set_public_dns(TUN_DNS) {
            log::warn!("set system dns to {TUN_DNS}: {e:#}");
        }
        Ok(())
    }

    fn disable(&self, api: &ApiClient) -> Result<()> {
        let _ = api.set_tun_enabled(false);
        if let Err(e) = restore_public_dns() {
            log::warn!("restore system dns after tun: {e:#}");
        }
        Ok(())
    }

    fn is_enabled(&self, api: &ApiClient) -> bool {
        api.tun_enabled().unwrap_or(false)
    }

    fn requires_privileged_core(&self) -> bool {
        true
    }

    fn prepare(&self, _core: &PathBuf) -> Result<()> {
        Ok(())
    }
}
