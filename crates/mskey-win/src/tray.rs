use std::ptr;

use mskey_core::{Charset, InputMethod};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateCompatibleDC, CreateDIBSection,
    CreateFontW, CreateSolidBrush, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteDC,
    DeleteObject, DrawTextW, HDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos,
    HICON, ICONINFO, MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
    SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TrackPopupMenu,
};

use crate::platform::to_wide;
use crate::state::WM_APP_TRAY;

pub const ID_TRAY_SHOW: usize = 4001;
pub const ID_TRAY_TOGGLE_VE: usize = 4002;
pub const ID_TRAY_CHARSET_UNI: usize = 4010;
pub const ID_TRAY_CHARSET_TCVN: usize = 4011;
pub const ID_TRAY_CHARSET_VNI: usize = 4012;
pub const ID_TRAY_CHARSET_COMP: usize = 4013;
pub const ID_TRAY_TYPE_TELEX: usize = 4020;
pub const ID_TRAY_TYPE_VNI: usize = 4021;
pub const ID_TRAY_TYPE_VIQR: usize = 4022;
pub const ID_TRAY_TYPE_SIMPLE: usize = 4023;
pub const ID_TRAY_TYPE_CUSTOM: usize = 4024;
pub const ID_TRAY_MACROS: usize = 4030;
pub const ID_TRAY_CONVERT: usize = 4031;
pub const ID_TRAY_APPS: usize = 4032;
pub const ID_TRAY_EXIT: usize = 4099;

pub struct TrayController {
    hwnd: HWND,
    icon_v: HICON,
    icon_e: HICON,
    installed: bool,
}

impl Drop for TrayController {
    fn drop(&mut self) {
        self.remove();
        unsafe {
            if !self.icon_v.is_null() {
                DestroyIcon(self.icon_v);
            }
            if !self.icon_e.is_null() {
                DestroyIcon(self.icon_e);
            }
        }
    }
}

impl TrayController {
    pub fn new(hwnd: HWND) -> Self {
        let icon_v = create_badge_icon("V", 0xD1, 0x34, 0x38);
        let icon_e = create_badge_icon("E", 0x00, 0x78, 0xD4);
        Self {
            hwnd,
            icon_v,
            icon_e,
            installed: false,
        }
    }

    pub fn prepare_update(&self, vietnamese: bool) -> (u32, NOTIFYICONDATAW, HICON) {
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_APP_TRAY;
        let icon = self.current_icon(vietnamese);
        nid.hIcon = icon;
        let tip = if vietnamese {
            "MS Key - Tiếng Việt (V)"
        } else {
            "MS Key - Tiếng Anh (E)"
        };
        let wide = to_wide(tip);
        let copy_len = wide.len().min(nid.szTip.len());
        nid.szTip[..copy_len].copy_from_slice(&wide[..copy_len]);
        let msg = if self.installed { NIM_MODIFY } else { NIM_ADD };
        (msg, nid, icon)
    }

    pub fn mark_installed(&mut self) {
        self.installed = true;
    }

    pub fn update(&mut self, vietnamese: bool) {
        let (msg, nid, _) = self.prepare_update(vietnamese);
        if unsafe { Shell_NotifyIconW(msg, &nid) } != 0 {
            self.installed = true;
        }
    }

    pub fn remove(&mut self) {
        if !self.installed {
            return;
        }
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = self.hwnd;
        nid.uID = 1;
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &nid);
        }
        self.installed = false;
    }

    pub fn current_icon(&self, vietnamese: bool) -> HICON {
        if vietnamese { self.icon_v } else { self.icon_e }
    }
}

pub fn show_tray_menu(hwnd: HWND, vietnamese: bool, charset: Charset, method: InputMethod) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let toggle_txt = if vietnamese {
            "Chế độ: Tiếng Việt (chuyển sang E)"
        } else {
            "Chế độ: Tiếng Anh (chuyển sang V)"
        };
        append_item(menu, ID_TRAY_SHOW, "Mở Bảng điều khiển", false);
        append_item(menu, ID_TRAY_TOGGLE_VE, toggle_txt, false);
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());

        let charset_menu = CreatePopupMenu();
        append_item(charset_menu, ID_TRAY_CHARSET_UNI, "Unicode (Dựng sẵn)", charset == Charset::Unicode);
        append_item(charset_menu, ID_TRAY_CHARSET_TCVN, "TCVN3 (ABC)", charset == Charset::Tcvn3);
        append_item(charset_menu, ID_TRAY_CHARSET_VNI, "VNI Windows", charset == Charset::VniWindows);
        append_item(charset_menu, ID_TRAY_CHARSET_COMP, "Unicode tổ hợp", charset == Charset::UnicodeComposite);
        let c_lbl = to_wide("Bảng mã");
        AppendMenuW(menu, MF_POPUP, charset_menu as usize, c_lbl.as_ptr());

        let method_menu = CreatePopupMenu();
        append_item(method_menu, ID_TRAY_TYPE_TELEX, "Telex", method == InputMethod::Telex);
        append_item(method_menu, ID_TRAY_TYPE_VNI, "VNI", method == InputMethod::Vni);
        append_item(method_menu, ID_TRAY_TYPE_VIQR, "VIQR", method == InputMethod::Viqr);
        append_item(method_menu, ID_TRAY_TYPE_SIMPLE, "Simple Telex", method == InputMethod::SimpleTelex);
        append_item(method_menu, ID_TRAY_TYPE_CUSTOM, "Tự định nghĩa", method == InputMethod::Custom);
        let m_lbl = to_wide("Kiểu gõ");
        AppendMenuW(menu, MF_POPUP, method_menu as usize, m_lbl.as_ptr());

        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
        append_item(menu, ID_TRAY_MACROS, "Bảng gõ tắt...", false);
        append_item(menu, ID_TRAY_CONVERT, "Chuyển mã Clipboard...", false);
        append_item(menu, ID_TRAY_APPS, "Danh sách ứng dụng...", false);
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
        append_item(menu, ID_TRAY_EXIT, "Thoát MS Key", false);

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, ptr::null());
        DestroyMenu(menu);
    }
}

unsafe fn append_item(menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU, id: usize, text: &str, checked: bool) {
    let wide = to_wide(text);
    let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
    unsafe {
        AppendMenuW(menu, flags, id, wide.as_ptr());
    }
}

fn fill_badge_pixels(pixels: &mut [u32], size: i32, r: u8, g: u8, b: u8) {
    let radius = 6.0f32;
    let half = 15.5f32;
    let max_c = 13.5f32 - radius;
    for y in 0..size {
        for x in 0..size {
            let dx = ((x as f32 + 0.5 - half).abs() - max_c).max(0.0);
            let dy = ((y as f32 + 0.5 - half).abs() - max_c).max(0.0);
            let dist = (dx * dx + dy * dy).sqrt() - radius;
            let cov = (0.5 - dist).clamp(0.0, 1.0);
            let a = (cov * 255.0).round() as u32;
            let pr = ((r as f32 * cov).round() as u32) & 0xFF;
            let pg = ((g as f32 * cov).round() as u32) & 0xFF;
            let pb = ((b as f32 * cov).round() as u32) & 0xFF;
            pixels[(y * size + x) as usize] = (a << 24) | (pr << 16) | (pg << 8) | pb;
        }
    }
}

fn draw_badge_glyph(dc: HDC, pixels: &mut [u32], size: i32, letter: &str) {
    let font_name = to_wide("Segoe UI");
    let hfont = unsafe {
        CreateFontW(24, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 4, 0, font_name.as_ptr())
    };
    let old_font = unsafe { SelectObject(dc, hfont) };
    unsafe {
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0x00FF_FFFF);
        let mut rc = RECT { left: 0, top: 0, right: size, bottom: size };
        let mut wide_txt = to_wide(letter);
        DrawTextW(dc, wide_txt.as_mut_ptr(), -1, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }
    for px in pixels.iter_mut() {
        let (b_val, g_val, r_val, a_val) = (*px & 0xFF, (*px >> 8) & 0xFF, (*px >> 16) & 0xFF, (*px >> 24) & 0xFF);
        if a_val == 0 && (r_val | g_val | b_val) > 0 {
            *px = (r_val.max(g_val).max(b_val) << 24) | 0x00FF_FFFF;
        }
    }
    unsafe {
        SelectObject(dc, old_font);
        DeleteObject(hfont);
    }
}

pub fn create_badge_icon(letter: &str, r: u8, g: u8, b: u8) -> HICON {
    let size = 32i32;
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [unsafe { std::mem::zeroed() }],
    };
    let mut bits_ptr: *mut std::ffi::c_void = ptr::null_mut();
    let dc = unsafe { CreateCompatibleDC(ptr::null_mut()) };
    let hbm = unsafe { CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits_ptr, ptr::null_mut(), 0) };
    if hbm.is_null() || bits_ptr.is_null() {
        unsafe { DeleteDC(dc) };
        return ptr::null_mut();
    }
    let old_bm = unsafe { SelectObject(dc, hbm) };
    let pixels = unsafe { std::slice::from_raw_parts_mut(bits_ptr as *mut u32, (size * size) as usize) };
    fill_badge_pixels(pixels, size, r, g, b);
    draw_badge_glyph(dc, pixels, size, letter);
    unsafe { SelectObject(dc, old_bm) };

    let mask_bm = unsafe { CreateBitmap(size, size, 1, 1, ptr::null()) };
    let icon_info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask_bm, hbmColor: hbm };
    let hicon = unsafe { CreateIconIndirect(&icon_info) };
    unsafe {
        DeleteObject(mask_bm);
        DeleteObject(hbm);
        DeleteDC(dc);
    }
    let _ = CreateSolidBrush;
    hicon
}
