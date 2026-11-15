use std::ptr;

use mskey_core::{Charset, InputMethod};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateCompatibleDC, CreateDIBSection,
    CreateFontW, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteDC,
    DeleteObject, DrawTextW, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, DestroyIcon, DestroyMenu, GetCursorPos,
    LoadImageW, IMAGE_ICON, LR_SHARED,
    HICON, ICONINFO, MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
    SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TrackPopupMenu,
};

use crate::platform::to_wide;
use crate::state::WM_APP_TRAY;

pub fn app_icon(size: i32) -> HICON {
    unsafe {
        LoadImageW(GetModuleHandleW(ptr::null()), 101usize as *const u16,
            IMAGE_ICON, size, size, LR_SHARED) as HICON
    }
}

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
pub const ID_TRAY_EXTRAS: usize = 4033;
pub const ID_TRAY_USER_DICT: usize = 4034;
pub const ID_TRAY_QUICK_TELEX: usize = 4035;
pub const ID_TRAY_SHOW_OSD: usize = 4036;
pub const ID_TRAY_CONVERT_SELECTION: usize = 4037;
pub const ID_TRAY_ADD_LAST_ESC: usize = 4038;
pub const ID_TRAY_CHECK_UPDATE: usize = 4039;
pub const ID_TRAY_EXIT: usize = 4099;
pub const ID_TRAY_PROFILE_BASE: usize = 4100;
pub const ID_TRAY_PROFILE_MAX: usize = 4199;

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
        let (qt, osd, active_profile, profile_names, last_word) = crate::state::with_state(|st| {
            let c = st.engine.config();
            (
                c.quick_telex,
                c.show_switch_osd,
                c.active_profile.clone(),
                c.all_profile_names(),
                st.engine.last_word_for_dict(),
            )
        });
        let profile_menu = CreatePopupMenu();
        for (idx, name) in profile_names.iter().enumerate() {
            let id = ID_TRAY_PROFILE_BASE + idx;
            if id <= ID_TRAY_PROFILE_MAX {
                append_item(
                    profile_menu,
                    id,
                    name,
                    name.eq_ignore_ascii_case(&active_profile),
                );
            }
        }
        let p_lbl = to_wide("Hồ sơ");
        AppendMenuW(menu, MF_POPUP, profile_menu as usize, p_lbl.as_ptr());

        if let Some(word) = last_word {
            let preview: String = word.chars().take(24).collect();
            append_item(
                menu,
                ID_TRAY_ADD_LAST_ESC,
                &format!("Thêm từ Esc gần nhất ({})", preview),
                false,
            );
        }
        append_item(menu, ID_TRAY_CONVERT_SELECTION, "Chuyển đổi vùng chọn", false);
        append_item(menu, ID_TRAY_MACROS, "Bảng gõ tắt...", false);
        append_item(menu, ID_TRAY_CONVERT, "Chuyển mã Clipboard...", false);
        append_item(menu, ID_TRAY_APPS, "Danh sách ứng dụng...", false);
        append_item(menu, ID_TRAY_USER_DICT, "Từ điển cá nhân...", false);
        append_item(menu, ID_TRAY_EXTRAS, "Mở rộng & Gõ thử...", false);
        AppendMenuW(menu, MF_SEPARATOR, 0, ptr::null());
        append_item(menu, ID_TRAY_QUICK_TELEX, "Gõ nhanh (Quick Telex)", qt);
        append_item(menu, ID_TRAY_SHOW_OSD, "Hiện OSD khi chuyển V/E", osd);
        append_item(menu, ID_TRAY_CHECK_UPDATE, "Kiểm tra cập nhật...", false);
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

const ICON_SIZE: i32 = 32;
const ICON_INK: i32 = 30;
const SCRATCH: i32 = 64;

type InkBox = (i32, i32, i32, i32);

fn letter_coverage(letter: &str, font_height: i32) -> Option<Vec<u8>> {
    let bmi = dib_info(SCRATCH);
    let mut bits_ptr: *mut std::ffi::c_void = ptr::null_mut();
    let dc = unsafe { CreateCompatibleDC(ptr::null_mut()) };
    let hbm = unsafe { CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits_ptr, ptr::null_mut(), 0) };
    if hbm.is_null() || bits_ptr.is_null() {
        unsafe { DeleteDC(dc) };
        return None;
    }
    let old_bm = unsafe { SelectObject(dc, hbm) };
    let font_name = to_wide("Segoe UI");
    let hfont = unsafe { CreateFontW(-font_height, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 4, 0, font_name.as_ptr()) };
    let old_font = unsafe { SelectObject(dc, hfont) };
    let pixels = unsafe { std::slice::from_raw_parts_mut(bits_ptr as *mut u32, (SCRATCH * SCRATCH) as usize) };
    pixels.fill(0);
    unsafe {
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0x00FF_FFFF);
        let mut rc = RECT { left: 0, top: 0, right: SCRATCH, bottom: SCRATCH };
        let mut wide_txt = to_wide(letter);
        DrawTextW(dc, wide_txt.as_mut_ptr(), -1, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }
    let coverage = pixels
        .iter()
        .map(|px| ((px & 0xFF).max((px >> 8) & 0xFF).max((px >> 16) & 0xFF)) as u8)
        .collect();
    unsafe {
        SelectObject(dc, old_font);
        DeleteObject(hfont);
        SelectObject(dc, old_bm);
        DeleteObject(hbm);
        DeleteDC(dc);
    }
    Some(coverage)
}

fn ink_bounds(coverage: &[u8], size: i32) -> Option<InkBox> {
    let (mut x0, mut y0, mut x1, mut y1) = (size, size, -1, -1);
    for y in 0..size {
        for x in 0..size {
            if coverage[(y * size + x) as usize] > 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    (x1 >= 0).then_some((x0, y0, x1 + 1, y1 + 1))
}

fn fitted_letter(letter: &str) -> Option<(Vec<u8>, InkBox)> {
    (12..=56).rev().find_map(|height| {
        let coverage = letter_coverage(letter, height)?;
        let bounds = ink_bounds(&coverage, SCRATCH)?;
        let fits = bounds.2 - bounds.0 <= ICON_INK && bounds.3 - bounds.1 <= ICON_INK;
        fits.then_some((coverage, bounds))
    })
}

fn dib_info(size: i32) -> BITMAPINFO {
    BITMAPINFO {
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
    }
}

pub fn letter_pixels(letter: &str, r: u8, g: u8, b: u8) -> Vec<u32> {
    let mut out = vec![0u32; (ICON_SIZE * ICON_SIZE) as usize];
    let Some((coverage, (x0, y0, x1, y1))) = fitted_letter(letter) else {
        return out;
    };
    let dx = (ICON_SIZE - (x1 - x0)) / 2 - x0;
    let dy = (ICON_SIZE - (y1 - y0)) / 2 - y0;
    for y in y0..y1 {
        for x in x0..x1 {
            let a = coverage[(y * SCRATCH + x) as usize] as u32;
            let (tx, ty) = (x + dx, y + dy);
            if a == 0 || !(0..ICON_SIZE).contains(&tx) || !(0..ICON_SIZE).contains(&ty) {
                continue;
            }
            let premul = |c: u8| (c as u32 * a + 127) / 255;
            out[(ty * ICON_SIZE + tx) as usize] = (a << 24) | (premul(r) << 16) | (premul(g) << 8) | premul(b);
        }
    }
    out
}

pub fn create_badge_icon(letter: &str, r: u8, g: u8, b: u8) -> HICON {
    let bmi = dib_info(ICON_SIZE);
    let mut bits_ptr: *mut std::ffi::c_void = ptr::null_mut();
    let dc = unsafe { CreateCompatibleDC(ptr::null_mut()) };
    let hbm = unsafe { CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits_ptr, ptr::null_mut(), 0) };
    if hbm.is_null() || bits_ptr.is_null() {
        unsafe { DeleteDC(dc) };
        return ptr::null_mut();
    }
    let pixels = unsafe { std::slice::from_raw_parts_mut(bits_ptr as *mut u32, (ICON_SIZE * ICON_SIZE) as usize) };
    pixels.copy_from_slice(&letter_pixels(letter, r, g, b));
    let mask_bm = unsafe { CreateBitmap(ICON_SIZE, ICON_SIZE, 1, 1, ptr::null()) };
    let icon_info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask_bm, hbmColor: hbm };
    let hicon = unsafe { CreateIconIndirect(&icon_info) };
    unsafe {
        DeleteObject(mask_bm);
        DeleteObject(hbm);
        DeleteDC(dc);
    }
    hicon
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(pixels: &[u32]) -> InkBox {
        let alpha: Vec<u8> = pixels.iter().map(|p| (p >> 24) as u8).collect();
        ink_bounds(&alpha, ICON_SIZE).expect("letter has ink")
    }

    #[test]
    fn letters_fill_the_icon_and_sit_centered() {
        for letter in ["V", "E"] {
            let (x0, y0, x1, y1) = ink(&letter_pixels(letter, 0xD1, 0x34, 0x38));
            let (w, h) = (x1 - x0, y1 - y0);
            assert!(w.max(h) >= ICON_INK - 2, "{letter} ink {w}x{h} too small");
            assert!(w <= ICON_INK && h <= ICON_INK, "{letter} ink {w}x{h} overflows");
            assert!((x0 - (ICON_SIZE - x1)).abs() <= 1, "{letter} off-center horizontally");
            assert!((y0 - (ICON_SIZE - y1)).abs() <= 1, "{letter} off-center vertically");
        }
    }

    #[test]
    fn letter_uses_the_requested_color_on_transparent_ground() {
        let pixels = letter_pixels("E", 0x00, 0x78, 0xD4);
        assert_eq!(pixels[0], 0);
        let solid = pixels.iter().find(|p| *p >> 24 == 0xFF).expect("opaque stroke");
        assert_eq!(solid & 0x00FF_FFFF, 0x0078D4);
    }
}
