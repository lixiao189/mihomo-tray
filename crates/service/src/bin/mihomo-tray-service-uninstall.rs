//! Uninstall the privileged LaunchDaemon (must run as root).

use std::fs;
use std::process::Command;

use anyhow::{Result, bail};
use mihomo_tray_service::{HELPER_DIR, LAUNCHD_PLIST, SERVICE_LABEL, SOCK_DIR, sock_path};

fn main() {
    if let Err(e) = mihomo_tray_service::init_logging("mihomo-tray-service-uninstall") {
        eprintln!("init logging failed: {e:#}");
    }
    if let Err(e) = run() {
        log::error!("mihomo-tray-service-uninstall error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    if !is_root() {
        bail!("uninstaller must run as root");
    }

    let _ = Command::new("launchctl")
        .args(["bootout", &format!("system/{SERVICE_LABEL}")])
        .status();
    let _ = Command::new("launchctl")
        .args(["unload", LAUNCHD_PLIST])
        .status();
    let _ = fs::remove_file(LAUNCHD_PLIST);
    let _ = fs::remove_dir_all(HELPER_DIR);
    let _ = fs::remove_file(sock_path());
    let _ = fs::remove_dir(SOCK_DIR);

    log::info!("uninstalled {SERVICE_LABEL}");
    Ok(())
}

fn is_root() -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            fn geteuid() -> u32;
        }
        unsafe { geteuid() == 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}
