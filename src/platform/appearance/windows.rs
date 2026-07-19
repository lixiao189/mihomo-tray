use std::time::Duration;

use crate::platform::traits::TrayAppearance;

const THEME_POLL_INTERVAL: Duration = Duration::from_secs(5);

pub struct WindowsAppearance;

impl TrayAppearance for WindowsAppearance {
    fn uses_template_icon(&self) -> bool {
        false
    }

    fn tray_background_is_dark(&self) -> bool {
        windows_taskbar_is_dark()
    }

    fn theme_poll_interval(&self) -> Option<Duration> {
        Some(THEME_POLL_INTERVAL)
    }

    fn apply_tray_icon(&self, tray: &tray_icon::TrayIcon, icon: tray_icon::Icon) {
        let _ = tray.set_icon(Some(icon));
    }
}

fn windows_taskbar_is_dark() -> bool {
    // 0 = dark Windows / taskbar, 1 = light. Falls back to AppsUseLightTheme,
    // then assumes light taskbar if the keys are missing.
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    const SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(key) = hkcu.open_subkey(SUBKEY) else {
        return false;
    };

    let system: Result<u32, _> = key.get_value("SystemUsesLightTheme");
    if let Ok(v) = system {
        return v == 0;
    }

    let apps: Result<u32, _> = key.get_value("AppsUseLightTheme");
    matches!(apps, Ok(0))
}
