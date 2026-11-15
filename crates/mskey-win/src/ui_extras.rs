use std::cell::RefCell;
use std::ptr;

use mskey_core::Config;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, HBRUSH, HFONT, InvalidateRect,
    PAINTSTRUCT,
};
use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, ODT_BUTTON};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CB_SETCURSEL, DefWindowProcW, DestroyWindow, GetClientRect, LB_ADDSTRING, LB_GETCURSEL,
    LB_RESETCONTENT, LBS_NOTIFY, SendMessageW, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLOREDIT,
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
use crate::theme::{create_ui_font, draw_checkbox, fill_solid};
use crate::ui_actions::{export_json_backup_dialog, import_json_backup_dialog};

const ID_EXT_QTELEX: usize = 6601;
const ID_EXT_QSTART: usize = 6602;
const ID_EXT_QEND: usize = 6603;
const ID_EXT_OSD: usize = 6604;
const ID_EXT_SHIFTINS: usize = 6605;
const ID_EXT_DELAY: usize = 6606;
const ID_EXT_EXP: usize = 6607;
const ID_EXT_IMP: usize = 6608;
const ID_EXT_CLEAR: usize = 6609;
const ID_EXT_SANDBOX: usize = 6610;
const ID_EXT_CLOSE: usize = 6611;
const CBN_SELCHANGE_CODE: usize = 1;

const DELAY_OPTIONS: [u8; 5] = [0, 1, 2, 5, 10];

#[derive(Clone, Copy)]
struct ExtrasDlg {
    font: HFONT,
    bg_br: HBRUSH,
    in_br: HBRUSH,
    delay_cb: HWND,
    sandbox: HWND,
}

thread_local! {
    static EXTRAS_STATE: RefCell<Option<ExtrasDlg>> = const { RefCell::new(None) };
    static DICT_STATE: RefCell<Option<DictDlg>> = const { RefCell::new(None) };
}

pub fn show_extras_sandbox_dialog(parent: HWND) -> bool {
    register_tool_class("MSKeyExtrasDlg", extras_wnd_proc);
    let modal = create_modal_window(
        parent,
        "MSKeyExtrasDlg",
        "Mở rộng & Gõ thử (MS Key)",
        476,
        486,
    );
    run_modal_loop(parent, modal);
    true
}

fn delay_to_idx(ms: u8) -> usize {
    DELAY_OPTIONS.iter().position(|&v| v == ms).unwrap_or(0)
}

fn extras_checkbox_state(id: u32, cfg: &Config) -> Option<bool> {
    Some(match id as usize {
        ID_EXT_QTELEX => cfg.quick_telex,
        ID_EXT_QSTART => cfg.quick_start_consonant,
        ID_EXT_QEND => cfg.quick_end_consonant,
        ID_EXT_OSD => cfg.show_switch_osd,
        ID_EXT_SHIFTINS => cfg.paste_via_shift_insert,
        _ => return None,
    })
}

fn toggle_extras_id(id: usize, cfg: &mut Config) -> bool {
    match id {
        ID_EXT_QTELEX => cfg.quick_telex = !cfg.quick_telex,
        ID_EXT_QSTART => cfg.quick_start_consonant = !cfg.quick_start_consonant,
        ID_EXT_QEND => cfg.quick_end_consonant = !cfg.quick_end_consonant,
        ID_EXT_OSD => cfg.show_switch_osd = !cfg.show_switch_osd,
        ID_EXT_SHIFTINS => cfg.paste_via_shift_insert = !cfg.paste_via_shift_insert,
        _ => return false,
    }
    true
}

fn create_extras_controls(hwnd: HWND, font: HFONT, dpi: i32) -> (HWND, HWND) {
    let items = [
        (ID_EXT_QTELEX, "Gõ nhanh phụ âm (Quick Telex: cc→ch, tt→th)"),
        (ID_EXT_QSTART, "Phụ âm đầu nhanh (f→ph, j→gi, w→qu)"),
        (ID_EXT_QEND, "Phụ âm cuối nhanh (-g→ng, -h→nh, -k→ch)"),
        (ID_EXT_OSD, "Hiện khung nổi OSD khi chuyển V/E"),
        (ID_EXT_SHIFTINS, "Dán bằng Shift+Insert (khi dùng Clipboard)"),
    ];
    for (idx, (id, lbl)) in items.iter().enumerate() {
        make_owner_button(hwnd, lbl, 16, 16 + (idx as i32) * 30, 444, 24, *id, font, dpi);
    }
    make_child(hwnd, "STATIC", "Độ trễ gửi phím:", 0, 16, 178, 136, 24, 0, font, dpi);
    let sel = with_state(|app| delay_to_idx(app.engine.config().send_delay_ms));
    let delay_cb = make_combo(
        hwnd,
        158,
        174,
        164,
        28,
        ID_EXT_DELAY,
        &["0 ms (Nhanh)", "1 ms", "2 ms", "5 ms", "10 ms"],
        sel,
        font,
        dpi,
    );
    make_child(hwnd, "STATIC", "Sao lưu JSON:", 0, 16, 218, 136, 24, 0, font, dpi);
    make_owner_button(hwnd, "Xuất JSON...", 158, 214, 144, 30, ID_EXT_EXP, font, dpi);
    make_owner_button(hwnd, "Nhập JSON...", 314, 214, 144, 30, ID_EXT_IMP, font, dpi);
    make_child(hwnd, "STATIC", "Khung gõ thử trực tiếp:", 0, 16, 260, 260, 24, 0, font, dpi);
    make_owner_button(hwnd, "Xóa chữ", 364, 256, 94, 26, ID_EXT_CLEAR, font, dpi);
    let sandbox = make_edit(hwnd, "", 16, 290, 442, 140, ID_EXT_SANDBOX, true, font, dpi);
    make_owner_button(hwnd, "Đóng", 364, 440, 94, 32, ID_EXT_CLOSE, font, dpi);
    (delay_cb, sandbox)
}

fn handle_extras_cmd(hwnd: HWND, id: usize, code: usize, st: ExtrasDlg) {
    let mut changed = false;
    with_state(|app| {
        let mut cfg = app.engine.config().clone();
        if toggle_extras_id(id, &mut cfg) {
            changed = true;
        } else if id == ID_EXT_DELAY && code == CBN_SELCHANGE_CODE {
            let idx = combo_index(st.delay_cb);
            cfg.send_delay_ms = *DELAY_OPTIONS.get(idx).unwrap_or(&0);
            changed = true;
        }
        if changed {
            save_config(&cfg);
            app.engine.update_config(cfg);
        }
    });
    if changed {
        unsafe { InvalidateRect(hwnd, ptr::null(), 1) };
        return;
    }
    match id {
        ID_EXT_EXP => export_json_backup_dialog(hwnd),
        ID_EXT_IMP if import_json_backup_dialog(hwnd) => {
            let idx = with_state(|app| delay_to_idx(app.engine.config().send_delay_ms));
            unsafe {
                SendMessageW(st.delay_cb, CB_SETCURSEL, idx, 0);
                InvalidateRect(hwnd, ptr::null(), 1);
            }
        }
        ID_EXT_CLEAR => {
            write_window_text(st.sandbox, "");
            with_state(|app| app.engine.reset_all());
        }
        ID_EXT_CLOSE => unsafe { DestroyWindow(hwnd); },
        _ => {}
    }
}

unsafe extern "system" fn extras_wnd_proc(
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
            let (delay_cb, sandbox) = create_extras_controls(hwnd, font, dpi);
            EXTRAS_STATE.with(|c| {
                *c.borrow_mut() = Some(ExtrasDlg { font, bg_br, in_br, delay_cb, sandbox });
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
        WM_DRAWITEM if lparam != 0 => {
            let font = EXTRAS_STATE.with(|c| c.borrow().map(|s| s.font).unwrap_or(ptr::null_mut()));
            let dis = unsafe { &*(lparam as *const DRAWITEMSTRUCT) };
            if dis.CtlType == ODT_BUTTON {
                let cfg = with_state(|st| st.engine.config().clone());
                if let Some(checked) = extras_checkbox_state(dis.CtlID, &cfg) {
                    let txt = read_window_text(dis.hwndItem);
                    let dpi = window_dpi(hwnd);
                    fill_solid(dis.hDC, &dis.rcItem, pal.card_bg);
                    draw_checkbox(dis.hDC, &dis.rcItem, &txt, checked, pal, font, dpi);
                    return 1;
                }
            }
            handle_draw_combo_or_btn(lparam, &[], pal, font).unwrap_or(0)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let (b1, b2) = EXTRAS_STATE.with(|c| {
                c.borrow().map(|s| (s.bg_br, s.in_br)).unwrap_or((ptr::null_mut(), ptr::null_mut()))
            });
            handle_ctl_color(msg, wparam as _, pal, b1, b2)
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            let code = (wparam >> 16) & 0xFFFF;
            if let Some(st) = EXTRAS_STATE.with(|c| *c.borrow()) {
                handle_extras_cmd(hwnd, id, code, st);
            }
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            if let Some(st) = EXTRAS_STATE.with(|c| c.borrow_mut().take()) {
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

const ID_DICT_EDIT: usize = 6701;
const ID_DICT_ADD: usize = 6702;
const ID_DICT_DEL: usize = 6703;
const ID_DICT_CLOSE: usize = 6704;
const ID_DICT_LIST: usize = 6705;

#[derive(Clone, Copy)]
struct DictDlg {
    font: HFONT,
    bg_br: HBRUSH,
    in_br: HBRUSH,
    edit: HWND,
    list: HWND,
}

pub fn show_user_dict_dialog(parent: HWND) {
    register_tool_class("MSKeyUserDictDlg", dict_wnd_proc);
    let modal = create_modal_window(
        parent,
        "MSKeyUserDictDlg",
        "Từ điển cá nhân (MS Key)",
        448,
        348,
    );
    run_modal_loop(parent, modal);
}

fn reload_dict_list(list: HWND) {
    unsafe {
        SendMessageW(list, LB_RESETCONTENT, 0, 0);
        with_state(|st| {
            for word in &st.engine.config().user_dict_words {
                let w = to_wide(word);
                SendMessageW(list, LB_ADDSTRING, 0, w.as_ptr() as LPARAM);
            }
        });
    }
}

fn handle_dict_cmd(hwnd: HWND, id: usize, st: DictDlg) {
    match id {
        ID_DICT_ADD => {
            let word = read_window_text(st.edit).trim().to_string();
            if !word.is_empty() {
                with_state(|app| {
                    let mut cfg = app.engine.config().clone();
                    if !cfg.user_dict_words.iter().any(|w| w.eq_ignore_ascii_case(&word)) {
                        cfg.user_dict_words.push(word);
                        cfg.user_dict_words.sort_by_key(|w| w.to_lowercase());
                        save_config(&cfg);
                        app.engine.update_config(cfg);
                    }
                });
                write_window_text(st.edit, "");
                reload_dict_list(st.list);
            }
        }
        ID_DICT_DEL => {
            let sel = unsafe { SendMessageW(st.list, LB_GETCURSEL, 0, 0) };
            if sel >= 0 {
                with_state(|app| {
                    let mut cfg = app.engine.config().clone();
                    if (sel as usize) < cfg.user_dict_words.len() {
                        cfg.user_dict_words.remove(sel as usize);
                        save_config(&cfg);
                        app.engine.update_config(cfg);
                    }
                });
                reload_dict_list(st.list);
            }
        }
        ID_DICT_CLOSE => unsafe { DestroyWindow(hwnd); },
        _ => {}
    }
}

unsafe extern "system" fn dict_wnd_proc(
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
            make_child(hwnd, "STATIC", "Thêm từ không bị khôi phục khi gõ:", 0, 16, 16, 360, 24, 0, font, dpi);
            let edit = make_edit(hwnd, "", 16, 42, 296, 28, ID_DICT_EDIT, false, font, dpi);
            make_owner_button(hwnd, "Thêm từ", 324, 42, 108, 28, ID_DICT_ADD, font, dpi);
            make_owner_button(hwnd, "Xóa mục chọn", 16, 78, 136, 30, ID_DICT_DEL, font, dpi);
            make_owner_button(hwnd, "Đóng", 348, 78, 84, 30, ID_DICT_CLOSE, font, dpi);
            let list = make_child(
                hwnd,
                "LISTBOX",
                "",
                (LBS_NOTIFY as u32) | WS_BORDER | WS_VSCROLL | WS_TABSTOP,
                16,
                118,
                416,
                212,
                ID_DICT_LIST,
                font,
                dpi,
            );
            reload_dict_list(list);
            DICT_STATE.with(|c| {
                *c.borrow_mut() = Some(DictDlg { font, bg_br, in_br, edit, list });
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
            let font = DICT_STATE.with(|c| c.borrow().map(|s| s.font).unwrap_or(ptr::null_mut()));
            handle_draw_combo_or_btn(lparam, &[ID_DICT_ADD as u32], pal, font).unwrap_or(0)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let (b1, b2) = DICT_STATE.with(|c| {
                c.borrow().map(|s| (s.bg_br, s.in_br)).unwrap_or((ptr::null_mut(), ptr::null_mut()))
            });
            handle_ctl_color(msg, wparam as _, pal, b1, b2)
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            if let Some(st) = DICT_STATE.with(|c| *c.borrow()) {
                handle_dict_cmd(hwnd, id, st);
            }
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            if let Some(st) = DICT_STATE.with(|c| c.borrow_mut().take()) {
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
