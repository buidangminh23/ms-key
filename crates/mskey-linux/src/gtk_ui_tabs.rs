use std::ptr;

use mskey_core::{Charset, Config, HoldEnglishKey, InputMethod, Mechanism, SwitchPreset, ThemeMode};

use crate::gio_ffi::cstr;
use crate::gtk_ffi::{
    GtkWidget, gtk_button_set_label, gtk_grid_attach, gtk_widget_set_hexpand,
    gtk_widget_set_size_request,
};
use crate::gtk_helpers::{
    get_combo, get_toggle, make_button, make_check, make_combo, make_entry_placeholder,
    make_frame, make_grid, make_hbox, make_label, make_radio, make_vbox, pack, read_entry,
    refill_combo, set_combo, set_label_text, set_toggle, write_entry,
};
use crate::gtk_theme::{add_css_class, set_ve_button_style};
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
    pub lbl_app: *mut GtkWidget,
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
    pub chk_smart_app: *mut GtkWidget,
    pub ed_profile: *mut GtkWidget,
    pub ed_sync_path: *mut GtkWidget,
    pub btn_custom: *mut GtkWidget,
    pub btn_conv: *mut GtkWidget,
    pub btn_close: *mut GtkWidget,
    pub btn_exit: *mut GtkWidget,
    pub btn_macro: *mut GtkWidget,
    pub btn_save_profile: *mut GtkWidget,
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

pub fn build_header_box(st: &mut UiState) -> *mut GtkWidget {
    let top_row = make_hbox(8);
    let (frame, frame_box) = make_frame("Điều khiển");
    let grid = make_grid(8, 8);
    st.cb_charset = make_combo(&["Unicode", "TCVN3 (ABC)", "VNI Windows", "Unicode tổ hợp"]);
    st.btn_ve = make_button("Tiếng Việt (V)", false);
    st.cb_method = make_combo(&["Telex", "VNI", "VIQR", "Simple Telex", "Tự định nghĩa"]);
    st.btn_custom = make_button("...", false);
    st.btn_conv = make_button("Chuyển mã...", false);
    st.cb_mech = make_combo(&["Preedit (Wayland/GTK)", "Tương thích trực tiếp"]);
    st.lbl_app = make_label("App: -", true);
    unsafe {
        gtk_widget_set_size_request(st.btn_custom, 36, 28);
        gtk_grid_attach(grid, make_label("Bảng mã", false), 0, 0, 1, 1);
        gtk_grid_attach(grid, st.cb_charset, 1, 0, 2, 1);
        gtk_grid_attach(grid, st.btn_ve, 3, 0, 1, 1);
        gtk_grid_attach(grid, make_label("Kiểu gõ", false), 0, 1, 1, 1);
        gtk_grid_attach(grid, st.cb_method, 1, 1, 1, 1);
        gtk_grid_attach(grid, st.btn_custom, 2, 1, 1, 1);
        gtk_grid_attach(grid, st.btn_conv, 3, 1, 1, 1);
        gtk_grid_attach(grid, make_label("Cơ chế", false), 0, 2, 1, 1);
        gtk_grid_attach(grid, st.cb_mech, 1, 2, 2, 1);
        gtk_grid_attach(grid, st.lbl_app, 3, 2, 1, 1);
    }
    pack(frame_box, grid, true, true, 0);
    pack(top_row, frame, true, true, 0);

    let act_col = make_vbox(8);
    st.btn_close = make_button("Đóng", false);
    st.btn_exit = make_button("Thoát", false);
    unsafe {
        gtk_widget_set_size_request(st.btn_close, 84, 32);
        gtk_widget_set_size_request(st.btn_exit, 84, 32);
    }
    pack(act_col, st.btn_close, false, false, 0);
    pack(act_col, st.btn_exit, false, false, 0);
    pack(top_row, act_col, false, false, 0);
    top_row
}

pub fn build_tab_basic(st: &mut UiState) -> *mut GtkWidget {
    let page = make_vbox(8);
    add_css_class(page, "card-box");
    let grid = make_grid(4, 16);
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
    st.chk_caret_osd = make_check("Hiện chỉ báo V/E cạnh con trỏ");
    let left = [
        st.chk_modern,
        st.chk_free_tone,
        st.chk_fjwz,
        st.chk_autocap,
        st.chk_smart_code,
        st.chk_clip_send,
        st.chk_std_send,
    ];
    let right = [
        st.chk_spell,
        st.chk_restore,
        st.chk_reedit,
        st.chk_non_us,
        st.chk_suggest1,
        st.chk_suggest2,
        st.chk_caret_osd,
    ];
    for (r, &w) in left.iter().enumerate() {
        unsafe { gtk_grid_attach(grid, w, 0, r as i32, 1, 1) };
    }
    for (r, &w) in right.iter().enumerate() {
        unsafe { gtk_grid_attach(grid, w, 1, r as i32, 1, 1) };
    }
    pack(page, grid, false, false, 0);

    let (mframe, mbox) = make_frame("Gõ tắt & phím tắt nhanh");
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
    pack(page, mframe, false, false, 0);

    let (pframe, pbox) = make_frame("Hồ sơ cấu hình");
    let prow = make_hbox(8);
    st.cb_profile = make_combo(&["Mặc định", "Lập trình", "Soạn thảo"]);
    st.ed_profile = make_entry_placeholder("", "Tên hồ sơ để lưu...");
    st.btn_save_profile = make_button("Lưu hồ sơ", false);
    unsafe { gtk_widget_set_hexpand(st.ed_profile, 1) };
    pack(prow, st.cb_profile, false, false, 0);
    pack(prow, st.ed_profile, true, true, 0);
    pack(prow, st.btn_save_profile, false, false, 0);
    pack(pbox, prow, false, false, 0);
    pack(page, pframe, false, false, 0);
    page
}

pub fn build_tab_hotkey(st: &mut UiState) -> *mut GtkWidget {
    let page = make_vbox(8);
    add_css_class(page, "card-box");
    let (sf, sb) = make_frame("Phím chuyển V/E");
    let rgrid = make_grid(4, 12);
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
    st.lbl_switch_custom = make_label("Tổ hợp tùy chọn: -", false);
    st.btn_cap_sw = make_button("Đặt phím tùy chọn", false);
    pack(crow, st.lbl_switch_custom, true, true, 0);
    pack(crow, st.btn_cap_sw, false, false, 0);
    pack(sb, rgrid, false, false, 0);
    pack(sb, crow, false, false, 0);
    pack(page, sf, false, false, 0);

    let trow = make_hbox(12);
    st.chk_esc = make_check("Phím Esc khôi phục từ gốc");
    st.chk_beep = make_check("Âm thanh báo khi chuyển V/E");
    pack(trow, st.chk_esc, true, true, 0);
    pack(trow, st.chk_beep, true, true, 0);
    pack(page, trow, false, false, 0);

    st.chk_caps_switch = make_check("Nhấn CapsLock để chuyển nhanh V/E");
    pack(page, st.chk_caps_switch, false, false, 0);

    let hrow = make_hbox(8);
    st.cb_hold_en = make_combo(&[
        "Không dùng",
        "Right Shift",
        "Left Shift",
        "Right Alt",
        "Right Ctrl",
        "Caps Lock",
    ]);
    pack(hrow, make_label("Giữ phím tạm chuyển Tiếng Anh", false), false, false, 0);
    pack(hrow, st.cb_hold_en, true, true, 0);
    pack(page, hrow, false, false, 0);

    let (cf, cb) = make_frame("Phím tắt chức năng & Nhập EVKey");
    let conv_r = make_hbox(8);
    st.lbl_conv_hotkey = make_label("Chuyển clipboard: Ctrl + Shift + F6", false);
    st.btn_cap_conv = make_button("Đổi phím...", false);
    pack(conv_r, st.lbl_conv_hotkey, true, true, 0);
    pack(conv_r, st.btn_cap_conv, false, false, 0);

    let sel_r = make_hbox(8);
    st.lbl_sel_conv_hotkey = make_label("Chuyển vùng chọn: Ctrl + Shift + F7", false);
    st.btn_cap_sel_conv = make_button("Đổi phím...", false);
    pack(sel_r, st.lbl_sel_conv_hotkey, true, true, 0);
    pack(sel_r, st.btn_cap_sel_conv, false, false, 0);

    let undo_r = make_hbox(8);
    st.lbl_undo_hotkey = make_label("Hoàn tác từ vừa gõ: Ctrl + Shift + Z", false);
    st.btn_cap_undo = make_button("Đổi phím...", false);
    pack(undo_r, st.lbl_undo_hotkey, true, true, 0);
    pack(undo_r, st.btn_cap_undo, false, false, 0);

    let io_r = make_hbox(8);
    st.btn_import = make_button("Nhập EVKey setting.tree...", false);
    st.btn_dir = make_button("Mở thư mục cấu hình", false);
    pack(io_r, st.btn_import, true, true, 0);
    pack(io_r, st.btn_dir, true, true, 0);
    pack(cb, conv_r, false, false, 0);
    pack(cb, sel_r, false, false, 0);
    pack(cb, undo_r, false, false, 0);
    pack(cb, io_r, false, false, 0);
    pack(page, cf, false, false, 0);
    page
}

pub fn build_tab_system(st: &mut UiState) -> *mut GtkWidget {
    let page = make_vbox(8);
    add_css_class(page, "card-box");
    st.chk_autostart = make_check("Khởi động cùng hệ thống (XDG Autostart)");
    let srow = make_hbox(12);
    st.chk_show_start = make_check("Hiện bảng điều khiển khi mở");
    st.chk_check_updates = make_check("Tự kiểm tra cập nhật khi mở");
    pack(srow, st.chk_show_start, true, true, 0);
    pack(srow, st.chk_check_updates, true, true, 0);
    let trow = make_hbox(8);
    st.cb_theme = make_combo(&["Theo hệ thống", "Sáng (Light)", "Tối (Dark)"]);
    pack(trow, make_label("Giao diện", false), false, false, 0);
    pack(trow, st.cb_theme, true, true, 0);
    pack(page, st.chk_autostart, false, false, 0);
    pack(page, srow, false, false, 0);
    pack(page, trow, false, false, 0);

    let (sf, sb) = make_frame("Đồng bộ cấu hình & Cập nhật");
    let sync_r = make_hbox(6);
    st.ed_sync_path = make_entry_placeholder("", "Đường dẫn tệp đồng bộ config.json...");
    st.btn_sync_pick = make_button("Chọn tệp...", false);
    st.btn_sync_clear = make_button("Xóa", false);
    unsafe { gtk_widget_set_hexpand(st.ed_sync_path, 1) };
    pack(sync_r, st.ed_sync_path, true, true, 0);
    pack(sync_r, st.btn_sync_pick, false, false, 0);
    pack(sync_r, st.btn_sync_clear, false, false, 0);
    let upd_r = make_hbox(8);
    st.btn_check_update = make_button("Kiểm tra cập nhật", false);
    st.lbl_update_status = make_label("", true);
    pack(upd_r, st.btn_check_update, false, false, 0);
    pack(upd_r, st.lbl_update_status, true, true, 0);
    pack(sb, sync_r, false, false, 0);
    pack(sb, upd_r, false, false, 0);
    pack(page, sf, false, false, 0);

    let (af, ab) = make_frame("Tự động chuyển V/E theo ứng dụng");
    st.chk_smart_app = make_check("Tự động nhớ V/E theo ứng dụng");
    st.lbl_cur_app = make_label("App gần nhất: -", false);
    let brow = make_hbox(8);
    st.btn_apps = make_button("Danh sách ứng dụng...", false);
    st.btn_add_app = make_button("Thêm app vào Tiếng Anh", false);
    pack(brow, st.btn_apps, true, true, 0);
    pack(brow, st.btn_add_app, true, true, 0);
    let hint = make_label("Hỗ trợ Luôn Tiếng Anh, Luôn Tiếng Việt & Blacklist", true);
    pack(ab, st.chk_smart_app, false, false, 0);
    pack(ab, st.lbl_cur_app, false, false, 0);
    pack(ab, brow, false, false, 0);
    pack(ab, hint, false, false, 0);
    pack(page, af, false, false, 0);

    let erow = make_hbox(8);
    st.btn_extras = make_button("Mở rộng & Gõ thử...", false);
    st.btn_user_dict = make_button("Từ điển cá nhân...", false);
    pack(erow, st.btn_extras, true, true, 0);
    pack(erow, st.btn_user_dict, true, true, 0);
    pack(page, erow, false, false, 0);
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
            lbl_app: n,
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
            chk_smart_app: n,
            ed_profile: n,
            ed_sync_path: n,
            btn_custom: n,
            btn_conv: n,
            btn_close: n,
            btn_exit: n,
            btn_macro: n,
            btn_save_profile: n,
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
        set_combo(
            self.cb_method,
            match c.input_method {
                InputMethod::Telex => 0,
                InputMethod::Vni => 1,
                InputMethod::Viqr => 2,
                InputMethod::SimpleTelex => 3,
                InputMethod::Custom => 4,
            },
        );
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

        let cp = cstr(if c.vietnamese_enabled { "Tiếng Việt (V)" } else { "Tiếng Anh (E)" });
        unsafe { gtk_button_set_label(self.btn_ve, cp.as_ptr()) };
        set_ve_button_style(self.btn_ve, c.vietnamese_enabled);
        set_label_text(self.lbl_app, &format!("App: {cur_app}"));
        set_label_text(self.lbl_cur_app, &format!("App gần nhất: {cur_app}"));
        set_label_text(
            self.lbl_switch_custom,
            &format!("Tổ hợp tùy chọn: {}", format_hotkey_linux(c.custom_switch_hotkey)),
        );
        set_label_text(
            self.lbl_conv_hotkey,
            &format!("Chuyển clipboard: {}", format_hotkey_linux(c.clipboard_convert_hotkey)),
        );
        set_label_text(
            self.lbl_sel_conv_hotkey,
            &format!("Chuyển vùng chọn: {}", format_hotkey_linux(c.convert_selection_hotkey)),
        );
        set_label_text(
            self.lbl_undo_hotkey,
            &format!("Hoàn tác từ vừa gõ: {}", format_hotkey_linux(c.undo_last_word_hotkey)),
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
        set_toggle(self.chk_smart_app, c.smart_app_switch);
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
        self.cfg.input_method = match get_combo(self.cb_method) {
            1 => InputMethod::Vni,
            2 => InputMethod::Viqr,
            3 => InputMethod::SimpleTelex,
            4 => InputMethod::Custom,
            _ => InputMethod::Telex,
        };
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
        self.cfg.smart_app_switch = get_toggle(self.chk_smart_app);
        self.cfg.sync_config_path = read_entry(self.ed_sync_path).trim().to_string();
        if self.cfg.theme != prev_theme { Some(self.cfg.theme) } else { None }
    }
}
