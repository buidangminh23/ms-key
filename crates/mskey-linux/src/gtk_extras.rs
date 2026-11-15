use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::os::raw::c_int;
use std::path::Path;
use std::ptr;

use mskey_core::{Config, Engine, EngineAction};

use crate::config_store::{export_config_json, import_config_json};
use crate::gio_ffi::cstr;
use crate::gtk_dialogs::pick_file_path;
use crate::gtk_ffi::{
    GTK_POLICY_AUTOMATIC, GTK_POLICY_NEVER, GdkEventKey, GtkTextIter, GtkWidget, GtkWindow,
    connect_signal, gtk_container_add, gtk_scrolled_window_new, gtk_scrolled_window_set_policy,
    gtk_spin_button_get_value_as_int, gtk_spin_button_new_with_range, gtk_spin_button_set_value,
    gtk_text_buffer_delete, gtk_text_buffer_get_insert, gtk_text_buffer_get_iter_at_mark,
    gtk_text_buffer_get_iter_at_offset, gtk_text_buffer_insert_at_cursor, gtk_text_iter_get_offset,
    gtk_text_view_get_buffer, gtk_text_view_new, gtk_widget_destroy, gtk_widget_set_hexpand,
    gtk_widget_set_vexpand,
};
use crate::gtk_helpers::{
    create_modal_gtk, get_toggle, make_button, make_check, make_frame, make_hbox, make_label,
    make_vbox, pack, read_textview, run_modal_gtk, set_toggle, write_textview,
};
use crate::keyval::{
    IBUS_CONTROL_MASK, IBUS_MOD1_MASK, IBUS_MOD4_MASK, KEY_BACKSPACE, KEY_ESCAPE,
    is_modifier_keyval, is_navigation_keyval, keyval_to_char,
};

struct ExtrasWidgets {
    dlg: *mut GtkWidget,
    cfg: Config,
    engine: Engine,
    chk_q_telex: *mut GtkWidget,
    chk_q_start: *mut GtkWidget,
    chk_q_end: *mut GtkWidget,
    chk_osd: *mut GtkWidget,
    chk_shift_ins: *mut GtkWidget,
    spin_delay: *mut GtkWidget,
    tv: *mut GtkWidget,
    syncing: bool,
    saved: bool,
}

struct DictWidgets {
    dlg: *mut GtkWidget,
    tv: *mut GtkWidget,
    words: Vec<String>,
    saved: bool,
}

thread_local! {
    static EXTRAS_DLG: RefCell<Option<ExtrasWidgets>> = const { RefCell::new(None) };
    static DICT_DLG: RefCell<Option<DictWidgets>> = const { RefCell::new(None) };
}

pub fn show_extras_sandbox_dialog(parent: *mut GtkWindow, cfg: &mut Config) -> bool {
    let dlg = create_modal_gtk(parent, "Mở rộng, Sao lưu & Gõ thử", 520, 460);
    let root = make_vbox(8);
    let (ef, eb) = make_frame("Tùy chọn mở rộng");
    let chk_q_telex = make_check("Gõ tắt Telex (cc->ch, gg->gi, nn->ng, pp->ph, qq->qu, tt->th)");
    let chk_q_start = make_check("Gõ tắt phụ âm đầu (f->ph, j->gi, w->qu)");
    let chk_q_end = make_check("Gõ tắt phụ âm cuối (g->ng, h->nh, k->ch)");
    let chk_osd = make_check("Hiện OSD nổi khi chuyển V/E");
    let chk_shift_ins = make_check("Dùng Shift+Insert khi dán clipboard");
    let drow = make_hbox(8);
    let spin_delay = unsafe { gtk_spin_button_new_with_range(0.0, 25.0, 1.0) };
    pack(drow, make_label("Độ trễ gửi phím (ms):", false), false, false, 0);
    pack(drow, spin_delay, false, false, 0);
    for &w in &[chk_q_telex, chk_q_start, chk_q_end, chk_osd, chk_shift_ins, drow] {
        pack(eb, w, false, false, 0);
    }
    pack(root, ef, false, false, 0);

    let jrow = make_hbox(8);
    let b_exp = make_button("Xuất JSON...", false);
    let b_imp = make_button("Nhập JSON...", false);
    pack(jrow, b_exp, true, true, 0);
    pack(jrow, b_imp, true, true, 0);
    pack(root, jrow, false, false, 0);

    let (sf, sb) = make_frame("Vùng gõ thử trực tiếp (không cần kích hoạt IBus)");
    let sw = unsafe { gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut()) };
    let tv = unsafe { gtk_text_view_new() };
    write_textview(tv, "");
    unsafe {
        gtk_widget_set_hexpand(sw, 1);
        gtk_widget_set_vexpand(sw, 1);
        gtk_scrolled_window_set_policy(sw, GTK_POLICY_NEVER, GTK_POLICY_AUTOMATIC);
        gtk_container_add(sw, tv);
    }
    pack(sb, sw, true, true, 0);
    pack(root, sf, true, true, 0);

    let bar = make_hbox(8);
    let b_clr = make_button("Xóa ô thử", false);
    let b_save = make_button("Lưu", true);
    let b_cancel = make_button("Hủy", false);
    pack(bar, b_clr, false, false, 0);
    pack(bar, make_label("", false), true, true, 0);
    pack(bar, b_save, false, false, 0);
    pack(bar, b_cancel, false, false, 0);
    pack(root, bar, false, false, 0);
    unsafe { gtk_container_add(dlg, root) };

    EXTRAS_DLG.with(|c| {
        *c.borrow_mut() = Some(ExtrasWidgets {
            dlg,
            cfg: cfg.clone(),
            engine: Engine::new(cfg.clone()),
            chk_q_telex,
            chk_q_start,
            chk_q_end,
            chk_osd,
            chk_shift_ins,
            spin_delay,
            tv,
            syncing: false,
            saved: false,
        });
    });
    sync_extras_widgets();
    for &w in &[chk_q_telex, chk_q_start, chk_q_end, chk_osd, chk_shift_ins] {
        connect_signal(w, "toggled", on_extras_changed as *const c_void, 0);
    }
    connect_signal(spin_delay, "value-changed", on_extras_changed as *const c_void, 0);
    connect_signal(b_exp, "clicked", on_extras_export as *const c_void, 0);
    connect_signal(b_imp, "clicked", on_extras_import as *const c_void, 0);
    connect_signal(b_clr, "clicked", on_extras_clear as *const c_void, 0);
    connect_signal(b_save, "clicked", on_extras_save as *const c_void, 0);
    connect_signal(b_cancel, "clicked", on_extras_cancel as *const c_void, 0);
    connect_signal(tv, "key-press-event", on_sandbox_key as *const c_void, 0);
    run_modal_gtk(dlg);

    if let Some(st) = EXTRAS_DLG.with(|c| c.borrow_mut().take()) {
        if st.saved {
            *cfg = st.cfg;
            return true;
        }
    }
    false
}

fn sync_extras_widgets() {
    EXTRAS_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            st.syncing = true;
            set_toggle(st.chk_q_telex, st.cfg.quick_telex);
            set_toggle(st.chk_q_start, st.cfg.quick_start_consonant);
            set_toggle(st.chk_q_end, st.cfg.quick_end_consonant);
            set_toggle(st.chk_osd, st.cfg.show_switch_osd);
            set_toggle(st.chk_shift_ins, st.cfg.paste_via_shift_insert);
            unsafe { gtk_spin_button_set_value(st.spin_delay, st.cfg.send_delay_ms as f64) };
            st.engine.update_config(st.cfg.clone());
            st.syncing = false;
        }
    });
}

fn read_extras_widgets() {
    EXTRAS_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            if st.syncing {
                return;
            }
            st.cfg.quick_telex = get_toggle(st.chk_q_telex);
            st.cfg.quick_start_consonant = get_toggle(st.chk_q_start);
            st.cfg.quick_end_consonant = get_toggle(st.chk_q_end);
            st.cfg.show_switch_osd = get_toggle(st.chk_osd);
            st.cfg.paste_via_shift_insert = get_toggle(st.chk_shift_ins);
            let d = unsafe { gtk_spin_button_get_value_as_int(st.spin_delay) };
            st.cfg.send_delay_ms = d.clamp(0, 25) as u8;
            st.engine.update_config(st.cfg.clone());
        }
    });
}

unsafe extern "C" fn on_extras_changed(_: *mut GtkWidget, _: *mut c_void) {
    read_extras_widgets();
}

unsafe extern "C" fn on_extras_export(_: *mut GtkWidget, _: *mut c_void) {
    read_extras_widgets();
    let pair = EXTRAS_DLG.with(|c| c.borrow().as_ref().map(|w| (w.dlg, w.cfg.clone())));
    if let Some((dlg, cfg)) = pair {
        if let Some(path) = pick_file_path(dlg, "Xuất cấu hình JSON", true, "mskey-config.json") {
            let _ = export_config_json(Path::new(&path), &cfg);
        }
    }
}

unsafe extern "C" fn on_extras_import(_: *mut GtkWidget, _: *mut c_void) {
    let dlg = EXTRAS_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg).unwrap_or(ptr::null_mut()));
    if let Some(path) = pick_file_path(dlg, "Nhập cấu hình JSON", false, "") {
        if let Ok(imported) = import_config_json(Path::new(&path)) {
            EXTRAS_DLG.with(|c| {
                if let Some(st) = c.borrow_mut().as_mut() {
                    st.cfg = imported;
                }
            });
            sync_extras_widgets();
        }
    }
}

unsafe extern "C" fn on_extras_clear(_: *mut GtkWidget, _: *mut c_void) {
    EXTRAS_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            st.engine.reset_all();
            write_textview(st.tv, "");
        }
    });
}

unsafe extern "C" fn on_extras_save(_: *mut GtkWidget, _: *mut c_void) {
    read_extras_widgets();
    let dlg = EXTRAS_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            st.saved = true;
            return st.dlg;
        }
        ptr::null_mut()
    });
    if !dlg.is_null() {
        unsafe { gtk_widget_destroy(dlg) };
    }
}

unsafe extern "C" fn on_extras_cancel(_: *mut GtkWidget, _: *mut c_void) {
    if let Some(dlg) = EXTRAS_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg)) {
        unsafe { gtk_widget_destroy(dlg) };
    }
}

unsafe extern "C" fn on_sandbox_key(
    tv: *mut GtkWidget,
    ev: *mut GdkEventKey,
    _: *mut c_void,
) -> c_int {
    let e = unsafe { &*ev };
    if is_modifier_keyval(e.keyval) {
        return 0;
    }
    if (e.state & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_MOD4_MASK)) != 0 {
        EXTRAS_DLG.with(|c| {
            if let Some(st) = c.borrow_mut().as_mut() {
                st.engine.reset_word();
            }
        });
        return 0;
    }
    let action = EXTRAS_DLG.with(|c| {
        let mut b = c.borrow_mut();
        let st = b.as_mut()?;
        if e.keyval == KEY_ESCAPE {
            return Some(st.engine.process_escape());
        }
        if e.keyval == KEY_BACKSPACE {
            return Some(st.engine.process_backspace());
        }
        if is_navigation_keyval(e.keyval) {
            st.engine.reset_word();
            return None;
        }
        let ch = keyval_to_char(e.keyval)?;
        Some(st.engine.process_key(ch))
    });
    let Some(EngineAction::Replace { backspaces, text }) = action else {
        return 0;
    };
    unsafe {
        let buf = gtk_text_view_get_buffer(tv);
        let mark = gtk_text_buffer_get_insert(buf);
        let mut end_it = GtkTextIter { dummy: [0; 80] };
        gtk_text_buffer_get_iter_at_mark(buf, &mut end_it, mark);
        if backspaces > 0 {
            let off = gtk_text_iter_get_offset(&end_it);
            let start_off = (off - backspaces as c_int).max(0);
            let mut start_it = GtkTextIter { dummy: [0; 80] };
            gtk_text_buffer_get_iter_at_offset(buf, &mut start_it, start_off);
            gtk_text_buffer_delete(buf, &mut start_it, &mut end_it);
        }
        if !text.is_empty() {
            let ct = cstr(&text);
            gtk_text_buffer_insert_at_cursor(buf, ct.as_ptr(), -1);
        }
    }
    1
}

pub fn show_user_dict_dialog(parent: *mut GtkWindow, cfg: &mut Config) -> bool {
    let dlg = create_modal_gtk(parent, "Từ điển cá nhân (1 từ / 1 dòng)", 420, 340);
    let root = make_vbox(8);
    pack(root, make_label("Các từ bên dưới luôn được giữ nguyên (không kiểm tra/khôi phục sai chính tả):", false), false, false, 0);
    let sw = unsafe { gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut()) };
    let tv = unsafe { gtk_text_view_new() };
    write_textview(tv, &cfg.user_dict_words.join("\n"));
    unsafe {
        gtk_widget_set_hexpand(sw, 1);
        gtk_widget_set_vexpand(sw, 1);
        gtk_scrolled_window_set_policy(sw, GTK_POLICY_NEVER, GTK_POLICY_AUTOMATIC);
        gtk_container_add(sw, tv);
    }
    pack(root, sw, true, true, 0);

    let bar = make_hbox(8);
    let b_save = make_button("Lưu", true);
    let b_cancel = make_button("Hủy", false);
    pack(bar, make_label("", false), true, true, 0);
    pack(bar, b_save, false, false, 0);
    pack(bar, b_cancel, false, false, 0);
    pack(root, bar, false, false, 0);
    unsafe { gtk_container_add(dlg, root) };

    DICT_DLG.with(|c| {
        *c.borrow_mut() = Some(DictWidgets {
            dlg,
            tv,
            words: cfg.user_dict_words.clone(),
            saved: false,
        });
    });
    connect_signal(b_save, "clicked", on_dict_save as *const c_void, 0);
    connect_signal(b_cancel, "clicked", on_dict_cancel as *const c_void, 0);
    run_modal_gtk(dlg);

    if let Some(st) = DICT_DLG.with(|c| c.borrow_mut().take()) {
        if st.saved {
            cfg.user_dict_words = st.words;
            return true;
        }
    }
    false
}

unsafe extern "C" fn on_dict_save(_: *mut GtkWidget, _: *mut c_void) {
    let dlg = DICT_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            let raw = read_textview(st.tv);
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for line in raw.lines() {
                let w = line.trim();
                if !w.is_empty() && seen.insert(w.to_lowercase()) {
                    out.push(w.to_string());
                }
            }
            st.words = out;
            st.saved = true;
            return st.dlg;
        }
        ptr::null_mut()
    });
    if !dlg.is_null() {
        unsafe { gtk_widget_destroy(dlg) };
    }
}

unsafe extern "C" fn on_dict_cancel(_: *mut GtkWidget, _: *mut c_void) {
    if let Some(dlg) = DICT_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg)) {
        unsafe { gtk_widget_destroy(dlg) };
    }
}
