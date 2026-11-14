use std::ptr;

use mskey_core::HotkeyCombo;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreatePen, DeleteObject, HBRUSH, HFONT, LineTo, MoveToEx, PS_SOLID, SetBkColor, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{
    DRAWITEMSTRUCT, ODS_SELECTED, ODT_BUTTON, ODT_COMBOBOX,
};
use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BS_OWNERDRAW, CBS_DROPDOWNLIST, CBS_HASSTRINGS, CBS_OWNERDRAWFIXED, CB_ADDSTRING, CB_GETCURSEL,
    CB_GETLBTEXT, CB_SETCURSEL, CB_SETITEMHEIGHT, CreateWindowExW, DispatchMessageW, ES_AUTOHSCROLL,
    ES_MULTILINE, ES_WANTRETURN, GetMessageW, GetSystemMetrics, GetWindowTextLengthW,
    GetWindowTextW, HMENU, IsWindow, MSG, RegisterClassExW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
    SendMessageW, SetWindowTextW, ShowWindow, TranslateMessage, WM_CTLCOLOREDIT,
    WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_SETFONT, WNDCLASSEXW, WS_BORDER, WS_CAPTION,
    WS_CHILD, WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

use crate::platform::to_wide;
use crate::state::with_state;
use crate::theme::{
    ThemePalette, apply_dwm_dark_titlebar, draw_owner_button, fill_solid, palette_for, rgb,
};

pub fn scale_px(px: i32, dpi: i32) -> i32 {
    (px * dpi + 48) / 96
}

pub fn window_dpi(hwnd: HWND) -> i32 {
    let d = unsafe { GetDpiForWindow(hwnd) };
    if d == 0 { 96 } else { d as i32 }
}

pub fn register_tool_class(
    class_name: &str,
    wnd_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
) {
    let hinst = unsafe { GetModuleHandleW(ptr::null()) };
    let cname = to_wide(class_name);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0x0003,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: ptr::null_mut(),
        hCursor: ptr::null_mut(),
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: cname.as_ptr(),
        hIconSm: ptr::null_mut(),
    };
    unsafe {
        RegisterClassExW(&wc);
    }
}

pub fn create_modal_window(
    parent: HWND,
    class_name: &str,
    title: &str,
    client_w: i32,
    client_h: i32,
) -> HWND {
    let hinst = unsafe { GetModuleHandleW(ptr::null()) };
    let cname = to_wide(class_name);
    let wtitle = to_wide(title);
    let dpi = if parent.is_null() { 96 } else { window_dpi(parent) };
    let style = WS_POPUP | WS_CAPTION | WS_SYSMENU;
    let ex_style = WS_EX_DLGMODALFRAME;
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: scale_px(client_w, dpi),
        bottom: scale_px(client_h, dpi),
    };
    unsafe {
        AdjustWindowRectExForDpi(&mut rc, style, 0, ex_style, dpi as u32);
    }
    let sw = rc.right - rc.left;
    let sh = rc.bottom - rc.top;
    let sx = (unsafe { GetSystemMetrics(SM_CXSCREEN) } - sw) / 2;
    let sy = (unsafe { GetSystemMetrics(SM_CYSCREEN) } - sh) / 2;
    let hwnd = unsafe {
        CreateWindowExW(
            ex_style,
            cname.as_ptr(),
            wtitle.as_ptr(),
            style,
            sx.max(0),
            sy.max(0),
            sw,
            sh,
            parent,
            ptr::null_mut(),
            hinst,
            ptr::null(),
        )
    };
    let pal = current_palette();
    apply_dwm_dark_titlebar(hwnd, pal.dark);
    hwnd
}

pub fn run_modal_loop(parent: HWND, modal: HWND) {
    if !parent.is_null() {
        unsafe {
            EnableWindow(parent, 0);
        }
    }
    unsafe {
        ShowWindow(modal, SW_SHOW);
        let mut msg: MSG = std::mem::zeroed();
        while IsWindow(modal) != 0 && GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if !parent.is_null() {
            EnableWindow(parent, 1);
        }
    }
}

pub fn current_palette() -> ThemePalette {
    let mode = with_state(|st| st.engine.config().theme);
    palette_for(mode)
}

pub fn make_child(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: usize,
    font: HFONT,
    dpi: i32,
) -> HWND {
    let c = to_wide(class);
    let t = to_wide(text);
    let hinst = unsafe { GetModuleHandleW(ptr::null()) };
    let hw = unsafe {
        CreateWindowExW(
            0,
            c.as_ptr(),
            t.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            scale_px(x, dpi),
            scale_px(y, dpi),
            scale_px(w, dpi),
            scale_px(h, dpi),
            parent,
            id as HMENU,
            hinst,
            ptr::null(),
        )
    };
    if !font.is_null() {
        unsafe {
            SendMessageW(hw, WM_SETFONT, font as WPARAM, 1);
        }
    }
    hw
}

pub fn make_owner_button(
    parent: HWND,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: usize,
    font: HFONT,
    dpi: i32,
) -> HWND {
    make_child(
        parent,
        "BUTTON",
        text,
        (BS_OWNERDRAW as u32) | WS_TABSTOP,
        x,
        y,
        w,
        h,
        id,
        font,
        dpi,
    )
}

pub fn make_combo(
    parent: HWND,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: usize,
    items: &[&str],
    selected: usize,
    font: HFONT,
    dpi: i32,
) -> HWND {
    let style = ((CBS_DROPDOWNLIST | CBS_OWNERDRAWFIXED | CBS_HASSTRINGS) as u32)
        | WS_TABSTOP
        | WS_VSCROLL;
    let hw = make_child(parent, "COMBOBOX", "", style, x, y, w, 180, id, font, dpi);
    let ih = scale_px(h - 6, dpi).max(18);
    unsafe {
        SendMessageW(hw, CB_SETITEMHEIGHT, -1i32 as usize, ih as LPARAM);
        SendMessageW(hw, CB_SETITEMHEIGHT, 0, scale_px(h, dpi) as LPARAM);
        for item in items {
            let w_item = to_wide(item);
            SendMessageW(hw, CB_ADDSTRING, 0, w_item.as_ptr() as LPARAM);
        }
        SendMessageW(hw, CB_SETCURSEL, selected, 0);
    }
    hw
}

pub fn make_edit(
    parent: HWND,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: usize,
    multiline: bool,
    font: HFONT,
    dpi: i32,
) -> HWND {
    let extra = if multiline {
        (ES_MULTILINE as u32) | (ES_WANTRETURN as u32) | WS_VSCROLL
    } else {
        ES_AUTOHSCROLL as u32
    };
    make_child(
        parent,
        "EDIT",
        text,
        WS_BORDER | WS_TABSTOP | extra,
        x,
        y,
        w,
        h,
        id,
        font,
        dpi,
    )
}

pub fn read_window_text(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let copied = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..copied.max(0) as usize])
}

pub fn write_window_text(hwnd: HWND, text: &str) {
    let w = to_wide(text);
    unsafe {
        SetWindowTextW(hwnd, w.as_ptr());
    }
}

pub fn combo_index(hwnd: HWND) -> usize {
    let idx = unsafe { SendMessageW(hwnd, CB_GETCURSEL, 0, 0) };
    idx.max(0) as usize
}

pub fn handle_ctl_color(
    msg: u32,
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    pal: ThemePalette,
    bg_brush: HBRUSH,
    input_brush: HBRUSH,
) -> LRESULT {
    let (fg, bg, br) = match msg {
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => (pal.text, pal.input_bg, input_brush),
        WM_CTLCOLORSTATIC => (pal.text, pal.card_bg, bg_brush),
        _ => (pal.text, pal.window_bg, bg_brush),
    };
    unsafe {
        SetTextColor(hdc, fg);
        SetBkColor(hdc, bg);
    }
    br as LRESULT
}

pub fn handle_draw_combo_or_btn(
    lparam: LPARAM,
    primary_ids: &[u32],
    pal: ThemePalette,
    font: HFONT,
) -> Option<LRESULT> {
    if lparam == 0 {
        return None;
    }
    let dis = unsafe { &*(lparam as *const DRAWITEMSTRUCT) };
    if dis.CtlType == ODT_BUTTON {
        let txt = read_window_text(dis.hwndItem);
        let primary = primary_ids.contains(&dis.CtlID);
        draw_owner_button(dis.hDC, &dis.rcItem, &txt, primary, pal, font);
        return Some(1);
    }
    if dis.CtlType == ODT_COMBOBOX {
        let in_edit = (dis.itemState & 0x1000) != 0;
        let selected = !in_edit && (dis.itemState & ODS_SELECTED) != 0;
        let fill = if selected { pal.accent } else { pal.input_bg };
        let fg = if selected { rgb(255, 255, 255) } else { pal.text };
        fill_solid(dis.hDC, &dis.rcItem, fill);
        if dis.itemID != u32::MAX {
            let mut buf = [0u16; 128];
            let len = unsafe {
                SendMessageW(
                    dis.hwndItem,
                    CB_GETLBTEXT,
                    dis.itemID as usize,
                    buf.as_mut_ptr() as LPARAM,
                )
            };
            if len > 0 {
                let s = String::from_utf16_lossy(&buf[..len as usize]);
                let mut tx = dis.rcItem;
                tx.left += 8;
                tx.right -= 18;
                unsafe {
                    let old_font = windows_sys::Win32::Graphics::Gdi::SelectObject(dis.hDC, font);
                    SetBkMode(dis.hDC, TRANSPARENT as i32);
                    SetTextColor(dis.hDC, fg);
                    let mut w = to_wide(&s);
                    windows_sys::Win32::Graphics::Gdi::DrawTextW(
                        dis.hDC,
                        w.as_mut_ptr(),
                        -1,
                        &mut tx,
                        0x24,
                    );
                    windows_sys::Win32::Graphics::Gdi::SelectObject(dis.hDC, old_font);
                }
            }
        }
        if in_edit {
            unsafe {
                let pen = CreatePen(PS_SOLID as i32, 1, pal.subtle_text);
                let old_pen = windows_sys::Win32::Graphics::Gdi::SelectObject(dis.hDC, pen);
                let cx = dis.rcItem.right - 10;
                let cy = (dis.rcItem.top + dis.rcItem.bottom) / 2;
                MoveToEx(dis.hDC, cx - 3, cy - 2, ptr::null_mut());
                LineTo(dis.hDC, cx, cy + 2);
                LineTo(dis.hDC, cx + 4, cy - 2);
                windows_sys::Win32::Graphics::Gdi::SelectObject(dis.hDC, old_pen);
                DeleteObject(pen);
            }
        }
        return Some(1);
    }
    None
}

pub fn format_hotkey(c: HotkeyCombo) -> String {
    let mut parts = Vec::new();
    if c.ctrl {
        parts.push("Ctrl".to_string());
    }
    if c.alt {
        parts.push("Alt".to_string());
    }
    if c.shift {
        parts.push("Shift".to_string());
    }
    if c.win_cmd {
        parts.push("Win".to_string());
    }
    if c.key_code != 0 {
        let key_name = match c.key_code {
            0x70..=0x7B => format!("F{}", c.key_code - 0x6F),
            0x41..=0x5A | 0x30..=0x39 => ((c.key_code as u8) as char).to_string(),
            0x20 => "Space".into(),
            k => format!("VK_{k:02X}"),
        };
        parts.push(key_name);
    }
    if parts.is_empty() {
        "(Chưa đặt)".into()
    } else {
        parts.join(" + ")
    }
}
