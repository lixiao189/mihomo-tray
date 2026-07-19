//! Shared IPC protocol and paths for the mihomo-tray privileged helper.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub const IPC_MAGIC: &str = "mihomo-tray-ipc-v1";
pub const SERVICE_LABEL: &str = "com.mihomotray.service";
pub const OLD_CORE_LABEL: &str = "com.mihomotray.core";
pub const LAUNCHD_PLIST: &str = "/Library/LaunchDaemons/com.mihomotray.service.plist";
pub const OLD_CORE_PLIST: &str = "/Library/LaunchDaemons/com.mihomotray.core.plist";
pub const HELPER_DIR: &str = "/Library/PrivilegedHelperTools/com.mihomotray.service";
pub const HELPER_BIN_NAME: &str = "mihomo-tray-service";
pub const SOCK_DIR: &str = "/tmp/mihomo-tray";
pub const SOCK_NAME: &str = "service.sock";
/// Local API endpoint filename for the mihomo core Unix socket.
#[cfg(unix)]
pub const CORE_SOCK_NAME: &str = "mihomo.sock";
/// Local API named pipe for the mihomo core on Windows.
#[cfg(windows)]
pub const CORE_PIPE_NAME: &str = r"\\.\pipe\mihomo-tray-core";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Ping {
        magic: String,
    },
    Start {
        magic: String,
        core_path: String,
        config_dir: String,
        profile_path: String,
        safe_paths: String,
    },
    Stop {
        magic: String,
    },
    Status {
        magic: String,
    },
}

impl Request {
    pub fn ping() -> Self {
        Self::Ping {
            magic: IPC_MAGIC.to_string(),
        }
    }

    pub fn stop() -> Self {
        Self::Stop {
            magic: IPC_MAGIC.to_string(),
        }
    }

    pub fn status() -> Self {
        Self::Status {
            magic: IPC_MAGIC.to_string(),
        }
    }

    pub fn start(
        core_path: impl AsRef<Path>,
        config_dir: impl AsRef<Path>,
        profile_path: impl AsRef<Path>,
        safe_paths: impl Into<String>,
    ) -> Self {
        Self::Start {
            magic: IPC_MAGIC.to_string(),
            core_path: core_path.as_ref().display().to_string(),
            config_dir: config_dir.as_ref().display().to_string(),
            profile_path: profile_path.as_ref().display().to_string(),
            safe_paths: safe_paths.into(),
        }
    }

    pub fn magic(&self) -> &str {
        match self {
            Self::Ping { magic }
            | Self::Start { magic, .. }
            | Self::Stop { magic }
            | Self::Status { magic } => magic,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub running: Option<bool>,
}

impl Response {
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            running: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            running: None,
        }
    }

    pub fn status(running: bool) -> Self {
        Self {
            ok: true,
            error: None,
            running: Some(running),
        }
    }
}

pub fn sock_path() -> PathBuf {
    PathBuf::from(SOCK_DIR).join(SOCK_NAME)
}

pub fn helper_bin_path() -> PathBuf {
    PathBuf::from(HELPER_DIR).join(HELPER_BIN_NAME)
}

pub fn ensure_magic(magic: &str) -> Result<()> {
    if magic != IPC_MAGIC {
        bail!("invalid IPC magic");
    }
    Ok(())
}

mod core_controller {
    use std::path::PathBuf;
    use std::process::Command;

    use anyhow::Result;

    /// Ensure the local controller endpoint is ready, then append platform flags.
    pub fn append_args(cmd: &mut Command) -> Result<()> {
        prepare()?;
        cmd.arg(FLAG).arg(ipc_path());
        Ok(())
    }

    #[cfg(unix)]
    const FLAG: &str = "-ext-ctl-unix";
    #[cfg(windows)]
    const FLAG: &str = "-ext-ctl-pipe";

    #[cfg(unix)]
    fn prepare() -> Result<()> {
        use std::fs;

        use anyhow::Context;

        use super::SOCK_DIR;

        fs::create_dir_all(SOCK_DIR).context("create core ipc dir")?;
        let _ = fs::remove_file(ipc_path());
        Ok(())
    }

    #[cfg(windows)]
    fn prepare() -> Result<()> {
        Ok(())
    }

    #[cfg(unix)]
    pub fn ipc_path() -> PathBuf {
        use super::{CORE_SOCK_NAME, SOCK_DIR};
        PathBuf::from(SOCK_DIR).join(CORE_SOCK_NAME)
    }

    #[cfg(windows)]
    pub fn ipc_path() -> PathBuf {
        use super::CORE_PIPE_NAME;
        PathBuf::from(CORE_PIPE_NAME)
    }
}

/// Path passed to mihomo via `-ext-ctl-unix` / `-ext-ctl-pipe`.
pub fn core_ipc_path() -> PathBuf {
    core_controller::ipc_path()
}

/// Append mihomo local-controller CLI flags for the current platform.
pub fn append_core_controller_args(cmd: &mut std::process::Command) -> Result<()> {
    core_controller::append_args(cmd)
}

#[cfg(unix)]
mod unix_client {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::path::Path;
    use std::time::Duration;

    use anyhow::{Context, Result, bail};

    use super::{Request, Response, sock_path};

    pub fn call_with_timeout(req: &Request, timeout: Duration) -> Result<Response> {
        let path = sock_path();
        let stream = UnixStream::connect(&path)
            .with_context(|| format!("connect to service socket {}", path.display()))?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        let mut stream = stream;
        let line = serde_json::to_string(req).context("serialize IPC request")?;
        stream
            .write_all(line.as_bytes())
            .context("write IPC request")?;
        stream.write_all(b"\n").context("write IPC newline")?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);
        let mut resp_line = String::new();
        reader
            .read_line(&mut resp_line)
            .context("read IPC response")?;
        if resp_line.is_empty() {
            bail!("empty IPC response");
        }
        let resp: Response =
            serde_json::from_str(resp_line.trim()).context("parse IPC response")?;
        Ok(resp)
    }

    pub fn call(req: &Request) -> Result<Response> {
        call_with_timeout(req, Duration::from_secs(10))
    }

    pub fn is_reachable() -> bool {
        matches!(call(&Request::ping()), Ok(r) if r.ok)
    }

    pub fn start_core(
        core_path: &Path,
        config_dir: &Path,
        profile_path: &Path,
        safe_paths: &str,
    ) -> Result<()> {
        let resp = call(&Request::start(
            core_path,
            config_dir,
            profile_path,
            safe_paths,
        ))?;
        if !resp.ok {
            bail!(resp.error.unwrap_or_else(|| "start failed".into()));
        }
        Ok(())
    }

    pub fn stop_core() -> Result<()> {
        let resp = call(&Request::stop())?;
        if !resp.ok {
            bail!(resp.error.unwrap_or_else(|| "stop failed".into()));
        }
        Ok(())
    }

    pub fn core_running() -> Result<bool> {
        let resp = call(&Request::status())?;
        if !resp.ok {
            bail!(resp.error.unwrap_or_else(|| "status failed".into()));
        }
        Ok(resp.running.unwrap_or(false))
    }
}

#[cfg(unix)]
pub use unix_client::*;

#[cfg(not(unix))]
mod win_stub {
    use std::path::Path;
    use std::time::Duration;

    use anyhow::{Result, bail};

    use super::{Request, Response};

    pub fn call(_req: &Request) -> Result<Response> {
        bail!("service IPC is not supported on this platform")
    }

    pub fn call_with_timeout(_req: &Request, _timeout: Duration) -> Result<Response> {
        bail!("service IPC is not supported on this platform")
    }

    pub fn is_reachable() -> bool {
        false
    }

    pub fn start_core(
        _core_path: &Path,
        _config_dir: &Path,
        _profile_path: &Path,
        _safe_paths: &str,
    ) -> Result<()> {
        bail!("service IPC is not supported on this platform")
    }

    pub fn stop_core() -> Result<()> {
        bail!("service IPC is not supported on this platform")
    }

    pub fn core_running() -> Result<bool> {
        bail!("service IPC is not supported on this platform")
    }
}

#[cfg(not(unix))]
pub use win_stub::*;
