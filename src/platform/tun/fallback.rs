use anyhow::{Context, Result};

use crate::mihomo::api::ApiClient;
use crate::platform::traits::TunMode;

/// Generic API-only TUN for platforms without dedicated backends.
pub struct FallbackTun;

impl TunMode for FallbackTun {
    fn enable(&self, api: &ApiClient) -> Result<()> {
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
}
