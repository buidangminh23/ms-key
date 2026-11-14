use std::cell::RefCell;
use std::ptr;

use mskey_core::{AppBehavior, AppRule, MacroEntry};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, HBRUSH, HFONT, PAINTSTRUCT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DestroyWindow, GetClientRect, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT,
    LBS_NOTIFY, SendMessageW, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLOREDIT,
    WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DRAWITEM, WM_PAINT, WS_BORDER,
    WS_TABSTOP, WS_VSCROLL,
};

use crate::dialog_helpers::{
    combo_index, create_modal_window, current_palette, handle_ctl_color, handle_draw_combo_or_btn,
    make_child, make_combo, make_edit, make_owner_button, read_window_text, register_tool_class,
    run_modal_loop, scale_px, window_dpi, write_window_text,
};
use crate::platform::{save_config, to_wide};
use crate::state::with_state;
use crate::theme::{create_ui_font, fill_solid};

const ID_MAC_LIST: usize = 6301;
const ID_MAC_KEY: usize = 6302;
const ID_MAC_VAL: usize = 6303;
const ID_MAC_ADD: usize = 6304;
const ID_MAC_DEL: usize = 6305;
const ID_MAC_CLOSE: usize = 6306;
const LBN_SELCHANGE_CODE: usize = 1;

#[derive(Clone, Copy)]
struct MacroDlg {
    font: HFONT,
    bg_br: HBRUSH,
    in_br: HBRUSH,
    list: HWND,
    key_edit: HWND,
    val_edit: HWND,
}

thread_local! {
    static MACRO_STATE: RefCell<Option<MacroDlg>> = const { RefCell::new(None) };
    static APP_STATE: RefCell<Option<AppDlg>> = const { RefCell::new(None) };
}

pub fn show_macro_dialog(parent: HWND) {
    register_tool_class("MSKeyMacroDlg", macro_wnd_proc);
    let modal = create_modal_window(parent, "MSKeyMacroDlg", "Bảng gõ tắt (MS Key)", 464, 340);
    run_modal_loop(parent, modal);
}

fn reload_macro_list(list: HWND) {
    unsafe {
        SendMessageW(list, LB_RESETCONTENT, 0, 0);
        with_state(|st| {
            for m in &st.engine.config().macros {
                let line = format!("{}  ->  {}", m.shortcut, m.expansion);
                let w = to_wide(&line);
                SendMessageW(list, LB_ADDSTRING, 0, w.as_ptr() as LPARAM);
            }
        });
    }
}

unsafe extern "system" fn macro_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let pal = current_palette();
    match msg {
        WM_CREATE => {
            let dpi = window_dpi(hwnd);
            let font = create_ui_font(scale_px(14, dpi), false);
            let bg_br = unsafe { CreateSolidBrush(pal.card_bg) };
            let in_br = unsafe { CreateSolidBrush(pal.input_bg) };
            make_child(hwnd, "STATIC", "Từ viết tắt", 0, 16, 16, 120, 24, 0, font, dpi);
            let key_edit = make_edit(hwnd, "", 16, 40, 136, 28, ID_MAC_KEY, false, font, dpi);
            make_child(hwnd, "STATIC", "Nội dung thay thế", 0, 164, 16, 200, 24, 0, font, dpi);
            let val_edit = make_edit(hwnd, "", 164, 40, 284, 28, ID_MAC_VAL, false, font, dpi);
            make_owner_button(hwnd, "Thêm / Cập nhật", 16, 76, 144, 32, ID_MAC_ADD, font, dpi);
            make_owner_button(hwnd, "Xóa mục chọn", 168, 76, 128, 32, ID_MAC_DEL, font, dpi);
            make_owner_button(hwnd, "Đóng", 364, 76, 84, 32, ID_MAC_CLOSE, font, dpi);
            let list = make_child(
                hwnd,
                "LISTBOX",
                "",
                (LBS_NOTIFY as u32) | WS_BORDER | WS_VSCROLL | WS_TABSTOP,
                16,
                120,
                432,
                204,
                ID_MAC_LIST,
                font,
                dpi,
            );
            reload_macro_list(list);
            MACRO_STATE.with(|c| {
                *c.borrow_mut() = Some(MacroDlg { font, bg_br, in_br, list, key_edit, val_edit });
            });
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            fill_solid(hdc, &rc, pal.card_bg);
            unsafe { EndPaint(hwnd, &ps) };
            0
        }
        WM_DRAWITEM => {
            let font = MACRO_STATE.with(|c| c.borrow().map(|s| s.font).unwrap_or(ptr::null_mut()));
            if let Some(res) = handle_draw_combo_or_btn(lparam, &[ID_MAC_ADD as u32], pal, font) {
                return res;
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let (b1, b2) = MACRO_STATE.with(|c| {
                c.borrow()
                    .map(|s| (s.bg_br, s.in_br))
                    .unwrap_or((ptr::null_mut(), ptr::null_mut()))
            });
            handle_ctl_color(msg, wparam as _, pal, b1, b2)
        }
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as usize;
            let code = (wparam >> 16) & 0xFFFF;
            let st = MACRO_STATE.with(|c| *c.borrow());
            if let Some(st) = st {
                match id {
                    ID_MAC_LIST if code == LBN_SELCHANGE_CODE => {
                        let sel = unsafe { SendMessageW(st.list, LB_GETCURSEL, 0, 0) };
                        if sel >= 0 {
                            with_state(|app| {
                                if let Some(entry) = app.engine.config().macros.get(sel as usize) {
                                    write_window_text(st.key_edit, &entry.shortcut);
                                    write_window_text(st.val_edit, &entry.expansion);
                                }
                            });
                        }
                    }
                    ID_MAC_ADD => {
                        let k = read_window_text(st.key_edit).trim().to_string();
                        let v = read_window_text(st.val_edit);
                        if !k.is_empty() {
                            with_state(|app| {
                                let mut cfg = app.engine.config().clone();
                                if let Some(ex) = cfg
                                    .macros
                                    .iter_mut()
                                    .find(|m| m.shortcut.eq_ignore_ascii_case(&k))
                                {
                                    ex.expansion = v;
                                    ex.enabled = true;
                                } else {
                                    cfg.macros.push(MacroEntry {
                                        shortcut: k,
                                        expansion: v,
                                        enabled: true,
                                    });
                                }
                                save_config(&cfg);
                                app.engine.update_config(cfg);
                            });
                            reload_macro_list(st.list);
                        }
                    }
                    ID_MAC_DEL => {
                        let sel = unsafe { SendMessageW(st.list, LB_GETCURSEL, 0, 0) };
                        if sel >= 0 {
                            with_state(|app| {
                                let mut cfg = app.engine.config().clone();
                                if (sel as usize) < cfg.macros.len() {
                                    cfg.macros.remove(sel as usize);
                                    save_config(&cfg);
                                    app.engine.update_config(cfg);
                                }
                            });
                            reload_macro_list(st.list);
                        }
                    }
                    ID_MAC_CLOSE => unsafe {
                        DestroyWindow(hwnd);
                    },
                    _ => {}
                }
            }
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            let taken = MACRO_STATE.with(|c| c.borrow_mut().take());
            if let Some(st) = taken {
                unsafe {
                    DeleteObject(st.font);
                    DeleteObject(st.bg_br);
                    DeleteObject(st.in_br);
                }
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

const ID_APP_LIST: usize = 6401;
const ID_APP_NAME: usize = 6402;
const ID_APP_MODE: usize = 6403;
const ID_APP_ADD: usize = 6404;
const ID_APP_DEL: usize = 6405;
const ID_APP_CLOSE: usize = 6406;

#[derive(Clone, Copy)]
struct AppDlg {
    font: HFONT,
    bg_br: HBRUSH,
    in_br: HBRUSH,
    list: HWND,
    name_edit: HWND,
    mode_cb: HWND,
}

pub fn show_app_rules_dialog(parent: HWND) {
    register_tool_class("MSKeyAppsDlg", app_wnd_proc);
    let modal = create_modal_window(parent, "MSKeyAppsDlg", "Danh sách ứng dụng thông minh", 484, 340);
    run_modal_loop(parent, modal);
}

fn reload_app_list(list: HWND) {
    unsafe {
        SendMessageW(list, LB_RESETCONTENT, 0, 0);
        with_state(|st| {
            for r in &st.engine.config().app_rules {
                let tag = match r.behavior {
                    AppBehavior::ForceEnglish => "Luôn Tiếng Anh (E)",
                    AppBehavior::ForceVietnamese => "Luôn Tiếng Việt (V)",
                    AppBehavior::Blacklist => "Bỏ qua (Blacklist)",
                    AppBehavior::Default => "Mặc định",
                };
                let line = format!("{}  —  {}", r.process_name, tag);
                let w = to_wide(&line);
                SendMessageW(list, LB_ADDSTRING, 0, w.as_ptr() as LPARAM);
            }
        });
    }
}

unsafe extern "system" fn app_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let pal = current_palette();
    match msg {
        WM_CREATE => {
            let dpi = window_dpi(hwnd);
            let font = create_ui_font(scale_px(14, dpi), false);
            let bg_br = unsafe { CreateSolidBrush(pal.card_bg) };
            let in_br = unsafe { CreateSolidBrush(pal.input_bg) };
            let fg_now = with_state(|st| st.fg_process.clone());
            make_child(hwnd, "STATIC", "Tên tiến trình (.exe)", 0, 16, 16, 160, 24, 0, font, dpi);
            let name_edit = make_edit(hwnd, &fg_now, 16, 40, 200, 28, ID_APP_NAME, false, font, dpi);
            make_child(hwnd, "STATIC", "Quy tắc", 0, 228, 16, 120, 24, 0, font, dpi);
            let modes = &["Luôn Tiếng Anh (E)", "Luôn Tiếng Việt (V)", "Bỏ qua (Blacklist)"];
            let mode_cb = make_combo(hwnd, 228, 40, 240, 28, ID_APP_MODE, modes, 0, font, dpi);
            make_owner_button(hwnd, "Lưu quy tắc", 16, 76, 136, 32, ID_APP_ADD, font, dpi);
            make_owner_button(hwnd, "Xóa mục chọn", 164, 76, 136, 32, ID_APP_DEL, font, dpi);
            make_owner_button(hwnd, "Đóng", 384, 76, 84, 32, ID_APP_CLOSE, font, dpi);
            let list = make_child(
                hwnd,
                "LISTBOX",
                "",
                (LBS_NOTIFY as u32) | WS_BORDER | WS_VSCROLL | WS_TABSTOP,
                16,
                120,
                452,
                204,
                ID_APP_LIST,
                font,
                dpi,
            );
            reload_app_list(list);
            APP_STATE.with(|c| {
                *c.borrow_mut() = Some(AppDlg { font, bg_br, in_br, list, name_edit, mode_cb });
            });
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            fill_solid(hdc, &rc, pal.card_bg);
            unsafe { EndPaint(hwnd, &ps) };
            0
        }
        WM_DRAWITEM => {
            let font = APP_STATE.with(|c| c.borrow().map(|s| s.font).unwrap_or(ptr::null_mut()));
            if let Some(res) = handle_draw_combo_or_btn(lparam, &[ID_APP_ADD as u32], pal, font) {
                return res;
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let (b1, b2) = APP_STATE.with(|c| {
                c.borrow()
                    .map(|s| (s.bg_br, s.in_br))
                    .unwrap_or((ptr::null_mut(), ptr::null_mut()))
            });
            handle_ctl_color(msg, wparam as _, pal, b1, b2)
        }
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as usize;
            let code = (wparam >> 16) & 0xFFFF;
            let st = APP_STATE.with(|c| *c.borrow());
            if let Some(st) = st {
                match id {
                    ID_APP_LIST if code == LBN_SELCHANGE_CODE => {
                        let sel = unsafe { SendMessageW(st.list, LB_GETCURSEL, 0, 0) };
                        if sel >= 0 {
                            with_state(|app| {
                                if let Some(rule) = app.engine.config().app_rules.get(sel as usize) {
                                    write_window_text(st.name_edit, &rule.process_name);
                                }
                            });
                        }
                    }
                    ID_APP_ADD => {
                        let name = read_window_text(st.name_edit).trim().to_string();
                        if !name.is_empty() {
                            let b = match combo_index(st.mode_cb) {
                                1 => AppBehavior::ForceVietnamese,
                                2 => AppBehavior::Blacklist,
                                _ => AppBehavior::ForceEnglish,
                            };
                            with_state(|app| {
                                let mut cfg = app.engine.config().clone();
                                cfg.set_app_rule(&name, b);
                                save_config(&cfg);
                                app.engine.update_config(cfg);
                            });
                            reload_app_list(st.list);
                        }
                    }
                    ID_APP_DEL => {
                        let sel = unsafe { SendMessageW(st.list, LB_GETCURSEL, 0, 0) };
                        if sel >= 0 {
                            with_state(|app| {
                                let mut cfg = app.engine.config().clone();
                                if (sel as usize) < cfg.app_rules.len() {
                                    let _: AppRule = cfg.app_rules.remove(sel as usize);
                                    save_config(&cfg);
                                    app.engine.update_config(cfg);
                                }
                            });
                            reload_app_list(st.list);
                        }
                    }
                    ID_APP_CLOSE => unsafe {
                        DestroyWindow(hwnd);
                    },
                    _ => {}
                }
            }
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            let taken = APP_STATE.with(|c| c.borrow_mut().take());
            if let Some(st) = taken {
                unsafe {
                    DeleteObject(st.font);
                    DeleteObject(st.bg_br);
                    DeleteObject(st.in_br);
                }
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
