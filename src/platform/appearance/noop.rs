use std::time::Duration;

use crate::platform::traits::TrayAppearance;

pub struct NoopAppearance;

impl TrayAppearance for NoopAppearance {
    fn uses_template_icon(&self) -> bool {
        false
    }

    fn tray_background_is_dark(&self) -> bool {
        false
    }

    fn theme_poll_interval(&self) -> Option<Duration> {
        None
    }

    fn apply_tray_icon(&self, tray: &tray_icon::TrayIcon, icon: tray_icon::Icon) {
        let _ = tray.set_icon(Some(icon));
    }
}
