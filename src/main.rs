#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod config;
mod download_ui;
mod i18n;
mod mihomo;
mod paths;
mod platform;
mod tray;

use anyhow::Context;
use winit::event_loop::EventLoop;

use crate::app::{App, UserEvent};
use crate::platform::Platform;

rust_i18n::i18n!("locales", fallback = "en");

fn main() {
    i18n::init();
    if let Err(e) = run() {
        eprintln!("mihomo-tray error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let platform = Platform::default_for_host();

    let mut builder = EventLoop::<UserEvent>::with_user_event();
    platform.event_loop.configure(&mut builder);
    let event_loop = builder.build().context("create event loop")?;

    let proxy = event_loop.create_proxy();
    app::install_event_handlers(proxy.clone());
    let mut application = App::new(proxy, platform);

    event_loop.run_app(&mut application).context("run app")?;
    Ok(())
}
