use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use crate::mihomo::api::ApiClient;
use crate::platform::traits::{PrivilegedService, TunMode};

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
        Ok(())
    }

    fn disable(&self, api: &ApiClient) -> Result<()> {
        let _ = api.set_tun_enabled(false);
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
