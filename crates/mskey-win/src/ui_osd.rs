use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, DeleteObject, DrawTextW, EndPaint, GetMonitorInfoW, InvalidateRect,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, PAINTSTRUCT, SelectObject,
    SetBkMode, SetTextColor,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetForegroundWindow, HWND_TOPMOST,
    KillTimer, LWA_ALPHA, RegisterClassExW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SetLayeredWindowAttributes, SetTimer, SetWindowPos, ShowWindow, WM_ERASEBKGND, WM_PAINT,
    WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP, DrawIconEx, DI_NORMAL,
};

use crate::dialog_helpers::{current_palette, scale_px, window_dpi};
use crate::platform::to_wide;
use crate::theme::{create_ui_font, draw_round_box, fill_solid, rgb};

const OSD_CLASS: &str = "MSKeySwitchOSD";
const OSD_TIMER_ID: usize = 91;

static OSD_HWND: AtomicUsize = AtomicUsize::new(0);
static OSD_IS_VI: AtomicBool = AtomicBool::new(true);
static OSD_CLASS_READY: AtomicBool = AtomicBool::new(false);

fn ensure_osd_class() {
    if OSD_CLASS_READY.swap(true, Ordering::SeqCst) {
        return;
    }
    let hinst = unsafe { GetModuleHandleW(ptr::null()) };
    let cls = to_wide(OSD_CLASS);
    let mut wc: WNDCLASSEXW = unsafe { std::mem::zeroed() };
    wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
    wc.lpfnWndProc = Some(osd_wnd_proc);
    wc.hInstance = hinst;
    wc.lpszClassName = cls.as_ptr();
    unsafe {
        RegisterClassExW(&wc);
    }
}

fn osd_window() -> HWND {
    let existing = OSD_HWND.load(Ordering::SeqCst) as HWND;
    if !existing.is_null() {
        return existing;
    }
    ensure_osd_class();
    let cls = to_wide(OSD_CLASS);
    let empty = to_wide("");
    let hinst = unsafe { GetModuleHandleW(ptr::null()) };
    let hw = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            cls.as_ptr(),
            empty.as_ptr(),
            WS_POPUP,
            0,
            0,
            222,
            48,
            ptr::null_mut(),
            ptr::null_mut(),
            hinst,
            ptr::null(),
        )
    };
    if !hw.is_null() {
        unsafe {
            SetLayeredWindowAttributes(hw, 0, 236, LWA_ALPHA);
        }
        OSD_HWND.store(hw as usize, Ordering::SeqCst);
    }
    hw
}

pub fn show_switch_osd(vietnamese: bool) {
    OSD_IS_VI.store(vietnamese, Ordering::SeqCst);
    let hw = osd_window();
    if hw.is_null() {
        return;
    }
    let dpi = window_dpi(hw);
    let w = scale_px(222, dpi);
    let h = scale_px(48, dpi);
    let anchor = unsafe { GetForegroundWindow() };
    let mon = unsafe { MonitorFromWindow(anchor, MONITOR_DEFAULTTONEAREST) };
    let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    let (x, y) = if unsafe { GetMonitorInfoW(mon, &mut mi) } != 0 {
        let wa = mi.rcWork;
        (wa.right - w - scale_px(24, dpi), wa.bottom - h - scale_px(20, dpi))
    } else {
        (scale_px(40, dpi), scale_px(40, dpi))
    };
    unsafe {
        SetWindowPos(hw, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
        ShowWindow(hw, SW_SHOWNOACTIVATE);
        InvalidateRect(hw, ptr::null(), 1);
        KillTimer(hw, OSD_TIMER_ID);
        SetTimer(hw, OSD_TIMER_ID, 950, None);
    }
}

unsafe extern "system" fn osd_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_TIMER => {
            unsafe {
                KillTimer(hwnd, OSD_TIMER_ID);
                ShowWindow(hwnd, SW_HIDE);
            }
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            let pal = current_palette();
            let dpi = window_dpi(hwnd);
            let s = |v: i32| scale_px(v, dpi);
            fill_solid(hdc, &rc, pal.window_bg);
            draw_round_box(hdc, &rc, s(10), pal.card_bg, pal.border);
            unsafe { DrawIconEx(hdc, s(10), s(10), crate::tray::app_icon(32), s(28), s(28), 0, ptr::null_mut(), DI_NORMAL) };

            let is_vi = OSD_IS_VI.load(Ordering::SeqCst);
            let badge_fill = if is_vi { pal.danger_badge } else { pal.accent };
            let badge_rc = RECT {
                left: s(48),
                top: s(9),
                right: s(82),
                bottom: rc.bottom - s(9),
            };
            draw_round_box(hdc, &badge_rc, s(7), badge_fill, badge_fill);

            let badge_font = create_ui_font(s(16), true);
            let text_font = create_ui_font(s(15), true);
            let mut b_txt = to_wide(if is_vi { "V" } else { "E" });
            let mut label = to_wide(if is_vi { "Tiếng Việt" } else { "English" });
            let mut l_rc = RECT {
                left: s(92),
                top: 0,
                right: rc.right - s(10),
                bottom: rc.bottom,
            };
            let mut b_rc = badge_rc;
            unsafe {
                SetBkMode(hdc, 1);
                let of = SelectObject(hdc, badge_font);
                SetTextColor(hdc, rgb(255, 255, 255));
                DrawTextW(hdc, b_txt.as_mut_ptr(), -1, &mut b_rc, 0x0825);
                SelectObject(hdc, text_font);
                SetTextColor(hdc, pal.text);
                DrawTextW(hdc, label.as_mut_ptr(), -1, &mut l_rc, 0x0824);
                SelectObject(hdc, of);
                DeleteObject(badge_font);
                DeleteObject(text_font);
                EndPaint(hwnd, &ps);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
