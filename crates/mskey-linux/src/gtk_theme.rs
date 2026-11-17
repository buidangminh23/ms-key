use std::cell::Cell;
use std::ptr;

use mskey_core::ThemeMode;

use crate::config_store::detect_system_dark_mode;
use crate::gio_ffi::cstr;
use crate::gtk_ffi::{
    GTK_STYLE_PROVIDER_PRIORITY_APPLICATION, GtkCssProvider, GtkWidget, g_object_set,
    g_object_unref, gdk_screen_get_default, gtk_css_provider_load_from_data, gtk_css_provider_new,
    gtk_button_set_label, gtk_settings_get_default, gtk_style_context_add_class, gtk_style_context_add_provider_for_screen,
    gtk_style_context_remove_class, gtk_style_context_remove_provider_for_screen,
    gtk_widget_get_style_context,
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
        let prov = unsafe { gtk_css_provider_new() };
        unsafe {
            if gtk_css_provider_load_from_data(prov, c_css.as_ptr(), -1, ptr::null_mut()) == 0 {
                g_object_unref(prov);
                return;
            }
            let settings = gtk_settings_get_default();
            if !settings.is_null() {
                let prefer_dark = cstr("gtk-application-prefer-dark-theme");
                g_object_set(settings, prefer_dark.as_ptr(), i32::from(dark), ptr::null::<u8>());
            }
            let old = cell.replace(prov);
            if !old.is_null() {
                gtk_style_context_remove_provider_for_screen(screen, old);
                g_object_unref(old);
            }
            gtk_style_context_add_provider_for_screen(
                screen,
                prov,
                GTK_STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}

pub fn set_language_button(btn: *mut GtkWidget, vietnamese: bool) {
    let ctx = unsafe { gtk_widget_get_style_context(btn) };
    let label = cstr(if vietnamese { "Tiếng Việt (V)" } else { "Tiếng Anh (E)" });
    let v_cls = cstr("ve-btn-v");
    let e_cls = cstr("ve-btn-e");
    unsafe {
        gtk_button_set_label(btn, label.as_ptr());
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

fn build_theme_css(dark: bool) -> String {
    let (window, surface, field, edge, text, subtle, accent, hover, selected) = if dark {
        (
            "#151D29", "#1D293A", "#25354A", "#394B63", "#EDF3FB", "#ADC0D7",
            "#96C7FF", "#30445D", "#263E5B",
        )
    } else {
        (
            "#F2F5FA", "#FFFFFF", "#F7F9FC", "#CED9E7", "#192B43", "#50647E",
            "#175BA6", "#E8EFF8", "#E1EEFD",
        )
    };
    format!(
        r#"
        .mskey-window {{ background-color: {window}; color: {text}; }}
        .mskey-window .brand-title {{ font-size: 21pt; font-weight: 700; }}
        .mskey-window .brand-header,
        .mskey-window .settings-section {{ background-color: {surface}; border: 1px solid {edge}; border-radius: 10px; padding: 12px 16px; }}
        .mskey-window .section-title {{ font-size: 11pt; font-weight: 700; }}
        .mskey-window .subtle-text {{ color: {subtle}; }}
        .mskey-window .settings-page {{ padding: 12px 4px 4px; }}
        .mskey-window .settings-tabs scrolledwindow,
        .mskey-window .settings-tabs viewport,
        .mskey-window .settings-tabs scrollbar trough {{ background-color: {window}; }}
        .mskey-window .settings-tabs scrollbar slider {{
            background-color: {subtle}; border-color: {window};
        }}

        .mskey-window frame > border {{ background-color: {surface}; border-color: {edge}; border-radius: 8px; }}
        .mskey-window frame > label {{ color: {text}; font-weight: 700; padding: 0 4px; }}
        .mskey-window entry, .mskey-window textview, .mskey-window textview text {{
            background-color: {field}; color: {text}; border-color: {edge};
        }}
        .mskey-window entry, .mskey-window spinbutton {{
            border-radius: 6px;
        }}
        .mskey-window spinbutton {{
            background-color: {field}; color: {text}; border-color: {edge};
        }}
        .mskey-window spinbutton entry {{ color: {text}; }}
        .mskey-window textview {{ padding: 8px; }}
        .mskey-window entry:focus {{ border-color: {accent}; }}

        .mskey-window button {{
            min-height: 24px;
            padding: 4px 12px;
            border-radius: 6px;
            background-image: none;
            background-color: {surface};
            color: {text};
            border: 1px solid {edge};
            box-shadow: none;
        }}
        .mskey-window combobox button {{ background-color: {field}; }}
        .mskey-window button:hover {{ background-color: {hover}; border-color: {accent}; }}
        .mskey-window button:active {{ background-color: {selected}; }}
        .mskey-window button:disabled {{ color: {subtle}; opacity: 0.55; }}
        .mskey-window button:focus, .mskey-window entry:focus, .mskey-window textview:focus,
        .mskey-window .settings-tabs > header > tabs > tab:focus {{
            outline: 2px solid {accent};
            outline-offset: 2px;
        }}
        .mskey-window button.primary-btn, .mskey-window button.ve-btn-e {{
            background-color: #175BA6; color: #FFFFFF; border-color: #175BA6; font-weight: 700;
        }}
        .mskey-window button.primary-btn:hover, .mskey-window button.ve-btn-e:hover {{
            background-color: #12477F; border-color: #12477F;
        }}
        .mskey-window button.language-toggle {{ min-width: 112px; min-height: 36px; }}
        .mskey-window button.ve-btn-v {{
            background-color: #B82D37; color: #FFFFFF; border-color: #B82D37; font-weight: 700;
        }}
        .mskey-window button.ve-btn-v:hover {{ background-color: #972530; border-color: #972530; }}
        .mskey-window checkbutton, .mskey-window radiobutton {{ min-height: 24px; }}
        .mskey-window checkbutton:hover, .mskey-window radiobutton:hover {{
            background-color: {hover}; border-radius: 4px;
        }}
        .mskey-window checkbutton check, .mskey-window radiobutton radio {{
            background-image: none; background-color: {field}; border-color: {subtle};
        }}
        .mskey-window checkbutton check:checked, .mskey-window radiobutton radio:checked {{
            background-color: #175BA6; color: #FFFFFF; border-color: #175BA6;
        }}

        .mskey-window .settings-tabs {{ border: none; background-color: transparent; }}
        .mskey-window .settings-tabs > header {{
            padding: 4px;
            background-color: {surface};
            background-image: none;
            border: 1px solid {edge};
            border-radius: 8px;
        }}
        .mskey-window .settings-tabs > header > tabs > tab {{
            color: {subtle};
            padding: 4px 16px;
            min-height: 24px;
            margin: 0 4px 0 0;
            border: none;
            border-radius: 6px;
            box-shadow: none;
        }}
        .mskey-window .settings-tabs > header > tabs > tab:hover {{ background-color: {hover}; }}
        .mskey-window .settings-tabs > header > tabs > tab:checked {{
            color: {accent};
            background-color: {selected};
            font-weight: 700;
        }}
        "#
    )
}
