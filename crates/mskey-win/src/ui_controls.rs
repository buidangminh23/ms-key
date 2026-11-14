use mskey_core::{Charset, Config, InputMethod, Mechanism, SwitchPreset, ThemeMode};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CB_SETCURSEL, SW_HIDE, SW_SHOW, SendMessageW, ShowWindow,
};

use crate::dialog_helpers::{combo_index, format_hotkey, make_child, make_combo, make_owner_button, write_window_text};
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
pub const IDC_CHK_MACRO: usize = 1120;
pub const IDC_BTN_MACRO: usize = 1121;
pub const IDC_CHK_MACRO_EN: usize = 1122;
pub const IDC_CHK_QUICK_W: usize = 1123;
pub const IDC_CHK_QUICK_BRK: usize = 1124;

pub const IDC_RAD_CTRL_SHIFT: usize = 1201;
pub const IDC_RAD_ALT_Z: usize = 1202;
pub const IDC_RAD_CUSTOM_KEY: usize = 1203;
pub const IDC_LBL_SWITCH_CUSTOM: usize = 1204;
pub const IDC_BTN_CAP_SWITCH: usize = 1205;
pub const IDC_CHK_ESC_RESTORE: usize = 1206;
pub const IDC_CHK_BEEP: usize = 1207;
pub const IDC_LBL_CONV_HOTKEY: usize = 1208;
pub const IDC_BTN_CAP_CONV: usize = 1209;
pub const IDC_BTN_IMPORT_TREE: usize = 1210;
pub const IDC_BTN_OPEN_CFG_DIR: usize = 1211;

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

pub const IDC_BTN_DEFAULT: usize = 1401;
pub const IDC_BTN_SAVE: usize = 1402;
pub const IDC_LBL_FOOTER_VER: usize = 1403;

pub struct MainUiCtrls {
    pub active_tab: usize,
    pub cb_charset: HWND,
    pub cb_method: HWND,
    pub cb_mech: HWND,
    pub cb_theme: HWND,
    pub ve_pill: HWND,
    pub lbl_active_app: HWND,
    pub lbl_switch_custom: HWND,
    pub lbl_conv_hotkey: HWND,
    pub lbl_cur_app: HWND,
    pub tab0: Vec<HWND>,
    pub tab1: Vec<HWND>,
    pub tab2: Vec<HWND>,
}

impl MainUiCtrls {
    pub fn build(hwnd: HWND, font: HFONT, dpi: i32) -> Self {
        let (cb_charset, cb_method, cb_mech, ve_pill, lbl_active_app) = build_header(hwnd, font, dpi);
        let tab0 = build_tab0(hwnd, font, dpi);
        let (tab1, lbl_switch_custom, lbl_conv_hotkey) = build_tab1(hwnd, font, dpi);
        let (tab2, cb_theme, lbl_cur_app) = build_tab2(hwnd, font, dpi);
        build_footer(hwnd, font, dpi);

        let mut ctrls = Self {
            active_tab: 0,
            cb_charset,
            cb_method,
            cb_mech,
            cb_theme,
            ve_pill,
            lbl_active_app,
            lbl_switch_custom,
            lbl_conv_hotkey,
            lbl_cur_app,
            tab0,
            tab1,
            tab2,
        };
        ctrls.set_tab(0);
        ctrls
    }

    pub fn set_tab(&mut self, tab: usize) {
        self.active_tab = tab.min(2);
        for (idx, list) in [&self.tab0, &self.tab1, &self.tab2].iter().enumerate() {
            let cmd = if idx == self.active_tab { SW_SHOW } else { SW_HIDE };
            for &h in *list {
                unsafe {
                    ShowWindow(h, cmd);
                }
            }
        }
    }

    pub fn sync_from_state(&self) {
        let (cfg, fg_proc) = with_state(|st| (st.engine.config().clone(), st.fg_process.clone()));
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
        let th_idx = match cfg.theme {
            ThemeMode::System => 0,
            ThemeMode::Light => 1,
            ThemeMode::Dark => 2,
        };
        unsafe {
            SendMessageW(self.cb_charset, CB_SETCURSEL, cs_idx, 0);
            SendMessageW(self.cb_method, CB_SETCURSEL, m_idx, 0);
            SendMessageW(self.cb_mech, CB_SETCURSEL, mech_idx, 0);
            SendMessageW(self.cb_theme, CB_SETCURSEL, th_idx, 0);
        }
        write_window_text(
            self.ve_pill,
            if cfg.vietnamese_enabled { "Tiếng Việt (V)" } else { "Tiếng Anh (E)" },
        );
        let app_short = if fg_proc.is_empty() { "-" } else { &fg_proc };
        write_window_text(self.lbl_active_app, &format!("App: {app_short}"));
        write_window_text(self.lbl_cur_app, &format!("App gần nhất: {app_short}"));
        write_window_text(
            self.lbl_switch_custom,
            &format!("Tổ hợp tùy chọn: {}", format_hotkey(cfg.custom_switch_hotkey)),
        );
        write_window_text(
            self.lbl_conv_hotkey,
            &format!("Chuyển mã: {}", format_hotkey(cfg.clipboard_convert_hotkey)),
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
            IDC_CHK_MACRO => cfg.macro_enabled,
            IDC_CHK_MACRO_EN => cfg.macro_when_english,
            IDC_CHK_QUICK_W => cfg.quick_w_to_u_horn,
            IDC_CHK_QUICK_BRK => cfg.quick_brackets,
            IDC_CHK_ESC_RESTORE => cfg.esc_restore,
            IDC_CHK_BEEP => cfg.beep_on_switch,
            IDC_CHK_AUTOSTART => cfg.auto_start,
            IDC_CHK_ADMIN => cfg.run_as_admin,
            IDC_CHK_SHOW_START => cfg.show_on_startup,
            IDC_CHK_SMART_APP => cfg.smart_app_switch,
            _ => return None,
        })
    }

    pub fn is_radio_selected(id: u32, cfg: &Config) -> Option<bool> {
        Some(match id as usize {
            IDC_RAD_CTRL_SHIFT => cfg.switch_preset == SwitchPreset::CtrlShift,
            IDC_RAD_ALT_Z => cfg.switch_preset == SwitchPreset::AltZ,
            IDC_RAD_CUSTOM_KEY => cfg.switch_preset == SwitchPreset::Custom,
            _ => return None,
        })
    }

    pub fn toggle_setting_id(&self, id: usize, cfg: &mut Config) -> bool {
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
            IDC_CHK_MACRO => cfg.macro_enabled = !cfg.macro_enabled,
            IDC_CHK_MACRO_EN => cfg.macro_when_english = !cfg.macro_when_english,
            IDC_CHK_QUICK_W => cfg.quick_w_to_u_horn = !cfg.quick_w_to_u_horn,
            IDC_CHK_QUICK_BRK => cfg.quick_brackets = !cfg.quick_brackets,
            IDC_CHK_ESC_RESTORE => cfg.esc_restore = !cfg.esc_restore,
            IDC_CHK_BEEP => cfg.beep_on_switch = !cfg.beep_on_switch,
            IDC_CHK_AUTOSTART => cfg.auto_start = !cfg.auto_start,
            IDC_CHK_ADMIN => cfg.run_as_admin = !cfg.run_as_admin,
            IDC_CHK_SHOW_START => cfg.show_on_startup = !cfg.show_on_startup,
            IDC_CHK_SMART_APP => cfg.smart_app_switch = !cfg.smart_app_switch,
            IDC_RAD_CTRL_SHIFT => cfg.switch_preset = SwitchPreset::CtrlShift,
            IDC_RAD_ALT_Z => cfg.switch_preset = SwitchPreset::AltZ,
            IDC_RAD_CUSTOM_KEY => cfg.switch_preset = SwitchPreset::Custom,
            _ => return false,
        }
        true
    }

    pub fn sync_combos_to_config(&self, id: usize, cfg: &mut Config) -> bool {
        match id {
            IDC_CHARSET => {
                cfg.charset = match combo_index(self.cb_charset) {
                    1 => Charset::Tcvn3,
                    2 => Charset::VniWindows,
                    3 => Charset::UnicodeComposite,
                    _ => Charset::Unicode,
                };
                true
            }
            IDC_METHOD => {
                cfg.input_method = match combo_index(self.cb_method) {
                    1 => InputMethod::Vni,
                    2 => InputMethod::Viqr,
                    3 => InputMethod::SimpleTelex,
                    4 => InputMethod::Custom,
                    _ => InputMethod::Telex,
                };
                true
            }
            IDC_MECHANISM => {
                cfg.mechanism = match combo_index(self.cb_mech) {
                    1 => Mechanism::CompatibleHook,
                    _ => Mechanism::KeyboardHook,
                };
                true
            }
            IDC_THEME => {
                cfg.theme = match combo_index(self.cb_theme) {
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

fn build_header(hwnd: HWND, font: HFONT, dpi: i32) -> (HWND, HWND, HWND, HWND, HWND) {
    make_child(hwnd, "STATIC", "Bảng mã", 0, 28, 40, 88, 24, 0, font, dpi);
    let cb_charset = make_combo(
        hwnd,
        120,
        36,
        188,
        28,
        IDC_CHARSET,
        &["Unicode", "TCVN3 (ABC)", "VNI Windows", "Unicode tổ hợp"],
        0,
        font,
        dpi,
    );
    let ve_pill = make_owner_button(hwnd, "Tiếng Việt (V)", 316, 36, 116, 28, IDC_VE_PILL, font, dpi);

    make_child(hwnd, "STATIC", "Kiểu gõ", 0, 28, 72, 88, 24, 0, font, dpi);
    let cb_method = make_combo(
        hwnd,
        120,
        68,
        148,
        28,
        IDC_METHOD,
        &["Telex", "VNI", "VIQR", "Simple Telex", "Tự định nghĩa"],
        0,
        font,
        dpi,
    );
    make_owner_button(hwnd, "...", 276, 68, 32, 28, IDC_BTN_CUSTOM_KEY, font, dpi);
    make_owner_button(hwnd, "Chuyển mã...", 316, 68, 116, 28, IDC_BTN_CONVERT, font, dpi);

    make_child(hwnd, "STATIC", "Cơ chế", 0, 28, 104, 88, 24, 0, font, dpi);
    let cb_mech = make_combo(
        hwnd,
        120,
        100,
        188,
        28,
        IDC_MECHANISM,
        &["Hook bàn phím", "Tương thích cao"],
        0,
        font,
        dpi,
    );
    let lbl_active_app =
        make_child(hwnd, "STATIC", "App: -", 0, 316, 104, 116, 24, IDC_LBL_ACTIVE_APP, font, dpi);

    make_owner_button(hwnd, "Đóng", 460, 20, 84, 32, IDC_BTN_CLOSE, font, dpi);
    make_owner_button(hwnd, "Thoát", 460, 60, 84, 32, IDC_BTN_EXIT, font, dpi);

    make_owner_button(hwnd, "Cơ bản", 16, 152, 104, 32, IDC_TAB_BASIC, font, dpi);
    make_owner_button(hwnd, "Phím tắt", 124, 152, 104, 32, IDC_TAB_HOTKEY, font, dpi);
    make_owner_button(hwnd, "Hệ thống", 232, 152, 104, 32, IDC_TAB_SYSTEM, font, dpi);
    (cb_charset, cb_method, cb_mech, ve_pill, lbl_active_app)
}

fn build_tab0(hwnd: HWND, font: HFONT, dpi: i32) -> Vec<HWND> {
    vec![
        make_owner_button(hwnd, "Đặt dấu kiểu mới (oà, uý)", 32, 200, 236, 24, IDC_CHK_MODERN, font, dpi),
        make_owner_button(hwnd, "Đặt dấu tự do", 32, 228, 236, 24, IDC_CHK_FREE_TONE, font, dpi),
        make_owner_button(hwnd, "Cho phép f, j, w, z", 32, 256, 236, 24, IDC_CHK_FJWZ, font, dpi),
        make_owner_button(hwnd, "Viết hoa đầu câu", 32, 284, 236, 24, IDC_CHK_AUTOCAP, font, dpi),
        make_owner_button(hwnd, "Dùng clipboard gửi phím", 32, 312, 236, 24, IDC_CHK_CLIP_SEND, font, dpi),
        make_owner_button(hwnd, "Sử dụng SendKey chuẩn", 32, 340, 236, 24, IDC_CHK_STD_SEND, font, dpi),
        make_owner_button(hwnd, "Kiểm tra chính tả", 284, 200, 244, 24, IDC_CHK_SPELL, font, dpi),
        make_owner_button(hwnd, "Khôi phục phím từ sai", 284, 228, 244, 24, IDC_CHK_RESTORE, font, dpi),
        make_owner_button(hwnd, "Tắt khi không phải phím US", 284, 256, 244, 24, IDC_CHK_NON_US, font, dpi),
        make_owner_button(hwnd, "Sửa lỗi dấu của Gợi ý 1", 284, 284, 244, 24, IDC_CHK_SUGGEST1, font, dpi),
        make_owner_button(hwnd, "Sửa lỗi dấu của Gợi ý 2", 284, 312, 244, 24, IDC_CHK_SUGGEST2, font, dpi),
        make_child(hwnd, "STATIC", "Tối ưu Chrome/Edge/Terminal", 0, 284, 340, 244, 24, IDC_LBL_SUGGEST_TIP, font, dpi),
        make_owner_button(hwnd, "Cho phép gõ tắt", 44, 400, 236, 24, IDC_CHK_MACRO, font, dpi),
        make_owner_button(hwnd, "Bảng gõ tắt...", 348, 396, 168, 28, IDC_BTN_MACRO, font, dpi),
        make_owner_button(hwnd, "Vẫn gõ tắt khi tắt Tiếng Việt", 44, 432, 360, 24, IDC_CHK_MACRO_EN, font, dpi),
        make_owner_button(hwnd, "Gõ w thành ư", 44, 464, 216, 24, IDC_CHK_QUICK_W, font, dpi),
        make_owner_button(hwnd, "Gõ [ thành ơ, ] thành ư", 276, 464, 236, 24, IDC_CHK_QUICK_BRK, font, dpi),
    ]
}

fn build_tab1(hwnd: HWND, font: HFONT, dpi: i32) -> (Vec<HWND>, HWND, HWND) {
    let lbl_switch_custom = make_child(hwnd, "STATIC", "Tổ hợp: -", 0, 44, 268, 284, 24, IDC_LBL_SWITCH_CUSTOM, font, dpi);
    let lbl_conv_hotkey = make_child(hwnd, "STATIC", "Chuyển mã: Ctrl + Shift + F6", 0, 44, 416, 284, 24, IDC_LBL_CONV_HOTKEY, font, dpi);
    let ctrls = vec![
        make_owner_button(hwnd, "Ctrl + Shift", 44, 228, 132, 24, IDC_RAD_CTRL_SHIFT, font, dpi),
        make_owner_button(hwnd, "Alt + Z", 184, 228, 112, 24, IDC_RAD_ALT_Z, font, dpi),
        make_owner_button(hwnd, "Phím tùy chọn", 308, 228, 160, 24, IDC_RAD_CUSTOM_KEY, font, dpi),
        lbl_switch_custom,
        make_owner_button(hwnd, "Đặt phím tùy chọn", 348, 264, 168, 28, IDC_BTN_CAP_SWITCH, font, dpi),
        make_owner_button(hwnd, "Phím Esc khôi phục từ gốc", 32, 320, 360, 24, IDC_CHK_ESC_RESTORE, font, dpi),
        make_owner_button(hwnd, "Âm thanh báo khi chuyển V/E", 32, 348, 360, 24, IDC_CHK_BEEP, font, dpi),
        lbl_conv_hotkey,
        make_owner_button(hwnd, "Đổi phím chuyển mã", 348, 412, 168, 28, IDC_BTN_CAP_CONV, font, dpi),
        make_owner_button(hwnd, "Nhập EVKey setting.tree...", 44, 452, 224, 32, IDC_BTN_IMPORT_TREE, font, dpi),
        make_owner_button(hwnd, "Mở thư mục cấu hình", 284, 452, 232, 32, IDC_BTN_OPEN_CFG_DIR, font, dpi),
    ];
    (ctrls, lbl_switch_custom, lbl_conv_hotkey)
}

fn build_tab2(hwnd: HWND, font: HFONT, dpi: i32) -> (Vec<HWND>, HWND, HWND) {
    let cb_theme = make_combo(
        hwnd,
        128,
        288,
        192,
        28,
        IDC_THEME,
        &["Theo Windows", "Sáng (Light)", "Tối (Dark)"],
        0,
        font,
        dpi,
    );
    let lbl_cur_app = make_child(hwnd, "STATIC", "App gần nhất: -", 0, 44, 388, 472, 24, IDC_LBL_CUR_APP, font, dpi);
    let admin_btn_txt = if is_elevated() { "Đang chạy Admin" } else { "Mở lại Admin" };
    let ctrls = vec![
        make_owner_button(hwnd, "Khởi động cùng Windows", 32, 200, 300, 24, IDC_CHK_AUTOSTART, font, dpi),
        make_owner_button(hwnd, "Chạy với quyền Admin", 32, 228, 260, 24, IDC_CHK_ADMIN, font, dpi),
        make_owner_button(hwnd, admin_btn_txt, 348, 224, 168, 28, IDC_BTN_RELAUNCH_ADMIN, font, dpi),
        make_owner_button(hwnd, "Hiện bảng điều khiển khi mở", 32, 256, 300, 24, IDC_CHK_SHOW_START, font, dpi),
        make_child(hwnd, "STATIC", "Giao diện", 0, 32, 292, 88, 24, IDC_LBL_THEME, font, dpi),
        cb_theme,
        make_owner_button(hwnd, "Tự động nhớ V/E theo ứng dụng", 44, 356, 420, 24, IDC_CHK_SMART_APP, font, dpi),
        lbl_cur_app,
        make_owner_button(hwnd, "Danh sách ứng dụng...", 44, 420, 224, 32, IDC_BTN_APPS, font, dpi),
        make_owner_button(hwnd, "Thêm app vào Tiếng Anh", 284, 420, 232, 32, IDC_BTN_ADD_CUR_EN, font, dpi),
        make_child(hwnd, "STATIC", "Hỗ trợ Luôn Tiếng Anh, Luôn Tiếng Việt & Blacklist", 0, 44, 460, 472, 24, IDC_LBL_APP_HINT, font, dpi),
    ];
    (ctrls, cb_theme, lbl_cur_app)
}

fn build_footer(hwnd: HWND, font: HFONT, dpi: i32) {
    make_owner_button(hwnd, "Th.Lập mặc định", 16, 524, 136, 32, IDC_BTN_DEFAULT, font, dpi);
    make_owner_button(hwnd, "Lưu", 164, 524, 96, 32, IDC_BTN_SAVE, font, dpi);
    make_child(hwnd, "STATIC", "MS Key v0.1 • Tiếng Việt Unicode", 0x0002, 280, 528, 264, 24, IDC_LBL_FOOTER_VER, font, dpi);
}
