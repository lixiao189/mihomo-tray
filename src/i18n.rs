/// Detect OS language and map to a rust-i18n locale (`zh-CN` or `en`).
pub fn init() {
    let locale = detect_locale();
    rust_i18n::set_locale(&locale);
}

pub fn detect_locale() -> String {
    let raw = std::env::var("MIHOMO_TRAY_LANG")
        .or_else(|_| std::env::var("LANG"))
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(sys_locale::get_locale)
        .unwrap_or_else(|| "en".into());
    normalize(&raw)
}

fn normalize(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase().replace('_', "-");
    let primary = lower
        .split(['.', '@'])
        .next()
        .unwrap_or("en")
        .trim();
    if primary.starts_with("zh") {
        "zh-CN".into()
    } else {
        "en".into()
    }
}

pub fn set_locale(locale: &str) {
    let locale = normalize(locale);
    rust_i18n::set_locale(&locale);
}

pub fn is_zh() -> bool {
    rust_i18n::locale().starts_with("zh")
}
