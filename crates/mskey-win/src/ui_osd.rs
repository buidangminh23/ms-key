use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, ClientToScreen, DeleteObject, DrawTextW, EndPaint, GetMonitorInfoW,
    InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MonitorFromWindow,
    PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DI_NORMAL, DefWindowProcW, DrawIconEx, GUITHREADINFO, GetClientRect,
    GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetWindowRect, GetWindowThreadProcessId,
    HWND_TOPMOST, KillTimer, LWA_ALPHA, RegisterClassExW, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SetLayeredWindowAttributes, SetTimer, SetWindowPos, ShowWindow, WM_ERASEBKGND,
    WM_NCHITTEST, WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::dialog_helpers::{current_palette, scale_px, window_dpi};
use crate::platform::to_wide;
use crate::state::with_state;
use crate::theme::{create_ui_font, draw_round_box, fill_solid, rgb};

const OSD_CLASS: &str = "MSKeySwitchOSD";
const CARET_OSD_CLASS: &str = "MSKeyCaretHUD";
const OSD_TIMER_ID: usize = 91;

static OSD_HWND: AtomicUsize = AtomicUsize::new(0);
static OSD_IS_VI: AtomicBool = AtomicBool::new(true);
static OSD_CLASS_READY: AtomicBool = AtomicBool::new(false);

static CARET_HWND: AtomicUsize = AtomicUsize::new(0);
static CARET_IS_VI: AtomicBool = AtomicBool::new(true);
static CARET_CLASS_READY: AtomicBool = AtomicBool::new(false);

fn ensure_osd_class(flag: &AtomicBool, name: &str, proc_fn: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT) {
    if flag.swap(true, Ordering::SeqCst) {
        return;
    }
    let hinst = unsafe { GetModuleHandleW(ptr::null()) };
    let cls = to_wide(name);
    let mut wc: WNDCLASSEXW = unsafe { std::mem::zeroed() };
    wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
    wc.lpfnWndProc = Some(proc_fn);
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
    ensure_osd_class(&OSD_CLASS_READY, OSD_CLASS, osd_wnd_proc);
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

fn caret_window() -> HWND {
    let existing = CARET_HWND.load(Ordering::SeqCst) as HWND;
    if !existing.is_null() {
        return existing;
    }
    ensure_osd_class(&CARET_CLASS_READY, CARET_OSD_CLASS, caret_wnd_proc);
    let cls = to_wide(CARET_OSD_CLASS);
    let empty = to_wide("");
    let hinst = unsafe { GetModuleHandleW(ptr::null()) };
    let hw = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            cls.as_ptr(),
            empty.as_ptr(),
            WS_POPUP,
            0,
            0,
            28,
            22,
            ptr::null_mut(),
            ptr::null_mut(),
            hinst,
            ptr::null(),
        )
    };
    if !hw.is_null() {
        unsafe {
            SetLayeredWindowAttributes(hw, 0, 230, LWA_ALPHA);
        }
        CARET_HWND.store(hw as usize, Ordering::SeqCst);
    }
    hw
}

pub fn clamp_to_work_area(x: i32, y: i32, w: i32, h: i32, pt: POINT, anchor: HWND) -> (i32, i32) {
    let mon = if !anchor.is_null() {
        unsafe { MonitorFromWindow(anchor, MONITOR_DEFAULTTONEAREST) }
    } else {
        unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) }
    };
    let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(mon, &mut mi) } != 0 {
        let wa = mi.rcWork;
        let max_x = (wa.right - w - 4).max(wa.left + 4);
        let max_y = (wa.bottom - h - 4).max(wa.top + 4);
        (x.clamp(wa.left + 4, max_x), y.clamp(wa.top + 4, max_y))
    } else {
        (x.max(4), y.max(4))
    }
}

pub fn query_caret_screen_point(fg_override: Option<HWND>) -> (Option<POINT>, HWND) {
    let fg = fg_override
        .filter(|h| !h.is_null())
        .unwrap_or_else(|| unsafe { GetForegroundWindow() });
    let tid = if !fg.is_null() {
        unsafe { GetWindowThreadProcessId(fg, ptr::null_mut()) }
    } else {
        0
    };
    let mut gti: GUITHREADINFO = unsafe { std::mem::zeroed() };
    gti.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if unsafe { GetGUIThreadInfo(tid, &mut gti) } != 0 && !gti.hwndCaret.is_null() {
        let rc = gti.rcCaret;
        let non_empty = !(rc.left == 0 && rc.top == 0 && rc.right == 0 && rc.bottom == 0)
            && (rc.right > rc.left || rc.bottom > rc.top);
        if non_empty {
            let mut pt = POINT {
                x: rc.left,
                y: rc.bottom.max(rc.top),
            };
            if unsafe { ClientToScreen(gti.hwndCaret, &mut pt) } != 0 {
                return (Some(pt), fg);
            }
        }
    }
    (None, fg)
}

pub fn hide_caret_osd() {
    let existing = CARET_HWND.load(Ordering::SeqCst) as HWND;
    if !existing.is_null() {
        unsafe {
            ShowWindow(existing, SW_HIDE);
        }
    }
}

pub fn update_caret_osd(vietnamese: bool) {
    let (enabled, blacklisted) = with_state(|st| (st.engine.config().show_caret_osd, st.is_blacklisted()));
    if !enabled || blacklisted {
        hide_caret_osd();
        return;
    }
    CARET_IS_VI.store(vietnamese, Ordering::SeqCst);
    let hw = caret_window();
    if hw.is_null() {
        return;
    }
    let dpi = window_dpi(hw);
    let s = |v: i32| scale_px(v, dpi);
    let w = s(26);
    let h = s(22);
    let (caret_pt, fg) = query_caret_screen_point(None);
    let (raw_x, raw_y, anchor_pt) = if let Some(pt) = caret_pt {
        (pt.x + s(6), pt.y + s(4), pt)
    } else {
        let mut fg_rc: RECT = unsafe { std::mem::zeroed() };
        if !fg.is_null()
            && unsafe { GetWindowRect(fg, &mut fg_rc) } != 0
            && fg_rc.right - fg_rc.left > w + s(32)
            && fg_rc.bottom - fg_rc.top > h + s(48)
        {
            let x = fg_rc.right - w - s(18);
            let y = fg_rc.top + s(42);
            (x, y, POINT { x, y })
        } else {
            let mut cur: POINT = unsafe { std::mem::zeroed() };
            unsafe {
                GetCursorPos(&mut cur);
            }
            (cur.x + s(12), cur.y + s(16), cur)
        }
    };
    let (x, y) = clamp_to_work_area(raw_x, raw_y, w, h, anchor_pt, fg);
    unsafe {
        SetWindowPos(hw, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
        ShowWindow(hw, SW_SHOWNOACTIVATE);
        InvalidateRect(hw, ptr::null(), 1);
    }
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

unsafe extern "system" fn caret_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_NCHITTEST => -1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            let pal = current_palette();
            let dpi = window_dpi(hwnd);
            let s = |v: i32| scale_px(v, dpi);
            let is_vi = CARET_IS_VI.load(Ordering::SeqCst);
            let fill = if is_vi { pal.danger_badge } else { pal.accent };
            fill_solid(hdc, &rc, pal.window_bg);
            draw_round_box(hdc, &rc, s(6), fill, pal.border);
            let font = create_ui_font(s(13), true);
            let mut txt = to_wide(if is_vi { "V" } else { "E" });
            let mut t_rc = rc;
            unsafe {
                SetBkMode(hdc, 1);
                let of = SelectObject(hdc, font);
                SetTextColor(hdc, rgb(255, 255, 255));
                DrawTextW(hdc, txt.as_mut_ptr(), -1, &mut t_rc, 0x0825);
                SelectObject(hdc, of);
                DeleteObject(font);
                EndPaint(hwnd, &ps);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
