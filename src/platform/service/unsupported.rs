use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::platform::traits::PrivilegedService;

pub struct UnsupportedService;

impl PrivilegedService for UnsupportedService {
    fn supported(&self) -> bool {
        false
    }

    fn is_available(&self) -> bool {
        false
    }

    fn install(&self) -> Result<()> {
        bail!("privileged service install is not supported on this platform")
    }

    fn uninstall(&self) -> Result<()> {
        bail!("privileged service uninstall is not supported on this platform")
    }

    fn start_core(&self, _core: &PathBuf, _profile: &PathBuf) -> Result<()> {
        bail!("privileged service is not supported on this platform")
    }

    fn stop_core(&self) -> Result<()> {
        bail!("privileged service is not supported on this platform")
    }
}
