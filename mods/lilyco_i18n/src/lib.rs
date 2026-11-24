use mod_api::{
    self, ClientHost, Corner, Hooks, InputKeyEvent, KeyAction, KeyConflictContext, KeyRegistration,
    Registrar, RegistrationContext, RenderGuiPostEvent, SettingChangedEvent, SettingSpec, Text,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

const ID: &str = "lilyco_i18n";
const KEY_CYCLE_LOCALE: &str = "cycle_locale";
const LOCALES: [&str; 2] = ["zh-CN", "en-US"];

static LOCALE_IDX: AtomicU32 = AtomicU32::new(0);

fn data_dir() -> PathBuf {
    PathBuf::from("horizon-mods/.horizon-mod-data/lilyco_i18n")
}

fn read_locale_idx() -> usize {
    let raw = fs::read_to_string(data_dir().join("active_locale")).unwrap_or_default();
    match raw.trim() {
        "en-US" => 1,
        _ => 0,
    }
}

fn write_locale_idx(idx: usize) {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    let code = LOCALES[idx % LOCALES.len()];
    let _ = fs::write(dir.join("active_locale"), code);
}

#[derive(Default)]
struct LilycoI18nMod;

#[mod_api::horizon_mod]
impl Hooks for LilycoI18nMod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let idx = read_locale_idx();
        LOCALE_IDX.store(idx as u32, Ordering::Relaxed);

        let opts = LOCALES.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let _ = r.register_setting(SettingSpec::choice(
            "locale",
            "lilyco_i18n.setting.locale",
            opts,
            idx,
        ));

        r.register_key_mapping(
            KeyRegistration::new(KEY_CYCLE_LOCALE, "key.keyboard.l")
                .label("key.lilyco_i18n.cycle_locale")
                .context(KeyConflictContext::InGame),
        );
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, _event: &mut SettingChangedEvent<'_>) {
        let next = host.setting_choice("locale").unwrap_or(0) as usize % LOCALES.len();
        LOCALE_IDX.store(next as u32, Ordering::Relaxed);
        write_locale_idx(next);
    }

    fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
        if !host.on(ID) || event.action != KeyAction::Press || !host.playing() {
            return;
        }
        if event.key == "key.keyboard.l" && !host.is_down(ID, KEY_CYCLE_LOCALE) {
            let next = (LOCALE_IDX.load(Ordering::Relaxed) as usize + 1) % LOCALES.len();
            LOCALE_IDX.store(next as u32, Ordering::Relaxed);
            host.set_setting(ID, "locale", &next.to_string());
            write_locale_idx(next);
            let msg = if next == 0 {
                "§b[Lilyco i18n]§r 已切换 Mod 语言为 §a简体中文 (zh-CN)"
            } else {
                "§b[Lilyco i18n]§r Switched mod locale to §aEnglish (en-US)"
            };
            let _ = host.chat_line(Text::literal(msg.to_string()));
        }
    }

    fn render_gui_post(&self, host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
        if !host.on(ID) {
            return;
        }
        let idx = read_locale_idx();
        let label = if idx == 0 {
            "Lilyco i18n: 简体中文 [L切换]"
        } else {
            "Lilyco i18n: EN-US [L]"
        };
        let draw = &mut *event.draw;
        let w = draw.text_width(label) + 8.0;
        let h = 14.0;
        let (x, y) = event.corners.place(Corner::TopRight, w, h);
        draw.fill([x, y, w, h], 0x7710_1824);
        draw.fill([x, y, 2.5, h], 0xFF38_BDF8);
        draw.text(label, [x + 5.0, y + 3.0], 0xFFE2_E8F0, true);
    }
}
