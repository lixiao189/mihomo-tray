//! Privileged helper daemon: spawn/kill mihomo as root on behalf of the tray app.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use anyhow::{Context, Result, bail};
use mihomo_tray_service::{
    HELPER_DIR, Request, Response, SOCK_DIR, append_core_controller_args, core_ipc_path,
    ensure_magic, init_logging, sock_path,
};

static CORE: Mutex<Option<Child>> = Mutex::new(None);

fn main() {
    if let Err(e) = init_logging("mihomo-tray-service") {
        eprintln!("init logging failed: {e:#}");
    }
    if let Err(e) = run() {
        log::error!("mihomo-tray-service error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    prepare_socket_dir()?;
    let path = sock_path();
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("bind {}", path.display()))?;
    // Allow the installing user's group to connect (dir is setgid).
    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o660));

    log::info!("mihomo-tray-service listening on {}", path.display());
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = handle_client(stream) {
                    log::error!("client error: {e:#}");
                }
            }
            Err(e) => log::error!("accept error: {e}"),
        }
    }
    Ok(())
}

fn prepare_socket_dir() -> Result<()> {
    fs::create_dir_all(SOCK_DIR).context("create socket dir")?;
    // Prefer permissions already set by the installer; best-effort otherwise.
    let _ = fs::set_permissions(SOCK_DIR, fs::Permissions::from_mode(0o2770));
    let _ = HELPER_DIR; // keep path constant referenced for docs/consistency
    Ok(())
}

fn handle_client(stream: UnixStream) -> Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).context("read request")?;
    let req: Request = serde_json::from_str(line.trim()).context("parse request")?;
    ensure_magic(req.magic())?;

    let resp = match req {
        Request::Ping { .. } => Response::ok(),
        Request::Status { .. } => Response::status(is_core_running()),
        Request::Stop { .. } => match stop_core() {
            Ok(()) => Response::ok(),
            Err(e) => Response::err(format!("{e:#}")),
        },
        Request::Start {
            core_path,
            config_dir,
            profile_path,
            safe_paths,
            ..
        } => match start_core(&core_path, &config_dir, &profile_path, &safe_paths) {
            Ok(()) => Response::ok(),
            Err(e) => Response::err(format!("{e:#}")),
        },
    };

    let mut stream = reader.into_inner();
    let out = serde_json::to_string(&resp)?;
    stream.write_all(out.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()?;
    Ok(())
}

fn is_core_running() -> bool {
    let mut guard = CORE.lock().unwrap();
    if let Some(child) = guard.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => {
                *guard = None;
                false
            }
            Ok(None) => true,
            Err(_) => {
                *guard = None;
                false
            }
        }
    } else {
        false
    }
}

fn stop_core() -> Result<()> {
    let mut guard = CORE.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
        log::info!("stopped mihomo core");
    }
    Ok(())
}

fn start_core(core: &str, config_dir: &str, profile: &str, safe_paths: &str) -> Result<()> {
    stop_core()?;
    let core_path = Path::new(core);
    if !core_path.exists() {
        bail!("core binary missing: {core}");
    }
    let mut cmd = Command::new(core_path);
    cmd.arg("-d")
        .arg(config_dir)
        .arg("-f")
        .arg(profile)
        .current_dir(config_dir)
        .env("SAFE_PATHS", safe_paths)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    append_core_controller_args(&mut cmd)?;
    let child = cmd
        .spawn()
        .with_context(|| format!("spawn {}", core_path.display()))?;
    *CORE.lock().unwrap() = Some(child);
    // Wait for the local API socket; root-created socket needs group-readable perms.
    wait_core_socket().with_context(|| {
        format!("mihomo started but API socket not ready ({core}, profile={profile})")
    })?;
    log::info!("started mihomo core: {core} (profile={profile})");
    Ok(())
}

fn wait_core_socket() -> Result<()> {
    let path = core_ipc_path();
    for _ in 0..50 {
        {
            let mut guard = CORE.lock().unwrap();
            if let Some(child) = guard.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        *guard = None;
                        bail!("mihomo exited immediately with {status}");
                    }
                    Ok(None) => {}
                    Err(e) => {
                        *guard = None;
                        bail!("failed to poll mihomo process: {e}");
                    }
                }
            } else {
                bail!("mihomo process handle missing");
            }
        }
        if path.exists() {
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o660));
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let _ = stop_core();
    bail!("timed out waiting for {}", path.display())
}
