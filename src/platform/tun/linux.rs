use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::mihomo::api::ApiClient;
use crate::platform::traits::TunMode;

pub struct LinuxTun;

impl TunMode for LinuxTun {
    fn prepare(&self, core: &PathBuf) -> Result<()> {
        let status = Command::new("pkexec")
            .args([
                "setcap",
                "cap_net_admin,cap_net_bind_service=+ep",
                &core.display().to_string(),
            ])
            .status();
        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => bail!("pkexec setcap failed: {s}"),
            Err(e) => bail!("pkexec not available: {e}"),
        }
    }

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
