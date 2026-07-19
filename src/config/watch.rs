use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

/// Watches the config directory and notifies when the active profile file changes.
pub struct ActiveConfigWatcher {
    _debouncer: Debouncer<RecommendedWatcher>,
    active: Arc<Mutex<PathBuf>>,
}

impl ActiveConfigWatcher {
    pub fn start(
        config_dir: PathBuf,
        active: PathBuf,
        on_change: impl Fn() + Send + 'static,
    ) -> Result<Self> {
        let active = Arc::new(Mutex::new(active));
        let filter = Arc::clone(&active);

        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            move |res: DebounceEventResult| {
                let events = match res {
                    Ok(events) => events,
                    Err(e) => {
                        eprintln!("config watch error: {e:?}");
                        return;
                    }
                };
                let active_path = match filter.lock() {
                    Ok(guard) => guard.clone(),
                    Err(_) => return,
                };
                for event in &events {
                    if same_profile(&event.path, &active_path) {
                        on_change();
                        return;
                    }
                }
            },
        )
        .context("create config file debouncer")?;

        debouncer
            .watcher()
            .watch(&config_dir, RecursiveMode::NonRecursive)
            .with_context(|| format!("watch config dir {}", config_dir.display()))?;

        Ok(Self {
            _debouncer: debouncer,
            active,
        })
    }

    pub fn set_active(&self, path: PathBuf) {
        if let Ok(mut guard) = self.active.lock() {
            *guard = path;
        }
    }
}

fn same_profile(event_path: &Path, active: &Path) -> bool {
    if event_path == active {
        return true;
    }
    match (event_path.canonicalize(), active.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => {
            event_path.file_name() == active.file_name()
                && event_path.parent() == active.parent()
        }
    }
}
