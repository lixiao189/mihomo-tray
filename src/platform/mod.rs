pub mod appearance;
pub mod core;
pub mod paths;
pub mod service;
pub mod system;
pub mod traits;
pub mod tun;

pub use core::InstallProgress;
pub use traits::{
    CoreInstaller, CoreRunner, PathLayout, PrivilegedService, SystemProxy, TrayAppearance, TunMode,
};

use std::sync::Arc;

pub struct Platform {
    pub system_proxy: Arc<dyn SystemProxy>,
    pub tun: Arc<dyn TunMode>,
    pub service: Arc<dyn PrivilegedService>,
    pub appearance: Arc<dyn TrayAppearance>,
    pub core_runner: Arc<dyn CoreRunner>,
    pub installer: Arc<dyn CoreInstaller>,
    pub paths: Arc<dyn PathLayout>,
}

impl Platform {
    pub fn default_for_host() -> Self {
        let paths = paths::create_path_layout();
        let service = service::create_service();
        let tun = tun::create_tun(Arc::clone(&service));
        Self {
            system_proxy: system::create_system_proxy(),
            tun,
            service,
            appearance: appearance::create_appearance(),
            core_runner: core::create_core_runner(Arc::clone(&paths)),
            installer: core::create_core_installer(Arc::clone(&paths)),
            paths,
        }
    }
}
