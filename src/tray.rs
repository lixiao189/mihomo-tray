use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use rust_i18n::t;
use tray_icon::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};

use crate::i18n;
use crate::mihomo::api::{self, ProxyInfo};
use crate::paths;

static ID_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_id(prefix: &str) -> MenuId {
    let n = ID_SEQ.fetch_add(1, Ordering::Relaxed);
    MenuId::new(format!("{prefix}:{n}"))
}

#[derive(Debug, Clone)]
pub enum Action {
    ToggleSystemProxy,
    ToggleTun,
    InstallService,
    UninstallService,
    SpeedTest(String),
    SelectProxy { group: String, name: String },
    SwitchProfile(String),
    ImportProfile,
    OpenConfigFolder,
    SetLocale(&'static str),
    Quit,
}

pub struct MenuIds {
    pub map: HashMap<String, Action>,
}

impl MenuIds {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn track(&mut self, id: &MenuId, action: Action) {
        self.map.insert(id.as_ref().to_string(), action);
    }

    pub fn resolve(&self, id: &MenuId) -> Option<&Action> {
        self.map.get(id.as_ref())
    }
}

pub struct TrayState {
    pub system_proxy: bool,
    pub tun: bool,
    pub service_ok: bool,
    pub service_supported: bool,
    pub groups: Vec<ProxyInfo>,
    pub delays: HashMap<String, HashMap<String, u16>>,
    pub profiles: Vec<std::path::PathBuf>,
    pub active_profile: Option<std::path::PathBuf>,
}

pub fn build_menu(state: &TrayState) -> (Menu, MenuIds) {
    let mut ids = MenuIds::new();
    let menu = Menu::new();

    let sys_id = next_id("sys");
    let sys = CheckMenuItem::with_id(
        sys_id.clone(),
        t!("menu.system_proxy").to_string(),
        true,
        state.system_proxy,
        None,
    );
    ids.track(&sys_id, Action::ToggleSystemProxy);
    let _ = menu.append(&sys);

    let tun_id = next_id("tun");
    let tun = CheckMenuItem::with_id(
        tun_id.clone(),
        t!("menu.tun_mode").to_string(),
        true,
        state.tun,
        None,
    );
    ids.track(&tun_id, Action::ToggleTun);
    let _ = menu.append(&tun);

    if state.service_supported {
        if state.service_ok {
            let un_id = next_id("svc");
            let un = MenuItem::with_id(
                un_id.clone(),
                t!("menu.uninstall_service").to_string(),
                true,
                None,
            );
            ids.track(&un_id, Action::UninstallService);
            let _ = menu.append(&un);
        } else {
            let in_id = next_id("svc");
            let ins = MenuItem::with_id(
                in_id.clone(),
                t!("menu.install_service").to_string(),
                true,
                None,
            );
            ids.track(&in_id, Action::InstallService);
            let _ = menu.append(&ins);
        }
    }

    let _ = menu.append(&PredefinedMenuItem::separator());

    // Proxy groups — each group as a top-level submenu
    if state.groups.is_empty() {
        let empty = MenuItem::new(t!("menu.no_groups").to_string(), false, None);
        let _ = menu.append(&empty);
    } else {
        for group in &state.groups {
            let sub = Submenu::new(&group.name, true);

            let test_id = next_id("speed");
            let test = MenuItem::with_id(
                test_id.clone(),
                t!("menu.speed_test").to_string(),
                true,
                None,
            );
            ids.track(&test_id, Action::SpeedTest(group.name.clone()));
            let _ = sub.append(&test);
            let _ = sub.append(&PredefinedMenuItem::separator());

            let group_delays = state.delays.get(&group.name);
            let now = group.now.as_deref().unwrap_or("");
            let members = group.all.clone().unwrap_or_default();
            for member in members {
                let delay = group_delays.and_then(|m| m.get(&member).copied());
                let label = match delay {
                    Some(0) => t!("menu.node_timeout", name = member.as_str()).to_string(),
                    Some(d) => t!("menu.node_delay", name = member.as_str(), delay = d).to_string(),
                    None => member.clone(),
                };
                let item_id = next_id("node");
                let checked = member == now;
                let item =
                    CheckMenuItem::with_id(item_id.clone(), label, true, checked, None);
                ids.track(
                    &item_id,
                    Action::SelectProxy {
                        group: group.name.clone(),
                        name: member,
                    },
                );
                let _ = sub.append(&item);
            }
            let _ = menu.append(&sub);
        }
    }

    let _ = menu.append(&PredefinedMenuItem::separator());

    // Config + Language together
    let config_menu = Submenu::new(t!("menu.config").to_string(), true);
    for profile in &state.profiles {
        let name = paths::profile_display_name(profile);
        let checked = state
            .active_profile
            .as_ref()
            .is_some_and(|a| a == profile);
        let item_id = next_id("profile");
        let item = CheckMenuItem::with_id(item_id.clone(), &name, true, checked, None);
        ids.track(&item_id, Action::SwitchProfile(name));
        let _ = config_menu.append(&item);
    }
    let _ = config_menu.append(&PredefinedMenuItem::separator());

    let import_id = next_id("import");
    let import = MenuItem::with_id(
        import_id.clone(),
        t!("menu.import_profile").to_string(),
        true,
        None,
    );
    ids.track(&import_id, Action::ImportProfile);
    let _ = config_menu.append(&import);

    let open_id = next_id("open");
    let open = MenuItem::with_id(
        open_id.clone(),
        t!("menu.open_config_folder").to_string(),
        true,
        None,
    );
    ids.track(&open_id, Action::OpenConfigFolder);
    let _ = config_menu.append(&open);
    let _ = menu.append(&config_menu);

    let lang_menu = Submenu::new(t!("menu.language").to_string(), true);
    let zh = i18n::is_zh();
    let zh_id = next_id("lang");
    let zh_item = CheckMenuItem::with_id(
        zh_id.clone(),
        t!("menu.lang_zh").to_string(),
        true,
        zh,
        None,
    );
    ids.track(&zh_id, Action::SetLocale("zh-CN"));
    let _ = lang_menu.append(&zh_item);

    let en_id = next_id("lang");
    let en_item = CheckMenuItem::with_id(
        en_id.clone(),
        t!("menu.lang_en").to_string(),
        true,
        !zh,
        None,
    );
    ids.track(&en_id, Action::SetLocale("en"));
    let _ = lang_menu.append(&en_item);
    let _ = menu.append(&lang_menu);

    let _ = menu.append(&PredefinedMenuItem::separator());

    let quit_id = next_id("quit");
    let quit = MenuItem::with_id(quit_id.clone(), t!("menu.quit").to_string(), true, None);
    ids.track(&quit_id, Action::Quit);
    let _ = menu.append(&quit);

    (menu, ids)
}

pub fn format_group_tooltip(groups: &[ProxyInfo]) -> String {
    if groups.is_empty() {
        return t!("app.name").to_string();
    }
    let mut parts = Vec::new();
    for g in groups.iter().take(3) {
        if let Some(now) = &g.now {
            let delay = api::latest_delay(g)
                .map(|d| format!(" {d}ms"))
                .unwrap_or_default();
            parts.push(format!("{}: {now}{delay}", g.name));
        }
    }
    if parts.is_empty() {
        t!("app.name").to_string()
    } else {
        parts.join(" | ")
    }
}

// Silence unused import warning for IsMenuItem trait usage via append.
#[allow(dead_code)]
fn _use_trait(_: &dyn IsMenuItem) {}
