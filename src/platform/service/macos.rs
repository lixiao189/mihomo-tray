use std::env;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::paths;
use crate::platform::traits::PrivilegedService;

const INSTALL_BIN: &str = "mihomo-tray-service-install";
const UNINSTALL_BIN: &str = "mihomo-tray-service-uninstall";

pub struct MacosLaunchDaemonService;

impl PrivilegedService for MacosLaunchDaemonService {
    fn supported(&self) -> bool {
        true
    }

    fn is_available(&self) -> bool {
        mihomo_tray_service::is_reachable()
    }

    fn start_core(&self, core: &PathBuf, profile: &PathBuf) -> Result<()> {
        let config_dir = paths::config_dir()?;
        let safe = format!(
            "{}:{}",
            config_dir.display(),
            profile
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        );
        mihomo_tray_service::start_core(core, &config_dir, profile, &safe)
            .context("service start_core")?;
        Ok(())
    }

    fn stop_core(&self) -> Result<()> {
        mihomo_tray_service::stop_core().context("service stop_core")?;
        Ok(())
    }

    fn install(&self) -> Result<()> {
        let install_path = resolve_helper_bin(INSTALL_BIN)?;
        let gid = current_gid();
        let prompt = rust_i18n::t!("dialog.service_install_prompt").to_string();
        let prompt = escape_osascript(&prompt);
        let install_quoted = shell_single_quote(&install_path.display().to_string());
        let shell = format!("sudo MIHOMO_TRAY_SERVICE_GID={gid} {install_quoted}");
        let shell = escape_osascript(&shell);
        let script = format!(
            r#"do shell script "{shell}" with administrator privileges with prompt "{prompt}""#
        );
        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .context("osascript install service")?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            let out = String::from_utf8_lossy(&output.stdout);
            bail!(
                "install service failed: {} {}",
                err.trim(),
                out.trim()
            );
        }
        for _ in 0..40 {
            if self.is_available() {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        bail!("service installed but IPC not reachable yet")
    }

    fn uninstall(&self) -> Result<()> {
        let uninstall_path = resolve_helper_bin(UNINSTALL_BIN)?;
        let prompt = rust_i18n::t!("dialog.service_uninstall_prompt").to_string();
        let prompt = escape_osascript(&prompt);
        let _ = self.stop_core();
        let uninstall_quoted = shell_single_quote(&uninstall_path.display().to_string());
        let shell = format!("sudo {uninstall_quoted}");
        let shell = escape_osascript(&shell);
        let script = format!(
            r#"do shell script "{shell}" with administrator privileges with prompt "{prompt}""#
        );
        let status = Command::new("osascript")
            .args(["-e", &script])
            .status()
            .context("osascript uninstall service")?;
        if !status.success() {
            bail!("uninstall service failed with {status}");
        }
        Ok(())
    }
}

fn resolve_helper_bin(name: &str) -> Result<PathBuf> {
    let exe = env::current_exe().context("current_exe")?;
    if let Some(dir) = exe.parent() {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    let candidate = paths::bin_dir()?.join(name);
    if candidate.exists() {
        return Ok(candidate);
    }
    bail!(
        "helper binary `{name}` not found next to {} or in bin dir",
        exe.display()
    )
}

fn current_gid() -> u32 {
    unsafe extern "C" {
        fn getgid() -> u32;
    }
    unsafe { getgid() }
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

fn escape_osascript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
