//! Install the privileged LaunchDaemon (must run as root via osascript/sudo).

use std::env;
use std::fs;
use std::os::unix::fs::{PermissionsExt, chown};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use mihomo_tray_service::{
    HELPER_BIN_NAME, HELPER_DIR, LAUNCHD_PLIST, OLD_CORE_LABEL, OLD_CORE_PLIST, SERVICE_LABEL,
    SOCK_DIR, helper_bin_path,
};

fn main() {
    if let Err(e) = mihomo_tray_service::init_logging("mihomo-tray-service-install") {
        eprintln!("init logging failed: {e:#}");
    }
    if let Err(e) = run() {
        log::error!("mihomo-tray-service-install error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    if !is_root() {
        bail!("installer must run as root");
    }

    let gid = env::var("MIHOMO_TRAY_SERVICE_GID")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    let src = sibling_bin(HELPER_BIN_NAME)?;
    if !src.exists() {
        bail!("service binary not found next to installer: {}", src.display());
    }

    // Remove legacy core LaunchDaemon if present.
    cleanup_old_core();

    fs::create_dir_all(HELPER_DIR).context("create helper dir")?;
    let dest = helper_bin_path();
    fs::copy(&src, &dest).with_context(|| {
        format!("copy {} -> {}", src.display(), dest.display())
    })?;
    let mut perms = fs::metadata(&dest)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&dest, perms)?;
    let _ = chown(&dest, Some(0), Some(0));

    let plist = render_plist(&dest);
    fs::write(LAUNCHD_PLIST, plist).context("write launchd plist")?;
    let mut plist_perms = fs::metadata(LAUNCHD_PLIST)?.permissions();
    plist_perms.set_mode(0o644);
    fs::set_permissions(LAUNCHD_PLIST, plist_perms)?;
    let _ = chown(Path::new(LAUNCHD_PLIST), Some(0), Some(0));

    prepare_sock_dir(gid)?;

    // Reload daemon.
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("system/{SERVICE_LABEL}")])
        .status();
    let _ = Command::new("launchctl")
        .args(["unload", LAUNCHD_PLIST])
        .status();
    let status = Command::new("launchctl")
        .args(["bootstrap", "system", LAUNCHD_PLIST])
        .status()
        .context("launchctl bootstrap")?;
    if !status.success() {
        // Older macOS fallback.
        let status = Command::new("launchctl")
            .args(["load", "-w", LAUNCHD_PLIST])
            .status()
            .context("launchctl load")?;
        if !status.success() {
            bail!("failed to load LaunchDaemon");
        }
    }
    let _ = Command::new("launchctl")
        .args(["kickstart", "-k", &format!("system/{SERVICE_LABEL}")])
        .status();

    log::info!("installed {SERVICE_LABEL}");
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
        false
    }
}

fn sibling_bin(name: &str) -> Result<PathBuf> {
    let exe = env::current_exe().context("current_exe")?;
    let dir = exe.parent().context("no parent dir")?;
    Ok(dir.join(name))
}

fn cleanup_old_core() {
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("system/{OLD_CORE_LABEL}")])
        .status();
    let _ = Command::new("launchctl")
        .args(["unload", OLD_CORE_PLIST])
        .status();
    let _ = fs::remove_file(OLD_CORE_PLIST);
}

fn prepare_sock_dir(gid: u32) -> Result<()> {
    fs::create_dir_all(SOCK_DIR).context("create sock dir")?;
    let _ = chown(Path::new(SOCK_DIR), Some(0), Some(gid));
    let mut perms = fs::metadata(SOCK_DIR)?.permissions();
    perms.set_mode(0o2770);
    fs::set_permissions(SOCK_DIR, perms)?;
    Ok(())
}

fn render_plist(bin: &Path) -> String {
    const TEMPLATE: &str = include_str!("../../../../assets/service.launchd.plist");
    TEMPLATE
        .replace("{{label}}", SERVICE_LABEL)
        .replace("{{bin}}", &bin.display().to_string())
}
