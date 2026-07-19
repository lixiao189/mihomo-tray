use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;

use crate::platform::traits::{CoreInstaller, PathLayout};

#[derive(Debug, serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone)]
pub enum InstallProgress {
    Resolving,
    Downloading { downloaded: u64, total: Option<u64> },
    Extracting,
    Done,
    Failed(String),
}

pub struct GithubCoreInstaller {
    paths: Arc<dyn PathLayout>,
}

impl GithubCoreInstaller {
    pub fn new(paths: Arc<dyn PathLayout>) -> Self {
        Self { paths }
    }
}

impl CoreInstaller for GithubCoreInstaller {
    fn needs_install(&self) -> Result<bool> {
        let path = self.paths.core_binary_path()?;
        if path.exists() && verify_core(&path).is_ok() {
            return Ok(false);
        }
        Ok(true)
    }

    fn ensure_installed(&self) -> Result<PathBuf> {
        self.ensure_installed_with_progress(None)
    }

    fn ensure_installed_with_progress(
        &self,
        progress: Option<Sender<InstallProgress>>,
    ) -> Result<PathBuf> {
        let path = self.paths.core_binary_path()?;
        if path.exists() && verify_core(&path).is_ok() {
            if let Some(tx) = &progress {
                let _ = tx.send(InstallProgress::Done);
            }
            return Ok(path);
        }
        install_latest_core(&self.paths, progress.as_ref())?;
        let path = self.paths.core_binary_path()?;
        verify_core(&path)?;
        if let Some(tx) = &progress {
            let _ = tx.send(InstallProgress::Done);
        }
        Ok(path)
    }
}

fn report(tx: Option<&Sender<InstallProgress>>, msg: InstallProgress) {
    if let Some(tx) = tx {
        let _ = tx.send(msg);
    }
}

fn verify_core(path: &Path) -> Result<()> {
    let output = Command::new(path)
        .arg("-v")
        .output()
        .with_context(|| format!("run {}", path.display()))?;
    if !output.status.success() {
        let output2 = Command::new(path)
            .arg("--version")
            .output()
            .with_context(|| format!("run {}", path.display()))?;
        if !output2.status.success() {
            bail!(
                "mihomo version check failed: {}",
                String::from_utf8_lossy(&output2.stderr)
            );
        }
    }
    Ok(())
}

fn platform_asset_candidates(tag: &str) -> Vec<String> {
    let ver = tag.trim_start_matches('v');
    let (os, archs) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => ("darwin", vec!["arm64"]),
        ("macos", "x86_64") => ("darwin", vec!["amd64", "amd64-compatible", "amd64-v1"]),
        ("linux", "aarch64") => ("linux", vec!["arm64"]),
        ("linux", "x86_64") => ("linux", vec!["amd64", "amd64-compatible", "amd64-v1"]),
        ("windows", "aarch64") => ("windows", vec!["arm64"]),
        ("windows", "x86_64") => ("windows", vec!["amd64", "amd64-compatible", "amd64-v1"]),
        (os, arch) => {
            return vec![format!("unsupported-{os}-{arch}")];
        }
    };

    let mut names = Vec::new();
    for arch in archs {
        if cfg!(windows) {
            names.push(format!("mihomo-{os}-{arch}-v{ver}.zip"));
            names.push(format!("mihomo-{os}-{arch}-{tag}.zip"));
        } else {
            names.push(format!("mihomo-{os}-{arch}-v{ver}.gz"));
            names.push(format!("mihomo-{os}-{arch}-{tag}.gz"));
        }
    }
    names
}

fn install_latest_core(
    paths: &Arc<dyn PathLayout>,
    progress: Option<&Sender<InstallProgress>>,
) -> Result<()> {
    paths.ensure_dirs()?;
    report(progress, InstallProgress::Resolving);
    log::info!("resolving latest mihomo release");

    let client = reqwest::blocking::Client::builder()
        .user_agent("mihomo-tray")
        .build()?;

    let release: GithubRelease = client
        .get("https://api.github.com/repos/MetaCubeX/mihomo/releases/latest")
        .send()
        .context("fetch mihomo latest release")?
        .error_for_status()
        .context("github release status")?
        .json()
        .context("parse github release json")?;

    let candidates = platform_asset_candidates(&release.tag_name);
    let asset = candidates
        .iter()
        .find_map(|want| release.assets.iter().find(|a| &a.name == want))
        .with_context(|| {
            format!(
                "no matching release asset for {:?}; available: {:?}",
                candidates,
                release
                    .assets
                    .iter()
                    .map(|a| &a.name)
                    .collect::<Vec<_>>()
            )
        })?;

    log::info!(
        "downloading mihomo {} asset {}",
        release.tag_name,
        asset.name
    );
    let mut resp = client
        .get(&asset.browser_download_url)
        .send()
        .context("download mihomo asset")?
        .error_for_status()
        .context("download status")?;
    let total = resp.content_length();
    let mut bytes = Vec::new();
    if let Some(len) = total {
        bytes.reserve(len as usize);
    }
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    loop {
        let n = resp.read(&mut buf).context("read download body")?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        downloaded += n as u64;
        report(
            progress,
            InstallProgress::Downloading {
                downloaded,
                total,
            },
        );
    }

    report(progress, InstallProgress::Extracting);
    log::info!("extracting mihomo core ({} bytes)", bytes.len());

    let dest = paths.core_binary_path()?;
    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }

    if asset.name.ends_with(".gz") && !asset.name.ends_with(".tar.gz") {
        let mut decoder = GzDecoder::new(&bytes[..]);
        let mut out = File::create(&dest)
            .with_context(|| format!("create {}", dest.display()))?;
        io::copy(&mut decoder, &mut out).context("decompress gz")?;
        out.flush()?;
    } else if asset.name.ends_with(".zip") {
        let cursor = std::io::Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(cursor).context("open zip")?;
        let mut found = false;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            let base = Path::new(&name)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if base.starts_with("mihomo") {
                let mut out = File::create(&dest)
                    .with_context(|| format!("create {}", dest.display()))?;
                io::copy(&mut file, &mut out)?;
                out.flush()?;
                found = true;
                break;
            }
        }
        if !found {
            bail!("zip did not contain mihomo binary");
        }
    } else {
        bail!("unsupported asset format: {}", asset.name);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms)?;
    }

    #[cfg(windows)]
    {
        let _ = install_wintun(&client, paths);
    }

    log::info!("mihomo core installed at {}", dest.display());
    Ok(())
}

#[cfg(windows)]
fn install_wintun(client: &reqwest::blocking::Client, paths: &Arc<dyn PathLayout>) -> Result<()> {
    let url = "https://www.wintun.net/builds/wintun-0.14.1.zip";
    let bytes = client.get(url).send()?.error_for_status()?.bytes()?;
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;
    let arch_dir = if cfg!(target_arch = "aarch64") {
        "wintun/bin/arm64/wintun.dll"
    } else {
        "wintun/bin/amd64/wintun.dll"
    };
    let dest = paths.bin_dir()?.join("wintun.dll");
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.name().replace('\\', "/") == arch_dir {
            let mut out = File::create(&dest)?;
            io::copy(&mut file, &mut out)?;
            break;
        }
    }
    Ok(())
}
