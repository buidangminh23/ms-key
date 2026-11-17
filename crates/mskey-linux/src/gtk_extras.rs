use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::fs;
use std::os::raw::c_int;
use std::path::Path;
use std::ptr;

use mskey_core::{Config, Engine, EngineAction, InputMethod};

use crate::config_store::{
    export_config_json, import_config_json, load_last_esc_word, save_last_esc_word,
};
use crate::gio_ffi::cstr;
use crate::gtk_dialogs::pick_file_path;
use crate::gtk_ffi::{
    GTK_POLICY_AUTOMATIC, GTK_POLICY_NEVER, GdkEventKey, GtkTextIter, GtkWidget, GtkWindow,
    connect_signal, g_object_set, gtk_container_add, gtk_frame_get_label_widget,
    gtk_label_set_mnemonic_widget, gtk_scrolled_window_new,
    gtk_scrolled_window_set_policy,
    gtk_spin_button_get_value_as_int, gtk_spin_button_new_with_range, gtk_spin_button_set_value,
    gtk_text_buffer_delete, gtk_text_buffer_get_insert, gtk_text_buffer_get_iter_at_mark,
    gtk_text_buffer_get_iter_at_offset, gtk_text_buffer_insert_at_cursor, gtk_text_iter_get_offset,
    gtk_text_view_get_buffer, gtk_text_view_new, gtk_widget_destroy, gtk_widget_grab_focus,
    gtk_widget_set_hexpand, gtk_widget_set_size_request, gtk_widget_set_vexpand,
};
use crate::gtk_helpers::{
    create_modal_gtk, get_input_method, get_toggle, make_button, make_check, make_entry_placeholder,
    make_frame, make_hbox, make_input_method_combo, make_label, make_vbox, pack, read_entry,
    read_textview, run_modal_gtk, set_input_method, set_label_text, set_toggle, write_textview,
};
use crate::keyval::{
    IBUS_CONTROL_MASK, IBUS_MOD1_MASK, IBUS_MOD4_MASK, KEY_BACKSPACE, KEY_ESCAPE,
    is_modifier_keyval, is_navigation_keyval, keyval_to_char,
};
use crate::gtk_theme::{add_css_class, set_language_button};

struct ExtrasWidgets {
    dlg: *mut GtkWidget,
    cfg: Config,
    chk_q_telex: *mut GtkWidget,
    chk_q_start: *mut GtkWidget,
    chk_q_end: *mut GtkWidget,
    chk_osd: *mut GtkWidget,
    chk_shift_ins: *mut GtkWidget,
    spin_delay: *mut GtkWidget,
    saved: bool,
}

struct SandboxWidgets {
    engine: Engine,
    editor: *mut GtkWidget,
    mode_button: *mut GtkWidget,
    preview_hint: *mut GtkWidget,
}

struct DictWidgets {
    dlg: *mut GtkWidget,
    tv: *mut GtkWidget,
    search_ed: *mut GtkWidget,
    lbl_status: *mut GtkWidget,
    all_words: Vec<String>,
    displayed_words: Vec<String>,
    saved: bool,
}

thread_local! {
    static EXTRAS_DLG: RefCell<Option<ExtrasWidgets>> = const { RefCell::new(None) };
    static SANDBOX_DLG: RefCell<Option<SandboxWidgets>> = const { RefCell::new(None) };
    static DICT_DLG: RefCell<Option<DictWidgets>> = const { RefCell::new(None) };
    static LAST_SANDBOX_ESC: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn show_sandbox_dialog(parent: *mut GtkWindow, cfg: Config) {
    let dialog = create_modal_gtk(parent, "Gõ thử · MS Key", 512, 376);
    let root = make_vbox(16);
    let controls = make_hbox(8);
    let method_label = make_label("Kiểu gõ", false);
    let method = make_input_method_combo();
    let mode_button = make_button("Tiếng Việt (V)", false);
    set_input_method(method, cfg.input_method);
    add_css_class(mode_button, "language-toggle");
    unsafe { gtk_label_set_mnemonic_widget(method_label, method) };
    pack(controls, method_label, false, false, 0);
    pack(controls, method, true, true, 0);
    pack(controls, mode_button, false, false, 0);
    pack(root, controls, false, false, 0);

    let (editor_frame, editor_panel) = make_frame("Vùng gõ thử");
    let sw = unsafe { gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut()) };
    let editor = unsafe { gtk_text_view_new() };
    let preview_hint = make_label("", true);
    write_textview(editor, "");
    unsafe {
        let im_property = cstr("im-module");
        let im_simple = cstr("gtk-im-context-simple");
        g_object_set(editor, im_property.as_ptr(), im_simple.as_ptr(), ptr::null::<u8>());
        gtk_label_set_mnemonic_widget(gtk_frame_get_label_widget(editor_frame), editor);
        gtk_widget_set_hexpand(sw, 1);
        gtk_widget_set_vexpand(sw, 1);
        gtk_widget_set_size_request(sw, -1, 112);
        gtk_scrolled_window_set_policy(sw, GTK_POLICY_NEVER, GTK_POLICY_AUTOMATIC);
        gtk_container_add(sw, editor);
    }
    pack(editor_panel, preview_hint, false, false, 0);
    pack(editor_panel, sw, true, true, 0);
    pack(root, editor_frame, true, true, 0);

    let footer = make_hbox(8);
    let clear = make_button("Xóa nội dung", false);
    let close = make_button("Đóng", true);
    pack(footer, clear, false, false, 0);
    pack(footer, make_label("", false), true, true, 0);
    pack(footer, close, false, false, 0);
    pack(root, footer, false, false, 0);
    unsafe { gtk_container_add(dialog, root) };

    let state = SandboxWidgets { engine: Engine::new(cfg), editor, mode_button, preview_hint };
    sync_sandbox_controls(&state);
    SANDBOX_DLG.with(|cell| *cell.borrow_mut() = Some(state));
    connect_signal(method, "changed", on_sandbox_method as *const c_void, 0);
    connect_signal(mode_button, "clicked", on_sandbox_mode as *const c_void, 0);
    connect_signal(clear, "clicked", on_sandbox_clear as *const c_void, 0);
    connect_signal(close, "clicked", on_sandbox_close as *const c_void, dialog as usize);
    connect_signal(editor, "key-press-event", on_sandbox_key as *const c_void, 0);
    connect_signal(editor, "button-press-event", on_sandbox_reset_word as *const c_void, 0);
    connect_signal(editor, "focus-out-event", on_sandbox_reset_word as *const c_void, 0);
    unsafe { gtk_widget_grab_focus(editor) };
    run_modal_gtk(dialog);
    SANDBOX_DLG.with(|cell| *cell.borrow_mut() = None);
}

fn sync_sandbox_controls(state: &SandboxWidgets) {
    let cfg = state.engine.config();
    let hint = if cfg.vietnamese_enabled {
        match cfg.input_method {
            InputMethod::Telex | InputMethod::SimpleTelex => "Thử gõ chaof → chào",
            InputMethod::Vni => "Thử gõ chao2 → chào",
            InputMethod::Viqr => "Thử gõ chao` → chào",
            InputMethod::Custom => "Thử gõ với bộ phím tùy chỉnh.",
        }
    } else {
        "Bật Tiếng Việt để thử đặt dấu."
    };
    set_language_button(state.mode_button, cfg.vietnamese_enabled);
    set_label_text(state.preview_hint, hint);
}

unsafe extern "C" fn on_sandbox_method(combo: *mut GtkWidget, _: *mut c_void) {
    SANDBOX_DLG.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            let mut cfg = state.engine.config().clone();
            cfg.input_method = get_input_method(combo);
            state.engine.update_config(cfg);
            state.engine.reset_all();
            sync_sandbox_controls(state);
        }
    });
}

unsafe extern "C" fn on_sandbox_mode(_: *mut GtkWidget, _: *mut c_void) {
    SANDBOX_DLG.with(|cell| {
        if let Some(state) = cell.borrow_mut().as_mut() {
            let next = !state.engine.config().vietnamese_enabled;
            state.engine.set_vietnamese_enabled(next);
            state.engine.reset_all();
            sync_sandbox_controls(state);
        }
    });
}

pub fn show_extras_dialog(parent: *mut GtkWindow, cfg: &mut Config) -> bool {
    let dlg = create_modal_gtk(parent, "Tùy chọn mở rộng", 520, 384);
    let root = make_vbox(16);
    let (ef, eb) = make_frame("Tùy chọn mở rộng");
    let chk_q_telex = make_check("Gõ nhanh Telex (cc → ch, gg → gi, nn → ng, pp → ph, qq → qu, tt → th)");
    let chk_q_start = make_check("Gõ nhanh phụ âm đầu (f → ph, j → gi, w → qu)");
    let chk_q_end = make_check("Gõ nhanh phụ âm cuối (g → ng, h → nh, k → ch)");
    let chk_osd = make_check("Hiện OSD nổi khi chuyển V/E");
    let chk_shift_ins = make_check("Dùng Shift+Insert khi dán clipboard");
    let drow = make_hbox(8);
    let spin_delay = unsafe { gtk_spin_button_new_with_range(0.0, 25.0, 1.0) };
    let delay_label = make_label("Độ trễ gửi phím (ms)", false);
    unsafe { gtk_label_set_mnemonic_widget(delay_label, spin_delay) };
    pack(drow, delay_label, false, false, 0);
    pack(drow, spin_delay, false, false, 0);
    for &w in &[chk_q_telex, chk_q_start, chk_q_end, chk_osd, chk_shift_ins, drow] {
        pack(eb, w, false, false, 0);
    }
    pack(root, ef, false, false, 0);

    let backup = make_hbox(8);
    let b_exp = make_button("Xuất bản sao JSON...", false);
    let b_imp = make_button("Nhập bản sao JSON...", false);
    pack(backup, b_exp, true, true, 0);
    pack(backup, b_imp, true, true, 0);
    pack(root, backup, false, false, 0);

    let bar = make_hbox(8);
    let b_save = make_button("Lưu", true);
    let b_cancel = make_button("Hủy", false);
    pack(bar, make_label("", false), true, true, 0);
    pack(bar, b_cancel, false, false, 0);
    pack(bar, b_save, false, false, 0);
    pack(root, bar, false, false, 0);
    unsafe { gtk_container_add(dlg, root) };

    EXTRAS_DLG.with(|c| {
        *c.borrow_mut() = Some(ExtrasWidgets {
            dlg,
            cfg: cfg.clone(),
            chk_q_telex,
            chk_q_start,
            chk_q_end,
            chk_osd,
            chk_shift_ins,
            spin_delay,
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
    connect_signal(b_save, "clicked", on_extras_save as *const c_void, 0);
    connect_signal(b_cancel, "clicked", on_extras_cancel as *const c_void, 0);
    run_modal_gtk(dlg);

    if let Some(st) = EXTRAS_DLG.with(|c| c.borrow_mut().take())
        && st.saved {
            *cfg = st.cfg;
            return true;
        }
    false
}

fn sync_extras_widgets() {
    EXTRAS_DLG.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            set_toggle(st.chk_q_telex, st.cfg.quick_telex);
            set_toggle(st.chk_q_start, st.cfg.quick_start_consonant);
            set_toggle(st.chk_q_end, st.cfg.quick_end_consonant);
            set_toggle(st.chk_osd, st.cfg.show_switch_osd);
            set_toggle(st.chk_shift_ins, st.cfg.paste_via_shift_insert);
            unsafe { gtk_spin_button_set_value(st.spin_delay, st.cfg.send_delay_ms as f64) };
        }
    });
}

fn read_extras_widgets() {
    EXTRAS_DLG.with(|c| {
        if let Ok(mut state) = c.try_borrow_mut()
            && let Some(st) = state.as_mut() {
            st.cfg.quick_telex = get_toggle(st.chk_q_telex);
            st.cfg.quick_start_consonant = get_toggle(st.chk_q_start);
            st.cfg.quick_end_consonant = get_toggle(st.chk_q_end);
            st.cfg.show_switch_osd = get_toggle(st.chk_osd);
            st.cfg.paste_via_shift_insert = get_toggle(st.chk_shift_ins);
            let d = unsafe { gtk_spin_button_get_value_as_int(st.spin_delay) };
            st.cfg.send_delay_ms = d.clamp(0, 25) as u8;
        }
    });
}

unsafe extern "C" fn on_extras_changed(_: *mut GtkWidget, _: *mut c_void) {
    read_extras_widgets();
}

unsafe extern "C" fn on_extras_export(_: *mut GtkWidget, _: *mut c_void) {
    read_extras_widgets();
    let pair = EXTRAS_DLG.with(|c| c.borrow().as_ref().map(|w| (w.dlg, w.cfg.clone())));
    if let Some((dlg, cfg)) = pair
        && let Some(path) = pick_file_path(dlg, "Xuất cấu hình JSON", true, "mskey-config.json") {
            let _ = export_config_json(Path::new(&path), &cfg);
        }
}

unsafe extern "C" fn on_extras_import(_: *mut GtkWidget, _: *mut c_void) {
    let dlg = EXTRAS_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg).unwrap_or(ptr::null_mut()));
    if let Some(path) = pick_file_path(dlg, "Nhập cấu hình JSON", false, "")
        && let Ok(imported) = import_config_json(Path::new(&path)) {
            EXTRAS_DLG.with(|c| {
                if let Some(st) = c.borrow_mut().as_mut() {
                    st.cfg = imported;
                }
            });
            sync_extras_widgets();
        }
}

unsafe extern "C" fn on_sandbox_clear(_: *mut GtkWidget, _: *mut c_void) {
    SANDBOX_DLG.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.engine.reset_all();
            write_textview(st.editor, "");
            unsafe { gtk_widget_grab_focus(st.editor) };
        }
    });
}

unsafe extern "C" fn on_sandbox_close(_: *mut GtkWidget, dialog: *mut c_void) {
    unsafe { gtk_widget_destroy(dialog) };
}

unsafe extern "C" fn on_sandbox_reset_word(_: *mut GtkWidget, _: *mut c_void, _: *mut c_void) -> c_int {
    SANDBOX_DLG.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.engine.reset_word();
        }
    });
    0
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
        SANDBOX_DLG.with(|c| {
            if let Some(st) = c.borrow_mut().as_mut() {
                st.engine.reset_word();
            }
        });
        return 0;
    }
    let action = SANDBOX_DLG.with(|c| {
        let mut b = c.borrow_mut();
        let st = b.as_mut()?;
        if e.keyval == KEY_ESCAPE {
            let act = st.engine.process_escape();
            let word = st.engine.last_word_for_dict();
            save_last_esc_word(word.as_deref());
            LAST_SANDBOX_ESC.with(|lc| *lc.borrow_mut() = word);
            return Some(act);
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

fn dedup_words(words: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for raw in words {
        let w = raw.trim();
        if !w.is_empty() && seen.insert(w.to_lowercase()) {
            out.push(w.to_string());
        }
    }
    out
}

fn sync_visible_into_all(st: &mut DictWidgets) {
    let edited_visible: Vec<String> = read_textview(st.tv)
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let was_displayed: HashSet<String> =
        st.displayed_words.iter().map(|w| w.to_lowercase()).collect();
    let mut merged = Vec::new();
    let mut seen = HashSet::new();
    for w in &st.all_words {
        let low = w.to_lowercase();
        if !was_displayed.contains(&low) && seen.insert(low) {
            merged.push(w.clone());
        }
    }
    for w in edited_visible {
        if seen.insert(w.to_lowercase()) {
            merged.push(w);
        }
    }
    st.all_words = merged;
}

fn apply_dict_filter(st: &mut DictWidgets) {
    let q = read_entry(st.search_ed).trim().to_lowercase();
    let filtered: Vec<String> = st
        .all_words
        .iter()
        .filter(|w| q.is_empty() || w.to_lowercase().contains(&q))
        .cloned()
        .collect();
    st.displayed_words = filtered.clone();
    write_textview(st.tv, &filtered.join("\n"));
}

pub fn show_user_dict_dialog(parent: *mut GtkWindow, cfg: &mut Config) -> bool {
    let dlg = create_modal_gtk(parent, "Từ điển cá nhân (1 từ / 1 dòng)", 520, 400);
    let root = make_vbox(8);
    pack(
        root,
        make_label("Các từ bên dưới luôn được giữ nguyên (không kiểm tra/khôi phục sai chính tả):", false),
        false,
        false,
        0,
    );

    let search_bar = make_hbox(6);
    let search_ed = make_entry_placeholder("", "Tìm từ trong từ điển...");
    let b_add_esc = make_button("Thêm từ Esc gần nhất", false);
    unsafe { gtk_widget_set_hexpand(search_ed, 1) };
    pack(search_bar, make_label("Tìm kiếm:", false), false, false, 0);
    pack(search_bar, search_ed, true, true, 0);
    pack(search_bar, b_add_esc, false, false, 0);
    pack(root, search_bar, false, false, 0);

    let sw = unsafe { gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut()) };
    let tv = unsafe { gtk_text_view_new() };
    let initial_words = dedup_words(cfg.user_dict_words.clone());
    write_textview(tv, &initial_words.join("\n"));
    unsafe {
        gtk_widget_set_hexpand(sw, 1);
        gtk_widget_set_vexpand(sw, 1);
        gtk_scrolled_window_set_policy(sw, GTK_POLICY_NEVER, GTK_POLICY_AUTOMATIC);
        gtk_container_add(sw, tv);
    }
    pack(root, sw, true, true, 0);

    let lbl_status = make_label(&format!("Tổng: {} từ", initial_words.len()), true);
    pack(root, lbl_status, false, false, 0);

    let bar = make_hbox(8);
    let b_imp = make_button("Nhập .txt...", false);
    let b_exp = make_button("Xuất .txt...", false);
    let b_save = make_button("Lưu", true);
    let b_cancel = make_button("Hủy", false);
    pack(bar, b_imp, false, false, 0);
    pack(bar, b_exp, false, false, 0);
    pack(bar, make_label("", false), true, true, 0);
    pack(bar, b_save, false, false, 0);
    pack(bar, b_cancel, false, false, 0);
    pack(root, bar, false, false, 0);
    unsafe { gtk_container_add(dlg, root) };

    DICT_DLG.with(|c| {
        *c.borrow_mut() = Some(DictWidgets {
            dlg,
            tv,
            search_ed,
            lbl_status,
            all_words: initial_words.clone(),
            displayed_words: initial_words,
            saved: false,
        });
    });
    connect_signal(search_ed, "changed", on_dict_search as *const c_void, 0);
    connect_signal(b_add_esc, "clicked", on_dict_add_esc as *const c_void, 0);
    connect_signal(b_imp, "clicked", on_dict_import as *const c_void, 0);
    connect_signal(b_exp, "clicked", on_dict_export as *const c_void, 0);
    connect_signal(b_save, "clicked", on_dict_save as *const c_void, 0);
    connect_signal(b_cancel, "clicked", on_dict_cancel as *const c_void, 0);
    run_modal_gtk(dlg);

    if let Some(st) = DICT_DLG.with(|c| c.borrow_mut().take())
        && st.saved
    {
        cfg.user_dict_words = st.all_words;
        return true;
    }
    false
}

unsafe extern "C" fn on_dict_search(_: *mut GtkWidget, _: *mut c_void) {
    DICT_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            sync_visible_into_all(st);
            apply_dict_filter(st);
            set_label_text(
                st.lbl_status,
                &format!("Hiển thị {}/{} từ", st.displayed_words.len(), st.all_words.len()),
            );
        }
    });
}

unsafe extern "C" fn on_dict_add_esc(_: *mut GtkWidget, _: *mut c_void) {
    let candidate = load_last_esc_word()
        .or_else(|| LAST_SANDBOX_ESC.with(|lc| lc.borrow().clone()));
    DICT_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            sync_visible_into_all(st);
            let Some(word) = candidate.map(|w| w.trim().to_string()).filter(|w| !w.is_empty()) else {
                set_label_text(st.lbl_status, "Chưa có từ vừa hoàn tác bằng Esc gần đây.");
                return;
            };
            if st.all_words.iter().any(|w| w.eq_ignore_ascii_case(&word)) {
                set_label_text(st.lbl_status, &format!("Từ \"{word}\" đã có trong từ điển."));
            } else {
                st.all_words.push(word.clone());
                apply_dict_filter(st);
                set_label_text(st.lbl_status, &format!("Đã thêm từ Esc gần nhất: {word}"));
            }
        }
    });
}

unsafe extern "C" fn on_dict_import(_: *mut GtkWidget, _: *mut c_void) {
    let dlg = DICT_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg).unwrap_or(ptr::null_mut()));
    if let Some(path) = pick_file_path(dlg, "Nhập từ điển cá nhân (.txt)", false, "")
        && let Ok(raw) = fs::read_to_string(path)
    {
        let imported = Config::parse_user_dict_text(&raw);
        DICT_DLG.with(|c| {
            if let Some(st) = c.borrow_mut().as_mut() {
                sync_visible_into_all(st);
                let before = st.all_words.len();
                for w in imported {
                    if !st.all_words.iter().any(|ex| ex.eq_ignore_ascii_case(&w)) {
                        st.all_words.push(w);
                    }
                }
                let added = st.all_words.len().saturating_sub(before);
                apply_dict_filter(st);
                set_label_text(
                    st.lbl_status,
                    &format!("Đã nhập {added} từ mới (tổng {} từ)", st.all_words.len()),
                );
            }
        });
    }
}

unsafe extern "C" fn on_dict_export(_: *mut GtkWidget, _: *mut c_void) {
    let pair = DICT_DLG.with(|c| {
        let mut b = c.borrow_mut();
        let st = b.as_mut()?;
        sync_visible_into_all(st);
        let tmp = Config { user_dict_words: st.all_words.clone(), ..Config::default() };
        Some((st.dlg, tmp.format_user_dict_text(), st.all_words.len()))
    });
    if let Some((dlg, txt, count)) = pair
        && let Some(path) = pick_file_path(dlg, "Xuất từ điển cá nhân (.txt)", true, "user-dict.txt")
        && fs::write(path, txt).is_ok()
    {
        DICT_DLG.with(|c| {
            if let Some(st) = c.borrow().as_ref() {
                set_label_text(st.lbl_status, &format!("Đã xuất {count} từ ra tệp .txt"));
            }
        });
    }
}

unsafe extern "C" fn on_dict_save(_: *mut GtkWidget, _: *mut c_void) {
    let dlg = DICT_DLG.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            sync_visible_into_all(st);
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
