use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum InputMethod {
    #[default]
    Telex,
    Vni,
    Viqr,
    SimpleTelex,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Charset {
    #[default]
    Unicode,
    Tcvn3,
    VniWindows,
    UnicodeComposite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Mechanism {
    #[default]
    KeyboardHook,
    CompatibleHook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SwitchPreset {
    #[default]
    CtrlShift,
    AltZ,
    CtrlSpace,
    ShiftShift,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotkeyCombo {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win_cmd: bool,
    pub key_code: u32,
}

impl Default for HotkeyCombo {
    fn default() -> Self {
        Self {
            ctrl: true,
            alt: false,
            shift: true,
            win_cmd: false,
            key_code: 0x75,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppBehavior {
    #[default]
    Default,
    ForceEnglish,
    ForceVietnamese,
    Blacklist,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRule {
    pub process_name: String,
    pub behavior: AppBehavior,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroEntry {
    pub shortcut: String,
    pub expansion: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomKeyMap {
    pub acute: char,
    pub grave: char,
    pub hook: char,
    pub tilde: char,
    pub dot: char,
    pub remove_tone: char,
    pub circumflex_a: char,
    pub circumflex_e: char,
    pub circumflex_o: char,
    pub horn_o: char,
    pub horn_u: char,
    pub breve_a: char,
    pub d_stroke: char,
    pub quick_u_horn: char,
    pub quick_o_horn_key: char,
    pub quick_u_horn_key: char,
}

impl Default for CustomKeyMap {
    fn default() -> Self {
        Self {
            acute: 's',
            grave: 'f',
            hook: 'r',
            tilde: 'x',
            dot: 'j',
            remove_tone: 'z',
            circumflex_a: 'a',
            circumflex_e: 'e',
            circumflex_o: 'o',
            horn_o: 'w',
            horn_u: 'w',
            breve_a: 'w',
            d_stroke: 'd',
            quick_u_horn: 'w',
            quick_o_horn_key: '[',
            quick_u_horn_key: ']',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub vietnamese_enabled: bool,
    pub charset: Charset,
    pub input_method: InputMethod,
    pub mechanism: Mechanism,
    pub modern_tone: bool,
    pub free_tone: bool,
    pub allow_fjwz: bool,
    pub auto_capitalize_sentence: bool,
    pub use_clipboard_send: bool,
    pub standard_key_send: bool,
    pub spelling_check: bool,
    pub auto_restore_wrong_word: bool,
    pub disable_on_non_us_layout: bool,
    pub fix_browser_suggest_1: bool,
    pub fix_browser_suggest_2: bool,
    pub macro_enabled: bool,
    pub macro_when_english: bool,
    pub quick_w_to_u_horn: bool,
    pub quick_brackets: bool,
    pub quick_telex: bool,
    pub quick_start_consonant: bool,
    pub quick_end_consonant: bool,
    pub esc_restore: bool,
    pub switch_preset: SwitchPreset,
    pub custom_switch_hotkey: HotkeyCombo,
    pub clipboard_convert_hotkey: HotkeyCombo,
    pub beep_on_switch: bool,
    pub show_switch_osd: bool,
    pub paste_via_shift_insert: bool,
    pub send_delay_ms: u8,
    pub auto_start: bool,
    pub run_as_admin: bool,
    pub show_on_startup: bool,
    pub theme: ThemeMode,
    pub smart_app_switch: bool,
    pub app_rules: Vec<AppRule>,
    pub macros: Vec<MacroEntry>,
    pub user_dict_words: Vec<String>,
    pub custom_keymap: CustomKeyMap,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vietnamese_enabled: true,
            charset: Charset::Unicode,
            input_method: InputMethod::Telex,
            mechanism: Mechanism::KeyboardHook,
            modern_tone: true,
            free_tone: true,
            allow_fjwz: true,
            auto_capitalize_sentence: false,
            use_clipboard_send: false,
            standard_key_send: false,
            spelling_check: true,
            auto_restore_wrong_word: true,
            disable_on_non_us_layout: true,
            fix_browser_suggest_1: true,
            fix_browser_suggest_2: false,
            macro_enabled: true,
            macro_when_english: false,
            quick_w_to_u_horn: true,
            quick_brackets: true,
            quick_telex: false,
            quick_start_consonant: false,
            quick_end_consonant: false,
            esc_restore: true,
            switch_preset: SwitchPreset::CtrlShift,
            custom_switch_hotkey: HotkeyCombo::default(),
            clipboard_convert_hotkey: HotkeyCombo {
                ctrl: true,
                alt: false,
                shift: true,
                win_cmd: false,
                key_code: 0x75,
            },
            beep_on_switch: false,
            show_switch_osd: true,
            paste_via_shift_insert: false,
            send_delay_ms: 2,
            auto_start: true,
            run_as_admin: false,
            show_on_startup: false,
            theme: ThemeMode::System,
            smart_app_switch: true,
            app_rules: Vec::new(),
            macros: Vec::new(),
            user_dict_words: Vec::new(),
            custom_keymap: CustomKeyMap::default(),
        }
    }
}

impl Config {
    pub fn from_json_str(raw: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(raw)
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn behavior_for_app(&self, process_name: &str) -> AppBehavior {
        let needle = process_name.trim();
        for rule in &self.app_rules {
            if rule.process_name.eq_ignore_ascii_case(needle) {
                return rule.behavior;
            }
        }
        for rule in &self.app_rules {
            if rule.process_name.contains('*') && wildcard_match(&rule.process_name, needle) {
                return rule.behavior;
            }
        }
        AppBehavior::Default
    }

    pub fn set_app_rule(&mut self, process_name: &str, behavior: AppBehavior) {
        let clean = process_name.trim().to_string();
        if clean.is_empty() {
            return;
        }
        if let Some(existing) = self
            .app_rules
            .iter_mut()
            .find(|r| r.process_name.eq_ignore_ascii_case(&clean))
        {
            if behavior == AppBehavior::Default {
                let target = clean.clone();
                self.app_rules
                    .retain(|r| !r.process_name.eq_ignore_ascii_case(&target));
            } else {
                existing.behavior = behavior;
            }
            return;
        }
        if behavior != AppBehavior::Default {
            self.app_rules.push(AppRule {
                process_name: clean,
                behavior,
            });
        }
    }

    pub fn import_evkey_tree(content: &str) -> Self {
        let mut config = Self::default();
        let mut section = "";
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_suffix('{') {
                section = name.trim();
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim();
                continue;
            }
            if line == "}" {
                section = "";
                continue;
            }
            match section {
                "setting" => {
                    if let Some((k, v)) = line.split_once('=') {
                        apply_evkey_setting(&mut config, k.trim(), v.trim());
                    }
                }
                "blacklist" => {
                    config.set_app_rule(line, AppBehavior::Blacklist);
                }
                "off_vn" => {
                    config.set_app_rule(line, AppBehavior::ForceEnglish);
                }
                "on_vn" => {
                    config.set_app_rule(line, AppBehavior::ForceVietnamese);
                }
                "macro" => {
                    if let Some((k, v)) = line.split_once(':').or_else(|| line.split_once('=')) {
                        let shortcut = k.trim().to_string();
                        if !shortcut.is_empty() {
                            config.macros.push(MacroEntry {
                                shortcut,
                                expansion: v.replace("\\n", "\n"),
                                enabled: true,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        config
    }

    pub fn format_macro_text(&self) -> String {
        let mut out = String::from("# MS Key Macros\n");
        for m in &self.macros {
            if !m.shortcut.is_empty() {
                out.push_str(&m.shortcut);
                out.push(':');
                out.push_str(&m.expansion.replace('\n', "\\n"));
                out.push('\n');
            }
        }
        out
    }

    pub fn parse_macro_text(content: &str) -> Vec<MacroEntry> {
        let mut entries = Vec::new();
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some((left, right)) = line.split_once(':') {
                let shortcut = left.trim().to_string();
                let expansion = right.replace("\\n", "\n");
                if !shortcut.is_empty() {
                    entries.push(MacroEntry {
                        shortcut,
                        expansion,
                        enabled: true,
                    });
                }
            }
        }
        entries
    }
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<u8> = pattern.bytes().map(|b| b.to_ascii_lowercase()).collect();
    let t: Vec<u8> = text.bytes().map(|b| b.to_ascii_lowercase()).collect();
    let (mut pi, mut ti, mut star, mut match_ti) = (0usize, 0usize, None, 0usize);
    while ti < t.len() {
        if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            match_ti = ti;
            pi += 1;
        } else if let Some(sp) = star {
            pi = sp + 1;
            match_ti += 1;
            ti = match_ti;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

fn apply_evkey_setting(config: &mut Config, key: &str, val: &str) {
    let flag = val == "1";
    match key {
        "Lang" => config.vietnamese_enabled = flag,
        "Type" => {
            config.input_method = match val {
                "1" => InputMethod::Telex,
                "2" => InputMethod::Vni,
                "3" => InputMethod::Viqr,
                "4" => InputMethod::SimpleTelex,
                "5" => InputMethod::Custom,
                _ => InputMethod::Telex,
            };
        }
        "Codec" => {
            config.charset = match val {
                "4" => Charset::Unicode,
                "2" => Charset::Tcvn3,
                "3" => Charset::VniWindows,
                "5" => Charset::UnicodeComposite,
                _ => Charset::Unicode,
            };
        }
        "SModern" => config.modern_tone = flag,
        "FMark" => config.free_tone = flag,
        "SConso" => config.allow_fjwz = flag,
        "AUpper" => config.auto_capitalize_sentence = flag,
        "UseSpCase" => config.use_clipboard_send = flag,
        "SInput" => config.standard_key_send = flag,
        "SCheck" => config.spelling_check = flag,
        "AResKey" => config.auto_restore_wrong_word = flag,
        "SBrowser" => config.fix_browser_suggest_1 = flag,
        "CSuggestion" => config.fix_browser_suggest_2 = flag,
        "EMacro" => config.macro_enabled = flag,
        "AMarcro" => config.macro_when_english = flag,
        "AutoStart" => config.auto_start = flag,
        "RAdmin" => config.run_as_admin = flag,
        "SGui" => config.show_on_startup = flag,
        "ThemeColor" => {
            config.theme = match val {
                "0" => ThemeMode::Light,
                "1" => ThemeMode::Dark,
                _ => ThemeMode::System,
            };
        }
        "EApp" => config.smart_app_switch = flag,
        "QTelex" | "FastTelex" => config.quick_telex = flag,
        "SStartConsonant" => config.quick_start_consonant = flag,
        "SEndConsonant" => config.quick_end_consonant = flag,
        "Beep" => config.beep_on_switch = flag,
        "SwitchLangEN" => config.disable_on_non_us_layout = flag,
        "EscRestore" => config.esc_restore = flag,
        "QW" => config.quick_w_to_u_horn = flag,
        "QBracket" => config.quick_brackets = flag,
        "ShowOSD" => config.show_switch_osd = flag,
        "SKey" => {
            config.switch_preset = match val {
                "1" => SwitchPreset::AltZ,
                "4" => SwitchPreset::ShiftShift,
                "5" => SwitchPreset::CtrlSpace,
                _ => SwitchPreset::CtrlShift,
            };
        }
        _ => {}
    }
}
