use std::env;
use std::process::Command;

use serde_json::Value;

use crate::config_store::detect_active_x11_app;

pub fn is_browser_app(app: &str) -> bool {
    let lower = app.to_ascii_lowercase();
    [
        "chrome",
        "chromium",
        "firefox",
        "brave",
        "msedge",
        "edge",
        "vivaldi",
        "opera",
        "zen",
        "librewolf",
    ]
    .iter()
    .any(|k| lower.contains(k))
}

pub fn is_terminal_app(app: &str) -> bool {
    let lower = app.to_ascii_lowercase();
    [
        "alacritty",
        "kitty",
        "wezterm",
        "gnome-terminal",
        "konsole",
        "foot",
        "xfce4-terminal",
        "tilix",
        "terminator",
        "urxvt",
        "xterm",
    ]
    .iter()
    .any(|k| lower.contains(k))
}

pub fn parse_hypr_active_class(raw: &str) -> Option<String> {
    let val: Value = serde_json::from_str(raw).ok()?;
    for key in ["class", "initialClass"] {
        if let Some(s) = val.get(key).and_then(|v| v.as_str()) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn find_sway_focused(node: &Value) -> Option<String> {
    if node.get("focused").and_then(|v| v.as_bool()) == Some(true) {
        if let Some(id) = node.get("app_id").and_then(|v| v.as_str()) {
            let trimmed = id.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if let Some(cls) = node
            .get("window_properties")
            .and_then(|w| w.get("class"))
            .and_then(|v| v.as_str())
        {
            let trimmed = cls.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(arr) = node.get(key).and_then(|v| v.as_array()) {
            for child in arr {
                if let Some(found) = find_sway_focused(child) {
                    return Some(found);
                }
            }
        }
    }
    None
}

pub fn parse_sway_focused_app(raw: &str) -> Option<String> {
    let val: Value = serde_json::from_str(raw).ok()?;
    find_sway_focused(&val)
}

pub fn parse_gnome_eval_class(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let rest = trimmed.strip_prefix("(true,")?.trim_end_matches(')').trim();
    let unquoted = rest.trim_matches('\'').trim_matches('"').trim();
    if unquoted.is_empty() || unquoted == "null" || unquoted == "undefined" {
        None
    } else {
        Some(unquoted.to_string())
    }
}

pub fn parse_setxkbmap_layout(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("layout:") {
            let first = rest.trim().split(',').next()?.trim();
            let base = first.split(['(', '+']).next()?.trim();
            if !base.is_empty() {
                return Some(base.to_string());
            }
        }
    }
    None
}

pub fn parse_gnome_mru_xkb(raw: &str) -> Option<String> {
    let start = raw.find("('")?;
    let rest = &raw[start + 2..];
    let q1 = rest.find('\'')?;
    let kind = &rest[..q1];
    if kind.eq_ignore_ascii_case("ibus") {
        return Some("us".to_string());
    }
    let after = &rest[q1 + 1..];
    let s2 = after.find('\'')?;
    let rem = &after[s2 + 1..];
    let e2 = rem.find('\'')?;
    let layout = rem[..e2].trim();
    let base = layout.split(['+', '(']).next()?.trim();
    if base.is_empty() {
        None
    } else {
        Some(base.to_string())
    }
}

pub fn is_us_xkb_layout(layout: &str) -> bool {
    let base = layout.trim().split(['(', '+', ',']).next().unwrap_or("").trim();
    matches!(base.to_ascii_lowercase().as_str(), "us" | "en")
}

pub fn detect_active_app() -> Option<String> {
    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        if let Ok(out) = Command::new("hyprctl").args(["activewindow", "-j"]).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Some(cls) = parse_hypr_active_class(&s) {
                    return Some(cls);
                }
            }
        }
    }
    if env::var_os("SWAYSOCK").is_some() {
        if let Ok(out) = Command::new("swaymsg").args(["-t", "get_tree"]).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Some(cls) = parse_sway_focused_app(&s) {
                    return Some(cls);
                }
            }
        }
    }
    if env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_ascii_lowercase().contains("gnome"))
        .unwrap_or(false)
    {
        if let Ok(out) = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.gnome.Shell",
                "--object-path",
                "/org/gnome/Shell",
                "--method",
                "org.gnome.Shell.Eval",
                "global.display.focus_window?.get_wm_class()",
            ])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Some(cls) = parse_gnome_eval_class(&s) {
                    return Some(cls);
                }
            }
        }
    }
    detect_active_x11_app()
}

pub fn is_us_keyboard_layout() -> bool {
    if env::var_os("DISPLAY").is_some() {
        if let Ok(out) = Command::new("setxkbmap").arg("-query").output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Some(layout) = parse_setxkbmap_layout(&s) {
                    return is_us_xkb_layout(&layout);
                }
            }
        }
    }
    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.input-sources", "mru-sources"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(layout) = parse_gnome_mru_xkb(&s) {
                return is_us_xkb_layout(&layout);
            }
        }
    }
    true
}

#[cfg(target_os = "linux")]
mod linux_gui {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions};
    use std::os::fd::AsRawFd;
    use std::os::raw::c_int;
    use std::process::Command;
    use std::ptr;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::config_store::config_dir;
    use crate::gio_ffi::{cstr, g_source_remove, g_timeout_add};
    use crate::gtk_ffi::*;
    use crate::gtk_helpers::{read_clipboard_gtk, write_clipboard_gtk};

    unsafe extern "C" {
        fn flock(fd: c_int, operation: c_int) -> c_int;
    }

    struct OsdWidgets {
        win: *mut GtkWidget,
        label: *mut GtkWidget,
        css_loaded: bool,
        timer_id: u32,
    }

    thread_local! {
        static OSD: RefCell<Option<OsdWidgets>> = const { RefCell::new(None) };
    }

    static CLIPBOARD_GEN: AtomicU64 = AtomicU64::new(0);

    pub fn ensure_gtk_init() -> bool {
        static INIT: OnceLock<bool> = OnceLock::new();
        *INIT.get_or_init(|| unsafe { gtk_init_check(ptr::null_mut(), ptr::null_mut()) != 0 })
    }

    pub fn flush_gtk_events() {
        if !ensure_gtk_init() {
            return;
        }
        unsafe {
            for _ in 0..64 {
                if gtk_events_pending() == 0 {
                    break;
                }
                gtk_main_iteration_do(0);
            }
        }
    }

    pub fn acquire_setup_lock() -> Option<File> {
        let dir = config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("setup.lock");
        let file = OpenOptions::new().create(true).read(true).write(true).open(path).ok()?;
        let rc = unsafe { flock(file.as_raw_fd(), 2 | 4) };
        if rc == 0 { Some(file) } else { None }
    }

    pub fn trigger_paste_keystroke(shift_insert: bool) {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        if wayland {
            let mut cmd = Command::new("wtype");
            if shift_insert {
                cmd.args(["-M", "shift", "-k", "Insert", "-m", "shift"]);
            } else {
                cmd.args(["-M", "ctrl", "v", "-m", "ctrl"]);
            }
            if let Ok(mut child) = cmd.spawn() {
                let _ = child.wait();
                return;
            }
        }
        let combo = if shift_insert { "Shift+Insert" } else { "ctrl+v" };
        if let Ok(mut child) = Command::new("xdotool")
            .args(["key", "--clearmodifiers", combo])
            .spawn()
        {
            let _ = child.wait();
        }
    }

    unsafe extern "C" fn restore_clipboard_cb(data: *mut c_void) -> c_int {
        if data.is_null() {
            return 0;
        }
        let boxed = unsafe { Box::from_raw(data as *mut (u64, String)) };
        if CLIPBOARD_GEN.load(Ordering::Relaxed) == boxed.0 {
            write_clipboard_gtk(&boxed.1);
            unsafe {
                let display = gdk_display_get_default();
                let atom = gdk_atom_intern_static_string(cstr("CLIPBOARD").as_ptr());
                if !display.is_null() && !atom.is_null() {
                    let cb = gtk_clipboard_get_for_display(display, atom);
                    if !cb.is_null() {
                        gtk_clipboard_store(cb);
                    }
                }
            }
            flush_gtk_events();
        }
        0
    }

    pub fn apply_clipboard_paste_linux(text: &str, shift_insert: bool, delay_ms: u32) {
        if !ensure_gtk_init() {
            return;
        }
        let paste_gen = CLIPBOARD_GEN.fetch_add(1, Ordering::Relaxed) + 1;
        let previous = read_clipboard_gtk();
        write_clipboard_gtk(text);
        unsafe {
            let display = gdk_display_get_default();
            let atom = gdk_atom_intern_static_string(cstr("CLIPBOARD").as_ptr());
            if !display.is_null() && !atom.is_null() {
                let cb = gtk_clipboard_get_for_display(display, atom);
                if !cb.is_null() {
                    gtk_clipboard_store(cb);
                }
            }
        }
        flush_gtk_events();
        trigger_paste_keystroke(shift_insert);
        flush_gtk_events();
        if !previous.is_empty() {
            let payload = Box::into_raw(Box::new((paste_gen, previous)));
            let wait = 95 + delay_ms.min(500);
            unsafe {
                g_timeout_add(wait, Some(restore_clipboard_cb), payload as *mut c_void);
            }
        }
    }

    unsafe extern "C" fn hide_osd_cb(_: *mut c_void) -> c_int {
        OSD.with(|cell| {
            if let Some(ref mut osd) = *cell.borrow_mut() {
                osd.timer_id = 0;
                unsafe {
                    gtk_widget_hide(osd.win);
                }
            }
        });
        0
    }

    fn init_osd_window() -> OsdWidgets {
        unsafe {
            let win = gtk_window_new(GTK_WINDOW_POPUP);
            gtk_window_set_decorated(win, 0);
            gtk_window_set_keep_above(win, 1);
            gtk_window_set_accept_focus(win, 0);
            gtk_window_set_skip_taskbar_hint(win, 1);
            gtk_window_set_skip_pager_hint(win, 1);
            gtk_window_set_default_size(win, 168, 38);
            gtk_container_set_border_width(win, 8);
            let ctx = gtk_widget_get_style_context(win);
            gtk_style_context_add_class(ctx, cstr("mskey-osd").as_ptr());

            let lbl = gtk_label_new(cstr("V  |  Tiếng Việt").as_ptr());
            gtk_label_set_xalign(lbl, 0.5);
            let lctx = gtk_widget_get_style_context(lbl);
            gtk_style_context_add_class(lctx, cstr("mskey-osd-text").as_ptr());
            gtk_container_add(win, lbl);

            let css = gtk_css_provider_new();
            let rules = "window.mskey-osd { border-radius: 10px; border: 1px solid #0e7490; background: #0f172a; }
                         window.mskey-osd.osd-en { border-color: #d97706; background: #1e293b; }
                         .mskey-osd-text { font-weight: 700; font-size: 13px; color: #f8fafc; padding: 2px 8px; }";
            let rc = cstr(rules);
            gtk_css_provider_load_from_data(css, rc.as_ptr(), -1, ptr::null_mut());
            let screen = gdk_screen_get_default();
            if !screen.is_null() {
                gtk_style_context_add_provider_for_screen(
                    screen,
                    css,
                    GTK_STYLE_PROVIDER_PRIORITY_APPLICATION + 10,
                );
            }
            OsdWidgets {
                win,
                label: lbl,
                css_loaded: true,
                timer_id: 0,
            }
        }
    }

    pub fn show_linux_switch_osd(vietnamese: bool, anchor: Option<(i32, i32)>) {
        if !ensure_gtk_init() {
            return;
        }
        OSD.with(|cell| {
            let mut opt = cell.borrow_mut();
            if opt.is_none() {
                *opt = Some(init_osd_window());
            }
            if let Some(ref mut osd) = *opt {
                if osd.timer_id != 0 {
                    unsafe {
                        g_source_remove(osd.timer_id);
                    }
                    osd.timer_id = 0;
                }
                let text = if vietnamese {
                    "V  |  Tiếng Việt"
                } else {
                    "E  |  English"
                };
                unsafe {
                    let _ = osd.css_loaded;
                    gtk_label_set_text(osd.label, cstr(text).as_ptr());
                    let ctx = gtk_widget_get_style_context(osd.win);
                    let cls = cstr("osd-en");
                    if vietnamese {
                        gtk_style_context_remove_class(ctx, cls.as_ptr());
                    } else {
                        gtk_style_context_add_class(ctx, cls.as_ptr());
                    }
                    let screen = gdk_screen_get_default();
                    let sw = if !screen.is_null() {
                        gdk_screen_get_width(screen)
                    } else {
                        1280
                    };
                    let sh = if !screen.is_null() {
                        gdk_screen_get_height(screen)
                    } else {
                        800
                    };
                    let (x, y) = match anchor {
                        Some((ax, ay)) if ax > 0 || ay > 0 => {
                            ((ax + 8).clamp(16, (sw - 188).max(16)), (ay + 28).clamp(16, (sh - 60).max(16)))
                        }
                        _ => ((sw - 196).max(24), 48),
                    };
                    gtk_window_move(osd.win, x, y);
                    gtk_widget_show_all(osd.win);
                    osd.timer_id = g_timeout_add(950, Some(hide_osd_cb), ptr::null_mut());
                }
            }
        });
    }
}

#[cfg(target_os = "linux")]
pub use linux_gui::{
    acquire_setup_lock, apply_clipboard_paste_linux, ensure_gtk_init, flush_gtk_events,
    show_linux_switch_osd,
};
