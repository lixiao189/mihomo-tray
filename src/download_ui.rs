//! Minimal download progress window (softbuffer fill bar).

use std::num::NonZeroU32;
use std::sync::Arc;

use softbuffer::{Context as SbContext, Surface as SbSurface};
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

use crate::platform::InstallProgress;

pub struct ProgressWindow {
    pub window: Arc<Window>,
    context: SbContext<Arc<Window>>,
    surface: SbSurface<Arc<Window>, Arc<Window>>,
    pub fraction: f32,
    pub label: String,
}

impl ProgressWindow {
    pub fn create(event_loop: &ActiveEventLoop) -> anyhow::Result<Self> {
        let title = rust_i18n::t!("dialog.download_title").to_string();
        let attrs = WindowAttributes::default()
            .with_title(title)
            .with_inner_size(LogicalSize::new(420.0, 96.0))
            .with_resizable(false);
        let window = Arc::new(event_loop.create_window(attrs)?);
        let context = SbContext::new(window.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
        let surface =
            SbSurface::new(&context, window.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut pw = Self {
            window,
            context,
            surface,
            fraction: 0.0,
            label: rust_i18n::t!("dialog.download_resolving").to_string(),
        };
        let _ = &pw.context;
        pw.redraw()?;
        Ok(pw)
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn apply(&mut self, progress: &InstallProgress) {
        match progress {
            InstallProgress::Resolving => {
                self.fraction = 0.0;
                self.label = rust_i18n::t!("dialog.download_resolving").to_string();
            }
            InstallProgress::Downloading { downloaded, total } => {
                if let Some(total) = *total {
                    if total > 0 {
                        self.fraction = (*downloaded as f32 / total as f32).clamp(0.0, 1.0);
                        let pct = (self.fraction * 100.0) as u32;
                        self.label = rust_i18n::t!(
                            "dialog.download_progress",
                            percent = pct,
                            downloaded = format_bytes(*downloaded),
                            total = format_bytes(total)
                        )
                        .to_string();
                    }
                } else {
                    self.fraction = 0.15;
                    self.label = rust_i18n::t!(
                        "dialog.download_bytes",
                        downloaded = format_bytes(*downloaded)
                    )
                    .to_string();
                }
            }
            InstallProgress::Extracting => {
                self.fraction = 0.95;
                self.label = rust_i18n::t!("dialog.download_extracting").to_string();
            }
            InstallProgress::Done => {
                self.fraction = 1.0;
                self.label = rust_i18n::t!("dialog.download_done").to_string();
            }
            InstallProgress::Failed(err) => {
                self.label =
                    rust_i18n::t!("dialog.download_failed", error = err.as_str()).to_string();
            }
        }
        let title = format!(
            "{} — {}",
            rust_i18n::t!("dialog.download_title"),
            self.label
        );
        self.window.set_title(&title);
        self.window.request_redraw();
    }

    pub fn redraw(&mut self) -> anyhow::Result<()> {
        let size = self.window.inner_size();
        let w = size.width.max(1);
        let h = size.height.max(1);
        self.surface
            .resize(
                NonZeroU32::new(w).unwrap(),
                NonZeroU32::new(h).unwrap(),
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let bg = 0xFF_F2_F2_F2u32;
        let track = 0xFF_D0_D0_D0u32;
        let fill = 0xFF_2D_6C_DFu32;
        for px in buffer.iter_mut() {
            *px = bg;
        }

        let bar_y0 = h * 40 / 96;
        let bar_y1 = h * 64 / 96;
        let bar_x0 = w * 24 / 420;
        let bar_x1 = w * 396 / 420;
        let bar_w = bar_x1.saturating_sub(bar_x0).max(1);
        let filled = ((bar_w as f32) * self.fraction.clamp(0.0, 1.0)) as u32;

        for y in bar_y0..bar_y1 {
            for x in bar_x0..bar_x1 {
                let idx = (y * w + x) as usize;
                if idx >= buffer.len() {
                    continue;
                }
                let local = x - bar_x0;
                buffer[idx] = if local < filled { fill } else { track };
            }
        }

        buffer.present().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }
}

fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let n = n as f64;
    if n >= MB {
        format!("{:.1} MB", n / MB)
    } else if n >= KB {
        format!("{:.1} KB", n / KB)
    } else {
        format!("{n:.0} B")
    }
}
