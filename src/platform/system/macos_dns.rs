use std::net::IpAddr;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

const ORIGINAL_DNS_FILE: &str = ".original_dns.txt";

fn backup_path() -> Result<PathBuf> {
    let dir = crate::paths::ensure_dirs()?;
    Ok(dir.join(ORIGINAL_DNS_FILE))
}

/// Resolve the network service name (hardware port) for the default route interface.
fn default_hardware_port() -> Result<String> {
    let route = Command::new("route")
        .args(["-n", "get", "default"])
        .output()
        .context("route -n get default")?;
    if !route.status.success() {
        bail!("route -n get default failed: {}", route.status);
    }
    let route_text = String::from_utf8_lossy(&route.stdout);
    let nic = route_text
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("interface:")
                .map(|s| s.trim().to_string())
        })
        .context("default route has no interface")?;

    let list = Command::new("networksetup")
        .arg("-listnetworkserviceorder")
        .output()
        .context("networksetup -listnetworkserviceorder")?;
    if !list.status.success() {
        bail!(
            "networksetup -listnetworkserviceorder failed: {}",
            list.status
        );
    }
    let list_text = String::from_utf8_lossy(&list.stdout);

    let mut current_port: Option<String> = None;
    for line in list_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('(') {
            // "(1) Wi-Fi" or "(Hardware Port: Wi-Fi, Device: en0)"
            if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                if let Some(name) = rest.split_once(')') {
                    current_port = Some(name.1.trim().to_string());
                }
                continue;
            }
        }
        if let Some(port) = &current_port {
            // "(Hardware Port: Wi-Fi, Device: en0)"
            if let Some(device) = extract_device(trimmed) {
                if device == nic {
                    return Ok(port.clone());
                }
            }
        }
    }

    bail!("no network service found for interface {nic}");
}

fn extract_device(line: &str) -> Option<&str> {
    let marker = "Device: ";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest.find(')').unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn looks_like_ip(s: &str) -> bool {
    s.parse::<IpAddr>().is_ok()
}

fn current_dns_servers(port: &str) -> Result<Vec<String>> {
    let output = Command::new("networksetup")
        .args(["-getdnsservers", port])
        .output()
        .context("networksetup -getdnsservers")?;
    if !output.status.success() {
        bail!(
            "networksetup -getdnsservers failed: {}",
            output.status
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let servers: Vec<String> = text
        .split_whitespace()
        .filter(|s| looks_like_ip(s))
        .map(ToOwned::to_owned)
        .collect();
    Ok(servers)
}

fn set_dnsservers(port: &str, servers: &[&str]) -> Result<()> {
    let mut cmd = Command::new("networksetup");
    cmd.arg("-setdnsservers").arg(port);
    for s in servers {
        cmd.arg(s);
    }
    let status = cmd.status().context("networksetup -setdnsservers")?;
    if !status.success() {
        bail!("networksetup -setdnsservers failed: {status}");
    }
    Ok(())
}

/// Save current DNS for the default interface, then set it to `dns_server`.
pub fn set_public_dns(dns_server: &str) -> Result<()> {
    let port = default_hardware_port()?;
    let original = current_dns_servers(&port)?;
    let backup = backup_path()?;
    if original.is_empty() {
        std::fs::write(&backup, "empty").context("write original dns backup")?;
    } else {
        std::fs::write(&backup, original.join(" ")).context("write original dns backup")?;
    }
    set_dnsservers(&port, &[dns_server])
}

/// Restore DNS from the backup file written by [`set_public_dns`].
pub fn restore_public_dns() -> Result<()> {
    let backup = backup_path()?;
    if !backup.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&backup).context("read original dns backup")?;
    let content = content.trim();
    let port = default_hardware_port()?;
    if content.is_empty() || content == "empty" {
        set_dnsservers(&port, &["empty"])?;
    } else {
        let servers: Vec<&str> = content.split_whitespace().collect();
        set_dnsservers(&port, &servers)?;
    }
    let _ = std::fs::remove_file(&backup);
    Ok(())
}
