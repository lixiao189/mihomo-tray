use std::time::Duration;

use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{NSCellImagePosition, NSImage};
use objc2_foundation::{NSData, NSSize};

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
        // Keep tray-icon's bookkeeping (TrayTarget frame sync), then replace the
        // NSImage using ClashX's @2x loading semantics.
        let _ = tray.set_icon_with_as_template(Some(icon), true);
        apply_clashx_menu_icon(tray);
    }
}

/// ClashX loads `menu_icon@2x.png` and treats it as a 16×16 pt template image
/// (`StatusItemView.swift` + `StatusItemView.xib` resource size).
///
/// `tray-icon` forces the status-item image to 18×18 pt, which turns this 32×32
/// asset into ~1.78× scale and looks soft on Retina. Setting size to half the
/// pixel dimensions restores a true 2× representation.
fn apply_clashx_menu_icon(tray: &tray_icon::TrayIcon) {
    const ICON: &[u8] = include_bytes!("../../../assets/menu_icon@2x.png");

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(status_item) = tray.ns_status_item() else {
        return;
    };

    let nsdata = NSData::with_bytes(ICON);
    let Some(nsimage) = NSImage::initWithData(NSImage::alloc(), &nsdata) else {
        return;
    };

    // Pixel size becomes the initial point size when loaded from raw PNG data.
    // Halve it so 32×32 px → 16×16 pt (@2x), matching ClashX's XIB declaration.
    let px = nsimage.size();
    if px.width > 0.0 && px.height > 0.0 {
        nsimage.setSize(NSSize::new(px.width / 2.0, px.height / 2.0));
    }
    nsimage.setTemplate(true);

    let Some(button) = status_item.button(mtm) else {
        return;
    };
    button.setImage(Some(&nsimage));
    button.setImagePosition(NSCellImagePosition::ImageLeft);
}
