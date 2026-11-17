use std::ffi::c_void;

use mskey_core::InputMethod;

use crate::gio_ffi::{cstr, from_cstr, g_free};
use crate::gtk_ffi::{
    GTK_ORIENTATION_HORIZONTAL, GTK_ORIENTATION_VERTICAL, GTK_WIN_POS_CENTER, GTK_WINDOW_TOPLEVEL,
    GTK_WRAP_WORD_CHAR, GtkWidget, GtkWindow, connect_signal, g_list_free, g_list_length,
    g_list_nth_data, gdk_atom_intern_static_string, gdk_display_get_default, gtk_box_new,
    gtk_box_pack_start, gtk_button_new_with_label, gtk_check_button_new_with_label,
    gtk_clipboard_get_for_display, gtk_clipboard_set_text, gtk_clipboard_wait_for_text,
    gtk_combo_box_get_active, gtk_combo_box_set_active, gtk_combo_box_text_append_text,
    gtk_combo_box_text_new, gtk_combo_box_text_remove_all, gtk_container_add, gtk_container_get_children, gtk_container_remove,
    gtk_container_set_border_width, gtk_entry_get_text, gtk_entry_new, gtk_entry_set_placeholder_text, gtk_entry_set_text,
    gtk_frame_new, gtk_grid_new, gtk_grid_set_column_spacing, gtk_grid_set_row_spacing,
    gtk_label_new, gtk_label_set_text, gtk_label_set_xalign, gtk_list_box_insert, gtk_main,
    gtk_main_quit, gtk_radio_button_new_with_label_from_widget, gtk_text_buffer_get_end_iter,
    gtk_text_buffer_get_start_iter, gtk_text_buffer_get_text, gtk_text_buffer_set_text,
    gtk_text_view_get_buffer, gtk_text_view_set_wrap_mode, gtk_toggle_button_get_active,
    gtk_toggle_button_set_active, gtk_widget_destroy, gtk_widget_set_hexpand, gtk_widget_show_all,
    gtk_window_set_default_size, gtk_window_set_modal, gtk_window_set_position,
    gtk_window_set_title, gtk_window_set_transient_for, GtkTextIter, gtk_window_new,
};
use crate::gtk_theme::add_css_class;

pub fn make_hbox(spacing: i32) -> *mut GtkWidget {
    unsafe { gtk_box_new(GTK_ORIENTATION_HORIZONTAL, spacing) }
}

pub fn make_vbox(spacing: i32) -> *mut GtkWidget {
    unsafe { gtk_box_new(GTK_ORIENTATION_VERTICAL, spacing) }
}

pub fn pack(bx: *mut GtkWidget, child: *mut GtkWidget, expand: bool, fill: bool, pad: u32) {
    unsafe {
        gtk_box_pack_start(bx, child, if expand { 1 } else { 0 }, if fill { 1 } else { 0 }, pad);
    }
}

pub fn make_grid(row_gap: u32, col_gap: u32) -> *mut GtkWidget {
    unsafe {
        let g = gtk_grid_new();
        gtk_grid_set_row_spacing(g, row_gap);
        gtk_grid_set_column_spacing(g, col_gap);
        g
    }
}

pub fn make_frame(title: &str) -> (*mut GtkWidget, *mut GtkWidget) {
    let ct = cstr(title);
    unsafe {
        let f = gtk_frame_new(ct.as_ptr());
        let inner = make_vbox(8);
        gtk_container_set_border_width(inner, 8);
        gtk_container_add(f, inner);
        (f, inner)
    }
}

pub fn make_label(text: &str, subtle: bool) -> *mut GtkWidget {
    let ct = cstr(text);
    unsafe {
        let l = gtk_label_new(ct.as_ptr());
        gtk_label_set_xalign(l, 0.0);
        if subtle {
            add_css_class(l, "subtle-text");
        }
        l
    }
}

pub fn set_label_text(lbl: *mut GtkWidget, text: &str) {
    let ct = cstr(text);
    unsafe {
        gtk_label_set_text(lbl, ct.as_ptr());
    }
}

pub fn make_button(text: &str, primary: bool) -> *mut GtkWidget {
    let ct = cstr(text);
    unsafe {
        let b = gtk_button_new_with_label(ct.as_ptr());
        if primary {
            add_css_class(b, "primary-btn");
        }
        b
    }
}

pub fn make_check(text: &str) -> *mut GtkWidget {
    let ct = cstr(text);
    unsafe { gtk_check_button_new_with_label(ct.as_ptr()) }
}

pub fn make_radio(group: *mut GtkWidget, text: &str) -> *mut GtkWidget {
    let ct = cstr(text);
    unsafe { gtk_radio_button_new_with_label_from_widget(group, ct.as_ptr()) }
}

pub fn set_toggle(w: *mut GtkWidget, active: bool) {
    unsafe {
        gtk_toggle_button_set_active(w, if active { 1 } else { 0 });
    }
}

pub fn get_toggle(w: *mut GtkWidget) -> bool {
    unsafe { gtk_toggle_button_get_active(w) != 0 }
}

pub fn make_combo(items: &[&str]) -> *mut GtkWidget {
    unsafe {
        let cb = gtk_combo_box_text_new();
        gtk_widget_set_hexpand(cb, 1);
        for &it in items {
            let ct = cstr(it);
            gtk_combo_box_text_append_text(cb, ct.as_ptr());
        }
        gtk_combo_box_set_active(cb, 0);
        cb
    }
}

pub fn set_combo(cb: *mut GtkWidget, idx: usize) {
    unsafe {
        gtk_combo_box_set_active(cb, idx as i32);
    }
}

pub fn get_combo(cb: *mut GtkWidget) -> usize {
    unsafe { gtk_combo_box_get_active(cb).max(0) as usize }
}

const INPUT_METHOD_CHOICES: [(InputMethod, &str); 5] = [
    (InputMethod::Telex, "Telex"),
    (InputMethod::Vni, "VNI"),
    (InputMethod::Viqr, "VIQR"),
    (InputMethod::SimpleTelex, "Simple Telex"),
    (InputMethod::Custom, "Tự định nghĩa"),
];

pub fn make_input_method_combo() -> *mut GtkWidget {
    make_combo(&INPUT_METHOD_CHOICES.map(|(_, label)| label))
}

pub fn set_input_method(combo: *mut GtkWidget, method: InputMethod) {
    let index = INPUT_METHOD_CHOICES.iter().position(|(value, _)| *value == method).unwrap_or(0);
    set_combo(combo, index);
}

pub fn get_input_method(combo: *mut GtkWidget) -> InputMethod {
    INPUT_METHOD_CHOICES.get(get_combo(combo)).map_or(InputMethod::default(), |(value, _)| *value)
}

pub fn refill_combo(cb: *mut GtkWidget, items: &[String], active_idx: usize) {
    unsafe {
        gtk_combo_box_text_remove_all(cb);
        for it in items {
            let ct = cstr(it);
            gtk_combo_box_text_append_text(cb, ct.as_ptr());
        }
        if !items.is_empty() {
            let idx = active_idx.min(items.len() - 1);
            gtk_combo_box_set_active(cb, idx as i32);
        }
    }
}

pub fn make_entry_placeholder(val: &str, placeholder: &str) -> *mut GtkWidget {
    let e = make_entry(val);
    let cp = cstr(placeholder);
    unsafe {
        gtk_entry_set_placeholder_text(e, cp.as_ptr());
    }
    e
}

pub fn make_entry(val: &str) -> *mut GtkWidget {
    let cv = cstr(val);
    unsafe {
        let e = gtk_entry_new();
        gtk_entry_set_text(e, cv.as_ptr());
        e
    }
}

pub fn read_entry(e: *mut GtkWidget) -> String {
    unsafe { from_cstr(gtk_entry_get_text(e)) }
}

pub fn write_entry(e: *mut GtkWidget, val: &str) {
    let cv = cstr(val);
    unsafe {
        gtk_entry_set_text(e, cv.as_ptr());
    }
}

pub fn write_textview(tv: *mut GtkWidget, val: &str) {
    let cv = cstr(val);
    unsafe {
        gtk_text_view_set_wrap_mode(tv, GTK_WRAP_WORD_CHAR);
        let buf = gtk_text_view_get_buffer(tv);
        gtk_text_buffer_set_text(buf, cv.as_ptr(), -1);
    }
}

pub fn read_textview(tv: *mut GtkWidget) -> String {
    unsafe {
        let buf = gtk_text_view_get_buffer(tv);
        let mut s = GtkTextIter { dummy: [0; 80] };
        let mut e = GtkTextIter { dummy: [0; 80] };
        gtk_text_buffer_get_start_iter(buf, &mut s);
        gtk_text_buffer_get_end_iter(buf, &mut e);
        let ptr = gtk_text_buffer_get_text(buf, &s, &e, 1);
        let out = from_cstr(ptr);
        g_free(ptr as *mut c_void);
        out
    }
}

pub fn clear_listbox(lb: *mut GtkWidget) {
    unsafe {
        let list = gtk_container_get_children(lb);
        let count = g_list_length(list);
        for i in 0..count {
            let item = g_list_nth_data(list, i);
            if !item.is_null() {
                gtk_container_remove(lb, item);
            }
        }
        g_list_free(list);
    }
}

pub fn append_listbox_row(lb: *mut GtkWidget, text: &str) {
    let lbl = make_label(text, false);
    unsafe {
        gtk_widget_show_all(lbl);
        gtk_list_box_insert(lb, lbl, -1);
    }
}

pub fn read_clipboard_gtk() -> String {
    unsafe {
        let disp = gdk_display_get_default();
        if disp.is_null() {
            return String::new();
        }
        let atom_name = cstr("CLIPBOARD");
        let atom = gdk_atom_intern_static_string(atom_name.as_ptr());
        let cb = gtk_clipboard_get_for_display(disp, atom);
        if cb.is_null() {
            return String::new();
        }
        let ptr = gtk_clipboard_wait_for_text(cb);
        let txt = from_cstr(ptr);
        g_free(ptr as *mut c_void);
        txt
    }
}

pub fn write_clipboard_gtk(text: &str) {
    let ct = cstr(text);
    unsafe {
        let disp = gdk_display_get_default();
        if disp.is_null() {
            return;
        }
        let atom_name = cstr("CLIPBOARD");
        let atom = gdk_atom_intern_static_string(atom_name.as_ptr());
        let cb = gtk_clipboard_get_for_display(disp, atom);
        if !cb.is_null() {
            gtk_clipboard_set_text(cb, ct.as_ptr(), -1);
        }
    }
}

unsafe extern "C" fn on_modal_destroy(_: *mut GtkWidget, _: *mut c_void) {
    unsafe { gtk_main_quit() };
}

pub fn create_modal_gtk(parent: *mut GtkWindow, title: &str, w: i32, h: i32) -> *mut GtkWidget {
    let ct = cstr(title);
    unsafe {
        let win = gtk_window_new(GTK_WINDOW_TOPLEVEL);
        add_css_class(win, "mskey-window");
        gtk_window_set_title(win, ct.as_ptr());
        gtk_window_set_default_size(win, w, h);
        gtk_window_set_position(win, GTK_WIN_POS_CENTER);
        gtk_container_set_border_width(win, 16);
        if !parent.is_null() {
            gtk_window_set_transient_for(win, parent);
            gtk_window_set_modal(win, 1);
        }
        connect_signal(win, "destroy", on_modal_destroy as *const c_void, 0);
        win
    }
}

unsafe extern "C" fn on_msg_close_click(_: *mut GtkWidget, win: *mut c_void) {
    unsafe { gtk_widget_destroy(win as *mut GtkWidget) };
}

pub fn show_info_modal(parent: *mut GtkWindow, title: &str, msg: &str) {
    let dlg = create_modal_gtk(parent, title, 400, 140);
    let vbox = make_vbox(12);
    let lbl = make_label(msg, false);
    let close_btn = make_button("Đóng", true);
    pack(vbox, lbl, true, true, 0);
    pack(vbox, close_btn, false, false, 0);
    unsafe {
        gtk_container_add(dlg, vbox);
        connect_signal(close_btn, "clicked", on_msg_close_click as *const c_void, dlg as usize);
    }
    run_modal_gtk(dlg);
}

pub fn run_modal_gtk(modal: *mut GtkWidget) {
    unsafe {
        gtk_widget_show_all(modal);
        gtk_main();
    }
}
