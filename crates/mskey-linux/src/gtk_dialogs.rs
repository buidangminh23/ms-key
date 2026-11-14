use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::fs;
use std::os::raw::c_int;
use std::ptr;

use mskey_core::{CaseTransform, Charset, Config, HotkeyCombo, convert_text};

use crate::gio_ffi::from_cstr;
use crate::gtk_ffi::{
    GTK_FILE_CHOOSER_ACTION_OPEN, GTK_POLICY_AUTOMATIC, GTK_POLICY_NEVER, GdkEventKey, GtkWidget,
    GtkWindow, connect_signal, gtk_container_add, gtk_file_chooser_get_filename,
    gtk_file_chooser_widget_new, gtk_grid_attach, gtk_scrolled_window_new,
    gtk_scrolled_window_set_policy, gtk_text_view_new, gtk_widget_destroy, gtk_widget_set_hexpand,
    gtk_widget_set_vexpand,
};
use crate::gtk_helpers::{
    create_modal_gtk, get_combo, make_button, make_combo, make_grid, make_hbox, make_label,
    make_vbox, pack, read_clipboard_gtk, read_textview, run_modal_gtk, write_clipboard_gtk,
    write_textview,
};
use crate::gio_ffi::g_free;
use crate::keyval::{
    IBUS_CONTROL_MASK, IBUS_MOD1_MASK, IBUS_MOD4_MASK, IBUS_SHIFT_MASK, format_hotkey_linux,
    gdk_keyval_to_vk, is_modifier_keyval,
};

struct ConvWidgets {
    dlg: *mut GtkWidget,
    from_cb: *mut GtkWidget,
    to_cb: *mut GtkWidget,
    case_cb: *mut GtkWidget,
    tv: *mut GtkWidget,
}

struct TreeChooser {
    dlg: *mut GtkWidget,
    chooser: *mut GtkWidget,
    path: Option<String>,
}

thread_local! {
    static CONV_DLG: RefCell<Option<ConvWidgets>> = const { RefCell::new(None) };
    static TREE_DLG: RefCell<Option<TreeChooser>> = const { RefCell::new(None) };
    static CAP_WIN: Cell<*mut GtkWidget> = const { Cell::new(ptr::null_mut()) };
    static CAP_OUT: Cell<Option<HotkeyCombo>> = const { Cell::new(None) };
}

pub fn show_converter_dialog(parent: *mut GtkWindow) {
    let dlg = create_modal_gtk(parent, "Chuyển mã Clipboard & Văn bản", 464, 340);
    let root = make_vbox(8);
    let grid = make_grid(8, 8);
    let cs = &["Unicode", "TCVN3 (ABC)", "VNI Windows", "Unicode tổ hợp"];
    let cases = &["Giữ nguyên chữ", "CHỮ HOA", "chữ thường", "Hoa đầu câu", "Hoa Mỗi Từ"];
    let from_cb = make_combo(cs);
    let to_cb = make_combo(cs);
    let case_cb = make_combo(cases);
    unsafe {
        gtk_grid_attach(grid, make_label("Bảng mã nguồn", false), 0, 0, 1, 1);
        gtk_grid_attach(grid, from_cb, 1, 0, 1, 1);
        gtk_grid_attach(grid, make_label("Bảng mã đích", false), 0, 1, 1, 1);
        gtk_grid_attach(grid, to_cb, 1, 1, 1, 1);
        gtk_grid_attach(grid, make_label("Chuyển hoa/thường", false), 0, 2, 1, 1);
        gtk_grid_attach(grid, case_cb, 1, 2, 1, 1);
    }
    pack(root, grid, false, false, 0);

    let sw = unsafe { gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut()) };
    let tv = unsafe { gtk_text_view_new() };
    write_textview(tv, &read_clipboard_gtk());
    unsafe {
        gtk_widget_set_hexpand(sw, 1);
        gtk_widget_set_vexpand(sw, 1);
        gtk_scrolled_window_set_policy(sw, GTK_POLICY_NEVER, GTK_POLICY_AUTOMATIC);
        gtk_container_add(sw, tv);
    }
    pack(root, sw, true, true, 0);

    let row = make_hbox(8);
    let b_read = make_button("Đọc Clipboard", false);
    let b_run = make_button("Chuyển & Chép", true);
    let b_close = make_button("Đóng", false);
    pack(row, b_read, false, false, 0);
    pack(row, b_run, false, false, 0);
    pack(row, make_label("", false), true, true, 0);
    pack(row, b_close, false, false, 0);
    pack(root, row, false, false, 0);
    unsafe { gtk_container_add(dlg, root) };

    CONV_DLG.with(|c| {
        *c.borrow_mut() = Some(ConvWidgets { dlg, from_cb, to_cb, case_cb, tv });
    });
    connect_signal(b_read, "clicked", on_conv_read as *const c_void, 0);
    connect_signal(b_run, "clicked", on_conv_run as *const c_void, 0);
    connect_signal(b_close, "clicked", on_conv_close as *const c_void, 0);
    run_modal_gtk(dlg);
    CONV_DLG.with(|c| {
        *c.borrow_mut() = None;
    });
}

unsafe extern "C" fn on_conv_read(_: *mut GtkWidget, _: *mut c_void) {
    let tv = CONV_DLG.with(|c| c.borrow().as_ref().map(|w| w.tv));
    if let Some(tv) = tv {
        write_textview(tv, &read_clipboard_gtk());
    }
}

unsafe extern "C" fn on_conv_run(_: *mut GtkWidget, _: *mut c_void) {
    let data = CONV_DLG.with(|c| {
        c.borrow()
            .as_ref()
            .map(|w| (get_combo(w.from_cb), get_combo(w.to_cb), get_combo(w.case_cb), w.tv))
    });
    if let Some((f, t, ca, tv)) = data {
        let map_cs = |idx: usize| match idx {
            1 => Charset::Tcvn3,
            2 => Charset::VniWindows,
            3 => Charset::UnicodeComposite,
            _ => Charset::Unicode,
        };
        let map_case = |idx: usize| match idx {
            1 => CaseTransform::Upper,
            2 => CaseTransform::Lower,
            3 => CaseTransform::Sentence,
            4 => CaseTransform::Title,
            _ => CaseTransform::None,
        };
        let out = convert_text(&read_textview(tv), map_cs(f), map_cs(t), map_case(ca));
        write_textview(tv, &out);
        write_clipboard_gtk(&out);
    }
}

unsafe extern "C" fn on_conv_close(_: *mut GtkWidget, _: *mut c_void) {
    if let Some(dlg) = CONV_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg)) {
        unsafe { gtk_widget_destroy(dlg) };
    }
}

pub fn capture_hotkey_dialog(parent: *mut GtkWindow, current: HotkeyCombo) -> Option<HotkeyCombo> {
    CAP_OUT.with(|c| c.set(None));
    let dlg = create_modal_gtk(
        parent,
        &format!("Nhấn phím tắt ({})", format_hotkey_linux(current)),
        360,
        104,
    );
    CAP_WIN.with(|c| c.set(dlg));
    let root = make_vbox(12);
    pack(root, make_label("Nhấn tổ hợp phím mới (vd Ctrl + Shift + F6)", false), false, false, 0);
    let bar = make_hbox(8);
    let b_cancel = make_button("Hủy", false);
    pack(bar, make_label("", false), true, true, 0);
    pack(bar, b_cancel, false, false, 0);
    pack(root, bar, false, false, 0);
    unsafe { gtk_container_add(dlg, root) };
    connect_signal(dlg, "key-press-event", on_cap_keypress as *const c_void, 0);
    connect_signal(b_cancel, "clicked", on_cap_cancel as *const c_void, 0);
    run_modal_gtk(dlg);
    CAP_WIN.with(|c| c.set(ptr::null_mut()));
    CAP_OUT.with(|c| c.get())
}

unsafe extern "C" fn on_cap_keypress(
    w: *mut GtkWidget,
    ev: *mut GdkEventKey,
    _: *mut c_void,
) -> c_int {
    let e = unsafe { &*ev };
    if !is_modifier_keyval(e.keyval) {
        CAP_OUT.with(|c| {
            c.set(Some(HotkeyCombo {
                ctrl: (e.state & IBUS_CONTROL_MASK) != 0,
                alt: (e.state & IBUS_MOD1_MASK) != 0,
                shift: (e.state & IBUS_SHIFT_MASK) != 0,
                win_cmd: (e.state & IBUS_MOD4_MASK) != 0,
                key_code: gdk_keyval_to_vk(e.keyval),
            }));
        });
        unsafe { gtk_widget_destroy(w) };
        return 1;
    }
    0
}

unsafe extern "C" fn on_cap_cancel(_: *mut GtkWidget, _: *mut c_void) {
    let w = CAP_WIN.with(|c| c.get());
    if !w.is_null() {
        unsafe { gtk_widget_destroy(w) };
    }
}

pub fn pick_and_import_evkey_tree(parent: *mut GtkWindow) -> Option<Config> {
    let dlg = create_modal_gtk(parent, "Chọn tệp setting.tree của EVKey", 600, 420);
    let root = make_vbox(8);
    let chooser = unsafe { gtk_file_chooser_widget_new(GTK_FILE_CHOOSER_ACTION_OPEN) };
    unsafe {
        gtk_widget_set_hexpand(chooser, 1);
        gtk_widget_set_vexpand(chooser, 1);
    }
    pack(root, chooser, true, true, 0);
    let bar = make_hbox(8);
    let b_open = make_button("Nhập tệp này", true);
    let b_cancel = make_button("Hủy", false);
    pack(bar, make_label("", false), true, true, 0);
    pack(bar, b_cancel, false, false, 0);
    pack(bar, b_open, false, false, 0);
    pack(root, bar, false, false, 0);
    unsafe { gtk_container_add(dlg, root) };

    TREE_DLG.with(|c| {
        *c.borrow_mut() = Some(TreeChooser { dlg, chooser, path: None });
    });
    connect_signal(b_open, "clicked", on_tree_open as *const c_void, 0);
    connect_signal(b_cancel, "clicked", on_tree_cancel as *const c_void, 0);
    run_modal_gtk(dlg);
    let path = TREE_DLG.with(|c| c.borrow_mut().take().and_then(|s| s.path))?;
    let raw = fs::read_to_string(path).ok()?;
    Some(Config::import_evkey_tree(&raw))
}

unsafe extern "C" fn on_tree_open(_: *mut GtkWidget, _: *mut c_void) {
    let dlg = TREE_DLG.with(|c| {
        let mut b = c.borrow_mut();
        let Some(st) = b.as_mut() else { return ptr::null_mut() };
        let ptr = unsafe { gtk_file_chooser_get_filename(st.chooser) };
        if !ptr.is_null() {
            let s = unsafe { from_cstr(ptr) };
            unsafe { g_free(ptr as *mut c_void) };
            st.path = Some(s);
        }
        st.dlg
    });
    if !dlg.is_null() {
        unsafe { gtk_widget_destroy(dlg) };
    }
}

unsafe extern "C" fn on_tree_cancel(_: *mut GtkWidget, _: *mut c_void) {
    if let Some(dlg) = TREE_DLG.with(|c| c.borrow().as_ref().map(|w| w.dlg)) {
        unsafe { gtk_widget_destroy(dlg) };
    }
}
