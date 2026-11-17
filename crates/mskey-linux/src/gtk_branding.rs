use std::ffi::c_void;
use std::ptr;
use std::time::Instant;

use crate::gio_ffi::cstr;
use crate::gtk_ffi::*;
use crate::gtk_helpers::{make_hbox, make_label, make_vbox, pack};
use crate::gtk_theme::add_css_class;

pub fn initialize_brand_icon() {
    unsafe {
        let loader = gdk_pixbuf_loader_new();
        if loader.is_null() {
            return;
        }
        let data = include_bytes!("../../../assets/branding/mskey-64.png");
        if gdk_pixbuf_loader_write(loader, data.as_ptr(), data.len(), ptr::null_mut()) != 0
            && gdk_pixbuf_loader_close(loader, ptr::null_mut()) != 0
        {
            let icon = gdk_pixbuf_loader_get_pixbuf(loader);
            if !icon.is_null() {
                gtk_window_set_default_icon(icon);
            }
        }
        g_object_unref(loader);
    }
}

pub fn make_brand_header(animate: bool, tools: *mut GtkWidget) -> *mut GtkWidget {
    let row = make_hbox(12);
    add_css_class(row, "brand-header");
    let image = unsafe {
        let icon = gtk_window_get_default_icon_list_first();
        if icon.is_null() {
            gtk_image_new_from_pixbuf(icon)
        } else {
            let display_icon = gdk_pixbuf_scale_simple(icon, 40, 40, GDK_INTERP_BILINEAR);
            let image = gtk_image_new_from_pixbuf(display_icon);
            if !display_icon.is_null() {
                g_object_unref(display_icon);
            }
            image
        }
    };
    pack(row, image, false, false, 0);
    let text = make_vbox(4);
    let name = make_label("MS Key", false);
    add_css_class(name, "brand-title");
    pack(text, name, false, false, 0);
    pack(text, make_label("Bộ gõ tiếng Việt", true), false, false, 0);
    pack(row, text, true, true, 0);
    unsafe { gtk_widget_set_valign(tools, GTK_ALIGN_CENTER) };
    pack(row, tools, false, false, 0);
    unsafe {
        let mut enabled: i32 = 0;
        let settings = gtk_widget_get_settings(image);
        if !settings.is_null() {
            g_object_get(
                settings,
                cstr("gtk-enable-animations").as_ptr(),
                &mut enabled as *mut i32,
                ptr::null::<u8>(),
            );
        }
        if animate && enabled != 0 {
            gtk_widget_set_opacity(image, 0.2);
            gtk_widget_add_tick_callback(
                image,
                Some(animate_mark),
                Box::into_raw(Box::new(Instant::now())).cast(),
                Some(drop_animation),
            );
        }
    }
    row
}

unsafe fn gtk_window_get_default_icon_list_first() -> *mut c_void {
    unsafe {
        let list = gtk_window_get_default_icon_list();
        let icon = if list.is_null() {
            ptr::null_mut()
        } else {
            g_list_nth_data(list, 0)
        };
        g_list_free(list);
        icon
    }
}

unsafe extern "C" fn animate_mark(
    widget: *mut GtkWidget,
    _: *mut c_void,
    data: *mut c_void,
) -> i32 {
    let elapsed = unsafe { &*data.cast::<Instant>() }.elapsed().as_secs_f64();
    let progress = (elapsed / 0.45).min(1.0);
    unsafe { gtk_widget_set_opacity(widget, 1.0 - 0.8 * (1.0 - progress).powi(3)) };
    i32::from(progress < 1.0)
}

unsafe extern "C" fn drop_animation(data: *mut c_void) {
    drop(unsafe { Box::from_raw(data.cast::<Instant>()) });
}
