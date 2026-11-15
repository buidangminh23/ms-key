use std::cell::RefCell;
use std::ptr;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, HBRUSH, HDC, HFONT,
    InvalidateRect, PAINTSTRUCT, SelectObject, SetBkColor, SetBkMode, SetTextColor,
};
use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, ODT_BUTTON};
use windows_sys::Win32::UI::Shell::Shell_NotifyIconW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetDlgCtrlID, ICON_BIG, ICON_SMALL, SendMessageW, WM_SETICON,
};

use crate::dialog_helpers::{handle_draw_combo_or_btn, read_window_text, scale_px, window_dpi};
use crate::platform::to_wide;
use crate::state::with_state;
use crate::theme::{
    ThemePalette, apply_combo_theme, apply_dwm_dark_titlebar, draw_checkbox, draw_group_box,
    draw_owner_button, draw_radio, draw_round_box, fill_solid, palette_for, rgb,
};
use crate::tray::TrayController;
use crate::ui_controls::{
    MainUiCtrls, apply_tab_visibility, sync_controls_from_config, *,
};

pub struct MainWinData {
    pub font: HFONT,
    pub bold_font: HFONT,
    pub bg_brush: HBRUSH,
    pub card_brush: HBRUSH,
    pub in_brush: HBRUSH,
    pub pal: ThemePalette,
    pub ctrls: MainUiCtrls,
    pub tray: TrayController,
}

thread_local! {
    static MAIN_DATA: RefCell<Option<MainWinData>> = const { RefCell::new(None) };
}

pub fn set_main_data(data: Option<MainWinData>) {
    MAIN_DATA.with(|c| {
        *c.borrow_mut() = data;
    });
}

pub fn take_main_data() -> Option<MainWinData> {
    MAIN_DATA.with(|c| c.borrow_mut().take())
}

pub fn with_main_data<R>(f: impl FnOnce(&MainWinData) -> R) -> Option<R> {
    MAIN_DATA.with(|c| {
        let borrow = c.borrow();
        borrow.as_ref().map(f)
    })
}

pub fn with_main_data_mut<R>(f: impl FnOnce(&mut MainWinData) -> R) -> Option<R> {
    MAIN_DATA.with(|c| {
        let mut borrow = c.borrow_mut();
        borrow.as_mut().map(f)
    })
}

pub fn rebuild_theme_resources(hwnd: HWND) {
    let mode = with_state(|st| st.engine.config().theme);
    let pal = palette_for(mode);
    apply_dwm_dark_titlebar(hwnd, pal.dark);
    if let Some(combos) = with_main_data(|d| d.ctrls.combo_list()) {
        for c in combos {
            apply_combo_theme(c, pal.dark);
        }
    }
    with_main_data_mut(|d| {
        d.pal = pal;
        unsafe {
            DeleteObject(d.bg_brush);
            DeleteObject(d.card_brush);
            DeleteObject(d.in_brush);
            d.bg_brush = CreateSolidBrush(d.pal.window_bg);
            d.card_brush = CreateSolidBrush(d.pal.card_bg);
            d.in_brush = CreateSolidBrush(d.pal.input_bg);
        }
    });
}

pub fn refresh_ui_and_tray(hwnd: HWND) {
    let (cfg, fg_proc) = with_state(|st| (st.engine.config().clone(), st.fg_process.clone()));
    if let Some(targets) = with_main_data(|d| d.ctrls.sync_targets()) {
        sync_controls_from_config(targets, &cfg, &fg_proc);
    }
    let tray_req = with_main_data(|d| d.tray.prepare_update(cfg.vietnamese_enabled));
    if let Some((msg, nid, icon)) = tray_req {
        let updated = unsafe { Shell_NotifyIconW(msg, &nid) } != 0;
        if updated {
            with_main_data_mut(|d| d.tray.mark_installed());
        }
        unsafe {
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, icon as LPARAM);
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, icon as LPARAM);
            InvalidateRect(hwnd, ptr::null(), 1);
        }
    }
}

pub fn set_tab_redraw(hwnd: HWND, tab: usize) {
    let updated = with_main_data_mut(|d| d.ctrls.set_tab_state(tab));
    if let Some((active, lists)) = updated {
        apply_tab_visibility(active, &lists);
    }
    unsafe {
        InvalidateRect(hwnd, ptr::null(), 1);
    }
}

pub fn paint_main_window(hwnd: HWND) {
    let snapshot = with_main_data(|d| (d.pal, d.bold_font, d.ctrls.active_tab));
    let Some((pal, bold_font, active_tab)) = snapshot else {
        return;
    };
    let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    let dpi = window_dpi(hwnd);
    let mut rc: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(hwnd, &mut rc) };
    fill_solid(hdc, &rc, pal.window_bg);

    let s = |v: i32| scale_px(v, dpi);
    let top_gb = RECT { left: s(16), top: s(12), right: s(444), bottom: s(140) };
    draw_group_box(hdc, &top_gb, "Điều khiển", pal, bold_font);

    let card = RECT { left: s(16), top: s(184), right: s(544), bottom: s(512) };
    draw_round_box(hdc, &card, s(8), pal.card_bg, pal.border);

    match active_tab {
        0 => {
            let mg = RECT { left: s(32), top: s(376), right: s(528), bottom: s(496) };
            draw_group_box(hdc, &mg, "Gõ tắt & phím tắt nhanh", pal, bold_font);
        }
        1 => {
            let sg = RECT { left: s(32), top: s(200), right: s(528), bottom: s(308) };
            draw_group_box(hdc, &sg, "Phím chuyển V/E", pal, bold_font);
            let cg = RECT { left: s(32), top: s(384), right: s(528), bottom: s(496) };
            draw_group_box(hdc, &cg, "Chuyển mã & Nhập EVKey", pal, bold_font);
        }
        _ => {
            let ag = RECT { left: s(32), top: s(332), right: s(528), bottom: s(496) };
            draw_group_box(hdc, &ag, "Tự động chuyển V/E theo ứng dụng", pal, bold_font);
        }
    }
    unsafe { EndPaint(hwnd, &ps) };
}

pub fn draw_main_item(hwnd: HWND, lparam: LPARAM) -> Option<LRESULT> {
    let snapshot = with_main_data(|d| (d.pal, d.font, d.bold_font, d.ctrls.active_tab));
    let (pal, font, bold_font, active_tab) = snapshot?;
    let dis = unsafe { &*(lparam as *const DRAWITEMSTRUCT) };
    let dpi = window_dpi(hwnd);
    let cfg = with_state(|st| st.engine.config().clone());
    if dis.CtlType == ODT_BUTTON {
        let txt = read_window_text(dis.hwndItem);
        if let Some(checked) = MainUiCtrls::is_checkbox_checked(dis.CtlID, &cfg) {
            fill_solid(dis.hDC, &dis.rcItem, pal.card_bg);
            draw_checkbox(dis.hDC, &dis.rcItem, &txt, checked, pal, font, dpi);
            return Some(1);
        }
        if let Some(sel) = MainUiCtrls::is_radio_selected(dis.CtlID, &cfg) {
            fill_solid(dis.hDC, &dis.rcItem, pal.card_bg);
            draw_radio(dis.hDC, &dis.rcItem, &txt, sel, pal, font, dpi);
            return Some(1);
        }
        let id = dis.CtlID as usize;
        if id == IDC_VE_PILL {
            let fill = if cfg.vietnamese_enabled { pal.danger_badge } else { pal.accent };
            draw_round_box(dis.hDC, &dis.rcItem, 6, fill, fill);
            draw_centered_label(dis.hDC, &dis.rcItem, &txt, bold_font, rgb(255, 255, 255));
            return Some(1);
        }
        if matches!(id, IDC_TAB_BASIC | IDC_TAB_HOTKEY | IDC_TAB_SYSTEM) {
            let active = (id == IDC_TAB_BASIC && active_tab == 0)
                || (id == IDC_TAB_HOTKEY && active_tab == 1)
                || (id == IDC_TAB_SYSTEM && active_tab == 2);
            let (fill, border, fg) = if active {
                (pal.card_bg, pal.accent, pal.accent)
            } else {
                (pal.tab_inactive_bg, pal.border, pal.subtle_text)
            };
            draw_round_box(dis.hDC, &dis.rcItem, 6, fill, border);
            let f = if active { bold_font } else { font };
            draw_centered_label(dis.hDC, &dis.rcItem, &txt, f, fg);
            return Some(1);
        }
        draw_owner_button(dis.hDC, &dis.rcItem, &txt, id == IDC_BTN_SAVE, pal, font);
        return Some(1);
    }
    handle_draw_combo_or_btn(lparam, &[IDC_BTN_SAVE as u32], pal, font)
}

fn draw_centered_label(hdc: HDC, rc: &RECT, txt: &str, font: HFONT, fg: u32) {
    let mut tx = *rc;
    let mut w = to_wide(txt);
    unsafe {
        let of = SelectObject(hdc, font);
        SetBkMode(hdc, 1);
        SetTextColor(hdc, fg);
        DrawTextW(hdc, w.as_mut_ptr(), -1, &mut tx, 0x0825);
        SelectObject(hdc, of);
    }
}

pub fn ctl_color_main(wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    let snapshot = with_main_data(|d| (d.pal, d.card_brush, d.bg_brush));
    let (pal, card_brush, bg_brush) = snapshot?;
    let child = lparam as HWND;
    let id = unsafe { GetDlgCtrlID(child) } as usize;
    let in_card = matches!(
        id,
        0 | IDC_LBL_ACTIVE_APP
            | IDC_LBL_SUGGEST_TIP
            | IDC_LBL_SWITCH_CUSTOM
            | IDC_LBL_CONV_HOTKEY
            | IDC_LBL_THEME
            | IDC_LBL_CUR_APP
            | IDC_LBL_APP_HINT
    );
    let hdc = wparam as HDC;
    let fg = if matches!(id, IDC_LBL_FOOTER_VER | IDC_LBL_SUGGEST_TIP | IDC_LBL_APP_HINT) {
        pal.subtle_text
    } else {
        pal.text
    };
    unsafe {
        SetTextColor(hdc, fg);
        SetBkColor(hdc, if in_card { pal.card_bg } else { pal.window_bg });
    }
    Some((if in_card { card_brush } else { bg_brush }) as LRESULT)
}
