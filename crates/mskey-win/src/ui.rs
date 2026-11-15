use std::path::Path;
use std::ptr;

use mskey_core::{Charset, Config, InputMethod};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::AdjustWindowRectExForDpi;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetSystemMetrics, ICON_BIG, ICON_SMALL, MSG, PostQuitMessage, RegisterClassExW, SM_CXSCREEN,
    SM_CYSCREEN, SW_HIDE, SW_RESTORE, SW_SHOW, SendMessageW, SetForegroundWindow, SetWindowPos,
    ShowWindow, TranslateMessage, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORSTATIC, WM_DESTROY,
    WM_DPICHANGED, WM_DRAWITEM, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_PAINT, WM_RBUTTONUP, WM_SETICON,
    WNDCLASSEXW, WS_CAPTION, WS_MINIMIZEBOX, WS_SYSMENU, WM_TIMER, KillTimer, SetTimer,
    SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, IsWindowVisible,
};

use crate::dialog_convert::{capture_hotkey_dialog, show_converter_dialog};
use crate::dialog_custom::show_custom_keymap_dialog;
use crate::dialog_helpers::{scale_px, window_dpi};
use crate::dialogs::{show_app_rules_dialog, show_macro_dialog};
use crate::platform::{is_elevated, relaunch_as_admin, save_config, sync_startup_setting, to_wide};
use crate::state::{
    WM_APP_OPEN_CONVERT, WM_APP_OPEN_MAIN, WM_APP_SYNC_UI, WM_APP_TOGGLE_VE, WM_APP_TRAY,
    with_state,
};
use crate::theme::{apply_dwm_dark_titlebar, create_ui_font, palette_for};
use crate::tray::{
    ID_TRAY_APPS, ID_TRAY_CHARSET_COMP, ID_TRAY_CHARSET_TCVN, ID_TRAY_CHARSET_UNI,
    ID_TRAY_CHARSET_VNI, ID_TRAY_CONVERT, ID_TRAY_EXIT, ID_TRAY_MACROS, ID_TRAY_SHOW,
    ID_TRAY_TOGGLE_VE, ID_TRAY_TYPE_CUSTOM, ID_TRAY_TYPE_SIMPLE, ID_TRAY_TYPE_TELEX,
    ID_TRAY_TYPE_VIQR, ID_TRAY_TYPE_VNI, TrayController, show_tray_menu,
};
use crate::ui_actions::{
    add_current_app_force_english, dump_layout_report, import_evkey_tree_via_dialog,
    open_config_directory,
};
use crate::ui_controls::*;
use crate::ui_paint::{
    MainWinData, ctl_color_main, draw_main_item, paint_main_window, rebuild_theme_resources,
    refresh_ui_and_tray, set_main_data, set_tab_redraw, take_main_data, with_main_data,
};

const MAIN_W: i32 = 560;
const MAIN_H: i32 = 568;
const CBN_SELCHANGE: usize = 1;

pub fn create_and_run_main(show_initially: bool, verify_smoke_path: Option<&str>) {
    let hinst = unsafe { GetModuleHandleW(ptr::null()) };
    let cname = to_wide("MSKeyMainWindow");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0x0003,
        lpfnWndProc: Some(main_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: crate::tray::app_icon(32),
        hCursor: ptr::null_mut(),
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: cname.as_ptr(),
        hIconSm: crate::tray::app_icon(16),
    };
    unsafe {
        RegisterClassExW(&wc);
    }

    let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
    let mut rc = RECT { left: 0, top: 0, right: MAIN_W, bottom: MAIN_H };
    unsafe {
        AdjustWindowRectExForDpi(&mut rc, style, 0, 0, 96);
    }
    let win_w = rc.right - rc.left;
    let win_h = rc.bottom - rc.top;
    let sx = (unsafe { GetSystemMetrics(SM_CXSCREEN) } - win_w) / 2;
    let sy = (unsafe { GetSystemMetrics(SM_CYSCREEN) } - win_h) / 2;
    let title = to_wide(concat!("MS Key ", env!("CARGO_PKG_VERSION")));

    let hwnd = unsafe {
        CreateWindowExW(
            0,
            cname.as_ptr(),
            title.as_ptr(),
            style,
            sx.max(0),
            sy.max(0),
            win_w,
            win_h,
            ptr::null_mut(),
            ptr::null_mut(),
            hinst,
            ptr::null(),
        )
    };
    with_state(|st| st.main_hwnd = hwnd as usize);

    if let Some(smoke_out) = verify_smoke_path {
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
        }
        dump_layout_report(hwnd, Path::new(smoke_out));
        unsafe {
            DestroyWindow(hwnd);
        }
        return;
    }

    if show_initially {
        show_and_focus(hwnd);
    }

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn show_and_focus(hwnd: HWND) {
    let visible = unsafe { IsWindowVisible(hwnd) } != 0;
    let mut animate: i32 = 0;
    unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &mut animate as *mut _ as *mut _, 0);
    }
    if !visible && animate != 0 {
        crate::ui_paint::with_main_data_mut(|d| d.reveal_started = Some(std::time::Instant::now()));
        unsafe { SetTimer(hwnd, 92, 16, None) };
    }
    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
    }
}

fn on_main_create(hwnd: HWND) {
    let dpi = window_dpi(hwnd);
    let mut rc = RECT { left: 0, top: 0, right: scale_px(MAIN_W, dpi), bottom: scale_px(MAIN_H, dpi) };
    unsafe {
        AdjustWindowRectExForDpi(
            &mut rc,
            WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            0,
            0,
            dpi as u32,
        );
        SetWindowPos(hwnd, ptr::null_mut(), 0, 0, rc.right - rc.left, rc.bottom - rc.top, 0x0016);
    }
    let font = create_ui_font(scale_px(14, dpi), false);
    let bold_font = create_ui_font(scale_px(14, dpi), true);
    let mode = with_state(|st| st.engine.config().theme);
    let pal = palette_for(mode);
    apply_dwm_dark_titlebar(hwnd, pal.dark);
    let bg_brush = unsafe { CreateSolidBrush(pal.window_bg) };
    let card_brush = unsafe { CreateSolidBrush(pal.card_bg) };
    let in_brush = unsafe { CreateSolidBrush(pal.input_bg) };
    let ctrls = MainUiCtrls::build(hwnd, font, dpi);
    let mut tray = TrayController::new(hwnd);
    let ve = with_state(|st| st.engine.config().vietnamese_enabled);
    tray.update(ve);
    unsafe {
        SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, crate::tray::app_icon(32) as LPARAM);
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, crate::tray::app_icon(16) as LPARAM);
    }
    ctrls.sync_from_state();
    set_main_data(Some(MainWinData {
        font,
        bold_font,
        bg_brush,
        card_brush,
        in_brush,
        pal,
        ctrls,
        tray,
        reveal_started: None,
    }));
}

unsafe extern "system" fn main_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            on_main_create(hwnd);
            0
        }
        WM_PAINT => {
            paint_main_window(hwnd);
            0
        }
        WM_TIMER if wparam == 92 => {
            let done = crate::ui_paint::with_main_data_mut(|d| {
                if d.reveal_started.is_some_and(|start| start.elapsed().as_millis() >= 480) {
                    d.reveal_started = None;
                }
                d.reveal_started.is_none()
            }).unwrap_or(true);
            if done {
                unsafe { KillTimer(hwnd, 92) };
            }
            let dpi = window_dpi(hwnd);
            let rc = RECT { left: scale_px(450, dpi), top: scale_px(96, dpi), right: scale_px(548, dpi), bottom: scale_px(148, dpi) };
            unsafe { windows_sys::Win32::Graphics::Gdi::InvalidateRect(hwnd, &rc, 0) };
            0
        }
        WM_DRAWITEM => draw_main_item(hwnd, lparam).unwrap_or(0),
        WM_CTLCOLORSTATIC => ctl_color_main(wparam, lparam).unwrap_or(0),
        WM_COMMAND => {
            handle_command(hwnd, wparam & 0xFFFF, (wparam >> 16) & 0xFFFF);
            0
        }
        WM_APP_TRAY => {
            let ev = lparam as u32;
            if ev == WM_LBUTTONUP {
                if !SKIP_TRAY_CLICK.replace(false) {
                    toggle_ve_and_refresh(hwnd);
                }
            } else if ev == WM_LBUTTONDBLCLK {
                SKIP_TRAY_CLICK.set(true);
                let current = with_state(|st| st.engine.config().vietnamese_enabled);
                set_ve_and_refresh(hwnd, !current, false);
                show_and_focus(hwnd);
            } else if ev == WM_RBUTTONUP {
                let (ve, cs, m) = with_state(|st| {
                    let c = st.engine.config();
                    (c.vietnamese_enabled, c.charset, c.input_method)
                });
                show_tray_menu(hwnd, ve, cs, m);
            }
            0
        }
        WM_APP_SYNC_UI => {
            refresh_ui_and_tray(hwnd);
            0
        }
        WM_APP_TOGGLE_VE => {
            toggle_ve_and_refresh(hwnd);
            0
        }
        WM_APP_OPEN_MAIN => {
            show_and_focus(hwnd);
            0
        }
        WM_APP_OPEN_CONVERT => {
            show_converter_dialog(hwnd);
            0
        }
        crate::state::WM_APP_SHOW_OSD => {
            crate::ui_osd::show_switch_osd(wparam != 0);
            0
        }
        WM_DPICHANGED => {
            refresh_ui_and_tray(hwnd);
            0
        }
        WM_CLOSE => {
            unsafe { ShowWindow(hwnd, SW_HIDE) };
            0
        }
        WM_DESTROY => {
            if let Some(mut d) = take_main_data() {
                d.tray.remove();
                unsafe {
                    DeleteObject(d.font);
                    DeleteObject(d.bold_font);
                    DeleteObject(d.bg_brush);
                    DeleteObject(d.card_brush);
                    DeleteObject(d.in_brush);
                }
            }
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

thread_local! {
    static SKIP_TRAY_CLICK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn toggle_ve_and_refresh(hwnd: HWND) {
    let next = with_state(|st| !st.engine.config().vietnamese_enabled);
    set_ve_and_refresh(hwnd, next, true);
}

fn set_ve_and_refresh(hwnd: HWND, next: bool, announce: bool) {
    let (beep, osd, fg_proc, smart) = with_state(|st| {
        let c = st.engine.config().clone();
        st.set_user_vietnamese(next);
        save_config(st.engine.config());
        (
            c.beep_on_switch && announce,
            c.show_switch_osd && announce,
            st.fg_process.clone(),
            c.smart_app_switch,
        )
    });
    if smart && !fg_proc.is_empty() {
        crate::platform_extra::record_app_ve_memory(&fg_proc, next);
    }
    if beep {
        crate::platform::play_switch_beep();
    }
    if osd {
        crate::ui_osd::show_switch_osd(next);
    }
    refresh_ui_and_tray(hwnd);
}

fn handle_command(hwnd: HWND, id: usize, code: usize) {
    match id {
        IDC_TAB_BASIC => set_tab_redraw(hwnd, 0),
        IDC_TAB_HOTKEY => set_tab_redraw(hwnd, 1),
        IDC_TAB_SYSTEM => set_tab_redraw(hwnd, 2),
        IDC_BTN_CLOSE => unsafe {
            ShowWindow(hwnd, SW_HIDE);
        },
        IDC_BTN_EXIT | ID_TRAY_EXIT => unsafe {
            DestroyWindow(hwnd);
        },
        ID_TRAY_SHOW => show_and_focus(hwnd),
        IDC_VE_PILL | ID_TRAY_TOGGLE_VE => toggle_ve_and_refresh(hwnd),
        IDC_BTN_CUSTOM_KEY => {
            show_custom_keymap_dialog(hwnd);
            refresh_ui_and_tray(hwnd);
        }
        IDC_BTN_MACRO | ID_TRAY_MACROS => show_macro_dialog(hwnd),
        IDC_BTN_CONVERT | ID_TRAY_CONVERT => show_converter_dialog(hwnd),
        IDC_BTN_APPS | ID_TRAY_APPS => show_app_rules_dialog(hwnd),
        IDC_BTN_EXTRAS | crate::tray::ID_TRAY_EXTRAS => {
            if crate::ui_extras::show_extras_sandbox_dialog(hwnd) {
                rebuild_theme_resources(hwnd);
                refresh_ui_and_tray(hwnd);
            }
        }
        IDC_BTN_USER_DICT | crate::tray::ID_TRAY_USER_DICT => {
            crate::ui_extras::show_user_dict_dialog(hwnd);
        }
        crate::tray::ID_TRAY_QUICK_TELEX | crate::tray::ID_TRAY_SHOW_OSD => {
            with_state(|st| {
                let mut cfg = st.engine.config().clone();
                if id == crate::tray::ID_TRAY_QUICK_TELEX {
                    cfg.quick_telex = !cfg.quick_telex;
                } else {
                    cfg.show_switch_osd = !cfg.show_switch_osd;
                }
                save_config(&cfg);
                st.engine.update_config(cfg);
            });
            refresh_ui_and_tray(hwnd);
        }
        IDC_BTN_ADD_CUR_EN => {
            add_current_app_force_english();
            refresh_ui_and_tray(hwnd);
        }
        IDC_BTN_CAP_SWITCH | IDC_BTN_CAP_CONV => handle_capture_hotkey(hwnd, id),
        IDC_BTN_IMPORT_TREE => {
            import_evkey_tree_via_dialog(hwnd);
            rebuild_theme_resources(hwnd);
            refresh_ui_and_tray(hwnd);
        }
        IDC_BTN_OPEN_CFG_DIR => open_config_directory(),
        IDC_BTN_RELAUNCH_ADMIN => {
            if !is_elevated() && relaunch_as_admin() {
                unsafe {
                    DestroyWindow(hwnd);
                }
            }
        }
        IDC_BTN_DEFAULT => {
            let def = Config::default();
            sync_startup_setting(def.auto_start, def.run_as_admin);
            save_config(&def);
            with_state(|st| {
                st.user_preferred_ve = def.vietnamese_enabled;
                st.engine.update_config(def);
            });
            rebuild_theme_resources(hwnd);
            refresh_ui_and_tray(hwnd);
        }
        IDC_BTN_SAVE => {
            let cfg = with_state(|st| {
                let c = st.engine.config().clone();
                save_config(&c);
                c
            });
            sync_startup_setting(cfg.auto_start, cfg.run_as_admin);
            refresh_ui_and_tray(hwnd);
        }
        ID_TRAY_CHARSET_UNI..=ID_TRAY_TYPE_CUSTOM => handle_tray_preset(hwnd, id),
        _ => handle_control_change(hwnd, id, code),
    }
}

fn handle_capture_hotkey(hwnd: HWND, id: usize) {
    let is_switch = id == IDC_BTN_CAP_SWITCH;
    let cur = with_state(|st| {
        if is_switch {
            st.engine.config().custom_switch_hotkey
        } else {
            st.engine.config().clipboard_convert_hotkey
        }
    });
    if let Some(combo) = capture_hotkey_dialog(hwnd, cur) {
        with_state(|st| {
            let mut cfg = st.engine.config().clone();
            if is_switch {
                cfg.custom_switch_hotkey = combo;
                cfg.switch_preset = mskey_core::SwitchPreset::Custom;
            } else {
                cfg.clipboard_convert_hotkey = combo;
            }
            save_config(&cfg);
            st.engine.update_config(cfg);
        });
        refresh_ui_and_tray(hwnd);
    }
}

fn handle_tray_preset(hwnd: HWND, id: usize) {
    match id {
        ID_TRAY_CHARSET_UNI..=ID_TRAY_CHARSET_COMP => {
            let cs = match id {
                ID_TRAY_CHARSET_TCVN => Charset::Tcvn3,
                ID_TRAY_CHARSET_VNI => Charset::VniWindows,
                ID_TRAY_CHARSET_COMP => Charset::UnicodeComposite,
                _ => Charset::Unicode,
            };
            with_state(|st| {
                let mut cfg = st.engine.config().clone();
                cfg.charset = cs;
                save_config(&cfg);
                st.engine.update_config(cfg);
            });
        }
        ID_TRAY_TYPE_TELEX..=ID_TRAY_TYPE_CUSTOM => {
            let m = match id {
                ID_TRAY_TYPE_VNI => InputMethod::Vni,
                ID_TRAY_TYPE_VIQR => InputMethod::Viqr,
                ID_TRAY_TYPE_SIMPLE => InputMethod::SimpleTelex,
                ID_TRAY_TYPE_CUSTOM => InputMethod::Custom,
                _ => InputMethod::Telex,
            };
            with_state(|st| {
                let mut cfg = st.engine.config().clone();
                cfg.input_method = m;
                save_config(&cfg);
                st.engine.update_config(cfg);
            });
        }
        _ => {}
    }
    refresh_ui_and_tray(hwnd);
}

fn handle_control_change(hwnd: HWND, id: usize, code: usize) {
    let combo_sel = if code == CBN_SELCHANGE {
        with_main_data(|d| d.ctrls.combo_for_id(id))
            .flatten()
            .map(crate::dialog_helpers::combo_index)
    } else {
        None
    };
    let (changed, theme_changed) = with_state(|st| {
        let mut cfg = st.engine.config().clone();
        let mut did_change = false;
        let mut is_theme = false;
        if let Some(idx) = combo_sel {
            if MainUiCtrls::apply_combo_selection(id, idx, &mut cfg) {
                is_theme = id == IDC_THEME;
                did_change = true;
            }
        } else if MainUiCtrls::toggle_setting_id(id, &mut cfg) {
            did_change = true;
        }
        if did_change {
            save_config(&cfg);
            st.engine.update_config(cfg);
        }
        (did_change, is_theme)
    });
    if theme_changed {
        rebuild_theme_resources(hwnd);
    }
    if changed {
        refresh_ui_and_tray(hwnd);
    }
}
