use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::fs;
use std::os::raw::c_int;
use std::process::Command;
use std::ptr;
use std::thread;

use mskey_core::{AppBehavior, Config, HotkeyCombo, SwitchPreset};

use crate::config_store::{config_dir, load_config, save_config, sync_linux_autostart};
use crate::gio_ffi::{cstr, g_idle_add, gdk_beep};
use crate::gtk_dialogs::{
    capture_hotkey_dialog, pick_and_import_evkey_tree, pick_sync_file_path, show_converter_dialog,
};
use crate::gtk_dialogs_edit::{show_app_rules_dialog, show_custom_keymap_dialog, show_macro_dialog};
use crate::gtk_extras::{show_extras_sandbox_dialog, show_user_dict_dialog};
use crate::gtk_ffi::{
    GTK_WIN_POS_CENTER, GTK_WINDOW_TOPLEVEL, GtkAllocation, GtkWidget, connect_signal,
    gtk_container_add, gtk_container_set_border_width, gtk_init_check, gtk_main, gtk_main_quit,
    gtk_notebook_append_page, gtk_notebook_new, gtk_widget_destroy, gtk_widget_get_allocation,
    gtk_widget_set_sensitive, gtk_widget_show_all, gtk_window_new, gtk_window_set_default_size,
    gtk_window_set_position, gtk_window_set_resizable, gtk_window_set_title,
};
use crate::gtk_helpers::{
    create_modal_gtk, get_combo, make_button, make_hbox, make_label, make_vbox, pack, read_entry,
    run_modal_gtk, set_label_text, show_info_modal, write_entry,
};
use crate::gtk_theme::apply_gtk_theme;
use crate::gtk_ui_tabs::{UiState, build_header_box, build_tab_basic, build_tab_hotkey, build_tab_system};
use crate::linux_extra::{
    LinuxInstallOutcome, acquire_setup_lock, apply_linux_staged_update, check_latest_release,
    detect_active_app, download_and_verify_linux_update, format_update_check_message,
    select_linux_update_asset, show_linux_switch_osd,
};

thread_local! {
    static UI: RefCell<Option<UiState>> = const { RefCell::new(None) };
    static LAST_APP: RefCell<String> = const { RefCell::new(String::new()) };
    static IS_INIT: Cell<bool> = const { Cell::new(false) };
    static SYNCING: Cell<bool> = const { Cell::new(false) };
    static UPDATE_MODAL_CHOICE: Cell<u8> = const { Cell::new(0) };
}

pub fn ensure_gtk_init() -> bool {
    if IS_INIT.with(|c| c.get()) {
        return true;
    }
    let ok = unsafe { gtk_init_check(ptr::null_mut(), ptr::null_mut()) != 0 };
    if ok {
        IS_INIT.with(|c| c.set(true));
        crate::gtk_branding::initialize_brand_icon();
    }
    ok
}

pub fn run_converter_only() -> i32 {
    if !ensure_gtk_init() {
        eprintln!("mskey-linux: no GTK display available for converter");
        return 1;
    }
    let cfg = load_config();
    apply_gtk_theme(cfg.theme);
    show_converter_dialog(ptr::null_mut());
    0
}

pub fn run_setup_ui(smoke_out: Option<&str>) -> i32 {
    let _lock = if smoke_out.is_none() {
        let Some(lock) = acquire_setup_lock() else {
            return 0;
        };
        Some(lock)
    } else {
        None
    };
    if !ensure_gtk_init() {
        if let Some(p) = smoke_out {
            let _ = fs::write(p, "client=560x568\nheadless=no_display\n");
        }
        eprintln!("mskey-linux: no GTK display available");
        return if smoke_out.is_some() { 0 } else { 1 };
    }
    let cfg = load_config();
    apply_gtk_theme(cfg.theme);
    LAST_APP.with(|c| *c.borrow_mut() = detect_active_app().unwrap_or_else(|| "-".to_string()));
    let check_updates_start = cfg.check_updates_on_start;
    let st = build_main_window(cfg, smoke_out.is_none());
    let win = st.win;
    UI.with(|c| {
        *c.borrow_mut() = Some(st);
    });
    sync_widgets_from_config();
    unsafe { gtk_widget_show_all(win) };

    if let Some(p) = smoke_out {
        let mut alloc = GtkAllocation::default();
        unsafe { gtk_widget_get_allocation(win, &mut alloc) };
        let _ = fs::write(p, format!("client={}x{}\ngtk_realized=true\n", alloc.width, alloc.height));
        unsafe { gtk_widget_destroy(win) };
        UI.with(|c| {
            *c.borrow_mut() = None;
        });
        return 0;
    }

    if check_updates_start {
        start_async_update_check(false);
    }

    connect_signal(win, "destroy", on_main_destroy as *const c_void, 0);
    unsafe { gtk_main() };
    0
}

fn build_main_window(cfg: Config, animate: bool) -> UiState {
    let title = cstr(&format!("MS Key {}", env!("CARGO_PKG_VERSION")));
    let win = unsafe {
        let w = gtk_window_new(GTK_WINDOW_TOPLEVEL);
        gtk_window_set_title(w, title.as_ptr());
        gtk_window_set_default_size(w, 600, 690);
        gtk_window_set_resizable(w, 1);
        gtk_window_set_position(w, GTK_WIN_POS_CENTER);
        gtk_container_set_border_width(w, 12);
        w
    };
    let mut st = UiState::empty(win, cfg);
    let root = make_vbox(8);
    pack(root, crate::gtk_branding::make_brand_header(animate), false, false, 0);
    pack(root, build_header_box(&mut st), false, false, 0);

    let nb = unsafe { gtk_notebook_new() };
    let p0 = build_tab_basic(&mut st);
    let p1 = build_tab_hotkey(&mut st);
    let p2 = build_tab_system(&mut st);
    unsafe {
        gtk_notebook_append_page(nb, p0, make_label("Cơ bản", false));
        gtk_notebook_append_page(nb, p1, make_label("Phím tắt", false));
        gtk_notebook_append_page(nb, p2, make_label("Hệ thống", false));
    }
    pack(root, nb, true, true, 0);

    let footer = make_hbox(8);
    st.btn_default = make_button("Th.Lập mặc định", false);
    st.btn_save = make_button("Lưu", true);
    let lbl_ver = make_label("MS Key • Tiếng Việt Unicode (IBus)", true);
    pack(footer, st.btn_default, false, false, 0);
    pack(footer, st.btn_save, false, false, 0);
    pack(footer, make_label("", false), true, true, 0);
    pack(footer, lbl_ver, false, false, 0);
    pack(root, footer, false, false, 0);
    unsafe { gtk_container_add(win, root) };
    connect_main_signals(&st);
    st
}

fn connect_main_signals(st: &UiState) {
    let toggles = [
        st.chk_modern,
        st.chk_free_tone,
        st.chk_fjwz,
        st.chk_autocap,
        st.chk_smart_code,
        st.chk_clip_send,
        st.chk_std_send,
        st.chk_spell,
        st.chk_restore,
        st.chk_reedit,
        st.chk_non_us,
        st.chk_suggest1,
        st.chk_suggest2,
        st.chk_caret_osd,
        st.chk_macro,
        st.chk_macro_en,
        st.chk_macro_tab,
        st.chk_quick_w,
        st.chk_quick_brk,
        st.chk_esc,
        st.chk_beep,
        st.chk_caps_switch,
        st.chk_autostart,
        st.chk_show_start,
        st.chk_check_updates,
        st.chk_smart_app,
        st.rad_ctrl_shift,
        st.rad_alt_z,
        st.rad_ctrl_space,
        st.rad_shift_shift,
        st.rad_custom,
    ];
    for &w in &toggles {
        connect_signal(w, "toggled", on_widget_changed as *const c_void, 0);
    }
    for &cb in &[st.cb_charset, st.cb_method, st.cb_mech, st.cb_theme, st.cb_hold_en] {
        connect_signal(cb, "changed", on_widget_changed as *const c_void, 0);
    }
    connect_signal(st.cb_profile, "changed", on_profile_changed as *const c_void, 0);
    connect_signal(st.btn_save_profile, "clicked", on_save_profile as *const c_void, 0);
    connect_signal(st.btn_ve, "clicked", on_toggle_ve as *const c_void, 0);
    connect_signal(st.btn_custom, "clicked", on_open_custom as *const c_void, 0);
    connect_signal(st.btn_conv, "clicked", on_open_convert as *const c_void, 0);
    connect_signal(st.btn_macro, "clicked", on_open_macro as *const c_void, 0);
    connect_signal(st.btn_cap_sw, "clicked", on_cap_switch as *const c_void, 0);
    connect_signal(st.btn_cap_conv, "clicked", on_cap_conv as *const c_void, 0);
    connect_signal(st.btn_cap_sel_conv, "clicked", on_cap_sel_conv as *const c_void, 0);
    connect_signal(st.btn_cap_undo, "clicked", on_cap_undo as *const c_void, 0);
    connect_signal(st.btn_import, "clicked", on_import_tree as *const c_void, 0);
    connect_signal(st.btn_dir, "clicked", on_open_dir as *const c_void, 0);
    connect_signal(st.btn_sync_pick, "clicked", on_sync_pick as *const c_void, 0);
    connect_signal(st.btn_sync_clear, "clicked", on_sync_clear as *const c_void, 0);
    connect_signal(st.ed_sync_path, "activate", on_widget_changed as *const c_void, 0);
    connect_signal(st.btn_check_update, "clicked", on_check_update as *const c_void, 0);
    connect_signal(st.btn_apps, "clicked", on_open_apps as *const c_void, 0);
    connect_signal(st.btn_add_app, "clicked", on_add_cur_app as *const c_void, 0);
    connect_signal(st.btn_extras, "clicked", on_open_extras as *const c_void, 0);
    connect_signal(st.btn_user_dict, "clicked", on_open_user_dict as *const c_void, 0);
    connect_signal(st.btn_default, "clicked", on_reset_default as *const c_void, 0);
    connect_signal(st.btn_save, "clicked", on_save_cfg as *const c_void, 0);
    connect_signal(st.btn_close, "clicked", on_close_win as *const c_void, 0);
    connect_signal(st.btn_exit, "clicked", on_close_win as *const c_void, 0);
}

fn sync_widgets_from_config() {
    let cur_app = LAST_APP.with(|c| c.borrow().clone());
    SYNCING.with(|s| s.set(true));
    UI.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.sync_from_config(&cur_app);
        }
    });
    SYNCING.with(|s| s.set(false));
}

fn read_widgets_to_config() -> Option<mskey_core::ThemeMode> {
    if SYNCING.with(|s| s.get()) {
        return None;
    }
    UI.with(|cell| {
        let mut b = cell.borrow_mut();
        let st = b.as_mut()?;
        if st.syncing {
            return None;
        }
        let changed = st.read_to_config();
        save_config(&st.cfg);
        changed
    })
}

unsafe extern "C" fn on_widget_changed(_: *mut GtkWidget, _: *mut c_void) {
    if let Some(t) = read_widgets_to_config() {
        apply_gtk_theme(t);
    }
}

unsafe extern "C" fn on_toggle_ve(_: *mut GtkWidget, _: *mut c_void) {
    let flags = UI.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            st.cfg.vietnamese_enabled = !st.cfg.vietnamese_enabled;
            save_config(&st.cfg);
            return Some((st.cfg.vietnamese_enabled, st.cfg.beep_on_switch, st.cfg.show_switch_osd));
        }
        None
    });
    sync_widgets_from_config();
    if let Some((next, beep, osd)) = flags {
        if beep {
            unsafe { gdk_beep() };
        }
        if osd {
            show_linux_switch_osd(next, None);
        }
    }
}

unsafe extern "C" fn on_open_extras(_: *mut GtkWidget, _: *mut c_void) {
    let pair = UI.with(|c| c.borrow().as_ref().map(|s| (s.win, s.cfg.clone())));
    if let Some((win, mut cfg)) = pair
        && show_extras_sandbox_dialog(win, &mut cfg) {
            save_config(&cfg);
            apply_gtk_theme(cfg.theme);
            UI.with(|c| {
                if let Some(st) = c.borrow_mut().as_mut() {
                    st.cfg = cfg;
                }
            });
            sync_widgets_from_config();
        }
}

unsafe extern "C" fn on_open_user_dict(_: *mut GtkWidget, _: *mut c_void) {
    let pair = UI.with(|c| c.borrow().as_ref().map(|s| (s.win, s.cfg.clone())));
    if let Some((win, mut cfg)) = pair
        && show_user_dict_dialog(win, &mut cfg) {
            save_config(&cfg);
            UI.with(|c| {
                if let Some(st) = c.borrow_mut().as_mut() {
                    st.cfg = cfg;
                }
            });
        }
}

unsafe extern "C" fn on_open_convert(_: *mut GtkWidget, _: *mut c_void) {
    let win = UI.with(|c| c.borrow().as_ref().map(|s| s.win).unwrap_or(ptr::null_mut()));
    show_converter_dialog(win);
}

unsafe extern "C" fn on_open_custom(_: *mut GtkWidget, _: *mut c_void) {
    let pair = UI.with(|c| c.borrow().as_ref().map(|s| (s.win, s.cfg.clone())));
    if let Some((win, mut cfg)) = pair
        && show_custom_keymap_dialog(win, &mut cfg) {
            save_config(&cfg);
            UI.with(|c| {
                if let Some(st) = c.borrow_mut().as_mut() {
                    st.cfg = cfg;
                }
            });
            sync_widgets_from_config();
        }
}

unsafe extern "C" fn on_open_macro(_: *mut GtkWidget, _: *mut c_void) {
    let pair = UI.with(|c| c.borrow().as_ref().map(|s| (s.win, s.cfg.clone())));
    if let Some((win, mut cfg)) = pair
        && show_macro_dialog(win, &mut cfg) {
            save_config(&cfg);
            UI.with(|c| {
                if let Some(st) = c.borrow_mut().as_mut() {
                    st.cfg = cfg;
                }
            });
        }
}

unsafe extern "C" fn on_open_apps(_: *mut GtkWidget, _: *mut c_void) {
    let pair = UI.with(|c| c.borrow().as_ref().map(|s| (s.win, s.cfg.clone())));
    if let Some((win, mut cfg)) = pair
        && show_app_rules_dialog(win, &mut cfg) {
            save_config(&cfg);
            UI.with(|c| {
                if let Some(st) = c.borrow_mut().as_mut() {
                    st.cfg = cfg;
                }
            });
        }
}

unsafe extern "C" fn on_add_cur_app(_: *mut GtkWidget, _: *mut c_void) {
    let app = LAST_APP.with(|c| c.borrow().clone());
    if app.is_empty() || app == "-" {
        return;
    }
    UI.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            st.cfg.set_app_rule(&app, AppBehavior::ForceEnglish);
            save_config(&st.cfg);
        }
    });
}

fn capture_and_apply_hotkey(
    extract: impl Fn(&Config) -> HotkeyCombo,
    apply: impl Fn(&mut Config, HotkeyCombo),
) {
    let pair = UI.with(|c| c.borrow().as_ref().map(|s| (s.win, extract(&s.cfg))));
    if let Some((win, cur)) = pair
        && let Some(combo) = capture_hotkey_dialog(win, cur)
    {
        UI.with(|c| {
            if let Some(st) = c.borrow_mut().as_mut() {
                apply(&mut st.cfg, combo);
                save_config(&st.cfg);
            }
        });
        sync_widgets_from_config();
    }
}

unsafe extern "C" fn on_cap_switch(_: *mut GtkWidget, _: *mut c_void) {
    capture_and_apply_hotkey(
        |cfg| cfg.custom_switch_hotkey,
        |cfg, combo| {
            cfg.custom_switch_hotkey = combo;
            cfg.switch_preset = SwitchPreset::Custom;
        },
    );
}

unsafe extern "C" fn on_cap_conv(_: *mut GtkWidget, _: *mut c_void) {
    capture_and_apply_hotkey(
        |cfg| cfg.clipboard_convert_hotkey,
        |cfg, combo| {
            cfg.clipboard_convert_hotkey = combo;
        },
    );
}

unsafe extern "C" fn on_cap_sel_conv(_: *mut GtkWidget, _: *mut c_void) {
    capture_and_apply_hotkey(
        |cfg| cfg.convert_selection_hotkey,
        |cfg, combo| {
            cfg.convert_selection_hotkey = combo;
        },
    );
}

unsafe extern "C" fn on_cap_undo(_: *mut GtkWidget, _: *mut c_void) {
    capture_and_apply_hotkey(
        |cfg| cfg.undo_last_word_hotkey,
        |cfg, combo| {
            cfg.undo_last_word_hotkey = combo;
        },
    );
}

unsafe extern "C" fn on_profile_changed(_: *mut GtkWidget, _: *mut c_void) {
    if SYNCING.with(|s| s.get()) {
        return;
    }
    let changed = UI.with(|c| {
        let mut b = c.borrow_mut();
        let st = b.as_mut()?;
        if st.syncing {
            return None;
        }
        let idx = get_combo(st.cb_profile);
        let names = st.cfg.all_profile_names();
        let name = names.get(idx)?.clone();
        if st.cfg.apply_profile(&name) {
            save_config(&st.cfg);
            return Some(());
        }
        None
    });
    if changed.is_some() {
        sync_widgets_from_config();
    }
}

unsafe extern "C" fn on_save_profile(_: *mut GtkWidget, _: *mut c_void) {
    let _ = read_widgets_to_config();
    let updated = UI.with(|c| {
        let mut b = c.borrow_mut();
        let st = b.as_mut()?;
        let typed = read_entry(st.ed_profile).trim().to_string();
        let target = if !typed.is_empty() {
            typed
        } else {
            let names = st.cfg.all_profile_names();
            names
                .get(get_combo(st.cb_profile))
                .cloned()
                .unwrap_or_else(|| st.cfg.active_profile.clone())
        };
        if target.trim().is_empty() {
            return None;
        }
        st.cfg.save_current_as_profile(&target);
        save_config(&st.cfg);
        write_entry(st.ed_profile, "");
        Some(())
    });
    if updated.is_some() {
        sync_widgets_from_config();
    }
}

unsafe extern "C" fn on_sync_pick(_: *mut GtkWidget, _: *mut c_void) {
    let pair = UI.with(|c| {
        c.borrow()
            .as_ref()
            .map(|s| (s.win, s.cfg.sync_config_path.clone()))
    });
    if let Some((win, cur)) = pair
        && let Some(path) = pick_sync_file_path(win, &cur)
    {
        UI.with(|c| {
            if let Some(st) = c.borrow_mut().as_mut() {
                st.cfg.sync_config_path = path;
                save_config(&st.cfg);
            }
        });
        sync_widgets_from_config();
    }
}

unsafe extern "C" fn on_sync_clear(_: *mut GtkWidget, _: *mut c_void) {
    UI.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            st.cfg.sync_config_path.clear();
            save_config(&st.cfg);
        }
    });
    sync_widgets_from_config();
}

struct UpdateResultPayload {
    show_modal: bool,
    has_newer: bool,
    msg: String,
    release: Option<mskey_core::update::ReleaseInfo>,
}

struct UpdateInstallPayload {
    result: Result<LinuxInstallOutcome, String>,
    html_url: String,
}

unsafe extern "C" fn on_upd_choice_install(_: *mut GtkWidget, dlg: *mut c_void) {
    UPDATE_MODAL_CHOICE.with(|c| c.set(1));
    unsafe { gtk_widget_destroy(dlg as *mut GtkWidget) };
}

unsafe extern "C" fn on_upd_choice_github(_: *mut GtkWidget, dlg: *mut c_void) {
    UPDATE_MODAL_CHOICE.with(|c| c.set(2));
    unsafe { gtk_widget_destroy(dlg as *mut GtkWidget) };
}

unsafe extern "C" fn on_upd_choice_close(_: *mut GtkWidget, dlg: *mut c_void) {
    UPDATE_MODAL_CHOICE.with(|c| c.set(0));
    unsafe { gtk_widget_destroy(dlg as *mut GtkWidget) };
}

fn show_update_available_dialog(
    parent: *mut GtkWidget,
    info: &mskey_core::update::ReleaseInfo,
    asset: Option<&mskey_core::update::ReleaseAsset>,
) -> u8 {
    UPDATE_MODAL_CHOICE.with(|c| c.set(0));
    let dlg = create_modal_gtk(parent, "Cập nhật MS Key", 460, 210);
    let vbox = make_vbox(10);
    let mut text = format!(
        "Đã có phiên bản mới v{} (hiện tại v{}).",
        info.version,
        env!("CARGO_PKG_VERSION")
    );
    if let Some(a) = asset {
        let sz = mskey_core::update::format_bytes_human(a.size);
        let sz_part = if sz.is_empty() { String::new() } else { format!(" ({sz})") };
        text.push_str(&format!("\nGói: {}{}\nXác thực chữ ký SHA-256 tự động.", a.name, sz_part));
    }
    let notes = mskey_core::update::release_notes_preview(&info.body, 280);
    if !notes.is_empty() {
        text.push_str("\n\nGhi chú:\n");
        text.push_str(&notes);
    }
    let lbl = make_label(&text, false);
    pack(vbox, lbl, true, true, 0);

    let brow = make_hbox(8);
    let btn_close = make_button("Để sau", false);
    let btn_github = make_button("Mở trang phát hành", false);
    pack(brow, btn_close, false, false, 0);
    pack(brow, btn_github, false, false, 0);
    if asset.is_some() {
        let btn_install = make_button("Tải & Cập nhật", true);
        connect_signal(btn_install, "clicked", on_upd_choice_install as *const c_void, dlg as usize);
        pack(brow, btn_install, true, true, 0);
    }
    connect_signal(btn_github, "clicked", on_upd_choice_github as *const c_void, dlg as usize);
    connect_signal(btn_close, "clicked", on_upd_choice_close as *const c_void, dlg as usize);
    unsafe {
        gtk_container_add(dlg, vbox);
    }
    pack(vbox, brow, false, false, 0);
    run_modal_gtk(dlg);
    UPDATE_MODAL_CHOICE.with(|c| c.get())
}

unsafe extern "C" fn on_update_install_done(data: *mut c_void) -> c_int {
    if data.is_null() {
        return 0;
    }
    let payload = unsafe { Box::from_raw(data as *mut UpdateInstallPayload) };
    let (modal_win, close_win) = UI.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            unsafe { gtk_widget_set_sensitive(st.btn_check_update, 1) };
            match &payload.result {
                Ok(out) => {
                    set_label_text(st.lbl_update_status, "Kiểm tra/cập nhật hoàn tất");
                    return (Some(st.win), out.relaunched);
                }
                Err(err) => {
                    set_label_text(st.lbl_update_status, err);
                    return (Some(st.win), false);
                }
            }
        }
        (None, false)
    });
    if let Some(win) = modal_win {
        match payload.result {
            Ok(out) => {
                show_info_modal(win, "Cập nhật MS Key", &out.message);
                if close_win {
                    unsafe { gtk_widget_destroy(win) };
                }
            }
            Err(err) => {
                show_info_modal(
                    win,
                    "Lỗi cập nhật MS Key",
                    &format!("{err}\n\nTrang phát hành: {}", payload.html_url),
                );
            }
        }
    }
    0
}

fn start_async_update_download(
    info: mskey_core::update::ReleaseInfo,
    asset: mskey_core::update::ReleaseAsset,
    sums: mskey_core::update::ReleaseAsset,
) {
    UI.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            set_label_text(st.lbl_update_status, &format!("Đang tải {}...", asset.name));
            unsafe { gtk_widget_set_sensitive(st.btn_check_update, 0) };
        }
    });
    let html_url = if info.html_url.trim().is_empty() {
        mskey_core::update::GITHUB_RELEASES_URL.to_string()
    } else {
        info.html_url.clone()
    };
    thread::spawn(move || {
        let result = download_and_verify_linux_update(&info.version, &asset, &sums, None)
            .and_then(|staged| apply_linux_staged_update(&staged));
        let payload = Box::into_raw(Box::new(UpdateInstallPayload { result, html_url }));
        unsafe {
            g_idle_add(Some(on_update_install_done), payload as *mut c_void);
        }
    });
}

unsafe extern "C" fn on_update_check_done(data: *mut c_void) -> c_int {
    if data.is_null() {
        return 0;
    }
    let payload = unsafe { Box::from_raw(data as *mut UpdateResultPayload) };
    let modal_win = UI.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            set_label_text(st.lbl_update_status, &payload.msg);
            unsafe { gtk_widget_set_sensitive(st.btn_check_update, 1) };
            if payload.show_modal || payload.has_newer {
                return Some(st.win);
            }
        }
        None
    });
    if let Some(win) = modal_win {
        if payload.has_newer
            && let Some(info) = payload.release
        {
            let pair = select_linux_update_asset(&info);
            let choice = show_update_available_dialog(win, &info, pair.as_ref().map(|p| &p.0));
            if choice == 1
                && let Some((asset, sums)) = pair
            {
                start_async_update_download(info, asset, sums);
            } else if choice == 2 {
                let target = if info.html_url.trim().is_empty() {
                    mskey_core::update::GITHUB_RELEASES_URL
                } else {
                    info.html_url.trim()
                };
                let _ = Command::new("xdg-open").arg(target).spawn();
            }
        } else {
            show_info_modal(win, "Kiểm tra cập nhật", &payload.msg);
        }
    }
    0
}

fn start_async_update_check(show_modal: bool) {
    UI.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            set_label_text(st.lbl_update_status, "Đang kiểm tra cập nhật...");
            unsafe { gtk_widget_set_sensitive(st.btn_check_update, 0) };
        }
    });
    thread::spawn(move || {
        let res = check_latest_release();
        let release = res.as_ref().ok().cloned();
        let (has_newer, msg) = format_update_check_message(env!("CARGO_PKG_VERSION"), res);
        let payload = Box::into_raw(Box::new(UpdateResultPayload {
            show_modal,
            has_newer,
            msg,
            release: if has_newer { release } else { None },
        }));
        unsafe {
            g_idle_add(Some(on_update_check_done), payload as *mut c_void);
        }
    });
}

unsafe extern "C" fn on_check_update(_: *mut GtkWidget, _: *mut c_void) {
    start_async_update_check(true);
}

unsafe extern "C" fn on_import_tree(_: *mut GtkWidget, _: *mut c_void) {
    let win = UI.with(|c| c.borrow().as_ref().map(|s| s.win).unwrap_or(ptr::null_mut()));
    if let Some(cfg) = pick_and_import_evkey_tree(win) {
        save_config(&cfg);
        apply_gtk_theme(cfg.theme);
        UI.with(|c| {
            if let Some(st) = c.borrow_mut().as_mut() {
                st.cfg = cfg;
            }
        });
        sync_widgets_from_config();
    }
}

unsafe extern "C" fn on_open_dir(_: *mut GtkWidget, _: *mut c_void) {
    let dir = config_dir();
    let _ = fs::create_dir_all(&dir);
    let _ = Command::new("xdg-open").arg(dir).spawn();
}

unsafe extern "C" fn on_reset_default(_: *mut GtkWidget, _: *mut c_void) {
    let def = Config::default();
    save_config(&def);
    sync_linux_autostart(def.auto_start);
    apply_gtk_theme(def.theme);
    UI.with(|c| {
        if let Some(st) = c.borrow_mut().as_mut() {
            st.cfg = def;
        }
    });
    sync_widgets_from_config();
}

unsafe extern "C" fn on_save_cfg(_: *mut GtkWidget, _: *mut c_void) {
    let _ = read_widgets_to_config();
    UI.with(|c| {
        if let Some(st) = c.borrow().as_ref() {
            save_config(&st.cfg);
            sync_linux_autostart(st.cfg.auto_start);
        }
    });
}

unsafe extern "C" fn on_close_win(_: *mut GtkWidget, _: *mut c_void) {
    if let Some(win) = UI.with(|c| c.borrow().as_ref().map(|s| s.win)) {
        unsafe { gtk_widget_destroy(win) };
    }
}

unsafe extern "C" fn on_main_destroy(_: *mut GtkWidget, _: *mut c_void) {
    UI.with(|c| {
        *c.borrow_mut() = None;
    });
    unsafe { gtk_main_quit() };
}
