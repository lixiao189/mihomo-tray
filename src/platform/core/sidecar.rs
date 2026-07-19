use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use mihomo_tray_service::append_core_controller_args;

use crate::platform::traits::{CoreRunner, PathLayout};

static PROCESS: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Child>> {
    PROCESS.get_or_init(|| Mutex::new(None))
}

pub struct SidecarCoreRunner {
    paths: Arc<dyn PathLayout>,
}

impl SidecarCoreRunner {
    pub fn new(paths: Arc<dyn PathLayout>) -> Self {
        Self { paths }
    }

    fn safe_paths_separator() -> &'static str {
        if cfg!(windows) { ";" } else { ":" }
    }
}

impl CoreRunner for SidecarCoreRunner {
    fn start(&self, core: &PathBuf, profile: &PathBuf) -> Result<()> {
        self.stop().ok();
        let config_dir = self.paths.config_dir()?;
        let mut cmd = Command::new(core);
        cmd.arg("-d")
            .arg(&config_dir)
            .arg("-f")
            .arg(profile)
            .current_dir(&config_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let safe = format!(
            "{}{}{}",
            config_dir.display(),
            Self::safe_paths_separator(),
            profile
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        );
        cmd.env("SAFE_PATHS", safe);
        append_core_controller_args(&mut cmd)?;

        let child = cmd
            .spawn()
            .with_context(|| format!("spawn {}", core.display()))?;
        *slot().lock().unwrap() = Some(child);
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let mut guard = slot().lock().unwrap();
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    fn restart(&self, core: &PathBuf, profile: &PathBuf) -> Result<()> {
        self.stop()?;
        self.start(core, profile)
    }
}
