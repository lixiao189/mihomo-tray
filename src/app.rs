use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime};

use anyhow::{Context, Result};
use rfd::{MessageButtons, MessageDialog, MessageLevel};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent, menu::MenuEvent};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::window::WindowId;

use crate::config::{self, ActiveConfigWatcher, ProfileMeta};
use crate::download_ui::ProgressWindow;
use crate::mihomo::api::ApiClient;
use crate::platform::{InstallProgress, Platform};
use crate::settings::Settings;
use crate::tray::{self, Action, MenuIds, TrayState};

/// Main-thread-only pointer to `App` for the tray pre-open refresh hook.
struct PreMenuAppPtr(*mut App);

// SAFETY: only written/read on the GUI thread; cleared before `App` is dropped.
unsafe impl Send for PreMenuAppPtr {}
unsafe impl Sync for PreMenuAppPtr {}

/// `tray-icon` invokes the click handler synchronously *before* showing the menu
/// (macOS/Windows). We rebuild there so checkmarks match live `GET /proxies`.
fn pre_menu_app() -> &'static Mutex<Option<PreMenuAppPtr>> {
    static SLOT: OnceLock<Mutex<Option<PreMenuAppPtr>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn register_pre_menu_app(app: &mut App) {
    *pre_menu_app().lock().unwrap() = Some(PreMenuAppPtr(std::ptr::from_mut(app)));
}

fn clear_pre_menu_app() {
    *pre_menu_app().lock().unwrap() = None;
}

fn refresh_tray_before_menu_open() {
    let ptr = pre_menu_app().lock().unwrap().as_ref().map(|p| p.0);
    let Some(ptr) = ptr else {
        return;
    };
    // SAFETY: pointer is set while `App` is owned by the event loop and cleared on Drop.
    // Called only on the GUI thread from the tray click handler before the menu pops up.
    unsafe {
        if let Some(app) = ptr.as_mut() {
            app.on_tray_menu_about_to_open();
        }
    }
}

#[derive(Debug)]
pub enum UserEvent {
    Menu(MenuEvent),
    Tray,
    DownloadProgress(InstallProgress),
    ActiveConfigChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoreMode {
    Sidecar,
    Service,
}

pub struct App {
    platform: Platform,
    proxy: EventLoopProxy<UserEvent>,
    settings: Settings,
    tray: Option<TrayIcon>,
    menu_ids: MenuIds,
    api: Option<ApiClient>,
    core_path: Option<PathBuf>,
    active_profile: Option<PathBuf>,
    profile_meta: Option<ProfileMeta>,
    /// Live runtime flags (may differ from persisted prefs until restored / saved).
    system_proxy: bool,
    tun: bool,
    service_ok: bool,
    core_mode: CoreMode,
    quitting: bool,
    ready: bool,
    progress: Option<ProgressWindow>,
    boot_started: bool,
    tray_bg_dark: bool,
    next_theme_poll: Option<Instant>,
    config_watcher: Option<ActiveConfigWatcher>,
    last_loaded_mtime: Option<SystemTime>,
}

impl App {
    pub fn new(proxy: EventLoopProxy<UserEvent>, platform: Platform, settings: Settings) -> Self {
        let tray_bg_dark = platform.appearance.tray_background_is_dark();
        let next_theme_poll = platform
            .appearance
            .theme_poll_interval()
            .map(|interval| Instant::now() + interval);
        let service_ok = platform.service.is_available();
        Self {
            platform,
            proxy,
            settings,
            tray: None,
            menu_ids: MenuIds {
                map: HashMap::new(),
            },
            api: None,
            core_path: None,
            active_profile: None,
            profile_meta: None,
            system_proxy: false,
            tun: false,
            service_ok,
            core_mode: CoreMode::Sidecar,
            quitting: false,
            ready: false,
            progress: None,
            boot_started: false,
            tray_bg_dark,
            next_theme_poll,
            config_watcher: None,
            last_loaded_mtime: None,
        }
    }

    fn start_bootstrap(&mut self, event_loop: &ActiveEventLoop) {
        if self.boot_started {
            return;
        }
        self.boot_started = true;
        log::info!("bootstrap started");

        match self.platform.installer.needs_install() {
            Ok(true) => {
                log::info!("mihomo core missing; starting download");
                match ProgressWindow::create(event_loop) {
                    Ok(pw) => self.progress = Some(pw),
                    Err(e) => {
                        log::error!("progress window failed: {e:#}");
                    }
                }
                let proxy = self.proxy.clone();
                let installer = std::sync::Arc::clone(&self.platform.installer);
                std::thread::spawn(move || {
                    let (tx, rx) = mpsc::channel::<InstallProgress>();
                    let proxy_fwd = proxy.clone();
                    std::thread::spawn(move || {
                        while let Ok(msg) = rx.recv() {
                            let done = matches!(
                                msg,
                                InstallProgress::Done | InstallProgress::Failed(_)
                            );
                            let _ = proxy_fwd.send_event(UserEvent::DownloadProgress(msg));
                            if done {
                                break;
                            }
                        }
                    });
                    if let Err(e) = installer.ensure_installed_with_progress(Some(tx.clone())) {
                        let _ = tx.send(InstallProgress::Failed(format!("{e:#}")));
                    }
                });
            }
            Ok(false) => {
                if let Err(e) = self.finish_bootstrap() {
                    show_error(&format!("{e:#}"));
                    event_loop.exit();
                }
            }
            Err(e) => {
                show_error(&format!("{e:#}"));
                event_loop.exit();
            }
        }
    }

    fn finish_bootstrap(&mut self) -> Result<()> {
        self.platform.paths.ensure_dirs()?;
        let profile = config::ensure_default_profile()?;
        let meta = config::parse_profile_meta(&profile)?;
        let core_path = self
            .platform
            .installer
            .ensure_installed()
            .context("install mihomo core")?;

        self.platform.core_runner.start(&core_path, &profile)?;
        let api = ApiClient::from_profile(&meta);
        api.wait_ready(40, 250).context("wait for mihomo API")?;

        self.core_path = Some(core_path.clone());
        self.active_profile = Some(profile.clone());
        self.profile_meta = Some(meta);
        self.api = Some(api);
        self.core_mode = CoreMode::Sidecar;
        self.service_ok = self.platform.service.is_available();
        self.ready = true;
        self.last_loaded_mtime = file_mtime(&profile);
        if let Err(e) = self.start_config_watcher(&profile) {
            log::warn!("config watcher failed: {e:#}");
        }
        self.refresh_runtime_flags();
        self.restore_switch_state();
        self.rebuild_tray()?;
        self.progress = None;
        log::info!(
            "bootstrap complete: core={}, profile={}, mode=sidecar",
            core_path.display(),
            profile.display()
        );
        Ok(())
    }

    fn start_config_watcher(&mut self, profile: &Path) -> Result<()> {
        let dir = self.platform.paths.config_dir()?;
        let proxy = self.proxy.clone();
        let watcher = ActiveConfigWatcher::start(dir, profile.to_path_buf(), move || {
            let _ = proxy.send_event(UserEvent::ActiveConfigChanged);
        })?;
        self.config_watcher = Some(watcher);
        Ok(())
    }

    fn rebuild_tray(&mut self) -> Result<()> {
        let api = self.api.as_ref().context("api not ready")?;
        let groups = api.proxy_groups().unwrap_or_default();
        let profiles = self.platform.paths.list_profiles().unwrap_or_default();
        let state = TrayState {
            system_proxy: self.system_proxy,
            tun: self.tun,
            service_ok: self.service_ok,
            service_supported: self.platform.service.supported(),
            groups: groups.clone(),
            profiles,
            active_profile: self.active_profile.clone(),
        };
        let built = tray::build_menu(&state);
        self.menu_ids = built.ids;
        let menu = built.menu;

        let icon = load_icon(self.platform.appearance.as_ref())?;
        let tooltip = tray::format_group_tooltip(&groups);
        self.tray_bg_dark = self.platform.appearance.tray_background_is_dark();

        if let Some(tray) = &mut self.tray {
            tray.set_menu(Some(Box::new(menu)));
            let _ = tray.set_tooltip(Some(tooltip));
            self.platform.appearance.apply_tray_icon(tray, icon);
        } else {
            let mut builder = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip(tooltip)
                .with_icon(icon.clone());
            if self.platform.appearance.uses_template_icon() {
                builder = builder.with_icon_as_template(true);
            }
            let tray = builder.build().context("create tray icon")?;
            self.platform.appearance.apply_tray_icon(&tray, icon);
            self.tray = Some(tray);
        }
        Ok(())
    }

    fn refresh_tray_icon_if_theme_changed(&mut self) {
        let dark = self.platform.appearance.tray_background_is_dark();
        if dark == self.tray_bg_dark {
            return;
        }
        self.tray_bg_dark = dark;
        if let (Some(tray), Ok(icon)) = (
            &self.tray,
            load_icon(self.platform.appearance.as_ref()),
        ) {
            self.platform.appearance.apply_tray_icon(tray, icon);
        }
    }

    /// Sync live proxy selection (and related flags) before the tray menu is shown.
    fn on_tray_menu_about_to_open(&mut self) {
        if !self.ready || self.quitting || self.api.is_none() {
            return;
        }
        self.refresh_runtime_flags();
        self.persist_switch_state();
        if let Err(e) = self.rebuild_tray() {
            log::error!("rebuild tray before menu open failed: {e:#}");
        }
    }

    fn refresh_runtime_flags(&mut self) {
        if let Some(api) = &self.api {
            self.tun = self.platform.tun.is_enabled(api);
        }
        self.system_proxy = self.platform.system_proxy.is_enabled();
        self.service_ok = self.platform.service.is_available();
    }

    /// Mirror live switch flags into `settings.json` (memory + disk).
    fn persist_switch_state(&mut self) {
        if self.settings.system_proxy() == self.system_proxy && self.settings.tun() == self.tun {
            return;
        }
        if let Err(e) = self
            .settings
            .set_switches(self.system_proxy, self.tun)
        {
            log::warn!("save switch state failed: {e:#}");
        }
    }

    /// Re-enable TUN / system proxy from the last session after the core is ready.
    fn restore_switch_state(&mut self) {
        let want_tun = self.settings.tun();
        let want_proxy = self.settings.system_proxy();
        if !want_tun && !want_proxy {
            return;
        }
        // Mutual exclusion: prefer TUN when both flags are set.
        if want_tun {
            log::info!("restoring TUN from previous session");
            match self.set_tun_enabled(true) {
                Ok(()) => self.persist_switch_state(),
                Err(e) => log::error!("restore TUN failed: {e:#}"),
            }
        } else if want_proxy {
            log::info!("restoring system proxy from previous session");
            match self.set_system_proxy_enabled(true) {
                Ok(()) => self.persist_switch_state(),
                Err(e) => log::error!("restore system proxy failed: {e:#}"),
            }
        }
    }

    fn set_system_proxy_enabled(&mut self, enable: bool) -> Result<()> {
        let api = self.api.clone().context("api not ready")?;
        let meta = self.profile_meta.clone();
        if !enable {
            let _ = self.platform.system_proxy.disable();
            self.system_proxy = false;
            log::info!("system proxy disabled");
            return Ok(());
        }
        if self.tun {
            let _ = self.platform.tun.disable(&api);
            self.tun = false;
            if self.core_mode == CoreMode::Service {
                let _ = self.switch_to_sidecar_core();
            }
        }
        // Prefer profile YAML ports; fall back to live /configs (0 = unset).
        let http = meta
            .as_ref()
            .map(|m| m.http_port())
            .unwrap_or_else(|| api.http_port().unwrap_or(7890));
        let socks = meta
            .as_ref()
            .map(|m| m.socks_port())
            .unwrap_or_else(|| api.socks_port().unwrap_or(7890));
        self.platform.system_proxy.enable(http, socks)?;
        self.system_proxy = true;
        log::info!("system proxy enabled (http={http}, socks={socks})");
        Ok(())
    }

    fn set_tun_enabled(&mut self, enable: bool) -> Result<()> {
        let api = self.api.clone().context("api not ready")?;
        if !enable {
            let _ = self.platform.tun.disable(&api);
            self.tun = false;
            log::info!("TUN disabled");
            if self.core_mode == CoreMode::Service {
                if let Err(e) = self.switch_to_sidecar_core() {
                    log::error!("switch to sidecar failed: {e:#}");
                }
            }
            self.service_ok = self.platform.service.is_available();
            return Ok(());
        }
        if self.system_proxy {
            let _ = self.platform.system_proxy.disable();
            self.system_proxy = false;
        }
        if self.platform.tun.requires_privileged_core() {
            self.ensure_service_for_tun()?;
            self.switch_to_service_core()?;
        }
        if let Some(core) = &self.core_path {
            if let Err(e) = self.platform.tun.prepare(core) {
                log::error!("prepare TUN capabilities failed: {e:#}");
            }
        }
        self.platform.tun.enable(&api)?;
        self.tun = self.platform.tun.is_enabled(&api);
        self.service_ok = self.platform.service.is_available();
        log::info!("TUN enabled={}", self.tun);
        Ok(())
    }

    fn ensure_service_for_tun(&mut self) -> Result<()> {
        if !self.platform.service.supported() {
            return Ok(());
        }
        if self.platform.service.is_available() {
            self.service_ok = true;
            return Ok(());
        }
        let ok = MessageDialog::new()
            .set_level(MessageLevel::Info)
            .set_title(rust_i18n::t!("dialog.service_install_title").to_string())
            .set_description(rust_i18n::t!("dialog.service_install_body").to_string())
            .set_buttons(MessageButtons::OkCancel)
            .show();
        if !matches!(ok, rfd::MessageDialogResult::Ok) {
            anyhow::bail!("service install cancelled");
        }
        self.platform
            .service
            .install()
            .context("install privileged service")?;
        self.service_ok = true;
        log::info!("privileged service installed");
        Ok(())
    }

    fn switch_to_service_core(&mut self) -> Result<()> {
        let core = self.core_path.as_ref().context("no core")?.clone();
        let profile = self.active_profile.as_ref().context("no profile")?.clone();
        let _ = self.platform.core_runner.stop();
        self.platform.service.start_core(&core, &profile)?;
        let api = self.api.as_ref().context("no api")?;
        api.wait_ready(40, 250)?;
        self.core_mode = CoreMode::Service;
        log::info!("switched core mode to service");
        Ok(())
    }

    fn switch_to_sidecar_core(&mut self) -> Result<()> {
        let core = self.core_path.as_ref().context("no core")?.clone();
        let profile = self.active_profile.as_ref().context("no profile")?.clone();
        let _ = self.platform.service.stop_core();
        self.platform.core_runner.start(&core, &profile)?;
        let api = self.api.as_ref().context("no api")?;
        api.wait_ready(40, 250)?;
        self.core_mode = CoreMode::Sidecar;
        log::info!("switched core mode to sidecar");
        Ok(())
    }

    fn handle_action(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        if !self.ready && !matches!(action, Action::Quit) {
            return;
        }
        match action {
            Action::ToggleSystemProxy => {
                let enable = !self.system_proxy;
                match self.set_system_proxy_enabled(enable) {
                    Ok(()) => self.persist_switch_state(),
                    Err(e) => log::error!(
                        "{} system proxy failed: {e:#}",
                        if enable { "enable" } else { "disable" }
                    ),
                }
                let _ = self.rebuild_tray();
            }
            Action::ToggleTun => {
                let enable = !self.tun;
                match self.set_tun_enabled(enable) {
                    Ok(()) => self.persist_switch_state(),
                    Err(e) => log::error!(
                        "{} TUN failed: {e:#}",
                        if enable { "enable" } else { "disable" }
                    ),
                }
                let _ = self.rebuild_tray();
            }
            Action::InstallService => {
                if !self.platform.service.supported() {
                    return;
                }
                match self.ensure_service_for_tun() {
                    Ok(()) => log::info!("install service succeeded"),
                    Err(e) => log::error!("install service failed: {e:#}"),
                }
                self.service_ok = self.platform.service.is_available();
                let _ = self.rebuild_tray();
            }
            Action::UninstallService => {
                if !self.platform.service.supported() {
                    return;
                }
                if self.tun {
                    if let Err(e) = self.set_tun_enabled(false) {
                        log::error!("disable TUN before uninstall failed: {e:#}");
                    } else {
                        self.persist_switch_state();
                    }
                }
                if self.core_mode == CoreMode::Service {
                    let _ = self.switch_to_sidecar_core();
                }
                let ok = MessageDialog::new()
                    .set_level(MessageLevel::Warning)
                    .set_title(rust_i18n::t!("dialog.service_uninstall_title").to_string())
                    .set_description(rust_i18n::t!("dialog.service_uninstall_body").to_string())
                    .set_buttons(MessageButtons::OkCancel)
                    .show();
                if matches!(ok, rfd::MessageDialogResult::Ok) {
                    match self.platform.service.uninstall() {
                        Ok(()) => log::info!("privileged service uninstalled"),
                        Err(e) => log::error!("uninstall service failed: {e:#}"),
                    }
                }
                self.service_ok = self.platform.service.is_available();
                let _ = self.rebuild_tray();
            }
            Action::SelectProxy { group, name } => {
                if let Some(api) = &self.api {
                    if let Err(e) = api.select_proxy(&group, &name) {
                        log::error!("select proxy failed: {e:#}");
                    }
                }
                let _ = self.rebuild_tray();
            }
            Action::SwitchProfile(name) => {
                let dir = match self.platform.paths.config_dir() {
                    Ok(d) => d,
                    Err(e) => {
                        log::error!("{e:#}");
                        return;
                    }
                };
                let path = dir.join(&name);
                if !path.exists() {
                    log::error!("profile missing: {name}");
                    return;
                }
                match self.switch_profile(path) {
                    Ok(()) => log::info!("switched profile to {name}"),
                    Err(e) => log::error!("switch profile failed: {e:#}"),
                }
                let _ = self.rebuild_tray();
            }
            Action::ImportProfile => match config::pick_and_import_profile() {
                Ok(Some(path)) => {
                    let name = path.display().to_string();
                    match self.switch_profile(path) {
                        Ok(()) => log::info!("imported and switched profile to {name}"),
                        Err(e) => log::error!("import/switch failed: {e:#}"),
                    }
                    let _ = self.rebuild_tray();
                }
                Ok(None) => {}
                Err(e) => log::error!("import failed: {e:#}"),
            },
            Action::OpenConfigFolder => {
                if let Err(e) = self.platform.paths.open_config_folder() {
                    log::error!("open config folder failed: {e:#}");
                }
            }
            Action::SetLocale(locale) => {
                crate::i18n::set_locale(locale);
                if let Err(e) = self.settings.set_locale(locale) {
                    log::warn!("save locale failed: {e:#}");
                }
                let _ = self.rebuild_tray();
            }
            Action::Quit => {
                self.shutdown();
                event_loop.exit();
            }
        }
    }

    fn switch_profile(&mut self, path: PathBuf) -> Result<()> {
        self.apply_profile(&path)?;
        self.platform.paths.set_active_profile(&path)?;
        if let Some(watcher) = &self.config_watcher {
            watcher.set_active(path);
        }
        Ok(())
    }

    fn reload_active_profile(&mut self) -> Result<()> {
        let path = self
            .active_profile
            .clone()
            .context("no active profile")?;
        let mtime = file_mtime(&path);
        if mtime.is_some() && mtime == self.last_loaded_mtime {
            return Ok(());
        }
        self.apply_profile(&path)?;
        log::info!("auto-reloaded active profile {}", path.display());
        Ok(())
    }

    /// Parse first so a half-written YAML does not clobber running state.
    fn apply_profile(&mut self, path: &Path) -> Result<()> {
        let meta = config::parse_profile_meta(path)?;
        let api = ApiClient::from_profile(&meta);

        // Prefer the live IPC client for reload (same socket; secret may differ).
        let reloaded = self
            .api
            .as_ref()
            .map(|existing| existing.reload_config(&path.display().to_string()).is_ok())
            .unwrap_or(false);

        if !reloaded {
            let core = self.core_path.as_ref().context("no core")?;
            let profile = path.to_path_buf();
            match self.core_mode {
                CoreMode::Sidecar => {
                    self.platform.core_runner.restart(core, &profile)?;
                }
                CoreMode::Service => {
                    self.platform.service.start_core(core, &profile)?;
                }
            }
            api.wait_ready(40, 250)?;
        }

        self.profile_meta = Some(meta);
        self.api = Some(api);
        self.active_profile = Some(path.to_path_buf());
        self.last_loaded_mtime = file_mtime(path);
        Ok(())
    }

    pub fn shutdown(&mut self) {
        if self.quitting {
            return;
        }
        self.quitting = true;
        log::info!("shutting down");
        self.config_watcher.take();
        let _ = self.platform.system_proxy.disable();
        if let Some(api) = &self.api {
            let _ = self.platform.tun.disable(api);
        }
        // Stop core only — never uninstall the LaunchDaemon (Clash Verge pattern).
        match self.core_mode {
            CoreMode::Service => {
                let _ = self.platform.service.stop_core();
            }
            CoreMode::Sidecar => {
                let _ = self.platform.core_runner.stop();
            }
        }
        self.tray.take();
        self.progress.take();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let _ = event_loop;
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        register_pre_menu_app(self);
        if matches!(cause, winit::event::StartCause::Init) {
            self.start_bootstrap(event_loop);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        register_pre_menu_app(self);
        let Some(interval) = self.platform.appearance.theme_poll_interval() else {
            return;
        };
        let now = Instant::now();
        let next = self.next_theme_poll.unwrap_or(now);
        if now >= next {
            self.refresh_tray_icon_if_theme_changed();
            self.next_theme_poll = Some(now + interval);
        }
        if let Some(deadline) = self.next_theme_poll {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(pw) = &self.progress {
            if pw.id() != id {
                return;
            }
        } else {
            return;
        }
        match event {
            WindowEvent::RedrawRequested => {
                if let Some(pw) = &mut self.progress {
                    let _ = pw.redraw();
                }
            }
            WindowEvent::CloseRequested => {
                // Ignore — download should finish; user can Force Quit the app.
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        register_pre_menu_app(self);
        match event {
            UserEvent::Menu(ev) => {
                let action = self.menu_ids.resolve(&ev.id).cloned();
                if let Some(action) = action {
                    self.handle_action(action, event_loop);
                }
            }
            UserEvent::Tray => {}
            UserEvent::ActiveConfigChanged => {
                if !self.ready || self.quitting {
                    return;
                }
                if let Err(e) = self.reload_active_profile() {
                    log::error!("auto-reload config failed: {e:#}");
                    return;
                }
                if let Err(e) = self.rebuild_tray() {
                    log::error!("rebuild tray after auto-reload failed: {e:#}");
                }
            }
            UserEvent::DownloadProgress(progress) => {
                if let Some(pw) = &mut self.progress {
                    pw.apply(&progress);
                }
                match progress {
                    InstallProgress::Done => {
                        if let Err(e) = self.finish_bootstrap() {
                            log::error!("bootstrap after download failed: {e:#}");
                            show_error(&format!("{e:#}"));
                            event_loop.exit();
                        }
                    }
                    InstallProgress::Failed(err) => {
                        log::error!("core download failed: {err}");
                        show_error(&err);
                        event_loop.exit();
                    }
                    _ => {}
                }
            }
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        clear_pre_menu_app();
        self.shutdown();
    }
}

fn show_error(msg: &str) {
    log::error!("{msg}");
    let _ = MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title(rust_i18n::t!("app.name").to_string())
        .set_description(msg)
        .set_buttons(MessageButtons::Ok)
        .show();
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn load_icon(appearance: &dyn crate::platform::TrayAppearance) -> Result<tray_icon::Icon> {
    // Same asset ClashX ships as `menu_icon@2x.png` (32×32 px → 16×16 pt).
    const ICON: &[u8] = include_bytes!("../assets/menu_icon@2x.png");
    let img = image::load_from_memory(ICON)?.into_rgba8();
    let (w, h) = img.dimensions();
    let mut rgba = img.into_raw();

    if !appearance.uses_template_icon() && appearance.tray_background_is_dark() {
        for px in rgba.chunks_exact_mut(4) {
            if px[3] == 0 {
                continue;
            }
            px[0] = 255;
            px[1] = 255;
            px[2] = 255;
        }
    }

    tray_icon::Icon::from_rgba(rgba, w, h).context("create tray icon from rgba")
}

pub fn install_event_handlers(proxy: EventLoopProxy<UserEvent>) {
    let p1 = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = p1.send_event(UserEvent::Menu(event));
    }));
    let p2 = proxy;
    TrayIconEvent::set_event_handler(Some(move |event| {
        // Runs before the platform pops up the menu — refresh from live API first.
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left | MouseButton::Right,
                button_state: MouseButtonState::Down,
                ..
            }
        ) {
            refresh_tray_before_menu_open();
        }
        let _ = p2.send_event(UserEvent::Tray);
    }));
}
