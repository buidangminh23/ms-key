use std::ptr;

use mskey_core::ThemeMode;
use windows_sys::Win32::Foundation::{COLORREF, HWND, RECT};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, CreatePen, CreateSolidBrush, DT_CENTER, DT_LEFT, DT_SINGLELINE, DT_VCENTER,
    DeleteObject, DrawTextW, Ellipse, FillRect, HDC, HFONT, LineTo, MoveToEx, PS_SOLID, RoundRect,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW};

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
            window_bg: rgb(31, 31, 31),
            card_bg: rgb(43, 43, 43),
            input_bg: rgb(53, 53, 53),
            border: rgb(66, 66, 66),
            text: rgb(242, 242, 242),
            subtle_text: rgb(168, 168, 168),
            accent: rgb(54, 158, 255),
            button_bg: rgb(54, 54, 54),
            button_border: rgb(82, 82, 82),
            tab_inactive_bg: rgb(36, 36, 36),
            danger_badge: rgb(209, 52, 56),
        }
    } else {
        ThemePalette {
            dark: false,
            window_bg: rgb(243, 243, 243),
            card_bg: rgb(255, 255, 255),
            input_bg: rgb(255, 255, 255),
            border: rgb(214, 214, 214),
            text: rgb(26, 26, 26),
            subtle_text: rgb(90, 90, 90),
            accent: rgb(0, 120, 212),
            button_bg: rgb(255, 255, 255),
            button_border: rgb(204, 204, 204),
            tab_inactive_bg: rgb(232, 232, 232),
            danger_badge: rgb(209, 52, 56),
        }
    }
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
        let pen = CreatePen(PS_SOLID as i32, 1, border);
        let old_br = SelectObject(hdc, br);
        let old_pen = SelectObject(hdc, pen);
        RoundRect(hdc, rc.left, rc.top, rc.right, rc.bottom, radius, radius);
        SelectObject(hdc, old_br);
        SelectObject(hdc, old_pen);
        DeleteObject(br);
        DeleteObject(pen);
    }
}

pub fn draw_group_box(hdc: HDC, rc: &RECT, title: &str, pal: ThemePalette, font: HFONT) {
    draw_round_box(hdc, rc, 8, pal.card_bg, pal.border);
    unsafe {
        let old_font = SelectObject(hdc, font);
        let pad = 12;
        let bg_rect = RECT {
            left: rc.left + pad,
            top: rc.top - 9,
            right: rc.left + pad + (title.chars().count() as i32) * 7 + 14,
            bottom: rc.top + 10,
        };
        fill_solid(hdc, &bg_rect, pal.card_bg);
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, pal.accent);
        let mut w = to_wide(title);
        let mut tx = RECT {
            left: bg_rect.left + 4,
            top: bg_rect.top,
            right: bg_rect.right + 16,
            bottom: bg_rect.bottom,
        };
        DrawTextW(hdc, w.as_mut_ptr(), -1, &mut tx, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        SelectObject(hdc, old_font);
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
            let pen = CreatePen(PS_SOLID as i32, 2, rgb(255, 255, 255));
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
        DrawTextW(hdc, w.as_mut_ptr(), -1, &mut tx, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
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
            PS_SOLID as i32,
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
            let npen = CreatePen(PS_SOLID as i32, 1, pal.accent);
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
        DrawTextW(hdc, w.as_mut_ptr(), -1, &mut tx, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
        SelectObject(hdc, old_font);
    }
}

pub fn draw_owner_button(
    hdc: HDC,
    rc: &RECT,
    text: &str,
    primary: bool,
    pal: ThemePalette,
    font: HFONT,
) {
    let (fill, border, fg) = if primary {
        (pal.accent, pal.accent, rgb(255, 255, 255))
    } else {
        (pal.button_bg, pal.button_border, pal.text)
    };
    draw_round_box(hdc, rc, 6, fill, border);
    unsafe {
        let old_font = SelectObject(hdc, font);
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, fg);
        let mut w = to_wide(text);
        let mut tx = *rc;
        DrawTextW(hdc, w.as_mut_ptr(), -1, &mut tx, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        SelectObject(hdc, old_font);
    }
}
