use std::ptr;

use mskey_core::{Charset, Config, HoldEnglishKey, Mechanism, SwitchPreset, ThemeMode};

use crate::gio_ffi::cstr;
use crate::gtk_ffi::{
    GtkWidget, PANGO_WRAP_WORD_CHAR, gtk_grid_attach, gtk_grid_set_column_homogeneous,
    gtk_label_set_line_wrap, gtk_label_set_line_wrap_mode, gtk_label_set_mnemonic_widget,
    gtk_widget_set_sensitive, gtk_widget_set_tooltip_text,
};
use crate::gtk_helpers::{
    get_combo, get_input_method, get_toggle, make_button, make_check, make_combo, make_entry_placeholder,
    make_grid, make_hbox, make_input_method_combo, make_label, make_radio, make_vbox, pack, read_entry,
    refill_combo, set_combo, set_input_method, set_label_text, set_toggle, write_entry,
};
use crate::gtk_theme::{add_css_class, set_language_button};
use crate::keyval::format_hotkey_linux;

pub struct UiState {
    pub win: *mut GtkWidget,
    pub cfg: Config,
    pub syncing: bool,
    pub cb_charset: *mut GtkWidget,
    pub cb_method: *mut GtkWidget,
    pub cb_mech: *mut GtkWidget,
    pub cb_theme: *mut GtkWidget,
    pub cb_hold_en: *mut GtkWidget,
    pub cb_profile: *mut GtkWidget,
    pub btn_ve: *mut GtkWidget,
    pub lbl_cur_app: *mut GtkWidget,
    pub lbl_switch_custom: *mut GtkWidget,
    pub lbl_conv_hotkey: *mut GtkWidget,
    pub lbl_sel_conv_hotkey: *mut GtkWidget,
    pub lbl_undo_hotkey: *mut GtkWidget,
    pub lbl_update_status: *mut GtkWidget,
    pub chk_modern: *mut GtkWidget,
    pub chk_free_tone: *mut GtkWidget,
    pub chk_fjwz: *mut GtkWidget,
    pub chk_autocap: *mut GtkWidget,
    pub chk_smart_code: *mut GtkWidget,
    pub chk_clip_send: *mut GtkWidget,
    pub chk_std_send: *mut GtkWidget,
    pub chk_spell: *mut GtkWidget,
    pub chk_restore: *mut GtkWidget,
    pub chk_reedit: *mut GtkWidget,
    pub chk_non_us: *mut GtkWidget,
    pub chk_suggest1: *mut GtkWidget,
    pub chk_suggest2: *mut GtkWidget,
    pub chk_caret_osd: *mut GtkWidget,
    pub chk_macro: *mut GtkWidget,
    pub chk_macro_en: *mut GtkWidget,
    pub chk_macro_tab: *mut GtkWidget,
    pub chk_quick_w: *mut GtkWidget,
    pub chk_quick_brk: *mut GtkWidget,
    pub rad_ctrl_shift: *mut GtkWidget,
    pub rad_alt_z: *mut GtkWidget,
    pub rad_ctrl_space: *mut GtkWidget,
    pub rad_shift_shift: *mut GtkWidget,
    pub rad_custom: *mut GtkWidget,
    pub chk_esc: *mut GtkWidget,
    pub chk_beep: *mut GtkWidget,
    pub chk_caps_switch: *mut GtkWidget,
    pub chk_autostart: *mut GtkWidget,
    pub chk_show_start: *mut GtkWidget,
    pub chk_check_updates: *mut GtkWidget,
    pub ed_profile: *mut GtkWidget,
    pub ed_sync_path: *mut GtkWidget,
    pub btn_custom: *mut GtkWidget,
    pub btn_conv: *mut GtkWidget,
    pub btn_close: *mut GtkWidget,
    pub btn_macro: *mut GtkWidget,
    pub btn_save_profile: *mut GtkWidget,
    pub btn_sandbox: *mut GtkWidget,
    pub btn_extras: *mut GtkWidget,
    pub btn_user_dict: *mut GtkWidget,
    pub btn_cap_sw: *mut GtkWidget,
    pub btn_cap_conv: *mut GtkWidget,
    pub btn_cap_sel_conv: *mut GtkWidget,
    pub btn_cap_undo: *mut GtkWidget,
    pub btn_import: *mut GtkWidget,
    pub btn_dir: *mut GtkWidget,
    pub btn_apps: *mut GtkWidget,
    pub btn_add_app: *mut GtkWidget,
    pub btn_sync_pick: *mut GtkWidget,
    pub btn_sync_clear: *mut GtkWidget,
    pub btn_check_update: *mut GtkWidget,
    pub btn_default: *mut GtkWidget,
    pub btn_save: *mut GtkWidget,
}

pub fn build_header_tools(st: &mut UiState) -> *mut GtkWidget {
    let tools = make_hbox(8);
    st.btn_ve = make_button("Tiếng Việt (V)", false);
    st.btn_conv = make_button("Chuyển mã", false);
    st.btn_sandbox = make_button("Gõ thử", false);
    add_css_class(st.btn_ve, "language-toggle");
    unsafe {
        let switch_hint = cstr("Chuyển giữa Tiếng Việt và Tiếng Anh");
        let sandbox_hint = cstr("Mở vùng thử bàn phím");
        gtk_widget_set_tooltip_text(st.btn_ve, switch_hint.as_ptr());
        gtk_widget_set_tooltip_text(st.btn_sandbox, sandbox_hint.as_ptr());
    }
    for button in [st.btn_ve, st.btn_sandbox, st.btn_conv] {
        pack(tools, button, false, false, 0);
    }
    tools
}

fn make_section(title: &str) -> *mut GtkWidget {
    let section = make_vbox(8);
    add_css_class(section, "settings-section");
    let label = make_label(title, false);
    add_css_class(label, "section-title");
    pack(section, label, false, false, 0);
    section
}

fn make_field(text: &str, body: *mut GtkWidget, focus: *mut GtkWidget) -> *mut GtkWidget {
    let field = make_vbox(4);
    let label = make_label(text, true);
    unsafe { gtk_label_set_mnemonic_widget(label, focus) };
    pack(field, label, false, false, 0);
    pack(field, body, false, false, 0);
    field
}

fn make_wrapped_label(text: &str, subtle: bool) -> *mut GtkWidget {
    let label = make_label(text, subtle);
    unsafe {
        gtk_label_set_line_wrap(label, 1);
        gtk_label_set_line_wrap_mode(label, PANGO_WRAP_WORD_CHAR);
    }
    label
}

fn settings_page() -> *mut GtkWidget {
    let page = make_vbox(16);
    add_css_class(page, "settings-page");
    page
}

pub fn build_input_settings(st: &mut UiState) -> *mut GtkWidget {
    let grid = make_grid(8, 12);
    add_css_class(grid, "settings-section");
    st.cb_charset = make_combo(&["Unicode", "TCVN3 (ABC)", "VNI Windows", "Unicode tổ hợp"]);
    st.cb_method = make_input_method_combo();
    st.btn_custom = make_button("Sửa", false);
    st.cb_mech = make_combo(&["Preedit (Wayland/GTK)", "Tương thích trực tiếp"]);
    let method_row = make_hbox(4);
    pack(method_row, st.cb_method, true, true, 0);
    pack(method_row, st.btn_custom, false, false, 0);
    unsafe {
        let custom_hint = cstr("Tự định nghĩa kiểu gõ");
        gtk_widget_set_tooltip_text(st.btn_custom, custom_hint.as_ptr());
        gtk_grid_set_column_homogeneous(grid, 1);
        gtk_grid_attach(grid, make_field("Bảng mã", st.cb_charset, st.cb_charset), 0, 0, 1, 1);
        gtk_grid_attach(grid, make_field("Kiểu gõ", method_row, st.cb_method), 1, 0, 1, 1);
        gtk_grid_attach(grid, make_field("Cơ chế gõ", st.cb_mech, st.cb_mech), 2, 0, 1, 1);
    }
    grid
}

pub fn build_tab_basic(st: &mut UiState) -> *mut GtkWidget {
    let page = settings_page();
    let grid = make_grid(0, 16);
    st.chk_modern = make_check("Đặt dấu kiểu mới (oà, uý)");
    st.chk_free_tone = make_check("Đặt dấu tự do");
    st.chk_fjwz = make_check("Cho phép f, j, w, z");
    st.chk_autocap = make_check("Viết hoa đầu câu");
    st.chk_smart_code = make_check("Tự bỏ qua từ code/camelCase");
    st.chk_clip_send = make_check("Dùng clipboard gửi phím");
    st.chk_std_send = make_check("Sử dụng SendKey chuẩn");
    st.chk_spell = make_check("Kiểm tra chính tả");
    st.chk_restore = make_check("Khôi phục phím từ sai");
    st.chk_reedit = make_check("Sửa lại từ trước con trỏ");
    st.chk_non_us = make_check("Tắt khi không phải phím US");
    st.chk_suggest1 = make_check("Sửa lỗi dấu của Gợi ý 1");
    st.chk_suggest2 = make_check("Sửa lỗi dấu của Gợi ý 2");
    let typing = make_section("Soạn tiếng Việt");
    let typing_options = make_vbox(4);
    st.btn_extras = make_button("Tùy chọn mở rộng...", false);
    for widget in [
        st.chk_modern,
        st.chk_free_tone,
        st.chk_fjwz,
        st.chk_autocap,
        st.chk_smart_code,
        st.chk_clip_send,
        st.chk_std_send,
    ] {
        pack(typing_options, widget, false, false, 0);
    }
    pack(typing, typing_options, true, true, 0);
    pack(typing, st.btn_extras, false, false, 0);
    let compatibility = make_section("Chính tả & tương thích");
    let compatibility_options = make_vbox(4);
    for widget in [
        st.chk_spell,
        st.chk_restore,
        st.chk_reedit,
        st.chk_non_us,
        st.chk_suggest1,
        st.chk_suggest2,
    ] {
        pack(compatibility_options, widget, false, false, 0);
    }
    st.btn_user_dict = make_button("Từ điển cá nhân...", false);
    pack(compatibility, compatibility_options, true, true, 0);
    pack(compatibility, st.btn_user_dict, false, false, 0);
    unsafe {
        gtk_grid_set_column_homogeneous(grid, 1);
        gtk_grid_attach(grid, typing, 0, 0, 1, 1);
        gtk_grid_attach(grid, compatibility, 1, 0, 1, 1);
    }
    pack(page, grid, false, false, 0);

    let mbox = make_section("Gõ tắt & phím nhanh");
    let r1 = make_hbox(8);
    st.chk_macro = make_check("Cho phép gõ tắt");
    st.btn_macro = make_button("Bảng gõ tắt...", false);
    pack(r1, st.chk_macro, true, true, 0);
    pack(r1, st.btn_macro, false, false, 0);
    let rm = make_hbox(12);
    st.chk_macro_en = make_check("Gõ tắt khi tắt Tiếng Việt");
    st.chk_macro_tab = make_check("Chỉ kích hoạt gõ tắt bằng Tab");
    pack(rm, st.chk_macro_en, true, true, 0);
    pack(rm, st.chk_macro_tab, true, true, 0);
    let r2 = make_hbox(12);
    st.chk_quick_w = make_check("Gõ w thành ư");
    st.chk_quick_brk = make_check("Gõ [ thành ơ, ] thành ư");
    pack(r2, st.chk_quick_w, true, true, 0);
    pack(r2, st.chk_quick_brk, true, true, 0);
    pack(mbox, r1, false, false, 0);
    pack(mbox, rm, false, false, 0);
    pack(mbox, r2, false, false, 0);
    pack(page, mbox, false, false, 0);

    let pbox = make_section("Hồ sơ cấu hình");
    let prow = make_hbox(8);
    st.cb_profile = make_combo(&["Mặc định", "Lập trình", "Soạn thảo"]);
    st.ed_profile = make_entry_placeholder("", "Tên hồ sơ để lưu...");
    st.btn_save_profile = make_button("Lưu hồ sơ", false);
    pack(prow, st.ed_profile, true, true, 0);
    pack(prow, st.btn_save_profile, false, false, 0);
    let profile_fields = make_hbox(12);
    pack(profile_fields, make_field("Đang dùng", st.cb_profile, st.cb_profile), true, true, 0);
    pack(profile_fields, make_field("Tên hồ sơ", prow, st.ed_profile), true, true, 0);
    pack(pbox, profile_fields, false, false, 0);
    pack(page, pbox, false, false, 0);
    page
}

pub fn build_tab_hotkey(st: &mut UiState) -> *mut GtkWidget {
    let page = settings_page();
    let sb = make_section("Chuyển Tiếng Việt / Tiếng Anh");
    let rgrid = make_grid(4, 16);
    st.rad_ctrl_shift = make_radio(ptr::null_mut(), "Ctrl + Shift");
    st.rad_alt_z = make_radio(st.rad_ctrl_shift, "Alt + Z");
    st.rad_ctrl_space = make_radio(st.rad_ctrl_shift, "Ctrl + Space");
    st.rad_shift_shift = make_radio(st.rad_ctrl_shift, "Shift + Shift");
    st.rad_custom = make_radio(st.rad_ctrl_shift, "Phím tùy chọn");
    unsafe {
        gtk_grid_attach(rgrid, st.rad_ctrl_shift, 0, 0, 1, 1);
        gtk_grid_attach(rgrid, st.rad_alt_z, 1, 0, 1, 1);
        gtk_grid_attach(rgrid, st.rad_ctrl_space, 2, 0, 1, 1);
        gtk_grid_attach(rgrid, st.rad_shift_shift, 0, 1, 1, 1);
        gtk_grid_attach(rgrid, st.rad_custom, 1, 1, 1, 1);
    }
    let crow = make_hbox(8);
    st.lbl_switch_custom = make_wrapped_label("Tổ hợp tùy chọn · —", false);
    st.btn_cap_sw = make_button("Đặt phím tùy chọn", false);
    pack(crow, st.lbl_switch_custom, true, true, 0);
    pack(crow, st.btn_cap_sw, false, false, 0);
    pack(sb, rgrid, false, false, 0);
    pack(sb, crow, false, false, 0);

    st.chk_beep = make_check("Phát âm thanh khi chuyển V/E");
    st.chk_caps_switch = make_check("Nhấn CapsLock để chuyển nhanh V/E");
    let switch_options = make_hbox(12);
    pack(switch_options, st.chk_beep, true, true, 0);
    pack(switch_options, st.chk_caps_switch, true, true, 0);
    pack(sb, switch_options, false, false, 0);

    let hold_row = make_hbox(8);
    st.cb_hold_en = make_combo(&[
        "Không dùng",
        "Right Shift",
        "Left Shift",
        "Right Alt",
        "Right Ctrl",
        "Caps Lock",
    ]);
    let hold_label = make_label("Giữ phím để tạm gõ Tiếng Anh", false);
    unsafe { gtk_label_set_mnemonic_widget(hold_label, st.cb_hold_en) };
    pack(hold_row, hold_label, false, false, 0);
    pack(hold_row, st.cb_hold_en, true, true, 0);
    pack(sb, hold_row, false, false, 0);
    pack(page, sb, false, false, 0);

    let cb = make_section("Chuyển mã & hoàn tác");
    st.chk_esc = make_check("Phím Esc khôi phục từ gốc");
    pack(cb, st.chk_esc, false, false, 0);
    let conv_r = make_hbox(8);
    st.lbl_conv_hotkey = make_wrapped_label("Chuyển clipboard · Ctrl + Shift + F6", false);
    st.btn_cap_conv = make_button("Đổi phím...", false);
    pack(conv_r, st.lbl_conv_hotkey, true, true, 0);
    pack(conv_r, st.btn_cap_conv, false, false, 0);

    let sel_r = make_hbox(8);
    st.lbl_sel_conv_hotkey = make_wrapped_label("Chuyển vùng chọn · Ctrl + Shift + F7", false);
    st.btn_cap_sel_conv = make_button("Đổi phím...", false);
    pack(sel_r, st.lbl_sel_conv_hotkey, true, true, 0);
    pack(sel_r, st.btn_cap_sel_conv, false, false, 0);

    let undo_r = make_hbox(8);
    st.lbl_undo_hotkey = make_wrapped_label("Hoàn tác từ vừa gõ · Ctrl + Shift + Z", false);
    st.btn_cap_undo = make_button("Đổi phím...", false);
    pack(undo_r, st.lbl_undo_hotkey, true, true, 0);
    pack(undo_r, st.btn_cap_undo, false, false, 0);

    pack(cb, conv_r, false, false, 0);
    pack(cb, sel_r, false, false, 0);
    pack(cb, undo_r, false, false, 0);
    pack(page, cb, false, false, 0);
    page
}

pub fn build_tab_system(st: &mut UiState) -> *mut GtkWidget {
    let page = settings_page();
    let appearance = make_section("Khởi động & hiển thị");
    st.chk_autostart = make_check("Khởi động cùng hệ thống");
    st.chk_show_start = make_check("Mở bảng cài đặt khi khởi động");
    st.chk_caret_osd = make_check("Hiện chỉ báo V/E cạnh con trỏ");
    let launch_row = make_hbox(12);
    pack(launch_row, st.chk_autostart, true, true, 0);
    pack(launch_row, st.chk_show_start, true, true, 0);
    st.cb_theme = make_combo(&["Theo hệ thống", "Sáng", "Tối"]);
    let appearance_row = make_hbox(12);
    pack(appearance_row, make_field("Giao diện", st.cb_theme, st.cb_theme), true, true, 0);
    pack(appearance_row, st.chk_caret_osd, true, true, 0);
    pack(appearance, launch_row, false, false, 0);
    pack(appearance, appearance_row, false, false, 0);
    pack(page, appearance, false, false, 0);

    let sync = make_section("Cấu hình & đồng bộ");
    let sync_r = make_hbox(8);
    st.ed_sync_path = make_entry_placeholder("", "Đường dẫn tệp đồng bộ config.json...");
    st.btn_sync_pick = make_button("Chọn tệp...", false);
    st.btn_sync_clear = make_button("Xóa", false);
    pack(sync_r, st.ed_sync_path, true, true, 0);
    pack(sync_r, st.btn_sync_pick, false, false, 0);
    pack(sync_r, st.btn_sync_clear, false, false, 0);
    pack(sync, make_field("Tệp cấu hình dùng chung", sync_r, st.ed_sync_path), false, false, 0);
    let config_tools = make_hbox(8);
    st.btn_import = make_button("Nhập cấu hình EVKey...", false);
    st.btn_dir = make_button("Mở thư mục cấu hình", false);
    pack(config_tools, st.btn_import, true, true, 0);
    pack(config_tools, st.btn_dir, true, true, 0);
    pack(sync, config_tools, false, false, 0);
    pack(page, sync, false, false, 0);

    let app_rules = make_section("Quy tắc theo ứng dụng");
    st.lbl_cur_app = make_wrapped_label("Ứng dụng gần nhất · —", true);
    let app_tools = make_hbox(8);
    st.btn_apps = make_button("Danh sách ứng dụng...", false);
    st.btn_add_app = make_button("Luôn gõ Tiếng Anh cho ứng dụng này", false);
    pack(app_tools, st.btn_apps, true, true, 0);
    pack(app_tools, st.btn_add_app, true, true, 0);
    pack(app_rules, st.lbl_cur_app, false, false, 0);
    pack(app_rules, app_tools, false, false, 0);
    pack(page, app_rules, false, false, 0);

    let updates = make_section("Cập nhật");
    st.chk_check_updates = make_check("Tự kiểm tra khi khởi động");
    st.btn_check_update = make_button("Kiểm tra cập nhật", false);
    st.lbl_update_status = make_wrapped_label("", true);
    let update_actions = make_hbox(8);
    pack(update_actions, st.chk_check_updates, true, true, 0);
    pack(update_actions, st.btn_check_update, false, false, 0);
    pack(updates, update_actions, false, false, 0);
    pack(updates, st.lbl_update_status, false, false, 0);
    pack(page, updates, false, false, 0);
    page
}

impl UiState {
    pub fn empty(win: *mut GtkWidget, cfg: Config) -> Self {
        let n = ptr::null_mut();
        Self {
            win,
            cfg,
            syncing: false,
            cb_charset: n,
            cb_method: n,
            cb_mech: n,
            cb_theme: n,
            cb_hold_en: n,
            cb_profile: n,
            btn_ve: n,
            lbl_cur_app: n,
            lbl_switch_custom: n,
            lbl_conv_hotkey: n,
            lbl_sel_conv_hotkey: n,
            lbl_undo_hotkey: n,
            lbl_update_status: n,
            chk_modern: n,
            chk_free_tone: n,
            chk_fjwz: n,
            chk_autocap: n,
            chk_smart_code: n,
            chk_clip_send: n,
            chk_std_send: n,
            chk_spell: n,
            chk_restore: n,
            chk_reedit: n,
            chk_non_us: n,
            chk_suggest1: n,
            chk_suggest2: n,
            chk_caret_osd: n,
            chk_macro: n,
            chk_macro_en: n,
            chk_macro_tab: n,
            chk_quick_w: n,
            chk_quick_brk: n,
            rad_ctrl_shift: n,
            rad_alt_z: n,
            rad_ctrl_space: n,
            rad_shift_shift: n,
            rad_custom: n,
            chk_esc: n,
            chk_beep: n,
            chk_caps_switch: n,
            chk_autostart: n,
            chk_show_start: n,
            chk_check_updates: n,
            ed_profile: n,
            ed_sync_path: n,
            btn_custom: n,
            btn_conv: n,
            btn_close: n,
            btn_macro: n,
            btn_save_profile: n,
            btn_sandbox: n,
            btn_extras: n,
            btn_user_dict: n,
            btn_cap_sw: n,
            btn_cap_conv: n,
            btn_cap_sel_conv: n,
            btn_cap_undo: n,
            btn_import: n,
            btn_dir: n,
            btn_apps: n,
            btn_add_app: n,
            btn_sync_pick: n,
            btn_sync_clear: n,
            btn_check_update: n,
            btn_default: n,
            btn_save: n,
        }
    }

    pub fn sync_from_config(&mut self, cur_app: &str) {
        self.syncing = true;
        self.sync_combos_and_labels(cur_app);
        self.sync_toggles();
        self.syncing = false;
    }

    fn sync_combos_and_labels(&self, cur_app: &str) {
        let c = &self.cfg;
        set_combo(
            self.cb_charset,
            match c.charset {
                Charset::Unicode => 0,
                Charset::Tcvn3 => 1,
                Charset::VniWindows => 2,
                Charset::UnicodeComposite => 3,
            },
        );
        set_input_method(self.cb_method, c.input_method);
        set_combo(self.cb_mech, if c.mechanism == Mechanism::CompatibleHook { 1 } else { 0 });
        set_combo(
            self.cb_theme,
            match c.theme {
                ThemeMode::System => 0,
                ThemeMode::Light => 1,
                ThemeMode::Dark => 2,
            },
        );
        set_combo(
            self.cb_hold_en,
            match c.hold_english_key {
                HoldEnglishKey::None => 0,
                HoldEnglishKey::RightShift => 1,
                HoldEnglishKey::LeftShift => 2,
                HoldEnglishKey::RightAlt => 3,
                HoldEnglishKey::RightCtrl => 4,
                HoldEnglishKey::CapsLock => 5,
            },
        );
        let profile_names = c.all_profile_names();
        let active_p = profile_names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(c.active_profile.trim()))
            .unwrap_or(0);
        refill_combo(self.cb_profile, &profile_names, active_p);
        write_entry(self.ed_sync_path, &c.sync_config_path);

        set_language_button(self.btn_ve, c.vietnamese_enabled);
        let recent_app = if cur_app == "-" || cur_app.is_empty() { "—" } else { cur_app };
        set_label_text(self.lbl_cur_app, &format!("Ứng dụng gần nhất · {recent_app}"));
        unsafe { gtk_widget_set_sensitive(self.btn_add_app, i32::from(recent_app != "—")) };
        set_label_text(
            self.lbl_switch_custom,
            &format!("Tổ hợp tùy chọn · {}", format_hotkey_linux(c.custom_switch_hotkey)),
        );
        set_label_text(
            self.lbl_conv_hotkey,
            &format!("Chuyển clipboard · {}", format_hotkey_linux(c.clipboard_convert_hotkey)),
        );
        set_label_text(
            self.lbl_sel_conv_hotkey,
            &format!("Chuyển vùng chọn · {}", format_hotkey_linux(c.convert_selection_hotkey)),
        );
        set_label_text(
            self.lbl_undo_hotkey,
            &format!("Hoàn tác từ vừa gõ · {}", format_hotkey_linux(c.undo_last_word_hotkey)),
        );
    }

    fn sync_toggles(&self) {
        let c = &self.cfg;
        set_toggle(self.chk_modern, c.modern_tone);
        set_toggle(self.chk_free_tone, c.free_tone);
        set_toggle(self.chk_fjwz, c.allow_fjwz);
        set_toggle(self.chk_autocap, c.auto_capitalize_sentence);
        set_toggle(self.chk_smart_code, c.smart_code_bypass);
        set_toggle(self.chk_clip_send, c.use_clipboard_send);
        set_toggle(self.chk_std_send, c.standard_key_send);
        set_toggle(self.chk_spell, c.spelling_check);
        set_toggle(self.chk_restore, c.auto_restore_wrong_word);
        set_toggle(self.chk_reedit, c.reedit_surrounding_word);
        set_toggle(self.chk_non_us, c.disable_on_non_us_layout);
        set_toggle(self.chk_suggest1, c.fix_browser_suggest_1);
        set_toggle(self.chk_suggest2, c.fix_browser_suggest_2);
        set_toggle(self.chk_caret_osd, c.show_caret_osd);
        set_toggle(self.chk_macro, c.macro_enabled);
        set_toggle(self.chk_macro_en, c.macro_when_english);
        set_toggle(self.chk_macro_tab, c.macro_trigger_tab_only);
        set_toggle(self.chk_quick_w, c.quick_w_to_u_horn);
        set_toggle(self.chk_quick_brk, c.quick_brackets);
        set_toggle(self.rad_ctrl_shift, c.switch_preset == SwitchPreset::CtrlShift);
        set_toggle(self.rad_alt_z, c.switch_preset == SwitchPreset::AltZ);
        set_toggle(self.rad_ctrl_space, c.switch_preset == SwitchPreset::CtrlSpace);
        set_toggle(self.rad_shift_shift, c.switch_preset == SwitchPreset::ShiftShift);
        set_toggle(self.rad_custom, c.switch_preset == SwitchPreset::Custom);
        set_toggle(self.chk_esc, c.esc_restore);
        set_toggle(self.chk_beep, c.beep_on_switch);
        set_toggle(self.chk_caps_switch, c.quick_capslock_switch);
        set_toggle(self.chk_autostart, c.auto_start);
        set_toggle(self.chk_show_start, c.show_on_startup);
        set_toggle(self.chk_check_updates, c.check_updates_on_start);
    }

    pub fn read_to_config(&mut self) -> Option<ThemeMode> {
        if self.syncing {
            return None;
        }
        let prev_theme = self.cfg.theme;
        self.cfg.charset = match get_combo(self.cb_charset) {
            1 => Charset::Tcvn3,
            2 => Charset::VniWindows,
            3 => Charset::UnicodeComposite,
            _ => Charset::Unicode,
        };
        self.cfg.input_method = get_input_method(self.cb_method);
        self.cfg.mechanism = if get_combo(self.cb_mech) == 1 {
            Mechanism::CompatibleHook
        } else {
            Mechanism::KeyboardHook
        };
        self.cfg.theme = match get_combo(self.cb_theme) {
            1 => ThemeMode::Light,
            2 => ThemeMode::Dark,
            _ => ThemeMode::System,
        };
        self.cfg.hold_english_key = match get_combo(self.cb_hold_en) {
            1 => HoldEnglishKey::RightShift,
            2 => HoldEnglishKey::LeftShift,
            3 => HoldEnglishKey::RightAlt,
            4 => HoldEnglishKey::RightCtrl,
            5 => HoldEnglishKey::CapsLock,
            _ => HoldEnglishKey::None,
        };
        self.cfg.modern_tone = get_toggle(self.chk_modern);
        self.cfg.free_tone = get_toggle(self.chk_free_tone);
        self.cfg.allow_fjwz = get_toggle(self.chk_fjwz);
        self.cfg.auto_capitalize_sentence = get_toggle(self.chk_autocap);
        self.cfg.smart_code_bypass = get_toggle(self.chk_smart_code);
        self.cfg.use_clipboard_send = get_toggle(self.chk_clip_send);
        self.cfg.standard_key_send = get_toggle(self.chk_std_send);
        self.cfg.spelling_check = get_toggle(self.chk_spell);
        self.cfg.auto_restore_wrong_word = get_toggle(self.chk_restore);
        self.cfg.reedit_surrounding_word = get_toggle(self.chk_reedit);
        self.cfg.disable_on_non_us_layout = get_toggle(self.chk_non_us);
        self.cfg.fix_browser_suggest_1 = get_toggle(self.chk_suggest1);
        self.cfg.fix_browser_suggest_2 = get_toggle(self.chk_suggest2);
        self.cfg.show_caret_osd = get_toggle(self.chk_caret_osd);
        self.cfg.macro_enabled = get_toggle(self.chk_macro);
        self.cfg.macro_when_english = get_toggle(self.chk_macro_en);
        self.cfg.macro_trigger_tab_only = get_toggle(self.chk_macro_tab);
        self.cfg.quick_w_to_u_horn = get_toggle(self.chk_quick_w);
        self.cfg.quick_brackets = get_toggle(self.chk_quick_brk);
        self.cfg.switch_preset = if get_toggle(self.rad_alt_z) {
            SwitchPreset::AltZ
        } else if get_toggle(self.rad_ctrl_space) {
            SwitchPreset::CtrlSpace
        } else if get_toggle(self.rad_shift_shift) {
            SwitchPreset::ShiftShift
        } else if get_toggle(self.rad_custom) {
            SwitchPreset::Custom
        } else {
            SwitchPreset::CtrlShift
        };
        self.cfg.esc_restore = get_toggle(self.chk_esc);
        self.cfg.beep_on_switch = get_toggle(self.chk_beep);
        self.cfg.quick_capslock_switch = get_toggle(self.chk_caps_switch);
        self.cfg.auto_start = get_toggle(self.chk_autostart);
        self.cfg.show_on_startup = get_toggle(self.chk_show_start);
        self.cfg.check_updates_on_start = get_toggle(self.chk_check_updates);
        self.cfg.sync_config_path = read_entry(self.ed_sync_path).trim().to_string();
        if self.cfg.theme != prev_theme { Some(self.cfg.theme) } else { None }
    }
}
