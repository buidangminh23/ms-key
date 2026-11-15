use std::cell::RefCell;
use std::ffi::c_void;
use std::fs;
use std::ptr;

use mskey_core::{AppBehavior, AppRule, Config, CustomKeyMap, MacroEntry};

use crate::gtk_dialogs::pick_file_path;
use crate::gtk_ffi::{
    GTK_POLICY_AUTOMATIC, GTK_POLICY_NEVER, GTK_SELECTION_SINGLE, GtkWidget, GtkWindow,
    connect_signal, gtk_container_add, gtk_entry_set_max_length, gtk_grid_attach,
    gtk_list_box_get_selected_row, gtk_list_box_new, gtk_list_box_row_get_index,
    gtk_list_box_set_selection_mode, gtk_scrolled_window_new, gtk_scrolled_window_set_policy,
    gtk_widget_destroy, gtk_widget_set_hexpand, gtk_widget_set_size_request,
    gtk_widget_set_vexpand,
};
use crate::gtk_helpers::{
    append_listbox_row, clear_listbox, create_modal_gtk, get_combo, make_button, make_combo,
    make_entry, make_grid, make_hbox, make_label, make_vbox, pack, read_entry, run_modal_gtk,
    set_combo, write_entry,
};

struct MacroWidgets {
    dlg: *mut GtkWidget,
    cfg: Config,
    list: *mut GtkWidget,
    key_ed: *mut GtkWidget,
    val_ed: *mut GtkWidget,
}

struct AppWidgets {
    dlg: *mut GtkWidget,
    cfg: Config,
    list: *mut GtkWidget,
    name_ed: *mut GtkWidget,
    mode_cb: *mut GtkWidget,
}

struct CustomWidgets {
    dlg: *mut GtkWidget,
    cfg: Config,
    edits: Vec<*mut GtkWidget>,
    saved: bool,
}

thread_local! {
    static MACRO_DLG: RefCell<Option<MacroWidgets>> = const { RefCell::new(None) };
    static APP_DLG: RefCell<Option<AppWidgets>> = const { RefCell::new(None) };
    static CUST_DLG: RefCell<Option<CustomWidgets>> = const { RefCell::new(None) };
}

pub fn show_macro_dialog(parent: *mut GtkWindow, cfg: &mut Config) -> bool {
    let dlg = create_modal_gtk(parent, "Bảng gõ tắt (MS Key)", 464, 340);
    let root = make_vbox(8);
    let grid = make_grid(4, 8);
    let key_ed = make_entry("");
    let val_ed = make_entry("");
    unsafe {
        gtk_widget_set_hexpand(val_ed, 1);
        gtk_grid_attach(grid, make_label("Từ viết tắt", false), 0, 0, 1, 1);
        gtk_grid_attach(grid, make_label("Nội dung thay thế", false), 1, 0, 1, 1);
        gtk_grid_attach(grid, key_ed, 0, 1, 1, 1);
        gtk_grid_attach(grid, val_ed, 1, 1, 1, 1);
    }
    pack(root, grid, false, false, 0);

    let bar = make_hbox(6);
    let b_add = make_button("Thêm / Cập nhật", true);
    let b_tgl = make_button("Bật / Tắt", false);
    let b_del = make_button("Xóa mục chọn", false);
    pack(bar, b_add, false, false, 0);
    pack(bar, b_tgl, false, false, 0);
    pack(bar, b_del, false, false, 0);
    pack(root, bar, false, false, 0);

    let bar2 = make_hbox(6);
    let b_imp = make_button("Nhập .txt...", false);
    let b_exp = make_button("Xuất .txt...", false);
    let b_close = make_button("Đóng", false);
    pack(bar2, b_imp, false, false, 0);
    pack(bar2, b_exp, false, false, 0);
    pack(bar2, make_label("", false), true, true, 0);
    pack(bar2, b_close, false, false, 0);
    pack(root, bar2, false, false, 0);

    let sw = unsafe { gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut()) };
    let list = unsafe { gtk_list_box_new() };
    unsafe {
        gtk_widget_set_hexpand(sw, 1);
        gtk_widget_set_vexpand(sw, 1);
        gtk_scrolled_window_set_policy(sw, GTK_POLICY_NEVER, GTK_POLICY_AUTOMATIC);
        gtk_list_box_set_selection_mode(list, GTK_SELECTION_SINGLE);
        gtk_container_add(sw, list);
    }
    pack(root, sw, true, true, 0);
    unsafe { gtk_container_add(dlg, root) };

    MACRO_DLG.with(|c| {
        *c.borrow_mut() = Some(MacroWidgets { dlg, cfg: cfg.clone(), list, key_ed, val_ed });
    });
    reload_macro_list();
    connect_signal(b_add, "clicked", on_macro_add as *const c_void, 0);
    connect_signal(b_tgl, "clicked", on_macro_toggle as *const c_void, 0);
    connect_signal(b_del, "clicked", on_macro_del as *const c_void, 0);
    connect_signal(b_imp, "clicked", on_macro_import as *const c_void, 0);
    connect_signal(b_exp, "clicked", on_macro_export as *const c_void, 0);
    connect_signal(b_close, "clicked", on_macro_close as *const c_void, 0);
    connect_signal(list, "row-selected", on_macro_sel as *const c_void, 0);
    run_modal_gtk(dlg);
    if let Some(st) = MACRO_DLG.with(|c| c.borrow_mut().take()) {
        *cfg = st.cfg;
        return true;
    }
    false
}

fn reload_macro_list() {
    MACRO_DLG.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            clear_listbox(st.list);
            for m in &st.cfg.macros {
                let prefix = if m.enabled { "" } else { "[Tắt] " };
                let exp = m.expansion.replace('\n', "\\n");
                append_listbox_row(st.list, &format!("{prefix}{}  ->  {exp}", m.shortcut));
            }
        }
    });
}

unsafe extern "C" fn on_macro_sel(_: *mut GtkWidget, _: *mut GtkWidget, _: *mut c_void) {
    MACRO_DLG.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            let row = unsafe { gtk_list_box_get_selected_row(st.list) };
            if !row.is_null() {
                let idx = unsafe { gtk_list_box_row_get_index(row) };
                if let Some(entry) = st.cfg.macros.get(idx as usize) {
                    write_entry(st.key_ed, &entry.shortcut);
                    write_entry(st.val_ed, &entry.expansion.replace('\n', "\\n"));
                }
            }
        }
    });
}

unsafe extern "C" fn on_macro_add(_: *mut GtkWidget, _: *mut c_void) {
    MACRO_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            let k = read_entry(st.key_ed).trim().to_string();
            let v = read_entry(st.val_ed).replace("\\n", "\n");
            if k.is_empty() {
                return;
            }
            if let Some(ex) = st.cfg.macros.iter_mut().find(|m| m.shortcut.eq_ignore_ascii_case(&k)) {
                ex.expansion = v;
                ex.enabled = true;
            } else {
                st.cfg.macros.push(MacroEntry { shortcut: k, expansion: v, enabled: true });
            }
        }
    });
    reload_macro_list();
}

unsafe extern "C" fn on_macro_toggle(_: *mut GtkWidget, _: *mut c_void) {
    MACRO_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            let row = unsafe { gtk_list_box_get_selected_row(st.list) };
            if !row.is_null() {
                let idx = unsafe { gtk_list_box_row_get_index(row) } as usize;
                if let Some(m) = st.cfg.macros.get_mut(idx) {
                    m.enabled = !m.enabled;
                }
            }
        }
    });
    reload_macro_list();
}

unsafe extern "C" fn on_macro_import(_: *mut GtkWidget, _: *mut c_void) {
    let dlg = MACRO_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg).unwrap_or(ptr::null_mut()));
    if let Some(path) = pick_file_path(dlg, "Nhập bảng gõ tắt (.txt)", false, "")
        && let Ok(raw) = fs::read_to_string(path) {
            let entries = Config::parse_macro_text(&raw);
            MACRO_DLG.with(|c| {
                if let Some(st) = c.borrow_mut().as_mut() {
                    for entry in entries {
                        if let Some(ex) = st
                            .cfg
                            .macros
                            .iter_mut()
                            .find(|m| m.shortcut.eq_ignore_ascii_case(&entry.shortcut))
                        {
                            ex.expansion = entry.expansion;
                            ex.enabled = entry.enabled;
                        } else {
                            st.cfg.macros.push(entry);
                        }
                    }
                }
            });
            reload_macro_list();
        }
}

unsafe extern "C" fn on_macro_export(_: *mut GtkWidget, _: *mut c_void) {
    let pair = MACRO_DLG.with(|c| c.borrow().as_ref().map(|w| (w.dlg, w.cfg.format_macro_text())));
    if let Some((dlg, txt)) = pair
        && let Some(path) = pick_file_path(dlg, "Xuất bảng gõ tắt (.txt)", true, "macros.txt") {
            let _ = fs::write(path, txt);
        }
}

unsafe extern "C" fn on_macro_del(_: *mut GtkWidget, _: *mut c_void) {
    MACRO_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            let row = unsafe { gtk_list_box_get_selected_row(st.list) };
            if !row.is_null() {
                let idx = unsafe { gtk_list_box_row_get_index(row) } as usize;
                if idx < st.cfg.macros.len() {
                    st.cfg.macros.remove(idx);
                }
            }
        }
    });
    reload_macro_list();
}

unsafe extern "C" fn on_macro_close(_: *mut GtkWidget, _: *mut c_void) {
    if let Some(dlg) = MACRO_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg)) {
        unsafe { gtk_widget_destroy(dlg) };
    }
}

pub fn show_app_rules_dialog(parent: *mut GtkWindow, cfg: &mut Config) -> bool {
    let dlg = create_modal_gtk(parent, "Danh sách ứng dụng thông minh", 484, 340);
    let root = make_vbox(8);
    let grid = make_grid(4, 8);
    let name_ed = make_entry("");
    let mode_cb = make_combo(&["Luôn Tiếng Anh (E)", "Luôn Tiếng Việt (V)", "Bỏ qua (Blacklist)"]);
    unsafe {
        gtk_widget_set_hexpand(name_ed, 1);
        gtk_grid_attach(grid, make_label("Tên ứng dụng / WM_CLASS (hỗ trợ *)", false), 0, 0, 1, 1);
        gtk_grid_attach(grid, make_label("Quy tắc", false), 1, 0, 1, 1);
        gtk_grid_attach(grid, name_ed, 0, 1, 1, 1);
        gtk_grid_attach(grid, mode_cb, 1, 1, 1, 1);
    }
    pack(root, grid, false, false, 0);

    let bar = make_hbox(8);
    let b_add = make_button("Lưu quy tắc", true);
    let b_del = make_button("Xóa mục chọn", false);
    let b_close = make_button("Đóng", false);
    pack(bar, b_add, false, false, 0);
    pack(bar, b_del, false, false, 0);
    pack(bar, make_label("", false), true, true, 0);
    pack(bar, b_close, false, false, 0);
    pack(root, bar, false, false, 0);

    let sw = unsafe { gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut()) };
    let list = unsafe { gtk_list_box_new() };
    unsafe {
        gtk_widget_set_hexpand(sw, 1);
        gtk_widget_set_vexpand(sw, 1);
        gtk_scrolled_window_set_policy(sw, GTK_POLICY_NEVER, GTK_POLICY_AUTOMATIC);
        gtk_list_box_set_selection_mode(list, GTK_SELECTION_SINGLE);
        gtk_container_add(sw, list);
    }
    pack(root, sw, true, true, 0);
    unsafe { gtk_container_add(dlg, root) };

    APP_DLG.with(|c| {
        *c.borrow_mut() = Some(AppWidgets { dlg, cfg: cfg.clone(), list, name_ed, mode_cb });
    });
    reload_app_list();
    connect_signal(b_add, "clicked", on_app_add as *const c_void, 0);
    connect_signal(b_del, "clicked", on_app_del as *const c_void, 0);
    connect_signal(b_close, "clicked", on_app_close as *const c_void, 0);
    connect_signal(list, "row-selected", on_app_sel as *const c_void, 0);
    run_modal_gtk(dlg);
    if let Some(st) = APP_DLG.with(|c| c.borrow_mut().take()) {
        *cfg = st.cfg;
        return true;
    }
    false
}

fn reload_app_list() {
    APP_DLG.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            clear_listbox(st.list);
            for r in &st.cfg.app_rules {
                let tag = match r.behavior {
                    AppBehavior::ForceEnglish => "Luôn Tiếng Anh (E)",
                    AppBehavior::ForceVietnamese => "Luôn Tiếng Việt (V)",
                    AppBehavior::Blacklist => "Bỏ qua (Blacklist)",
                    AppBehavior::Default => "Mặc định",
                };
                append_listbox_row(st.list, &format!("{}  —  {}", r.process_name, tag));
            }
        }
    });
}

unsafe extern "C" fn on_app_sel(_: *mut GtkWidget, _: *mut GtkWidget, _: *mut c_void) {
    APP_DLG.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            let row = unsafe { gtk_list_box_get_selected_row(st.list) };
            if !row.is_null() {
                let idx = unsafe { gtk_list_box_row_get_index(row) } as usize;
                if let Some(rule) = st.cfg.app_rules.get(idx) {
                    write_entry(st.name_ed, &rule.process_name);
                    let mode_idx = match rule.behavior {
                        AppBehavior::ForceVietnamese => 1,
                        AppBehavior::Blacklist => 2,
                        _ => 0,
                    };
                    set_combo(st.mode_cb, mode_idx);
                }
            }
        }
    });
}

unsafe extern "C" fn on_app_add(_: *mut GtkWidget, _: *mut c_void) {
    APP_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            let name = read_entry(st.name_ed).trim().to_string();
            if name.is_empty() {
                return;
            }
            let b = match get_combo(st.mode_cb) {
                1 => AppBehavior::ForceVietnamese,
                2 => AppBehavior::Blacklist,
                _ => AppBehavior::ForceEnglish,
            };
            st.cfg.set_app_rule(&name, b);
        }
    });
    reload_app_list();
}

unsafe extern "C" fn on_app_del(_: *mut GtkWidget, _: *mut c_void) {
    APP_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            let row = unsafe { gtk_list_box_get_selected_row(st.list) };
            if !row.is_null() {
                let idx = unsafe { gtk_list_box_row_get_index(row) } as usize;
                if idx < st.cfg.app_rules.len() {
                    let _: AppRule = st.cfg.app_rules.remove(idx);
                }
            }
        }
    });
    reload_app_list();
}

unsafe extern "C" fn on_app_close(_: *mut GtkWidget, _: *mut c_void) {
    if let Some(dlg) = APP_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg)) {
        unsafe { gtk_widget_destroy(dlg) };
    }
}

pub fn show_custom_keymap_dialog(parent: *mut GtkWindow, cfg: &mut Config) -> bool {
    let dlg = create_modal_gtk(parent, "Tự định nghĩa kiểu gõ", 420, 360);
    let root = make_vbox(8);
    let grid = make_grid(4, 12);
    let k = &cfg.custom_keymap;
    let fields = [
        ("Dấu sắc", k.acute),
        ("Dấu huyền", k.grave),
        ("Dấu hỏi", k.hook),
        ("Dấu ngã", k.tilde),
        ("Dấu nặng", k.dot),
        ("Xóa dấu", k.remove_tone),
        ("Chữ â", k.circumflex_a),
        ("Chữ ê", k.circumflex_e),
        ("Chữ ô", k.circumflex_o),
        ("Chữ ơ", k.horn_o),
        ("Chữ ư", k.horn_u),
        ("Chữ ă", k.breve_a),
        ("Chữ đ", k.d_stroke),
        ("Móc ơ nhanh", k.quick_o_horn_key),
        ("Móc ư nhanh", k.quick_u_horn_key),
    ];
    let mut edits = Vec::with_capacity(fields.len());
    for (idx, (lbl, ch)) in fields.iter().enumerate() {
        let col = (idx / 8) as i32 * 2;
        let row = (idx % 8) as i32;
        let ed = make_entry(&ch.to_string());
        unsafe {
            gtk_entry_set_max_length(ed, 1);
            gtk_widget_set_size_request(ed, 56, 28);
            gtk_grid_attach(grid, make_label(lbl, false), col, row, 1, 1);
            gtk_grid_attach(grid, ed, col + 1, row, 1, 1);
        }
        edits.push(ed);
    }
    pack(root, grid, true, true, 0);

    let bar = make_hbox(8);
    let b_def = make_button("Mặc định", false);
    let b_save = make_button("Lưu", true);
    let b_close = make_button("Đóng", false);
    pack(bar, b_def, false, false, 0);
    pack(bar, make_label("", false), true, true, 0);
    pack(bar, b_save, false, false, 0);
    pack(bar, b_close, false, false, 0);
    pack(root, bar, false, false, 0);
    unsafe { gtk_container_add(dlg, root) };

    CUST_DLG.with(|c| {
        *c.borrow_mut() = Some(CustomWidgets { dlg, cfg: cfg.clone(), edits, saved: false });
    });
    connect_signal(b_def, "clicked", on_cust_reset as *const c_void, 0);
    connect_signal(b_save, "clicked", on_cust_save as *const c_void, 0);
    connect_signal(b_close, "clicked", on_cust_close as *const c_void, 0);
    run_modal_gtk(dlg);
    if let Some(st) = CUST_DLG.with(|c| c.borrow_mut().take())
        && st.saved {
            *cfg = st.cfg;
            return true;
        }
    false
}

unsafe extern "C" fn on_cust_reset(_: *mut GtkWidget, _: *mut c_void) {
    CUST_DLG.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            let d = CustomKeyMap::default();
            let vals = [
                d.acute, d.grave, d.hook, d.tilde, d.dot, d.remove_tone, d.circumflex_a,
                d.circumflex_e, d.circumflex_o, d.horn_o, d.horn_u, d.breve_a, d.d_stroke,
                d.quick_o_horn_key, d.quick_u_horn_key,
            ];
            for (ed, ch) in st.edits.iter().zip(vals.iter()) {
                write_entry(*ed, &ch.to_string());
            }
        }
    });
}

unsafe extern "C" fn on_cust_save(_: *mut GtkWidget, _: *mut c_void) {
    let dlg = CUST_DLG.with(|c| {
        let mut b = c.borrow_mut();
        let Some(st) = b.as_mut() else { return ptr::null_mut() };
        let g = |i: usize, fb: char| read_entry(st.edits[i]).trim().chars().next().unwrap_or(fb);
        let d = CustomKeyMap::default();
        st.cfg.custom_keymap = CustomKeyMap {
            acute: g(0, d.acute),
            grave: g(1, d.grave),
            hook: g(2, d.hook),
            tilde: g(3, d.tilde),
            dot: g(4, d.dot),
            remove_tone: g(5, d.remove_tone),
            circumflex_a: g(6, d.circumflex_a),
            circumflex_e: g(7, d.circumflex_e),
            circumflex_o: g(8, d.circumflex_o),
            horn_o: g(9, d.horn_o),
            horn_u: g(10, d.horn_u),
            breve_a: g(11, d.breve_a),
            d_stroke: g(12, d.d_stroke),
            quick_u_horn: st.cfg.custom_keymap.quick_u_horn,
            quick_o_horn_key: g(13, d.quick_o_horn_key),
            quick_u_horn_key: g(14, d.quick_u_horn_key),
        };
        st.saved = true;
        st.dlg
    });
    if !dlg.is_null() {
        unsafe { gtk_widget_destroy(dlg) };
    }
}

unsafe extern "C" fn on_cust_close(_: *mut GtkWidget, _: *mut c_void) {
    if let Some(dlg) = CUST_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg)) {
        unsafe { gtk_widget_destroy(dlg) };
    }
}
