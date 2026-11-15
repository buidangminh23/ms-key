use std::cell::{Cell, RefCell};
use std::ptr;

use mskey_core::{CaseTransform, HotkeyCombo};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, HBRUSH, HFONT, PAINTSTRUCT,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DestroyWindow, GetClientRect, WM_CLOSE, WM_COMMAND, WM_CREATE,
    WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DRAWITEM, WM_KEYDOWN,
    WM_PAINT,
};

use crate::dialog_helpers::{
    combo_index, create_modal_window, current_palette, format_hotkey, handle_ctl_color,
    handle_draw_combo_or_btn, make_child, make_combo, make_edit, make_owner_button,
    read_window_text, register_tool_class, run_modal_loop, scale_px, window_dpi, write_window_text,
};
use crate::platform::{read_clipboard_text, write_clipboard_text};
use crate::state::with_state;
use crate::theme::{create_ui_font, fill_solid};

const ID_CONV_FROM: usize = 6101;
const ID_CONV_TO: usize = 6102;
const ID_CONV_CASE: usize = 6103;
const ID_CONV_INPUT: usize = 6104;
const ID_CONV_READ: usize = 6105;
const ID_CONV_RUN: usize = 6106;
const ID_CONV_CLOSE: usize = 6107;

#[derive(Clone, Copy)]
struct ConvDlg {
    font: HFONT,
    bg_br: HBRUSH,
    in_br: HBRUSH,
    from_cb: HWND,
    to_cb: HWND,
    case_cb: HWND,
    edit: HWND,
}

thread_local! {
    static CONV_STATE: RefCell<Option<ConvDlg>> = const { RefCell::new(None) };
    static CAP_RESULT: Cell<Option<HotkeyCombo>> = const { Cell::new(None) };
    static CAP_FONT: Cell<HFONT> = const { Cell::new(ptr::null_mut()) };
}

pub fn show_converter_dialog(parent: HWND) {
    register_tool_class("MSKeyConvDlg", conv_wnd_proc);
    let modal = create_modal_window(parent, "MSKeyConvDlg", "Chuyển mã Clipboard & Văn bản", 464, 324);
    run_modal_loop(parent, modal);
}

unsafe extern "system" fn conv_wnd_proc(
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
            let cs = &[
                "Unicode",
                "TCVN3 (ABC)",
                "VNI Windows",
                "Unicode tổ hợp",
                "VIQR",
                "Tự động (Auto)",
            ];
            let cases = &[
                "Giữ nguyên chữ",
                "CHỮ HOA",
                "chữ thường",
                "Hoa đầu câu",
                "Hoa Mỗi Từ",
                "Bỏ dấu tiếng Việt",
            ];
            make_child(hwnd, "STATIC", "Bảng mã nguồn", 0, 16, 16, 120, 24, 0, font, dpi);
            let from_cb = make_combo(hwnd, 140, 12, 152, 28, ID_CONV_FROM, cs, 0, font, dpi);
            make_child(hwnd, "STATIC", "Bảng mã đích", 0, 16, 52, 120, 24, 0, font, dpi);
            let to_cb = make_combo(hwnd, 140, 48, 152, 28, ID_CONV_TO, cs, 0, font, dpi);
            make_child(hwnd, "STATIC", "Chuyển hoa/thường", 0, 16, 88, 120, 24, 0, font, dpi);
            let case_cb = make_combo(hwnd, 140, 84, 152, 28, ID_CONV_CASE, cases, 0, font, dpi);
            let initial = read_clipboard_text().unwrap_or_default();
            let edit = make_edit(hwnd, &initial, 16, 124, 432, 140, ID_CONV_INPUT, true, font, dpi);
            make_owner_button(hwnd, "Đọc Clipboard", 16, 276, 128, 32, ID_CONV_READ, font, dpi);
            make_owner_button(hwnd, "Chuyển & Chép", 156, 276, 152, 32, ID_CONV_RUN, font, dpi);
            make_owner_button(hwnd, "Đóng", 364, 276, 84, 32, ID_CONV_CLOSE, font, dpi);
            CONV_STATE.with(|c| {
                *c.borrow_mut() =
                    Some(ConvDlg { font, bg_br, in_br, from_cb, to_cb, case_cb, edit });
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
            let font = CONV_STATE.with(|c| c.borrow().map(|s| s.font).unwrap_or(ptr::null_mut()));
            if let Some(res) = handle_draw_combo_or_btn(lparam, &[ID_CONV_RUN as u32], pal, font) {
                return res;
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let (b1, b2) = CONV_STATE.with(|c| {
                c.borrow()
                    .map(|s| (s.bg_br, s.in_br))
                    .unwrap_or((ptr::null_mut(), ptr::null_mut()))
            });
            handle_ctl_color(msg, wparam as _, pal, b1, b2)
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            let st = CONV_STATE.with(|c| *c.borrow());
            if let Some(st) = st {
                match id {
                    ID_CONV_READ => {
                        if let Some(txt) = read_clipboard_text() {
                            write_window_text(st.edit, &txt);
                        }
                    }
                    ID_CONV_RUN => {
                        let src = read_window_text(st.edit);
                        let map_from = |idx: usize| match idx {
                            1 => mskey_core::ConvertEncoding::Tcvn3,
                            2 => mskey_core::ConvertEncoding::VniWindows,
                            3 => mskey_core::ConvertEncoding::UnicodeComposite,
                            4 => mskey_core::ConvertEncoding::Viqr,
                            5 => mskey_core::detect_convert_encoding(&src),
                            _ => mskey_core::ConvertEncoding::Unicode,
                        };
                        let map_to = |idx: usize| match idx {
                            1 => mskey_core::ConvertEncoding::Tcvn3,
                            2 => mskey_core::ConvertEncoding::VniWindows,
                            3 => mskey_core::ConvertEncoding::UnicodeComposite,
                            4 => mskey_core::ConvertEncoding::Viqr,
                            _ => mskey_core::ConvertEncoding::Unicode,
                        };
                        let map_case = |idx: usize| match idx {
                            1 => CaseTransform::Upper,
                            2 => CaseTransform::Lower,
                            3 => CaseTransform::Sentence,
                            4 => CaseTransform::Title,
                            5 => CaseTransform::StripDiacritics,
                            _ => CaseTransform::None,
                        };
                        let out = mskey_core::convert_with_encoding(
                            &src,
                            map_from(combo_index(st.from_cb)),
                            map_to(combo_index(st.to_cb)),
                            map_case(combo_index(st.case_cb)),
                        );
                        write_window_text(st.edit, &out);
                        write_clipboard_text(&out);
                    }
                    ID_CONV_CLOSE => unsafe {
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
            let taken = CONV_STATE.with(|c| c.borrow_mut().take());
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

const ID_CAP_CANCEL: usize = 6201;

pub fn capture_hotkey_dialog(parent: HWND, current: HotkeyCombo) -> Option<HotkeyCombo> {
    CAP_RESULT.with(|c| c.set(None));
    with_state(|st| st.suspend_typing_for_hotkey = true);
    register_tool_class("MSKeyHotkeyCap", cap_wnd_proc);
    let modal = create_modal_window(
        parent,
        "MSKeyHotkeyCap",
        &format!("Nhấn phím tắt ({})", format_hotkey(current)),
        364,
        108,
    );
    run_modal_loop(parent, modal);
    with_state(|st| st.suspend_typing_for_hotkey = false);
    CAP_RESULT.with(|c| c.get())
}

unsafe extern "system" fn cap_wnd_proc(
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
            CAP_FONT.with(|c| c.set(font));
            make_child(
                hwnd,
                "STATIC",
                "Nhấn phím tắt mới (vd Ctrl + Shift + F6)",
                0,
                16,
                16,
                332,
                24,
                0,
                font,
                dpi,
            );
            make_owner_button(hwnd, "Hủy", 264, 56, 84, 32, ID_CAP_CANCEL, font, dpi);
            0
        }
        WM_KEYDOWN | 0x0104 => {
            let vk = wparam as u32;
            if !matches!(vk, 0x10..=0x12 | 0xA0..=0xA5 | 0x5B | 0x5C) {
                let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL as i32) } < 0;
                let shift = unsafe { GetAsyncKeyState(VK_SHIFT as i32) } < 0;
                let alt = unsafe { GetAsyncKeyState(VK_MENU as i32) } < 0;
                let win = unsafe { GetAsyncKeyState(0x5B) } < 0
                    || unsafe { GetAsyncKeyState(0x5C) } < 0;
                CAP_RESULT.with(|c| {
                    c.set(Some(HotkeyCombo { ctrl, alt, shift, win_cmd: win, key_code: vk }));
                });
                unsafe {
                    DestroyWindow(hwnd);
                }
            }
            0
        }
        WM_DRAWITEM => {
            let font = CAP_FONT.with(|c| c.get());
            if let Some(res) = handle_draw_combo_or_btn(lparam, &[], pal, font) {
                return res;
            }
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
        WM_COMMAND if (wparam & 0xFFFF) == ID_CAP_CANCEL => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            let f = CAP_FONT.with(|c| c.replace(ptr::null_mut()));
            if !f.is_null() {
                unsafe {
                    DeleteObject(f);
                }
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
