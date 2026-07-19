use std::time::Duration;

use crate::platform::traits::TrayAppearance;

const THEME_POLL_INTERVAL: Duration = Duration::from_secs(5);

pub struct LinuxAppearance;

impl TrayAppearance for LinuxAppearance {
    fn uses_template_icon(&self) -> bool {
        false
    }

    fn tray_background_is_dark(&self) -> bool {
        matches!(dark_light::detect(), Ok(dark_light::Mode::Dark))
    }

    fn theme_poll_interval(&self) -> Option<Duration> {
        Some(THEME_POLL_INTERVAL)
    }

    fn apply_tray_icon(&self, tray: &tray_icon::TrayIcon, icon: tray_icon::Icon) {
        let _ = tray.set_icon(Some(icon));
    }
}
