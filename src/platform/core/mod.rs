mod installer;
mod sidecar;

pub use installer::{GithubCoreInstaller, InstallProgress};
pub use sidecar::SidecarCoreRunner;

use std::sync::Arc;

use super::traits::{CoreInstaller, CoreRunner, PathLayout};

pub fn create_core_runner(paths: Arc<dyn PathLayout>) -> Arc<dyn CoreRunner> {
    Arc::new(SidecarCoreRunner::new(paths))
}

pub fn create_core_installer(paths: Arc<dyn PathLayout>) -> Arc<dyn CoreInstaller> {
    Arc::new(GithubCoreInstaller::new(paths))
}
