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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HoldEnglishKey {
    #[default]
    None,
    RightShift,
    LeftShift,
    RightAlt,
    RightCtrl,
    CapsLock,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppRule {
    pub process_name: String,
    pub title_pattern: String,
    pub behavior: AppBehavior,
}

impl Default for AppRule {
    fn default() -> Self {
        Self {
            process_name: String::new(),
            title_pattern: String::new(),
            behavior: AppBehavior::Default,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigProfile {
    pub name: String,
    pub modern_tone: bool,
    pub free_tone: bool,
    pub allow_fjwz: bool,
    pub auto_capitalize_sentence: bool,
    pub spelling_check: bool,
    pub auto_restore_wrong_word: bool,
    pub smart_code_bypass: bool,
    pub macro_enabled: bool,
    pub macro_when_english: bool,
    pub macro_trigger_tab_only: bool,
    pub quick_w_to_u_horn: bool,
    pub quick_brackets: bool,
    pub quick_telex: bool,
    pub quick_start_consonant: bool,
    pub quick_end_consonant: bool,
}

impl Default for ConfigProfile {
    fn default() -> Self {
        Self {
            name: "Mặc định".to_string(),
            modern_tone: true,
            free_tone: true,
            allow_fjwz: true,
            auto_capitalize_sentence: false,
            spelling_check: true,
            auto_restore_wrong_word: true,
            smart_code_bypass: true,
            macro_enabled: true,
            macro_when_english: false,
            macro_trigger_tab_only: false,
            quick_w_to_u_horn: true,
            quick_brackets: true,
            quick_telex: false,
            quick_start_consonant: false,
            quick_end_consonant: false,
        }
    }
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
    pub app_rules: Vec<AppRule>,
    pub macros: Vec<MacroEntry>,
    pub user_dict_words: Vec<String>,
    pub custom_keymap: CustomKeyMap,
    pub smart_code_bypass: bool,
    pub macro_trigger_tab_only: bool,
    pub hold_english_key: HoldEnglishKey,
    pub quick_capslock_switch: bool,
    pub undo_last_word_hotkey: HotkeyCombo,
    pub convert_selection_hotkey: HotkeyCombo,
    pub show_caret_osd: bool,
    pub reedit_surrounding_word: bool,
    pub check_updates_on_start: bool,
    pub active_profile: String,
    pub profiles: Vec<ConfigProfile>,
    pub sync_config_path: String,
    #[serde(default)]
    pub config_version: u32,
}

pub const CONFIG_VERSION: u32 = 1;

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
            show_switch_osd: false,
            paste_via_shift_insert: false,
            send_delay_ms: 2,
            auto_start: true,
            run_as_admin: false,
            show_on_startup: false,
            theme: ThemeMode::System,
            app_rules: Vec::new(),
            macros: Vec::new(),
            user_dict_words: Vec::new(),
            custom_keymap: CustomKeyMap::default(),
            smart_code_bypass: true,
            macro_trigger_tab_only: false,
            hold_english_key: HoldEnglishKey::None,
            quick_capslock_switch: false,
            undo_last_word_hotkey: HotkeyCombo {
                ctrl: true,
                alt: false,
                shift: true,
                win_cmd: false,
                key_code: 0x5A,
            },
            convert_selection_hotkey: HotkeyCombo {
                ctrl: true,
                alt: false,
                shift: true,
                win_cmd: false,
                key_code: 0x76,
            },
            show_caret_osd: false,
            reedit_surrounding_word: true,
            check_updates_on_start: false,
            active_profile: "Mặc định".to_string(),
            profiles: Vec::new(),
            sync_config_path: String::new(),
            config_version: CONFIG_VERSION,
        }
    }
}

impl Config {
    pub fn from_json_str(raw: &str) -> Result<Self, serde_json::Error> {
        let mut cfg: Self = serde_json::from_str(raw)?;
        cfg.migrate();
        Ok(cfg)
    }

    pub fn migrate(&mut self) {
        if self.config_version < 1 {
            self.show_switch_osd = false;
            self.show_caret_osd = false;
        }
        self.config_version = self.config_version.max(CONFIG_VERSION);
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn behavior_for_app(&self, process_name: &str) -> AppBehavior {
        self.behavior_for_window(process_name, "")
    }

    pub fn behavior_for_window(&self, process_name: &str, window_title: &str) -> AppBehavior {
        let needle = process_name.trim();
        let title = window_title.trim();
        if !title.is_empty() {
            for rule in &self.app_rules {
                if !rule.title_pattern.trim().is_empty()
                    && (rule.process_name.trim().is_empty()
                        || rule.process_name.trim() == "*"
                        || rule.process_name.eq_ignore_ascii_case(needle))
                    && wildcard_match(rule.title_pattern.trim(), title)
                {
                    return rule.behavior;
                }
            }
            for rule in &self.app_rules {
                if !rule.title_pattern.trim().is_empty()
                    && rule.process_name.contains('*')
                    && wildcard_match(&rule.process_name, needle)
                    && wildcard_match(rule.title_pattern.trim(), title)
                {
                    return rule.behavior;
                }
            }
        }
        for rule in &self.app_rules {
            if rule.title_pattern.trim().is_empty() && rule.process_name.eq_ignore_ascii_case(needle) {
                return rule.behavior;
            }
        }
        for rule in &self.app_rules {
            if rule.title_pattern.trim().is_empty()
                && rule.process_name.contains('*')
                && wildcard_match(&rule.process_name, needle)
            {
                return rule.behavior;
            }
        }
        AppBehavior::Default
    }

    pub fn set_app_rule(&mut self, process_name: &str, behavior: AppBehavior) {
        self.set_window_rule(process_name, "", behavior);
    }

    pub fn set_window_rule(
        &mut self,
        process_name: &str,
        title_pattern: &str,
        behavior: AppBehavior,
    ) {
        let clean_proc = process_name.trim().to_string();
        let clean_title = title_pattern.trim().to_string();
        if clean_proc.is_empty() && clean_title.is_empty() {
            return;
        }
        if let Some(existing) = self.app_rules.iter_mut().find(|r| {
            r.process_name.eq_ignore_ascii_case(&clean_proc)
                && r.title_pattern.eq_ignore_ascii_case(&clean_title)
        }) {
            if behavior == AppBehavior::Default {
                self.app_rules.retain(|r| {
                    !(r.process_name.eq_ignore_ascii_case(&clean_proc)
                        && r.title_pattern.eq_ignore_ascii_case(&clean_title))
                });
            } else {
                existing.behavior = behavior;
            }
            return;
        }
        if behavior != AppBehavior::Default {
            self.app_rules.push(AppRule {
                process_name: clean_proc,
                title_pattern: clean_title,
                behavior,
            });
        }
    }

    pub fn builtin_profile_names() -> [&'static str; 3] {
        ["Mặc định", "Lập trình", "Soạn thảo"]
    }

    pub fn all_profile_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Self::builtin_profile_names()
            .into_iter()
            .map(ToOwned::to_owned)
            .collect();
        for p in &self.profiles {
            if !p.name.trim().is_empty()
                && !names.iter().any(|n| n.eq_ignore_ascii_case(p.name.trim()))
            {
                names.push(p.name.trim().to_string());
            }
        }
        names
    }

    pub fn snapshot_profile(&self, name: &str) -> ConfigProfile {
        ConfigProfile {
            name: name.trim().to_string(),
            modern_tone: self.modern_tone,
            free_tone: self.free_tone,
            allow_fjwz: self.allow_fjwz,
            auto_capitalize_sentence: self.auto_capitalize_sentence,
            spelling_check: self.spelling_check,
            auto_restore_wrong_word: self.auto_restore_wrong_word,
            smart_code_bypass: self.smart_code_bypass,
            macro_enabled: self.macro_enabled,
            macro_when_english: self.macro_when_english,
            macro_trigger_tab_only: self.macro_trigger_tab_only,
            quick_w_to_u_horn: self.quick_w_to_u_horn,
            quick_brackets: self.quick_brackets,
            quick_telex: self.quick_telex,
            quick_start_consonant: self.quick_start_consonant,
            quick_end_consonant: self.quick_end_consonant,
        }
    }

    pub fn save_current_as_profile(&mut self, name: &str) {
        let clean = name.trim();
        if clean.is_empty() {
            return;
        }
        let snap = self.snapshot_profile(clean);
        if let Some(ex) = self
            .profiles
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(clean))
        {
            *ex = snap;
        } else {
            self.profiles.push(snap);
        }
        self.active_profile = clean.to_string();
    }

    pub fn apply_profile(&mut self, name: &str) -> bool {
        let clean = name.trim();
        let profile = if let Some(custom) = self
            .profiles
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(clean))
            .cloned()
        {
            custom
        } else if clean.eq_ignore_ascii_case("Lập trình") || clean.eq_ignore_ascii_case("coding") {
            ConfigProfile {
                name: "Lập trình".to_string(),
                auto_capitalize_sentence: false,
                smart_code_bypass: true,
                auto_restore_wrong_word: true,
                macro_when_english: true,
                macro_trigger_tab_only: true,
                quick_brackets: false,
                ..ConfigProfile::default()
            }
        } else if clean.eq_ignore_ascii_case("Soạn thảo") || clean.eq_ignore_ascii_case("writing") {
            ConfigProfile {
                name: "Soạn thảo".to_string(),
                auto_capitalize_sentence: true,
                smart_code_bypass: true,
                spelling_check: true,
                auto_restore_wrong_word: true,
                macro_trigger_tab_only: false,
                quick_brackets: true,
                ..ConfigProfile::default()
            }
        } else if clean.eq_ignore_ascii_case("Mặc định") || clean.eq_ignore_ascii_case("default") {
            ConfigProfile::default()
        } else {
            return false;
        };
        self.modern_tone = profile.modern_tone;
        self.free_tone = profile.free_tone;
        self.allow_fjwz = profile.allow_fjwz;
        self.auto_capitalize_sentence = profile.auto_capitalize_sentence;
        self.spelling_check = profile.spelling_check;
        self.auto_restore_wrong_word = profile.auto_restore_wrong_word;
        self.smart_code_bypass = profile.smart_code_bypass;
        self.macro_enabled = profile.macro_enabled;
        self.macro_when_english = profile.macro_when_english;
        self.macro_trigger_tab_only = profile.macro_trigger_tab_only;
        self.quick_w_to_u_horn = profile.quick_w_to_u_horn;
        self.quick_brackets = profile.quick_brackets;
        self.quick_telex = profile.quick_telex;
        self.quick_start_consonant = profile.quick_start_consonant;
        self.quick_end_consonant = profile.quick_end_consonant;
        self.active_profile = profile.name;
        true
    }

    pub fn format_user_dict_text(&self) -> String {
        let mut out = String::from("# MS Key User Dictionary\n");
        for w in &self.user_dict_words {
            let t = w.trim();
            if !t.is_empty() {
                out.push_str(t);
                out.push('\n');
            }
        }
        out
    }

    pub fn parse_user_dict_text(content: &str) -> Vec<String> {
        let mut words = Vec::new();
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if !words.iter().any(|w: &String| w.eq_ignore_ascii_case(line)) {
                words.push(line.to_string());
            }
        }
        words
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
