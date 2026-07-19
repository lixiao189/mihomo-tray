#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod config;
mod download_ui;
mod i18n;
mod logging;
mod mihomo;
mod paths;
mod platform;
mod settings;
mod tray;

use anyhow::Context;
use winit::event_loop::EventLoop;

use crate::app::{App, UserEvent};
use crate::platform::Platform;

rust_i18n::i18n!("locales", fallback = "en");

fn main() {
    if let Err(e) = logging::init() {
        eprintln!("init logging failed: {e:#}");
    }
    // Detect / load prefs first, then apply locale from the resolved settings.
    let settings = settings::Settings::load_or_init();
    i18n::init(settings.locale());
    if let Err(e) = run(settings) {
        log::error!("mihomo-tray error: {e:#}");
        std::process::exit(1);
    }
}

fn run(settings: settings::Settings) -> anyhow::Result<()> {
    let platform = Platform::default_for_host();

    let mut builder = EventLoop::<UserEvent>::with_user_event();
    platform.event_loop.configure(&mut builder);
    let event_loop = builder.build().context("create event loop")?;

    let proxy = event_loop.create_proxy();
    app::install_event_handlers(proxy.clone());
    let mut application = App::new(proxy, platform, settings);

    event_loop.run_app(&mut application).context("run app")?;
    Ok(())
}
