use anyhow::{Context, Result, bail};

use crate::platform::traits::SystemProxy;

pub struct MacosElevatedBackend;

impl SystemProxy for MacosElevatedBackend {
    fn enable(&self, http_port: u16, socks_port: u16) -> Result<()> {
        let services = network_services()?;
        let mut cmds = Vec::new();
        for svc in services {
            let escaped = svc.replace('"', "\\\"");
            cmds.push(format!(
                "networksetup -setwebproxy \"{escaped}\" 127.0.0.1 {http_port}"
            ));
            cmds.push(format!(
                "networksetup -setsecurewebproxy \"{escaped}\" 127.0.0.1 {http_port}"
            ));
            cmds.push(format!(
                "networksetup -setsocksfirewallproxy \"{escaped}\" 127.0.0.1 {socks_port}"
            ));
            cmds.push(format!("networksetup -setwebproxystate \"{escaped}\" on"));
            cmds.push(format!(
                "networksetup -setsecurewebproxystate \"{escaped}\" on"
            ));
            cmds.push(format!(
                "networksetup -setsocksfirewallproxystate \"{escaped}\" on"
            ));
        }
        run_osascript_admin(&cmds.join(" && "))
    }

    fn disable(&self) -> Result<()> {
        let services = network_services()?;
        let mut cmds = Vec::new();
        for svc in services {
            let escaped = svc.replace('"', "\\\"");
            cmds.push(format!("networksetup -setwebproxystate \"{escaped}\" off"));
            cmds.push(format!(
                "networksetup -setsecurewebproxystate \"{escaped}\" off"
            ));
            cmds.push(format!(
                "networksetup -setsocksfirewallproxystate \"{escaped}\" off"
            ));
        }
        run_osascript_admin(&cmds.join(" && "))
    }

    fn is_enabled(&self) -> bool {
        // Elevated path does not query state; primary (sysproxy) owns is_enabled.
        false
    }
}

fn network_services() -> Result<Vec<String>> {
    let output = std::process::Command::new("networksetup")
        .arg("-listallnetworkservices")
        .output()
        .context("networksetup list")?;
    let text = String::from_utf8_lossy(&output.stdout);
    let services: Vec<String> = text
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with('*'))
        .map(ToOwned::to_owned)
        .collect();
    Ok(services)
}

fn run_osascript_admin(shell: &str) -> Result<()> {
    let escaped = shell.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("do shell script \"{escaped}\" with administrator privileges");
    let status = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
        .context("osascript")?;
    if !status.success() {
        bail!("osascript failed with {status}");
    }
    Ok(())
}
