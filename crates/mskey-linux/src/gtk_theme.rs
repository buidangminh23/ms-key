use std::cell::Cell;
use std::ptr;

use mskey_core::ThemeMode;

use crate::config_store::detect_system_dark_mode;
use crate::gio_ffi::cstr;
use crate::gtk_ffi::{
    GTK_STYLE_PROVIDER_PRIORITY_APPLICATION, GtkCssProvider, GtkWidget, gdk_screen_get_default,
    gtk_css_provider_load_from_data, gtk_css_provider_new,
    gtk_style_context_add_provider_for_screen, gtk_style_context_remove_class,
    gtk_style_context_add_class, gtk_style_context_remove_provider_for_screen,
};

thread_local! {
    static ACTIVE_PROVIDER: Cell<*mut GtkCssProvider> = const { Cell::new(ptr::null_mut()) };
}

pub fn apply_gtk_theme(mode: ThemeMode) {
    let dark = match mode {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::System => detect_system_dark_mode(),
    };
    let css = build_theme_css(dark);
    let c_css = cstr(&css);
    let screen = unsafe { gdk_screen_get_default() };
    if screen.is_null() {
        return;
    }
    ACTIVE_PROVIDER.with(|cell| {
        let old = cell.get();
        if !old.is_null() {
            unsafe {
                gtk_style_context_remove_provider_for_screen(screen, old);
            }
        }
        let prov = unsafe { gtk_css_provider_new() };
        unsafe {
            gtk_css_provider_load_from_data(prov, c_css.as_ptr(), -1, ptr::null_mut());
            gtk_style_context_add_provider_for_screen(
                screen,
                prov,
                GTK_STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        cell.set(prov);
    });
}

pub fn set_ve_button_style(btn: *mut GtkWidget, vietnamese: bool) {
    let ctx = unsafe { gtk_widget_get_style_context(btn) };
    let v_cls = cstr("ve-btn-v");
    let e_cls = cstr("ve-btn-e");
    unsafe {
        if vietnamese {
            gtk_style_context_remove_class(ctx, e_cls.as_ptr());
            gtk_style_context_add_class(ctx, v_cls.as_ptr());
        } else {
            gtk_style_context_remove_class(ctx, v_cls.as_ptr());
            gtk_style_context_add_class(ctx, e_cls.as_ptr());
        }
    }
}

pub fn add_css_class(w: *mut GtkWidget, class_name: &str) {
    let cls = cstr(class_name);
    unsafe {
        let ctx = gtk_widget_get_style_context(w);
        gtk_style_context_add_class(ctx, cls.as_ptr());
    }
}

use crate::gtk_ffi::gtk_widget_get_style_context;

fn build_theme_css(dark: bool) -> String {
    let (win_bg, card_bg, input_bg, border, text, subtle, accent, btn_bg) = if dark {
        ("#1F1F1F", "#2B2B2B", "#353535", "#424242", "#F2F2F2", "#A8A8A8", "#369EFF", "#363636")
    } else {
        ("#F3F3F3", "#FFFFFF", "#FFFFFF", "#D6D6D6", "#1A1A1A", "#5A5A5A", "#0078D4", "#FFFFFF")
    };
    format!(
        "window, dialog {{ background-color: {win_bg}; color: {text}; font-size: 13px; }}\n\
         .card-box {{ background-color: {card_bg}; border: 1px solid {border}; border-radius: 8px; padding: 12px; }}\n\
         frame > border {{ border-color: {border}; border-radius: 6px; padding: 8px; }}\n\
         frame > label {{ color: {accent}; font-weight: bold; padding: 0 4px; }}\n\
         entry, textview, textview text {{ background-color: {input_bg}; color: {text}; border-color: {border}; }}\n\
         button {{ min-height: 28px; padding: 4px 12px; border-radius: 6px; background-image: none; background-color: {btn_bg}; color: {text}; border: 1px solid {border}; }}\n\
         button:hover {{ border-color: {accent}; }}\n\
         button.primary-btn {{ background-color: {accent}; color: #FFFFFF; border-color: {accent}; font-weight: bold; }}\n\
         button.ve-btn-v {{ background-color: #D13438; color: #FFFFFF; border-color: #D13438; font-weight: bold; }}\n\
         button.ve-btn-e {{ background-color: {accent}; color: #FFFFFF; border-color: {accent}; font-weight: bold; }}\n\
         .subtle-text {{ color: {subtle}; font-size: 12px; }}\n\
         notebook > header > tabs > tab {{ padding: 6px 14px; }}\n\
         notebook > header > tabs > tab:checked {{ color: {accent}; font-weight: bold; }}\n"
    )
}
