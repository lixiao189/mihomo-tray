use std::time::Duration;

use crate::platform::traits::TrayAppearance;

pub struct MacosAppearance;

impl TrayAppearance for MacosAppearance {
    fn uses_template_icon(&self) -> bool {
        true
    }

    fn tray_background_is_dark(&self) -> bool {
        false
    }

    fn theme_poll_interval(&self) -> Option<Duration> {
        None
    }

    fn apply_tray_icon(&self, tray: &tray_icon::TrayIcon, icon: tray_icon::Icon) {
        let _ = tray.set_icon_with_as_template(Some(icon), true);
    }
}
