use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use mskey_core::{
    CaseTransform, ConvertEncoding, apply_case_transform, convert_with_encoding,
    detect_convert_encoding,
};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, DeleteObject, DrawTextW, EndPaint, InvalidateRect, PAINTSTRUCT, SelectObject,
    SetBkMode, SetTextColor,
};
use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SetActiveWindow, SetFocus, VK_DOWN, VK_ESCAPE, VK_NUMPAD1, VK_NUMPAD8, VK_RETURN, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos,
    GetForegroundWindow, HWND_TOPMOST, RegisterClassExW, SW_HIDE, SW_SHOW, SetForegroundWindow,
    SetWindowPos, ShowWindow, WA_INACTIVE, WM_ACTIVATE, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_PAINT, WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::dialog_helpers::{current_palette, scale_px, window_dpi};
use crate::hook_send::send_ctrl_key;
use crate::platform::to_wide;
use crate::platform_extra::{
    ClipboardSnapshot, backup_clipboard, clear_clipboard, read_clipboard_text,
    restore_clipboard_now, write_clipboard_text,
};
use crate::theme::{create_ui_font, draw_round_box, fill_solid, rgb};
use crate::ui_osd::{clamp_to_work_area, query_caret_screen_point};

const POPUP_CLASS: &str = "MSKeyConvertPopup";
const HEADER_H: i32 = 26;
const ROW_H: i32 = 28;
const POPUP_W: i32 = 212;
const POPUP_PAD: i32 = 6;

const ACTIONS: [(&str, Option<CaseTransform>); 8] = [
    ("1. Chữ HOA", Some(CaseTransform::Upper)),
    ("2. chữ thường", Some(CaseTransform::Lower)),
    ("3. Đầu câu", Some(CaseTransform::Sentence)),
    ("4. Tiêu đề", Some(CaseTransform::Title)),
    ("5. Bỏ dấu", Some(CaseTransform::StripDiacritics)),
    ("6. Dấu mới (hoà)", Some(CaseTransform::NormalizeModernTone)),
    ("7. Dấu cũ (hòa)", Some(CaseTransform::NormalizeClassicTone)),
    ("8. Chuyển → Unicode", None),
];

struct PopupState {
    target_fg: usize,
    selected_text: String,
    backup: Option<ClipboardSnapshot>,
    hover_idx: usize,
}

static POPUP_CLASS_READY: AtomicBool = AtomicBool::new(false);
static POPUP_HWND: AtomicUsize = AtomicUsize::new(0);
static POPUP_STATE: Mutex<Option<PopupState>> = Mutex::new(None);

pub fn is_popup_hwnd(hwnd: usize) -> bool {
    hwnd != 0 && POPUP_HWND.load(Ordering::Acquire) == hwnd
}

fn ensure_popup_class() {
    if POPUP_CLASS_READY.swap(true, Ordering::SeqCst) {
        return;
    }
    let hinst = unsafe { GetModuleHandleW(ptr::null()) };
    let cls = to_wide(POPUP_CLASS);
    let mut wc: WNDCLASSEXW = unsafe { std::mem::zeroed() };
    wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
    wc.lpfnWndProc = Some(popup_wnd_proc);
    wc.hInstance = hinst;
    wc.lpszClassName = cls.as_ptr();
    unsafe {
        RegisterClassExW(&wc);
    }
}

pub fn handle_convert_selection(target_hwnd: HWND) {
    let existing = POPUP_HWND.swap(0, Ordering::AcqRel) as HWND;
    if !existing.is_null() {
        cancel_popup(existing, false);
    }

    let target_fg = if !target_hwnd.is_null() {
        target_hwnd
    } else {
        unsafe { GetForegroundWindow() }
    };

    let backup = backup_clipboard();
    let _ = clear_clipboard();
    send_ctrl_key(b'C' as u16);

    let deadline = Instant::now() + Duration::from_millis(120);
    let mut selected = None;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(15));
        if let Some(txt) = read_clipboard_text()
            && !txt.is_empty()
        {
            selected = Some(txt);
            break;
        }
    }

    let Some(selected_text) = selected else {
        if let Some(snap) = backup {
            restore_clipboard_now(snap);
        }
        return;
    };

    if let Ok(mut guard) = POPUP_STATE.lock() {
        *guard = Some(PopupState {
            target_fg: target_fg as usize,
            selected_text,
            backup,
            hover_idx: 0,
        });
    }

    ensure_popup_class();
    let cls = to_wide(POPUP_CLASS);
    let empty = to_wide("");
    let hinst = unsafe { GetModuleHandleW(ptr::null()) };
    let hw = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            cls.as_ptr(),
            empty.as_ptr(),
            WS_POPUP,
            0,
            0,
            POPUP_W,
            HEADER_H + ROW_H * ACTIONS.len() as i32 + POPUP_PAD * 2,
            ptr::null_mut(),
            ptr::null_mut(),
            hinst,
            ptr::null(),
        )
    };
    if hw.is_null() {
        if let Some(st) = POPUP_STATE.lock().ok().and_then(|mut g| g.take())
            && let Some(snap) = st.backup
        {
            restore_clipboard_now(snap);
        }
        return;
    }

    let dpi = window_dpi(hw);
    let s = |v: i32| scale_px(v, dpi);
    let w = s(POPUP_W);
    let h = s(HEADER_H + ROW_H * ACTIONS.len() as i32 + POPUP_PAD * 2);
    let (caret_pt, fg_for_mon) = query_caret_screen_point(Some(target_fg));
    let (raw_x, raw_y, anchor_pt) = if let Some(pt) = caret_pt {
        (pt.x + s(6), pt.y + s(6), pt)
    } else {
        let mut cur: POINT = unsafe { std::mem::zeroed() };
        unsafe {
            GetCursorPos(&mut cur);
        }
        (cur.x + s(8), cur.y + s(8), cur)
    };
    let (x, y) = clamp_to_work_area(raw_x, raw_y, w, h, anchor_pt, fg_for_mon);

    POPUP_HWND.store(hw as usize, Ordering::Release);
    unsafe {
        SetWindowPos(hw, HWND_TOPMOST, x, y, w, h, 0);
        ShowWindow(hw, SW_SHOW);
        SetForegroundWindow(hw);
        SetActiveWindow(hw);
        SetFocus(hw);
    }
}

fn transform_for_action(text: &str, idx: usize) -> String {
    match ACTIONS[idx].1 {
        Some(case_mode) => apply_case_transform(text, case_mode),
        None => {
            let from = detect_convert_encoding(text);
            convert_with_encoding(text, from, ConvertEncoding::Unicode, CaseTransform::None)
        }
    }
}

fn cancel_popup(hwnd: HWND, refocus_target: bool) {
    POPUP_HWND.compare_exchange(hwnd as usize, 0, Ordering::AcqRel, Ordering::Acquire).ok();
    let taken = POPUP_STATE.lock().ok().and_then(|mut g| g.take());
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
        DestroyWindow(hwnd);
    }
    if let Some(st) = taken {
        if let Some(snap) = st.backup {
            restore_clipboard_now(snap);
        }
        if refocus_target && st.target_fg != 0 {
            unsafe {
                SetForegroundWindow(st.target_fg as HWND);
            }
        }
    }
}

fn apply_popup_choice(hwnd: HWND, idx: usize) {
    if idx >= ACTIONS.len() {
        return;
    }
    POPUP_HWND.compare_exchange(hwnd as usize, 0, Ordering::AcqRel, Ordering::Acquire).ok();
    let taken = POPUP_STATE.lock().ok().and_then(|mut g| g.take());
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
        DestroyWindow(hwnd);
    }
    let Some(st) = taken else {
        return;
    };
    let out = transform_for_action(&st.selected_text, idx);
    let wrote = write_clipboard_text(&out);
    let seq_after = unsafe { GetClipboardSequenceNumber() };
    let target_fg_raw = st.target_fg;
    let backup = st.backup;
    std::thread::spawn(move || {
        if target_fg_raw != 0 {
            unsafe {
                SetForegroundWindow(target_fg_raw as HWND);
            }
        }
        std::thread::sleep(Duration::from_millis(45));
        if wrote {
            send_ctrl_key(b'V' as u16);
        }
        std::thread::sleep(Duration::from_millis(100));
        if let Some(snap) = backup
            && unsafe { GetClipboardSequenceNumber() } == seq_after
        {
            restore_clipboard_now(snap);
        }
    });
}

fn row_index_at_y(hwnd: HWND, y_px: i32) -> Option<usize> {
    let dpi = window_dpi(hwnd);
    let s = |v: i32| scale_px(v, dpi);
    let top = s(POPUP_PAD + HEADER_H);
    let row_h = s(ROW_H);
    if y_px < top || row_h <= 0 {
        return None;
    }
    let idx = ((y_px - top) / row_h) as usize;
    if idx < ACTIONS.len() { Some(idx) } else { None }
}

unsafe extern "system" fn popup_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_ACTIVATE => {
            if ((wparam & 0xFFFF) as u32) == WA_INACTIVE && is_popup_hwnd(hwnd as usize) {
                cancel_popup(hwnd, false);
            }
            0
        }
        WM_MOUSEMOVE => {
            let y_px = ((lparam >> 16) as i16) as i32;
            if let Some(idx) = row_index_at_y(hwnd, y_px) {
                let changed = POPUP_STATE
                    .lock()
                    .ok()
                    .and_then(|mut g| {
                        g.as_mut().map(|st| {
                            let prev = st.hover_idx;
                            st.hover_idx = idx;
                            prev != idx
                        })
                    })
                    .unwrap_or(false);
                if changed {
                    unsafe {
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let y_px = ((lparam >> 16) as i16) as i32;
            if let Some(idx) = row_index_at_y(hwnd, y_px) {
                apply_popup_choice(hwnd, idx);
            }
            0
        }
        WM_KEYDOWN => {
            let vk = wparam as u16;
            if vk == VK_ESCAPE {
                cancel_popup(hwnd, true);
                return 0;
            }
            if vk == VK_RETURN {
                let idx = POPUP_STATE
                    .lock()
                    .ok()
                    .and_then(|g| g.as_ref().map(|s| s.hover_idx))
                    .unwrap_or(0);
                apply_popup_choice(hwnd, idx);
                return 0;
            }
            if vk == VK_UP || vk == VK_DOWN {
                let changed = POPUP_STATE
                    .lock()
                    .ok()
                    .and_then(|mut g| {
                        g.as_mut().map(|st| {
                            st.hover_idx = if vk == VK_UP {
                                if st.hover_idx == 0 {
                                    ACTIONS.len() - 1
                                } else {
                                    st.hover_idx - 1
                                }
                            } else {
                                (st.hover_idx + 1) % ACTIONS.len()
                            };
                            true
                        })
                    })
                    .unwrap_or(false);
                if changed {
                    unsafe {
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                }
                return 0;
            }
            let num_idx = if (b'1' as u16..=b'8' as u16).contains(&vk) {
                Some((vk - b'1' as u16) as usize)
            } else if (VK_NUMPAD1..=VK_NUMPAD8).contains(&vk) {
                Some((vk - VK_NUMPAD1) as usize)
            } else {
                None
            };
            if let Some(idx) = num_idx {
                apply_popup_choice(hwnd, idx);
                return 0;
            }
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe {
                GetClientRect(hwnd, &mut rc);
            }
            let pal = current_palette();
            let dpi = window_dpi(hwnd);
            let s = |v: i32| scale_px(v, dpi);
            let hover = POPUP_STATE
                .lock()
                .ok()
                .and_then(|g| g.as_ref().map(|st| st.hover_idx))
                .unwrap_or(0);

            fill_solid(hdc, &rc, pal.window_bg);
            draw_round_box(hdc, &rc, s(8), pal.card_bg, pal.border);

            let hdr_font = create_ui_font(s(12), true);
            let row_font = create_ui_font(s(14), false);
            let mut hdr_txt = to_wide("Chuyển đổi vùng chọn");
            let mut hdr_rc = RECT {
                left: rc.left + s(12),
                top: rc.top + s(POPUP_PAD),
                right: rc.right - s(10),
                bottom: rc.top + s(POPUP_PAD + HEADER_H),
            };
            unsafe {
                SetBkMode(hdc, 1);
                let of = SelectObject(hdc, hdr_font);
                SetTextColor(hdc, pal.subtle_text);
                DrawTextW(hdc, hdr_txt.as_mut_ptr(), -1, &mut hdr_rc, 0x0824);
                SelectObject(hdc, row_font);
                for (idx, (label, _)) in ACTIONS.iter().enumerate() {
                    let top = rc.top + s(POPUP_PAD + HEADER_H) + (idx as i32) * s(ROW_H);
                    let item_rc = RECT {
                        left: rc.left + s(6),
                        top,
                        right: rc.right - s(6),
                        bottom: top + s(ROW_H - 2),
                    };
                    let selected = idx == hover;
                    if selected {
                        draw_round_box(hdc, &item_rc, s(6), pal.accent, pal.accent);
                        SetTextColor(hdc, rgb(255, 255, 255));
                    } else {
                        SetTextColor(hdc, pal.text);
                    }
                    let mut txt_rc = RECT {
                        left: item_rc.left + s(10),
                        top: item_rc.top,
                        right: item_rc.right - s(8),
                        bottom: item_rc.bottom,
                    };
                    let mut wtxt = to_wide(label);
                    DrawTextW(hdc, wtxt.as_mut_ptr(), -1, &mut txt_rc, 0x0824);
                }
                SelectObject(hdc, of);
                DeleteObject(hdr_font);
                DeleteObject(row_font);
                EndPaint(hwnd, &ps);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
