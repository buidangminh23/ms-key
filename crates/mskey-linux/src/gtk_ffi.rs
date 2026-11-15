use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_uint, c_ulong};
use std::ptr;

pub type GtkWidget = c_void;
pub type GtkWindow = c_void;
pub type GtkContainer = c_void;
pub type GtkBox = c_void;
pub type GtkGrid = c_void;
pub type GtkNotebook = c_void;
pub type GtkComboBox = c_void;
pub type GtkComboBoxText = c_void;
pub type GtkToggleButton = c_void;
pub type GtkRadioButton = c_void;
pub type GtkButton = c_void;
pub type GtkLabel = c_void;
pub type GtkEntry = c_void;
pub type GtkTextView = c_void;
pub type GtkTextBuffer = c_void;
pub type GtkListBox = c_void;
pub type GtkListBoxRow = c_void;
pub type GtkCssProvider = c_void;
pub type GtkStyleContext = c_void;
pub type GtkClipboard = c_void;
pub type GtkFileChooser = c_void;
pub type GtkTextMark = c_void;
pub type GtkSpinButton = c_void;
pub type GdkScreen = c_void;
pub type GList = c_void;

pub const GTK_WINDOW_TOPLEVEL: c_int = 0;
pub const GTK_WINDOW_POPUP: c_int = 1;
pub const GTK_WIN_POS_CENTER: c_int = 1;
pub const GTK_ORIENTATION_HORIZONTAL: c_int = 0;
pub const GTK_ORIENTATION_VERTICAL: c_int = 1;
pub const GTK_POLICY_AUTOMATIC: c_int = 1;
pub const GTK_POLICY_NEVER: c_int = 2;
pub const GTK_SELECTION_SINGLE: c_int = 1;
pub const GTK_WRAP_WORD_CHAR: c_int = 3;
pub const GTK_FILE_CHOOSER_ACTION_OPEN: c_int = 0;
pub const GTK_FILE_CHOOSER_ACTION_SAVE: c_int = 1;
pub const GTK_STYLE_PROVIDER_PRIORITY_APPLICATION: c_uint = 600;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GtkAllocation {
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
}

#[repr(C)]
pub struct GdkEventKey {
    pub type_: c_int,
    pub window: *mut c_void,
    pub send_event: i8,
    pub time: u32,
    pub state: u32,
    pub keyval: u32,
    pub length: c_int,
    pub string: *mut c_char,
    pub hardware_keycode: u16,
    pub group: u8,
    pub is_modifier: u32,
}

#[repr(C)]
pub struct GtkTextIter {
    pub dummy: [u8; 80],
}

unsafe extern "C" {
    pub fn gdk_pixbuf_loader_new() -> *mut c_void;
    pub fn gdk_pixbuf_loader_write(loader: *mut c_void, data: *const u8, size: usize, error: *mut *mut c_void) -> c_int;
    pub fn gdk_pixbuf_loader_close(loader: *mut c_void, error: *mut *mut c_void) -> c_int;
    pub fn gdk_pixbuf_loader_get_pixbuf(loader: *mut c_void) -> *mut c_void;
    pub fn g_object_unref(object: *mut c_void);
    pub fn g_object_get(object: *mut c_void, property: *const c_char, ...);
    pub fn gtk_window_set_default_icon(icon: *mut c_void);
    pub fn gtk_window_get_default_icon_list() -> *mut GList;
    pub fn gtk_image_new_from_pixbuf(icon: *mut c_void) -> *mut GtkWidget;
    pub fn gtk_widget_get_settings(widget: *mut GtkWidget) -> *mut c_void;
    pub fn gtk_widget_set_opacity(widget: *mut GtkWidget, opacity: f64);
    pub fn gtk_widget_add_tick_callback(widget: *mut GtkWidget, callback: Option<unsafe extern "C" fn(*mut GtkWidget, *mut c_void, *mut c_void) -> c_int>, data: *mut c_void, notify: Option<unsafe extern "C" fn(*mut c_void)>) -> c_uint;
    pub fn gtk_init_check(argc: *mut c_int, argv: *mut *mut *mut c_char) -> c_int;
    pub fn gtk_main();
    pub fn gtk_main_quit();
    pub fn gtk_events_pending() -> c_int;
    pub fn gtk_main_iteration_do(blocking: c_int) -> c_int;

    pub fn g_signal_connect_data(
        instance: *mut c_void,
        detailed_signal: *const c_char,
        c_handler: *const c_void,
        data: *mut c_void,
        destroy_data: *const c_void,
        connect_flags: c_uint,
    ) -> c_ulong;

    pub fn gtk_window_new(kind: c_int) -> *mut GtkWidget;
    pub fn gtk_window_set_title(win: *mut GtkWindow, title: *const c_char);
    pub fn gtk_window_set_default_size(win: *mut GtkWindow, width: c_int, height: c_int);
    pub fn gtk_window_set_resizable(win: *mut GtkWindow, resizable: c_int);
    pub fn gtk_window_set_position(win: *mut GtkWindow, position: c_int);
    pub fn gtk_window_set_transient_for(win: *mut GtkWindow, parent: *mut GtkWindow);
    pub fn gtk_window_set_modal(win: *mut GtkWindow, modal: c_int);
    pub fn gtk_window_present(win: *mut GtkWindow);
    pub fn gtk_window_move(win: *mut GtkWindow, x: c_int, y: c_int);
    pub fn gtk_window_set_decorated(win: *mut GtkWindow, setting: c_int);
    pub fn gtk_window_set_keep_above(win: *mut GtkWindow, setting: c_int);
    pub fn gtk_window_set_accept_focus(win: *mut GtkWindow, setting: c_int);
    pub fn gtk_window_set_skip_taskbar_hint(win: *mut GtkWindow, setting: c_int);
    pub fn gtk_window_set_skip_pager_hint(win: *mut GtkWindow, setting: c_int);

    pub fn gtk_box_new(orientation: c_int, spacing: c_int) -> *mut GtkWidget;
    pub fn gtk_box_pack_start(
        bx: *mut GtkBox,
        child: *mut GtkWidget,
        expand: c_int,
        fill: c_int,
        padding: c_uint,
    );
    pub fn gtk_grid_new() -> *mut GtkWidget;
    pub fn gtk_grid_set_row_spacing(grid: *mut GtkGrid, spacing: c_uint);
    pub fn gtk_grid_set_column_spacing(grid: *mut GtkGrid, spacing: c_uint);
    pub fn gtk_grid_set_column_homogeneous(grid: *mut GtkGrid, homogeneous: c_int);
    pub fn gtk_grid_attach(
        grid: *mut GtkGrid,
        child: *mut GtkWidget,
        left: c_int,
        top: c_int,
        width: c_int,
        height: c_int,
    );

    pub fn gtk_frame_new(label: *const c_char) -> *mut GtkWidget;
    pub fn gtk_container_add(container: *mut GtkContainer, widget: *mut GtkWidget);
    pub fn gtk_container_remove(container: *mut GtkContainer, widget: *mut GtkWidget);
    pub fn gtk_container_set_border_width(container: *mut GtkContainer, width: c_uint);
    pub fn gtk_container_get_children(container: *mut GtkContainer) -> *mut GList;
    pub fn g_list_length(list: *mut GList) -> c_uint;
    pub fn g_list_nth_data(list: *mut GList, n: c_uint) -> *mut c_void;
    pub fn g_list_free(list: *mut GList);

    pub fn gtk_notebook_new() -> *mut GtkWidget;
    pub fn gtk_notebook_append_page(
        nb: *mut GtkNotebook,
        child: *mut GtkWidget,
        tab_label: *mut GtkWidget,
    ) -> c_int;

    pub fn gtk_combo_box_text_new() -> *mut GtkWidget;
    pub fn gtk_combo_box_text_append_text(cb: *mut GtkComboBoxText, text: *const c_char);
    pub fn gtk_combo_box_set_active(cb: *mut GtkComboBox, index: c_int);
    pub fn gtk_combo_box_get_active(cb: *mut GtkComboBox) -> c_int;

    pub fn gtk_check_button_new_with_label(label: *const c_char) -> *mut GtkWidget;
    pub fn gtk_radio_button_new_with_label_from_widget(
        group: *mut GtkRadioButton,
        label: *const c_char,
    ) -> *mut GtkWidget;
    pub fn gtk_toggle_button_set_active(btn: *mut GtkToggleButton, active: c_int);
    pub fn gtk_toggle_button_get_active(btn: *mut GtkToggleButton) -> c_int;

    pub fn gtk_button_new_with_label(label: *const c_char) -> *mut GtkWidget;
    pub fn gtk_button_set_label(btn: *mut GtkButton, label: *const c_char);
    pub fn gtk_label_new(str_ptr: *const c_char) -> *mut GtkWidget;
    pub fn gtk_label_set_text(lbl: *mut GtkLabel, str_ptr: *const c_char);
    pub fn gtk_label_set_xalign(lbl: *mut GtkLabel, xalign: f32);

    pub fn gtk_entry_new() -> *mut GtkWidget;
    pub fn gtk_entry_set_text(entry: *mut GtkEntry, text: *const c_char);
    pub fn gtk_entry_get_text(entry: *mut GtkEntry) -> *const c_char;
    pub fn gtk_entry_set_max_length(entry: *mut GtkEntry, max: c_int);
    pub fn gtk_spin_button_new_with_range(min: f64, max: f64, step: f64) -> *mut GtkWidget;
    pub fn gtk_spin_button_set_value(sb: *mut GtkSpinButton, value: f64);
    pub fn gtk_spin_button_get_value_as_int(sb: *mut GtkSpinButton) -> c_int;

    pub fn gtk_text_view_new() -> *mut GtkWidget;
    pub fn gtk_text_view_get_buffer(tv: *mut GtkTextView) -> *mut GtkTextBuffer;
    pub fn gtk_text_view_set_wrap_mode(tv: *mut GtkTextView, wrap_mode: c_int);
    pub fn gtk_text_buffer_set_text(buf: *mut GtkTextBuffer, text: *const c_char, len: c_int);
    pub fn gtk_text_buffer_get_start_iter(buf: *mut GtkTextBuffer, iter: *mut GtkTextIter);
    pub fn gtk_text_buffer_get_end_iter(buf: *mut GtkTextBuffer, iter: *mut GtkTextIter);
    pub fn gtk_text_buffer_get_text(
        buf: *mut GtkTextBuffer,
        start: *const GtkTextIter,
        end: *const GtkTextIter,
        include_hidden: c_int,
    ) -> *mut c_char;
    pub fn gtk_text_buffer_get_insert(buf: *mut GtkTextBuffer) -> *mut GtkTextMark;
    pub fn gtk_text_buffer_get_iter_at_mark(
        buf: *mut GtkTextBuffer,
        iter: *mut GtkTextIter,
        mark: *mut GtkTextMark,
    );
    pub fn gtk_text_iter_get_offset(iter: *const GtkTextIter) -> c_int;
    pub fn gtk_text_buffer_get_iter_at_offset(
        buf: *mut GtkTextBuffer,
        iter: *mut GtkTextIter,
        char_offset: c_int,
    );
    pub fn gtk_text_buffer_delete(
        buf: *mut GtkTextBuffer,
        start: *mut GtkTextIter,
        end: *mut GtkTextIter,
    );
    pub fn gtk_text_buffer_insert_at_cursor(
        buf: *mut GtkTextBuffer,
        text: *const c_char,
        len: c_int,
    );
    pub fn gtk_text_buffer_place_cursor(buf: *mut GtkTextBuffer, where_iter: *const GtkTextIter);

    pub fn gtk_scrolled_window_new(hadj: *mut c_void, vadj: *mut c_void) -> *mut GtkWidget;
    pub fn gtk_scrolled_window_set_policy(sw: *mut c_void, hpol: c_int, vpol: c_int);
    pub fn gtk_list_box_new() -> *mut GtkWidget;
    pub fn gtk_list_box_set_selection_mode(lb: *mut GtkListBox, mode: c_int);
    pub fn gtk_list_box_get_selected_row(lb: *mut GtkListBox) -> *mut GtkListBoxRow;
    pub fn gtk_list_box_row_get_index(row: *mut GtkListBoxRow) -> c_int;
    pub fn gtk_list_box_insert(lb: *mut GtkListBox, child: *mut GtkWidget, pos: c_int);

    pub fn gtk_widget_show(w: *mut GtkWidget);
    pub fn gtk_widget_hide(w: *mut GtkWidget);
    pub fn gtk_widget_show_all(w: *mut GtkWidget);
    pub fn gtk_widget_set_size_request(w: *mut GtkWidget, width: c_int, height: c_int);
    pub fn gtk_widget_set_hexpand(w: *mut GtkWidget, expand: c_int);
    pub fn gtk_widget_set_vexpand(w: *mut GtkWidget, expand: c_int);
    pub fn gtk_widget_set_margin_top(w: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_set_margin_bottom(w: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_set_margin_start(w: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_set_margin_end(w: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_get_allocation(w: *mut GtkWidget, allocation: *mut GtkAllocation);
    pub fn gtk_widget_get_name(w: *mut GtkWidget) -> *const c_char;

    pub fn gtk_widget_get_style_context(w: *mut GtkWidget) -> *mut GtkStyleContext;
    pub fn gtk_style_context_add_class(ctx: *mut GtkStyleContext, class_name: *const c_char);
    pub fn gtk_style_context_remove_class(ctx: *mut GtkStyleContext, class_name: *const c_char);
    pub fn gtk_style_context_add_provider_for_screen(
        screen: *mut GdkScreen,
        provider: *mut c_void,
        priority: c_uint,
    );
    pub fn gtk_style_context_remove_provider_for_screen(
        screen: *mut GdkScreen,
        provider: *mut c_void,
    );
    pub fn gtk_css_provider_new() -> *mut GtkCssProvider;
    pub fn gtk_css_provider_load_from_data(
        provider: *mut GtkCssProvider,
        data: *const c_char,
        length: isize,
        error: *mut *mut c_void,
    ) -> c_int;
    pub fn gdk_screen_get_default() -> *mut GdkScreen;
    pub fn gdk_screen_get_width(screen: *mut GdkScreen) -> c_int;
    pub fn gdk_screen_get_height(screen: *mut GdkScreen) -> c_int;

    pub fn gdk_atom_intern_static_string(name: *const c_char) -> *mut c_void;
    pub fn gdk_display_get_default() -> *mut c_void;
    pub fn gtk_clipboard_get_for_display(
        display: *mut c_void,
        selection: *mut c_void,
    ) -> *mut GtkClipboard;
    pub fn gtk_clipboard_set_text(cb: *mut GtkClipboard, text: *const c_char, len: c_int);
    pub fn gtk_clipboard_wait_for_text(cb: *mut GtkClipboard) -> *mut c_char;
    pub fn gtk_clipboard_store(cb: *mut GtkClipboard);

    pub fn gtk_file_chooser_widget_new(action: c_int) -> *mut GtkWidget;
    pub fn gtk_file_chooser_get_filename(chooser: *mut GtkFileChooser) -> *mut c_char;
    pub fn gtk_file_chooser_set_current_name(chooser: *mut GtkFileChooser, name: *const c_char);
    pub fn gtk_file_chooser_set_do_overwrite_confirmation(
        chooser: *mut GtkFileChooser,
        do_conf: c_int,
    );
    pub fn gtk_widget_destroy(w: *mut GtkWidget);
}

pub fn connect_signal(instance: *mut c_void, signal: &str, cb: *const c_void, data: usize) {
    let cs = crate::gio_ffi::cstr(signal);
    unsafe {
        g_signal_connect_data(
            instance,
            cs.as_ptr(),
            cb,
            data as *mut c_void,
            ptr::null(),
            0,
        );
    }
}
