use mskey_core::{Charset, Config, HoldEnglishKey, InputMethod, Mechanism, SwitchPreset, ThemeMode};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CB_SETCURSEL, SW_HIDE, SW_SHOW, SendMessageW, ShowWindow,
};

use crate::dialog_helpers::{
    format_hotkey, make_child, make_combo, make_edit, make_owner_button, read_window_text,
    reset_combo_items, set_cue_banner, write_window_text,
};
use crate::platform::is_elevated;
use crate::state::with_state;

pub const IDC_CHARSET: usize = 1001;
pub const IDC_METHOD: usize = 1002;
pub const IDC_MECHANISM: usize = 1003;
pub const IDC_BTN_CUSTOM_KEY: usize = 1004;
pub const IDC_VE_PILL: usize = 1005;
pub const IDC_BTN_CONVERT: usize = 1006;
pub const IDC_LBL_ACTIVE_APP: usize = 1007;

pub const IDC_BTN_CLOSE: usize = 1010;
pub const IDC_BTN_EXIT: usize = 1011;

pub const IDC_TAB_BASIC: usize = 1020;
pub const IDC_TAB_HOTKEY: usize = 1021;
pub const IDC_TAB_SYSTEM: usize = 1022;
pub const IDC_LBL_PROFILE: usize = 1023;
pub const IDC_PROFILE_COMBO: usize = 1024;
pub const IDC_BTN_SAVE_PROFILE: usize = 1025;

pub const IDC_CHK_MODERN: usize = 1101;
pub const IDC_CHK_FREE_TONE: usize = 1102;
pub const IDC_CHK_FJWZ: usize = 1103;
pub const IDC_CHK_AUTOCAP: usize = 1104;
pub const IDC_CHK_CLIP_SEND: usize = 1105;
pub const IDC_CHK_STD_SEND: usize = 1106;
pub const IDC_CHK_SPELL: usize = 1107;
pub const IDC_CHK_RESTORE: usize = 1108;
pub const IDC_CHK_NON_US: usize = 1109;
pub const IDC_CHK_SUGGEST1: usize = 1110;
pub const IDC_CHK_SUGGEST2: usize = 1111;
pub const IDC_LBL_SUGGEST_TIP: usize = 1112;
pub const IDC_CHK_SMART_CODE: usize = 1113;
pub const IDC_CHK_MACRO: usize = 1120;
pub const IDC_BTN_MACRO: usize = 1121;
pub const IDC_CHK_MACRO_EN: usize = 1122;
pub const IDC_CHK_QUICK_W: usize = 1123;
pub const IDC_CHK_QUICK_BRK: usize = 1124;
pub const IDC_BTN_EXTRAS: usize = 1125;
pub const IDC_CHK_MACRO_TAB: usize = 1126;

pub const IDC_RAD_CTRL_SHIFT: usize = 1201;
pub const IDC_RAD_ALT_Z: usize = 1202;
pub const IDC_RAD_CUSTOM_KEY: usize = 1203;
pub const IDC_RAD_CTRL_SPACE: usize = 1212;
pub const IDC_RAD_SHIFT_SHIFT: usize = 1213;
pub const IDC_LBL_SWITCH_CUSTOM: usize = 1204;
pub const IDC_BTN_CAP_SWITCH: usize = 1205;
pub const IDC_CHK_ESC_RESTORE: usize = 1206;
pub const IDC_CHK_BEEP: usize = 1207;
pub const IDC_LBL_CONV_HOTKEY: usize = 1208;
pub const IDC_BTN_CAP_CONV: usize = 1209;
pub const IDC_BTN_IMPORT_TREE: usize = 1210;
pub const IDC_BTN_OPEN_CFG_DIR: usize = 1211;
pub const IDC_CHK_QUICK_CAPS: usize = 1214;
pub const IDC_LBL_HOLD_EN: usize = 1215;
pub const IDC_HOLD_EN_COMBO: usize = 1216;
pub const IDC_LBL_UNDO_HOTKEY: usize = 1217;
pub const IDC_BTN_CAP_UNDO: usize = 1218;
pub const IDC_LBL_CONV_SEL_HOTKEY: usize = 1219;
pub const IDC_BTN_CAP_CONV_SEL: usize = 1220;

pub const IDC_CHK_AUTOSTART: usize = 1301;
pub const IDC_CHK_ADMIN: usize = 1302;
pub const IDC_BTN_RELAUNCH_ADMIN: usize = 1303;
pub const IDC_CHK_SHOW_START: usize = 1304;
pub const IDC_LBL_THEME: usize = 1305;
pub const IDC_THEME: usize = 1306;
pub const IDC_CHK_SMART_APP: usize = 1307;
pub const IDC_LBL_CUR_APP: usize = 1308;
pub const IDC_BTN_APPS: usize = 1309;
pub const IDC_BTN_ADD_CUR_EN: usize = 1310;
pub const IDC_LBL_APP_HINT: usize = 1311;
pub const IDC_BTN_USER_DICT: usize = 1312;
pub const IDC_CHK_CARET_OSD: usize = 1313;
pub const IDC_CHK_CHECK_UPDATES: usize = 1314;
pub const IDC_BTN_CHECK_UPDATE: usize = 1315;
pub const IDC_LBL_SYNC_PATH: usize = 1316;
pub const IDC_EDIT_SYNC_PATH: usize = 1317;
pub const IDC_BTN_BROWSE_SYNC: usize = 1318;

pub const IDC_BTN_DEFAULT: usize = 1401;
pub const IDC_BTN_SAVE: usize = 1402;
pub const IDC_LBL_FOOTER_VER: usize = 1403;

pub struct MainUiCtrls {
    pub active_tab: usize,
    pub cb_charset: HWND,
    pub cb_method: HWND,
    pub cb_mech: HWND,
    pub cb_profile: HWND,
    pub cb_hold_en: HWND,
    pub cb_theme: HWND,
    pub ve_pill: HWND,
    pub lbl_active_app: HWND,
    pub lbl_switch_custom: HWND,
    pub lbl_conv_hotkey: HWND,
    pub lbl_undo_hotkey: HWND,
    pub lbl_conv_sel_hotkey: HWND,
    pub lbl_cur_app: HWND,
    pub edit_sync_path: HWND,
    pub profile_items: Vec<String>,
    pub tab0: Vec<HWND>,
    pub tab1: Vec<HWND>,
    pub tab2: Vec<HWND>,
}

struct HeaderHandles {
    cb_charset: HWND,
    cb_method: HWND,
    cb_mech: HWND,
    cb_profile: HWND,
    ve_pill: HWND,
    lbl_active_app: HWND,
}

struct Tab1Handles {
    ctrls: Vec<HWND>,
    lbl_switch_custom: HWND,
    lbl_conv_hotkey: HWND,
    lbl_undo_hotkey: HWND,
    lbl_conv_sel_hotkey: HWND,
    cb_hold_en: HWND,
}

struct Tab2Handles {
    ctrls: Vec<HWND>,
    cb_theme: HWND,
    lbl_cur_app: HWND,
    edit_sync_path: HWND,
}

impl MainUiCtrls {
    pub fn build(hwnd: HWND, font: HFONT, dpi: i32) -> Self {
        let hdr = build_header(hwnd, font, dpi);
        let tab0 = build_tab0(hwnd, font, dpi);
        let t1 = build_tab1(hwnd, font, dpi);
        let t2 = build_tab2(hwnd, font, dpi);
        build_footer(hwnd, font, dpi);

        let mut ctrls = Self {
            active_tab: 0,
            cb_charset: hdr.cb_charset,
            cb_method: hdr.cb_method,
            cb_mech: hdr.cb_mech,
            cb_profile: hdr.cb_profile,
            cb_hold_en: t1.cb_hold_en,
            cb_theme: t2.cb_theme,
            ve_pill: hdr.ve_pill,
            lbl_active_app: hdr.lbl_active_app,
            lbl_switch_custom: t1.lbl_switch_custom,
            lbl_conv_hotkey: t1.lbl_conv_hotkey,
            lbl_undo_hotkey: t1.lbl_undo_hotkey,
            lbl_conv_sel_hotkey: t1.lbl_conv_sel_hotkey,
            lbl_cur_app: t2.lbl_cur_app,
            edit_sync_path: t2.edit_sync_path,
            profile_items: Vec::new(),
            tab0,
            tab1: t1.ctrls,
            tab2: t2.ctrls,
        };
        ctrls.set_tab(0);
        ctrls
    }

    pub fn set_tab_state(&mut self, tab: usize) -> (usize, [Vec<HWND>; 3]) {
        self.active_tab = tab.min(2);
        (
            self.active_tab,
            [self.tab0.clone(), self.tab1.clone(), self.tab2.clone()],
        )
    }

    pub fn set_tab(&mut self, tab: usize) {
        let (active, lists) = self.set_tab_state(tab);
        apply_tab_visibility(active, &lists);
    }

    pub fn combo_list(&self) -> [HWND; 6] {
        [
            self.cb_charset,
            self.cb_method,
            self.cb_mech,
            self.cb_profile,
            self.cb_hold_en,
            self.cb_theme,
        ]
    }

    pub fn combo_for_id(&self, id: usize) -> Option<HWND> {
        match id {
            IDC_CHARSET => Some(self.cb_charset),
            IDC_METHOD => Some(self.cb_method),
            IDC_MECHANISM => Some(self.cb_mech),
            IDC_PROFILE_COMBO => Some(self.cb_profile),
            IDC_HOLD_EN_COMBO => Some(self.cb_hold_en),
            IDC_THEME => Some(self.cb_theme),
            _ => None,
        }
    }

    pub fn sync_targets(&self) -> SyncTargets {
        SyncTargets {
            cb_charset: self.cb_charset,
            cb_method: self.cb_method,
            cb_mech: self.cb_mech,
            cb_profile: self.cb_profile,
            cb_hold_en: self.cb_hold_en,
            cb_theme: self.cb_theme,
            ve_pill: self.ve_pill,
            lbl_active_app: self.lbl_active_app,
            lbl_cur_app: self.lbl_cur_app,
            lbl_switch_custom: self.lbl_switch_custom,
            lbl_conv_hotkey: self.lbl_conv_hotkey,
            lbl_undo_hotkey: self.lbl_undo_hotkey,
            lbl_conv_sel_hotkey: self.lbl_conv_sel_hotkey,
            edit_sync_path: self.edit_sync_path,
        }
    }

    pub fn sync_from_state(&mut self) {
        let (cfg, fg_proc, eff_vi) = with_state(|st| {
            (
                st.engine.config().clone(),
                st.fg_process.clone(),
                st.effective_vietnamese(),
            )
        });
        let names = cfg.all_profile_names();
        let profiles_changed = names != self.profile_items;
        if profiles_changed {
            self.profile_items = names;
        }
        sync_controls_from_config(
            self.sync_targets(),
            &cfg,
            &fg_proc,
            eff_vi,
            &self.profile_items,
            profiles_changed,
        );
    }

    pub fn is_checkbox_checked(id: u32, cfg: &Config) -> Option<bool> {
        Some(match id as usize {
            IDC_CHK_MODERN => cfg.modern_tone,
            IDC_CHK_FREE_TONE => cfg.free_tone,
            IDC_CHK_FJWZ => cfg.allow_fjwz,
            IDC_CHK_AUTOCAP => cfg.auto_capitalize_sentence,
            IDC_CHK_CLIP_SEND => cfg.use_clipboard_send,
            IDC_CHK_STD_SEND => cfg.standard_key_send,
            IDC_CHK_SPELL => cfg.spelling_check,
            IDC_CHK_RESTORE => cfg.auto_restore_wrong_word,
            IDC_CHK_NON_US => cfg.disable_on_non_us_layout,
            IDC_CHK_SUGGEST1 => cfg.fix_browser_suggest_1,
            IDC_CHK_SUGGEST2 => cfg.fix_browser_suggest_2,
            IDC_CHK_SMART_CODE => cfg.smart_code_bypass,
            IDC_CHK_MACRO => cfg.macro_enabled,
            IDC_CHK_MACRO_EN => cfg.macro_when_english,
            IDC_CHK_MACRO_TAB => cfg.macro_trigger_tab_only,
            IDC_CHK_QUICK_W => cfg.quick_w_to_u_horn,
            IDC_CHK_QUICK_BRK => cfg.quick_brackets,
            IDC_CHK_ESC_RESTORE => cfg.esc_restore,
            IDC_CHK_BEEP => cfg.beep_on_switch,
            IDC_CHK_QUICK_CAPS => cfg.quick_capslock_switch,
            IDC_CHK_AUTOSTART => cfg.auto_start,
            IDC_CHK_ADMIN => cfg.run_as_admin,
            IDC_CHK_SHOW_START => cfg.show_on_startup,
            IDC_CHK_CARET_OSD => cfg.show_caret_osd,
            IDC_CHK_CHECK_UPDATES => cfg.check_updates_on_start,
            IDC_CHK_SMART_APP => cfg.smart_app_switch,
            _ => return None,
        })
    }

    pub fn is_radio_selected(id: u32, cfg: &Config) -> Option<bool> {
        Some(match id as usize {
            IDC_RAD_CTRL_SHIFT => cfg.switch_preset == SwitchPreset::CtrlShift,
            IDC_RAD_ALT_Z => cfg.switch_preset == SwitchPreset::AltZ,
            IDC_RAD_CTRL_SPACE => cfg.switch_preset == SwitchPreset::CtrlSpace,
            IDC_RAD_SHIFT_SHIFT => cfg.switch_preset == SwitchPreset::ShiftShift,
            IDC_RAD_CUSTOM_KEY => cfg.switch_preset == SwitchPreset::Custom,
            _ => return None,
        })
    }

    pub fn toggle_setting_id(id: usize, cfg: &mut Config) -> bool {
        match id {
            IDC_CHK_MODERN => cfg.modern_tone = !cfg.modern_tone,
            IDC_CHK_FREE_TONE => cfg.free_tone = !cfg.free_tone,
            IDC_CHK_FJWZ => cfg.allow_fjwz = !cfg.allow_fjwz,
            IDC_CHK_AUTOCAP => cfg.auto_capitalize_sentence = !cfg.auto_capitalize_sentence,
            IDC_CHK_CLIP_SEND => cfg.use_clipboard_send = !cfg.use_clipboard_send,
            IDC_CHK_STD_SEND => cfg.standard_key_send = !cfg.standard_key_send,
            IDC_CHK_SPELL => cfg.spelling_check = !cfg.spelling_check,
            IDC_CHK_RESTORE => cfg.auto_restore_wrong_word = !cfg.auto_restore_wrong_word,
            IDC_CHK_NON_US => cfg.disable_on_non_us_layout = !cfg.disable_on_non_us_layout,
            IDC_CHK_SUGGEST1 => cfg.fix_browser_suggest_1 = !cfg.fix_browser_suggest_1,
            IDC_CHK_SUGGEST2 => cfg.fix_browser_suggest_2 = !cfg.fix_browser_suggest_2,
            IDC_CHK_SMART_CODE => cfg.smart_code_bypass = !cfg.smart_code_bypass,
            IDC_CHK_MACRO => cfg.macro_enabled = !cfg.macro_enabled,
            IDC_CHK_MACRO_EN => cfg.macro_when_english = !cfg.macro_when_english,
            IDC_CHK_MACRO_TAB => cfg.macro_trigger_tab_only = !cfg.macro_trigger_tab_only,
            IDC_CHK_QUICK_W => cfg.quick_w_to_u_horn = !cfg.quick_w_to_u_horn,
            IDC_CHK_QUICK_BRK => cfg.quick_brackets = !cfg.quick_brackets,
            IDC_CHK_ESC_RESTORE => cfg.esc_restore = !cfg.esc_restore,
            IDC_CHK_BEEP => cfg.beep_on_switch = !cfg.beep_on_switch,
            IDC_CHK_QUICK_CAPS => cfg.quick_capslock_switch = !cfg.quick_capslock_switch,
            IDC_CHK_AUTOSTART => cfg.auto_start = !cfg.auto_start,
            IDC_CHK_ADMIN => cfg.run_as_admin = !cfg.run_as_admin,
            IDC_CHK_SHOW_START => cfg.show_on_startup = !cfg.show_on_startup,
            IDC_CHK_CARET_OSD => cfg.show_caret_osd = !cfg.show_caret_osd,
            IDC_CHK_CHECK_UPDATES => cfg.check_updates_on_start = !cfg.check_updates_on_start,
            IDC_CHK_SMART_APP => cfg.smart_app_switch = !cfg.smart_app_switch,
            IDC_RAD_CTRL_SHIFT => cfg.switch_preset = SwitchPreset::CtrlShift,
            IDC_RAD_ALT_Z => cfg.switch_preset = SwitchPreset::AltZ,
            IDC_RAD_CTRL_SPACE => cfg.switch_preset = SwitchPreset::CtrlSpace,
            IDC_RAD_SHIFT_SHIFT => cfg.switch_preset = SwitchPreset::ShiftShift,
            IDC_RAD_CUSTOM_KEY => cfg.switch_preset = SwitchPreset::Custom,
            _ => return false,
        }
        true
    }

    pub fn apply_combo_selection(id: usize, idx: usize, cfg: &mut Config) -> bool {
        match id {
            IDC_CHARSET => {
                cfg.charset = match idx {
                    1 => Charset::Tcvn3,
                    2 => Charset::VniWindows,
                    3 => Charset::UnicodeComposite,
                    _ => Charset::Unicode,
                };
                true
            }
            IDC_METHOD => {
                cfg.input_method = match idx {
                    1 => InputMethod::Vni,
                    2 => InputMethod::Viqr,
                    3 => InputMethod::SimpleTelex,
                    4 => InputMethod::Custom,
                    _ => InputMethod::Telex,
                };
                true
            }
            IDC_MECHANISM => {
                cfg.mechanism = match idx {
                    1 => Mechanism::CompatibleHook,
                    _ => Mechanism::KeyboardHook,
                };
                true
            }
            IDC_PROFILE_COMBO => {
                let names = cfg.all_profile_names();
                if let Some(name) = names.get(idx).cloned() {
                    cfg.apply_profile(&name)
                } else {
                    false
                }
            }
            IDC_HOLD_EN_COMBO => {
                cfg.hold_english_key = match idx {
                    1 => HoldEnglishKey::RightShift,
                    2 => HoldEnglishKey::LeftShift,
                    3 => HoldEnglishKey::RightAlt,
                    4 => HoldEnglishKey::RightCtrl,
                    5 => HoldEnglishKey::CapsLock,
                    _ => HoldEnglishKey::None,
                };
                true
            }
            IDC_THEME => {
                cfg.theme = match idx {
                    1 => ThemeMode::Light,
                    2 => ThemeMode::Dark,
                    _ => ThemeMode::System,
                };
                true
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SyncTargets {
    pub cb_charset: HWND,
    pub cb_method: HWND,
    pub cb_mech: HWND,
    pub cb_profile: HWND,
    pub cb_hold_en: HWND,
    pub cb_theme: HWND,
    pub ve_pill: HWND,
    pub lbl_active_app: HWND,
    pub lbl_cur_app: HWND,
    pub lbl_switch_custom: HWND,
    pub lbl_conv_hotkey: HWND,
    pub lbl_undo_hotkey: HWND,
    pub lbl_conv_sel_hotkey: HWND,
    pub edit_sync_path: HWND,
}

pub fn apply_tab_visibility(active: usize, lists: &[Vec<HWND>; 3]) {
    for (idx, list) in lists.iter().enumerate() {
        let cmd = if idx == active { SW_SHOW } else { SW_HIDE };
        for &h in list {
            unsafe {
                ShowWindow(h, cmd);
            }
        }
    }
}

pub fn sync_controls_from_config(
    t: SyncTargets,
    cfg: &Config,
    fg_proc: &str,
    effective_vi: bool,
    profile_items: &[String],
    profiles_changed: bool,
) {
    let cs_idx = match cfg.charset {
        Charset::Unicode => 0,
        Charset::Tcvn3 => 1,
        Charset::VniWindows => 2,
        Charset::UnicodeComposite => 3,
    };
    let m_idx = match cfg.input_method {
        InputMethod::Telex => 0,
        InputMethod::Vni => 1,
        InputMethod::Viqr => 2,
        InputMethod::SimpleTelex => 3,
        InputMethod::Custom => 4,
    };
    let mech_idx = match cfg.mechanism {
        Mechanism::KeyboardHook => 0,
        Mechanism::CompatibleHook => 1,
    };
    let prof_idx = profile_items
        .iter()
        .position(|p| p.eq_ignore_ascii_case(&cfg.active_profile))
        .unwrap_or(0);
    if profiles_changed {
        let refs: Vec<&str> = profile_items.iter().map(String::as_str).collect();
        reset_combo_items(t.cb_profile, &refs, prof_idx);
    }
    let hold_idx = match cfg.hold_english_key {
        HoldEnglishKey::None => 0,
        HoldEnglishKey::RightShift => 1,
        HoldEnglishKey::LeftShift => 2,
        HoldEnglishKey::RightAlt => 3,
        HoldEnglishKey::RightCtrl => 4,
        HoldEnglishKey::CapsLock => 5,
    };
    let th_idx = match cfg.theme {
        ThemeMode::System => 0,
        ThemeMode::Light => 1,
        ThemeMode::Dark => 2,
    };
    unsafe {
        SendMessageW(t.cb_charset, CB_SETCURSEL, cs_idx, 0);
        SendMessageW(t.cb_method, CB_SETCURSEL, m_idx, 0);
        SendMessageW(t.cb_mech, CB_SETCURSEL, mech_idx, 0);
        SendMessageW(t.cb_profile, CB_SETCURSEL, prof_idx, 0);
        SendMessageW(t.cb_hold_en, CB_SETCURSEL, hold_idx, 0);
        SendMessageW(t.cb_theme, CB_SETCURSEL, th_idx, 0);
    }
    write_window_text(
        t.ve_pill,
        if effective_vi { "Tiếng Việt (V)" } else { "Tiếng Anh (E)" },
    );
    let app_short = if fg_proc.is_empty() { "-" } else { fg_proc };
    write_window_text(t.lbl_active_app, &format!("App: {app_short}"));
    write_window_text(t.lbl_cur_app, &format!("App gần nhất: {app_short}"));
    write_window_text(
        t.lbl_switch_custom,
        &format!("Tổ hợp tùy chọn: {}", format_hotkey(cfg.custom_switch_hotkey)),
    );
    write_window_text(
        t.lbl_conv_hotkey,
        &format!("Chuyển mã: {}", format_hotkey(cfg.clipboard_convert_hotkey)),
    );
    write_window_text(
        t.lbl_undo_hotkey,
        &format!("Hoàn tác từ vừa gõ: {}", format_hotkey(cfg.undo_last_word_hotkey)),
    );
    write_window_text(
        t.lbl_conv_sel_hotkey,
        &format!("Chuyển đổi vùng chọn: {}", format_hotkey(cfg.convert_selection_hotkey)),
    );
    if unsafe { GetFocus() } != t.edit_sync_path && read_window_text(t.edit_sync_path) != cfg.sync_config_path {
        write_window_text(t.edit_sync_path, &cfg.sync_config_path);
    }
}

fn build_header(hwnd: HWND, font: HFONT, dpi: i32) -> HeaderHandles {
    make_child(hwnd, "STATIC", "Bảng mã", 0, 28, 40, 88, 24, 0, font, dpi);
    let cb_charset = make_combo(
        hwnd,
        120,
        36,
        196,
        28,
        IDC_CHARSET,
        &["Unicode", "TCVN3 (ABC)", "VNI Windows", "Unicode tổ hợp"],
        0,
        font,
        dpi,
    );
    let ve_pill = make_owner_button(hwnd, "Tiếng Việt (V)", 324, 36, 124, 28, IDC_VE_PILL, font, dpi);

    make_child(hwnd, "STATIC", "Kiểu gõ", 0, 28, 72, 88, 24, 0, font, dpi);
    let cb_method = make_combo(
        hwnd,
        120,
        68,
        156,
        28,
        IDC_METHOD,
        &["Telex", "VNI", "VIQR", "Simple Telex", "Tự định nghĩa"],
        0,
        font,
        dpi,
    );
    make_owner_button(hwnd, "...", 284, 68, 32, 28, IDC_BTN_CUSTOM_KEY, font, dpi);
    make_owner_button(hwnd, "Chuyển mã...", 324, 68, 124, 28, IDC_BTN_CONVERT, font, dpi);

    make_child(hwnd, "STATIC", "Cơ chế", 0, 28, 104, 88, 24, 0, font, dpi);
    let cb_mech = make_combo(
        hwnd,
        120,
        100,
        196,
        28,
        IDC_MECHANISM,
        &["Hook bàn phím", "Tương thích cao"],
        0,
        font,
        dpi,
    );
    let lbl_active_app =
        make_child(hwnd, "STATIC", "App: -", 0, 324, 104, 124, 24, IDC_LBL_ACTIVE_APP, font, dpi);

    make_owner_button(hwnd, "Đóng", 476, 18, 88, 30, IDC_BTN_CLOSE, font, dpi);
    make_owner_button(hwnd, "Thoát", 476, 54, 88, 30, IDC_BTN_EXIT, font, dpi);

    make_owner_button(hwnd, "Cơ bản", 16, 152, 98, 30, IDC_TAB_BASIC, font, dpi);
    make_owner_button(hwnd, "Phím tắt", 120, 152, 98, 30, IDC_TAB_HOTKEY, font, dpi);
    make_owner_button(hwnd, "Hệ thống", 224, 152, 98, 30, IDC_TAB_SYSTEM, font, dpi);
    make_child(hwnd, "STATIC", "Hồ sơ", 0, 330, 158, 44, 22, IDC_LBL_PROFILE, font, dpi);
    let cb_profile = make_combo(
        hwnd,
        376,
        154,
        132,
        28,
        IDC_PROFILE_COMBO,
        &["Mặc định"],
        0,
        font,
        dpi,
    );
    make_owner_button(hwnd, "Lưu", 514, 154, 50, 28, IDC_BTN_SAVE_PROFILE, font, dpi);
    HeaderHandles {
        cb_charset,
        cb_method,
        cb_mech,
        cb_profile,
        ve_pill,
        lbl_active_app,
    }
}

fn build_tab0(hwnd: HWND, font: HFONT, dpi: i32) -> Vec<HWND> {
    vec![
        make_owner_button(hwnd, "Đặt dấu kiểu mới (oà, uý)", 32, 204, 244, 24, IDC_CHK_MODERN, font, dpi),
        make_owner_button(hwnd, "Đặt dấu tự do", 32, 232, 244, 24, IDC_CHK_FREE_TONE, font, dpi),
        make_owner_button(hwnd, "Cho phép f, j, w, z", 32, 260, 244, 24, IDC_CHK_FJWZ, font, dpi),
        make_owner_button(hwnd, "Viết hoa đầu câu", 32, 288, 244, 24, IDC_CHK_AUTOCAP, font, dpi),
        make_owner_button(hwnd, "Dùng clipboard gửi phím", 32, 316, 244, 24, IDC_CHK_CLIP_SEND, font, dpi),
        make_owner_button(hwnd, "Sử dụng SendKey chuẩn", 32, 344, 244, 24, IDC_CHK_STD_SEND, font, dpi),
        make_owner_button(hwnd, "Kiểm tra chính tả", 292, 204, 256, 24, IDC_CHK_SPELL, font, dpi),
        make_owner_button(hwnd, "Khôi phục phím từ sai", 292, 232, 256, 24, IDC_CHK_RESTORE, font, dpi),
        make_owner_button(hwnd, "Tắt khi không phải phím US", 292, 260, 256, 24, IDC_CHK_NON_US, font, dpi),
        make_owner_button(hwnd, "Sửa lỗi dấu của Gợi ý 1", 292, 288, 256, 24, IDC_CHK_SUGGEST1, font, dpi),
        make_owner_button(hwnd, "Sửa lỗi dấu của Gợi ý 2", 292, 316, 256, 24, IDC_CHK_SUGGEST2, font, dpi),
        make_owner_button(hwnd, "Bỏ qua từ kỹ thuật / code", 292, 344, 256, 24, IDC_CHK_SMART_CODE, font, dpi),
        make_child(
            hwnd,
            "STATIC",
            "Tối ưu Chrome/Edge/Terminal & tự nhận diện code",
            0,
            32,
            372,
            516,
            22,
            IDC_LBL_SUGGEST_TIP,
            font,
            dpi,
        ),
        make_owner_button(hwnd, "Cho phép gõ tắt", 44, 428, 236, 24, IDC_CHK_MACRO, font, dpi),
        make_owner_button(hwnd, "Bảng gõ tắt...", 364, 424, 172, 28, IDC_BTN_MACRO, font, dpi),
        make_owner_button(hwnd, "Vẫn gõ tắt khi tắt Tiếng Việt", 44, 460, 292, 24, IDC_CHK_MACRO_EN, font, dpi),
        make_owner_button(hwnd, "Mở rộng & Gõ thử...", 364, 456, 172, 28, IDC_BTN_EXTRAS, font, dpi),
        make_owner_button(hwnd, "Chỉ kích hoạt gõ tắt bằng Tab", 44, 492, 292, 24, IDC_CHK_MACRO_TAB, font, dpi),
        make_owner_button(hwnd, "Gõ w thành ư", 44, 528, 210, 24, IDC_CHK_QUICK_W, font, dpi),
        make_owner_button(hwnd, "Gõ [ thành ơ, ] thành ư", 276, 528, 256, 24, IDC_CHK_QUICK_BRK, font, dpi),
    ]
}

fn build_tab1(hwnd: HWND, font: HFONT, dpi: i32) -> Tab1Handles {
    let lbl_switch_custom = make_child(hwnd, "STATIC", "Tổ hợp: -", 0, 44, 274, 304, 22, IDC_LBL_SWITCH_CUSTOM, font, dpi);
    let cb_hold_en = make_combo(
        hwnd,
        284,
        368,
        252,
        28,
        IDC_HOLD_EN_COMBO,
        &["Không dùng", "Right Shift", "Left Shift", "Right Alt", "Right Ctrl", "CapsLock"],
        0,
        font,
        dpi,
    );
    let lbl_conv_hotkey = make_child(hwnd, "STATIC", "Chuyển mã: Ctrl + Shift + F6", 0, 44, 428, 312, 22, IDC_LBL_CONV_HOTKEY, font, dpi);
    let lbl_undo_hotkey = make_child(hwnd, "STATIC", "Hoàn tác từ vừa gõ: Không", 0, 44, 460, 312, 22, IDC_LBL_UNDO_HOTKEY, font, dpi);
    let lbl_conv_sel_hotkey = make_child(hwnd, "STATIC", "Chuyển đổi vùng chọn: Không", 0, 44, 492, 312, 22, IDC_LBL_CONV_SEL_HOTKEY, font, dpi);
    let ctrls = vec![
        make_owner_button(hwnd, "Ctrl + Shift", 44, 222, 128, 22, IDC_RAD_CTRL_SHIFT, font, dpi),
        make_owner_button(hwnd, "Alt + Z", 184, 222, 100, 22, IDC_RAD_ALT_Z, font, dpi),
        make_owner_button(hwnd, "Ctrl + Space", 296, 222, 132, 22, IDC_RAD_CTRL_SPACE, font, dpi),
        make_owner_button(hwnd, "Shift + Shift", 44, 248, 128, 22, IDC_RAD_SHIFT_SHIFT, font, dpi),
        make_owner_button(hwnd, "Phím tùy chọn", 184, 248, 152, 22, IDC_RAD_CUSTOM_KEY, font, dpi),
        lbl_switch_custom,
        make_owner_button(hwnd, "Đặt phím tùy chọn", 364, 270, 172, 26, IDC_BTN_CAP_SWITCH, font, dpi),
        make_owner_button(hwnd, "Phím Esc khôi phục từ gốc", 32, 312, 256, 24, IDC_CHK_ESC_RESTORE, font, dpi),
        make_owner_button(hwnd, "Âm thanh báo khi chuyển V/E", 300, 312, 248, 24, IDC_CHK_BEEP, font, dpi),
        make_owner_button(
            hwnd,
            "Nhấn nhanh CapsLock chuyển V/E (giữ >250ms đổi Hoa)",
            32,
            340,
            516,
            24,
            IDC_CHK_QUICK_CAPS,
            font,
            dpi,
        ),
        make_child(hwnd, "STATIC", "Giữ phím tạm chuyển Tiếng Anh", 0, 32, 372, 244, 22, IDC_LBL_HOLD_EN, font, dpi),
        cb_hold_en,
        lbl_conv_hotkey,
        make_owner_button(hwnd, "Đổi phím chuyển mã", 364, 424, 172, 26, IDC_BTN_CAP_CONV, font, dpi),
        lbl_undo_hotkey,
        make_owner_button(hwnd, "Đổi phím hoàn tác", 364, 456, 172, 26, IDC_BTN_CAP_UNDO, font, dpi),
        lbl_conv_sel_hotkey,
        make_owner_button(hwnd, "Đổi phím vùng chọn", 364, 488, 172, 26, IDC_BTN_CAP_CONV_SEL, font, dpi),
        make_owner_button(hwnd, "Nhập EVKey setting.tree...", 44, 526, 236, 30, IDC_BTN_IMPORT_TREE, font, dpi),
        make_owner_button(hwnd, "Mở thư mục cấu hình", 296, 526, 240, 30, IDC_BTN_OPEN_CFG_DIR, font, dpi),
    ];
    Tab1Handles {
        ctrls,
        lbl_switch_custom,
        lbl_conv_hotkey,
        lbl_undo_hotkey,
        lbl_conv_sel_hotkey,
        cb_hold_en,
    }
}

fn build_tab2(hwnd: HWND, font: HFONT, dpi: i32) -> Tab2Handles {
    let cb_theme = make_combo(
        hwnd,
        112,
        296,
        152,
        28,
        IDC_THEME,
        &["Theo Windows", "Sáng (Light)", "Tối (Dark)"],
        0,
        font,
        dpi,
    );
    let edit_sync_path = make_edit(hwnd, "", 160, 334, 292, 28, IDC_EDIT_SYNC_PATH, false, font, dpi);
    set_cue_banner(edit_sync_path, "Đường dẫn JSON đồng bộ giữa các máy...");
    let lbl_cur_app = make_child(hwnd, "STATIC", "App gần nhất: -", 0, 44, 428, 492, 24, IDC_LBL_CUR_APP, font, dpi);
    let admin_btn_txt = if is_elevated() { "Đang chạy Admin" } else { "Mở lại Admin" };
    let ctrls = vec![
        make_owner_button(hwnd, "Khởi động cùng Windows", 32, 202, 250, 24, IDC_CHK_AUTOSTART, font, dpi),
        make_owner_button(hwnd, "Hiện bảng điều khiển khi mở", 296, 202, 252, 24, IDC_CHK_SHOW_START, font, dpi),
        make_owner_button(hwnd, "Chạy với quyền Admin", 32, 232, 250, 24, IDC_CHK_ADMIN, font, dpi),
        make_owner_button(hwnd, admin_btn_txt, 364, 228, 172, 28, IDC_BTN_RELAUNCH_ADMIN, font, dpi),
        make_owner_button(hwnd, "Hiện chỉ báo V/E cạnh con trỏ", 32, 262, 256, 24, IDC_CHK_CARET_OSD, font, dpi),
        make_owner_button(hwnd, "Tự kiểm tra cập nhật khi mở", 296, 262, 252, 24, IDC_CHK_CHECK_UPDATES, font, dpi),
        make_child(hwnd, "STATIC", "Giao diện", 0, 32, 300, 76, 22, IDC_LBL_THEME, font, dpi),
        cb_theme,
        make_owner_button(hwnd, "Từ điển cá nhân...", 276, 296, 134, 28, IDC_BTN_USER_DICT, font, dpi),
        make_owner_button(hwnd, "Kiểm tra cập nhật", 418, 296, 130, 28, IDC_BTN_CHECK_UPDATE, font, dpi),
        make_child(hwnd, "STATIC", "File đồng bộ", 0, 32, 338, 124, 22, IDC_LBL_SYNC_PATH, font, dpi),
        edit_sync_path,
        make_owner_button(hwnd, "Chọn...", 460, 334, 88, 28, IDC_BTN_BROWSE_SYNC, font, dpi),
        make_owner_button(hwnd, "Tự động nhớ V/E theo ứng dụng", 44, 396, 420, 24, IDC_CHK_SMART_APP, font, dpi),
        lbl_cur_app,
        make_owner_button(hwnd, "Danh sách ứng dụng...", 44, 460, 236, 32, IDC_BTN_APPS, font, dpi),
        make_owner_button(hwnd, "Thêm app vào Tiếng Anh", 296, 460, 240, 32, IDC_BTN_ADD_CUR_EN, font, dpi),
        make_child(hwnd, "STATIC", "Hỗ trợ Luôn Tiếng Anh, Luôn Tiếng Việt & Blacklist", 0x0080, 44, 502, 492, 24, IDC_LBL_APP_HINT, font, dpi),
    ];
    Tab2Handles {
        ctrls,
        cb_theme,
        lbl_cur_app,
        edit_sync_path,
    }
}

fn build_footer(hwnd: HWND, font: HFONT, dpi: i32) {
    make_owner_button(hwnd, "Th.Lập mặc định", 16, 590, 136, 32, IDC_BTN_DEFAULT, font, dpi);
    make_owner_button(hwnd, "Lưu", 160, 590, 96, 32, IDC_BTN_SAVE, font, dpi);
    make_child(hwnd, "STATIC", concat!("MS Key v", env!("CARGO_PKG_VERSION"), " • Tiếng Việt Unicode"), 0x0082, 272, 594, 292, 24, IDC_LBL_FOOTER_VER, font, dpi);
}
