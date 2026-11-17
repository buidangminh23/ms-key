use std::ptr;

use mskey_core::ThemeMode;
use windows_sys::Win32::Foundation::{COLORREF, HWND, RECT};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, CreatePen, CreateSolidBrush, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX,
    DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawFocusRect, DrawTextW, Ellipse, FillRect, HDC, HFONT, LineTo,
    MoveToEx, PS_SOLID, RoundRect, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW};
use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_FOCUS, ODS_NOFOCUSRECT, ODS_SELECTED, SetWindowTheme};

use crate::platform::to_wide;

const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub dark: bool,
    pub window_bg: COLORREF,
    pub card_bg: COLORREF,
    pub input_bg: COLORREF,
    pub border: COLORREF,
    pub text: COLORREF,
    pub subtle_text: COLORREF,
    pub accent: COLORREF,
    pub button_bg: COLORREF,
    pub button_border: COLORREF,
    pub tab_inactive_bg: COLORREF,
    pub danger_badge: COLORREF,
}

pub fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

pub fn resolve_is_dark(mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::System => !query_apps_use_light_theme(),
    }
}

fn query_apps_use_light_theme() -> bool {
    let subkey = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let val = to_wide("AppsUseLightTheme");
    let mut hkey: HKEY = ptr::null_mut();
    let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) };
    if status != 0 || hkey.is_null() {
        return true;
    }
    let mut data: u32 = 1;
    let mut size: u32 = 4;
    let mut kind: u32 = 0;
    let q = unsafe {
        RegQueryValueExW(
            hkey,
            val.as_ptr(),
            ptr::null_mut(),
            &mut kind,
            &mut data as *mut u32 as *mut u8,
            &mut size,
        )
    };
    unsafe {
        RegCloseKey(hkey);
    }
    if q == 0 { data != 0 } else { true }
}

pub fn palette_for(mode: ThemeMode) -> ThemePalette {
    if resolve_is_dark(mode) {
        ThemePalette {
            dark: true,
            window_bg: rgb(17, 21, 29),
            card_bg: rgb(26, 32, 43),
            input_bg: rgb(33, 42, 57),
            border: rgb(52, 63, 81),
            text: rgb(237, 242, 250),
            subtle_text: rgb(163, 178, 200),
            accent: rgb(105, 176, 255),
            button_bg: rgb(33, 42, 57),
            button_border: rgb(97, 116, 144),
            tab_inactive_bg: rgb(23, 28, 38),
            danger_badge: rgb(185, 43, 58),
        }
    } else {
        ThemePalette {
            dark: false,
            window_bg: rgb(241, 245, 251),
            card_bg: rgb(255, 255, 255),
            input_bg: rgb(255, 255, 255),
            border: rgb(213, 223, 237),
            text: rgb(22, 33, 51),
            subtle_text: rgb(79, 96, 120),
            accent: rgb(0, 91, 171),
            button_bg: rgb(255, 255, 255),
            button_border: rgb(138, 157, 184),
            tab_inactive_bg: rgb(230, 237, 247),
            danger_badge: rgb(185, 43, 58),
        }
    }
}

pub fn accent_ink(pal: ThemePalette) -> COLORREF {
    if pal.dark { rgb(13, 26, 44) } else { rgb(255, 255, 255) }
}

pub fn apply_dwm_dark_titlebar(hwnd: HWND, dark: bool) {
    let val: i32 = if dark { 1 } else { 0 };
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &val as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

pub fn apply_combo_theme(hwnd: HWND, dark: bool) {
    let sub = to_wide(if dark { "DarkMode_CFD" } else { "Explorer" });
    unsafe {
        SetWindowTheme(hwnd, sub.as_ptr(), ptr::null());
    }
}

pub fn create_ui_font(height_px: i32, bold: bool) -> HFONT {
    let name = to_wide("Segoe UI");
    unsafe {
        CreateFontW(
            -height_px,
            0,
            0,
            0,
            if bold { 600 } else { 400 },
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            name.as_ptr(),
        )
    }
}

pub fn fill_solid(hdc: HDC, rc: &RECT, color: COLORREF) {
    unsafe {
        let br = CreateSolidBrush(color);
        FillRect(hdc, rc, br);
        DeleteObject(br);
    }
}

pub fn draw_round_box(
    hdc: HDC,
    rc: &RECT,
    radius: i32,
    fill: COLORREF,
    border: COLORREF,
) {
    unsafe {
        let br = CreateSolidBrush(fill);
        let pen = CreatePen(PS_SOLID, 1, border);
        let old_br = SelectObject(hdc, br);
        let old_pen = SelectObject(hdc, pen);
        RoundRect(hdc, rc.left, rc.top, rc.right, rc.bottom, radius, radius);
        SelectObject(hdc, old_br);
        SelectObject(hdc, old_pen);
        DeleteObject(br);
        DeleteObject(pen);
    }
}

pub fn draw_checkbox(
    hdc: HDC,
    rc: &RECT,
    label: &str,
    checked: bool,
    pal: ThemePalette,
    font: HFONT,
    dpi: i32,
) {
    let box_sz = (16 * dpi / 96).max(14);
    let cy = (rc.top + rc.bottom) / 2;
    let bx = RECT {
        left: rc.left,
        top: cy - box_sz / 2,
        right: rc.left + box_sz,
        bottom: cy + box_sz / 2,
    };
    let fill = if checked { pal.accent } else { pal.input_bg };
    let border = if checked { pal.accent } else { pal.button_border };
    draw_round_box(hdc, &bx, 4, fill, border);
    if checked {
        unsafe {
            let pen = CreatePen(PS_SOLID, (2 * dpi / 96).max(1), accent_ink(pal));
            let old = SelectObject(hdc, pen);
            let x1 = bx.left + box_sz / 4;
            let y1 = cy;
            let x2 = bx.left + (box_sz * 7) / 16;
            let y2 = cy + box_sz / 4;
            let x3 = bx.right - box_sz / 5;
            let y3 = cy - box_sz / 4;
            MoveToEx(hdc, x1, y1, ptr::null_mut());
            LineTo(hdc, x2, y2);
            LineTo(hdc, x3, y3);
            SelectObject(hdc, old);
            DeleteObject(pen);
        }
    }
    unsafe {
        let old_font = SelectObject(hdc, font);
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, pal.text);
        let mut tx = RECT {
            left: bx.right + (8 * dpi / 96),
            top: rc.top,
            right: rc.right,
            bottom: rc.bottom,
        };
        let mut w = to_wide(label);
        DrawTextW(
            hdc,
            w.as_mut_ptr(),
            -1,
            &mut tx,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(hdc, old_font);
    }
}

pub fn draw_radio(
    hdc: HDC,
    rc: &RECT,
    label: &str,
    selected: bool,
    pal: ThemePalette,
    font: HFONT,
    dpi: i32,
) {
    let sz = (16 * dpi / 96).max(14);
    let cy = (rc.top + rc.bottom) / 2;
    let cx = rc.left + sz / 2;
    unsafe {
        let pen = CreatePen(
            PS_SOLID,
            1,
            if selected { pal.accent } else { pal.button_border },
        );
        let br = CreateSolidBrush(pal.input_bg);
        let old_pen = SelectObject(hdc, pen);
        let old_br = SelectObject(hdc, br);
        Ellipse(hdc, cx - sz / 2, cy - sz / 2, cx + sz / 2, cy + sz / 2);
        SelectObject(hdc, old_br);
        SelectObject(hdc, old_pen);
        DeleteObject(br);
        DeleteObject(pen);

        if selected {
            let dot = CreateSolidBrush(pal.accent);
            let npen = CreatePen(PS_SOLID, 1, pal.accent);
            let op = SelectObject(hdc, npen);
            let ob = SelectObject(hdc, dot);
            let r = (sz / 4).max(4);
            Ellipse(hdc, cx - r, cy - r, cx + r, cy + r);
            SelectObject(hdc, ob);
            SelectObject(hdc, op);
            DeleteObject(dot);
            DeleteObject(npen);
        }

        let old_font = SelectObject(hdc, font);
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, pal.text);
        let mut tx = RECT {
            left: rc.left + sz + (8 * dpi / 96),
            top: rc.top,
            right: rc.right,
            bottom: rc.bottom,
        };
        let mut w = to_wide(label);
        DrawTextW(
            hdc,
            w.as_mut_ptr(),
            -1,
            &mut tx,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(hdc, old_font);
    }
}

pub fn draw_owner_button(
    item: &DRAWITEMSTRUCT,
    text: &str,
    primary: bool,
    pal: ThemePalette,
    font: HFONT,
    dpi: i32,
) {
    let pressed = (item.itemState & ODS_SELECTED) != 0;
    let (fill, border, fg) = if primary {
        let fill = if pressed { pal.text } else { pal.accent };
        (fill, fill, accent_ink(pal))
    } else {
        let fill = if pressed { pal.tab_inactive_bg } else { pal.button_bg };
        (fill, pal.button_border, pal.text)
    };
    draw_round_box(item.hDC, &item.rcItem, 8 * dpi / 96, fill, border);
    unsafe {
        let old_font = SelectObject(item.hDC, font);
        SetBkMode(item.hDC, TRANSPARENT as i32);
        SetTextColor(item.hDC, fg);
        let mut w = to_wide(text);
        let padding = (4 * dpi / 96).max(2);
        let mut tx = RECT { left: item.rcItem.left + padding, right: item.rcItem.right - padding, ..item.rcItem };
        DrawTextW(
            item.hDC,
            w.as_mut_ptr(),
            -1,
            &mut tx,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        SelectObject(item.hDC, old_font);
    }
    draw_item_focus(item, dpi);
}

pub fn draw_item_focus(item: &DRAWITEMSTRUCT, dpi: i32) {
    if item.itemState & (ODS_FOCUS | ODS_NOFOCUSRECT) == ODS_FOCUS {
        let inset = (2 * dpi / 96).max(2);
        let rc = RECT {
            left: item.rcItem.left + inset,
            top: item.rcItem.top + inset,
            right: item.rcItem.right - inset,
            bottom: item.rcItem.bottom - inset,
        };
        unsafe { DrawFocusRect(item.hDC, &rc) };
    }
}
