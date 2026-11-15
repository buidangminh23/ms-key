use std::cell::RefCell;
use std::ptr;

use mskey_core::CustomKeyMap;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, HBRUSH, HFONT, PAINTSTRUCT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DestroyWindow, GetClientRect, WM_CLOSE, WM_COMMAND, WM_CREATE,
    WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DRAWITEM, WM_PAINT,
};

use crate::dialog_helpers::{
    create_modal_window, current_palette, handle_ctl_color, handle_draw_combo_or_btn, make_child,
    make_edit, make_owner_button, read_window_text, register_tool_class, run_modal_loop, scale_px,
    window_dpi, write_window_text,
};
use crate::platform::save_config;
use crate::state::with_state;
use crate::theme::{create_ui_font, fill_solid};

const ID_CUST_SAVE: usize = 6501;
const ID_CUST_RESET: usize = 6502;
const ID_CUST_CLOSE: usize = 6503;

#[derive(Clone)]
struct CustomDlg {
    font: HFONT,
    bg_br: HBRUSH,
    in_br: HBRUSH,
    edits: Vec<HWND>,
}

thread_local! {
    static CUSTOM_STATE: RefCell<Option<CustomDlg>> = const { RefCell::new(None) };
}

pub fn show_custom_keymap_dialog(parent: HWND) {
    register_tool_class("MSKeyCustomDlg", custom_wnd_proc);
    let modal = create_modal_window(parent, "MSKeyCustomDlg", "Tự định nghĩa kiểu gõ", 408, 360);
    run_modal_loop(parent, modal);
}

unsafe extern "system" fn custom_wnd_proc(
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
            let k = with_state(|st| st.engine.config().custom_keymap.clone());
            let fields = [
                ("Dấu sắc", k.acute),
                ("Dấu huyền", k.grave),
                ("Dấu hỏi", k.hook),
                ("Dấu ngã", k.tilde),
                ("Dấu nặng", k.dot),
                ("Xóa dấu", k.remove_tone),
                ("Chữ â", k.circumflex_a),
                ("Chữ ê", k.circumflex_e),
                ("Chữ ô", k.circumflex_o),
                ("Chữ ơ", k.horn_o),
                ("Chữ ư", k.horn_u),
                ("Chữ ă", k.breve_a),
                ("Chữ đ", k.d_stroke),
                ("Móc ơ nhanh", k.quick_o_horn_key),
                ("Móc ư nhanh", k.quick_u_horn_key),
            ];
            let mut edits = Vec::with_capacity(fields.len());
            for (idx, (lbl, ch)) in fields.iter().enumerate() {
                let col = (idx / 8) as i32;
                let row = (idx % 8) as i32;
                let x = 16 + col * 196;
                let y = 16 + row * 36;
                make_child(hwnd, "STATIC", lbl, 0, x, y + 4, 112, 24, 0, font, dpi);
                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                let ed = make_edit(hwnd, s, x + 116, y, 56, 28, 6510 + idx, false, font, dpi);
                edits.push(ed);
            }
            make_owner_button(hwnd, "Mặc định", 16, 312, 108, 32, ID_CUST_RESET, font, dpi);
            make_owner_button(hwnd, "Lưu", 204, 312, 92, 32, ID_CUST_SAVE, font, dpi);
            make_owner_button(hwnd, "Đóng", 308, 312, 84, 32, ID_CUST_CLOSE, font, dpi);
            CUSTOM_STATE.with(|c| {
                *c.borrow_mut() = Some(CustomDlg { font, bg_br, in_br, edits });
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
            let font =
                CUSTOM_STATE.with(|c| c.borrow().as_ref().map(|s| s.font).unwrap_or(ptr::null_mut()));
            if let Some(res) = handle_draw_combo_or_btn(lparam, &[ID_CUST_SAVE as u32], pal, font) {
                return res;
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let (b1, b2) = CUSTOM_STATE.with(|c| {
                c.borrow()
                    .as_ref()
                    .map(|s| (s.bg_br, s.in_br))
                    .unwrap_or((ptr::null_mut(), ptr::null_mut()))
            });
            handle_ctl_color(msg, wparam as _, pal, b1, b2)
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            let st = CUSTOM_STATE.with(|c| c.borrow().clone());
            if let Some(st) = st {
                match id {
                    ID_CUST_RESET => {
                        let d = CustomKeyMap::default();
                        let vals = [
                            d.acute, d.grave, d.hook, d.tilde, d.dot, d.remove_tone, d.circumflex_a,
                            d.circumflex_e, d.circumflex_o, d.horn_o, d.horn_u, d.breve_a,
                            d.d_stroke, d.quick_o_horn_key, d.quick_u_horn_key,
                        ];
                        for (ed, ch) in st.edits.iter().zip(vals.iter()) {
                            write_window_text(*ed, &ch.to_string());
                        }
                    }
                    ID_CUST_SAVE => {
                        let first_char = |idx: usize, fallback: char| {
                            read_window_text(st.edits[idx]).trim().chars().next().unwrap_or(fallback)
                        };
                        let d = CustomKeyMap::default();
                        let updated = CustomKeyMap {
                            acute: first_char(0, d.acute),
                            grave: first_char(1, d.grave),
                            hook: first_char(2, d.hook),
                            tilde: first_char(3, d.tilde),
                            dot: first_char(4, d.dot),
                            remove_tone: first_char(5, d.remove_tone),
                            circumflex_a: first_char(6, d.circumflex_a),
                            circumflex_e: first_char(7, d.circumflex_e),
                            circumflex_o: first_char(8, d.circumflex_o),
                            horn_o: first_char(9, d.horn_o),
                            horn_u: first_char(10, d.horn_u),
                            breve_a: first_char(11, d.breve_a),
                            d_stroke: first_char(12, d.d_stroke),
                            quick_u_horn: d.quick_u_horn,
                            quick_o_horn_key: first_char(13, d.quick_o_horn_key),
                            quick_u_horn_key: first_char(14, d.quick_u_horn_key),
                        };
                        with_state(|app| {
                            let mut cfg = app.engine.config().clone();
                            let mut kept = updated;
                            kept.quick_u_horn = cfg.custom_keymap.quick_u_horn;
                            cfg.custom_keymap = kept;
                            save_config(&cfg);
                            app.engine.update_config(cfg);
                        });
                        unsafe {
                            DestroyWindow(hwnd);
                        }
                    }
                    ID_CUST_CLOSE => unsafe {
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
            let taken = CUSTOM_STATE.with(|c| c.borrow_mut().take());
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
