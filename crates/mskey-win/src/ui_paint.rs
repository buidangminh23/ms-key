use std::cell::RefCell;
use std::ptr;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, HBRUSH, HDC, HFONT,
    PAINTSTRUCT, RDW_ALLCHILDREN, RDW_INVALIDATE, RedrawWindow, SelectObject, SetBkColor, SetBkMode, SetTextColor,
};
use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_SELECTED, ODT_BUTTON};
use windows_sys::Win32::UI::Shell::Shell_NotifyIconW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DI_NORMAL, DrawIconEx, GetClientRect, GetDlgCtrlID, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX,
};

use crate::dialog_helpers::{handle_draw_combo_or_btn, read_window_text, scale_px};
use crate::platform::to_wide;
use crate::state::with_state;
use crate::theme::{
    ThemePalette, apply_combo_theme, apply_dwm_dark_titlebar, draw_checkbox, draw_item_focus,
    draw_owner_button, draw_radio, draw_round_box, fill_solid, palette_for, rgb,
};
use crate::tray::TrayController;
use crate::ui_controls::{MainUiCtrls, apply_tab_visibility, *};

pub struct MainWinData {
    pub font: HFONT,
    pub bold_font: HFONT,
    pub title_font: HFONT,
    pub dpi: i32,
    pub bg_brush: HBRUSH,
    pub card_brush: HBRUSH,
    pub in_brush: HBRUSH,
    pub pal: ThemePalette,
    pub ctrls: MainUiCtrls,
    pub tray: TrayController,
    pub reveal_started: Option<std::time::Instant>,
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
    MAIN_DATA.with(|c| c.try_borrow_mut().ok().and_then(|mut b| b.take()))
}

pub fn with_main_data<R>(f: impl FnOnce(&MainWinData) -> R) -> Option<R> {
    MAIN_DATA.with(|c| {
        let borrow = c.try_borrow().ok()?;
        borrow.as_ref().map(f)
    })
}

pub fn with_main_data_mut<R>(f: impl FnOnce(&mut MainWinData) -> R) -> Option<R> {
    MAIN_DATA.with(|c| {
        let mut borrow = c.try_borrow_mut().ok()?;
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
    let effective_vi = with_state(|st| st.effective_vietnamese());
    with_main_data_mut(|d| d.ctrls.sync_from_state());
    let tray_req = with_main_data(|d| d.tray.prepare_update(effective_vi));
    if let Some((msg, nid, _)) = tray_req {
        let updated = unsafe { Shell_NotifyIconW(msg, &nid) } != 0;
        if updated {
            with_main_data_mut(|d| d.tray.mark_installed());
        }
    }
    unsafe {
        RedrawWindow(hwnd, ptr::null(), ptr::null_mut(), RDW_INVALIDATE | RDW_ALLCHILDREN);
    }
}

pub fn set_tab_redraw(hwnd: HWND, tab: usize) {
    let updated = with_main_data_mut(|d| d.ctrls.set_tab_state(tab));
    if let Some((active, lists)) = updated {
        apply_tab_visibility(active, &lists);
    }
    unsafe {
        RedrawWindow(hwnd, ptr::null(), ptr::null_mut(), RDW_INVALIDATE | RDW_ALLCHILDREN);
    }
}

pub fn paint_main_window(hwnd: HWND) {
    let Some((pal, active_tab, reveal_started, dpi)) =
        with_main_data(|d| (d.pal, d.ctrls.active_tab, d.reveal_started, d.dpi))
    else {
        return;
    };
    let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    let mut rc: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(hwnd, &mut rc) };
    fill_solid(hdc, &rc, pal.window_bg);

    let s = |v| scale_px(v, dpi);
    let rect = |left, top, right, bottom| RECT {
        left: s(left), top: s(top), right: s(right), bottom: s(bottom),
    };
    let progress = reveal_started.map_or(1.0, |start| (start.elapsed().as_secs_f32() / 0.48).min(1.0));
    let eased = 1.0 - (1.0 - progress).powi(3);
    let size = s((34.0 + 14.0 * eased).round() as i32);
    unsafe {
        DrawIconEx(
            hdc, s(44) - size / 2, s(36) - size / 2, crate::tray::app_icon(64),
            size, size, 0, ptr::null_mut(), DI_NORMAL,
        );
    }
    if reveal_started.is_some() {
        let track = rect(24, 62, 64, 64);
        fill_solid(hdc, &track, pal.border);
        let bar = RECT { right: track.left + ((track.right - track.left) as f32 * eased) as i32, ..track };
        fill_solid(hdc, &bar, pal.accent);
    }
    let panel = |x, y, width, bottom| {
        draw_round_box(hdc, &rect(x, y, x + width, bottom), s(12), pal.card_bg, pal.border);
    };
    panel(20, 84, 600, 164);
    draw_round_box(hdc, &rect(20, 176, 296, 216), s(10), pal.tab_inactive_bg, pal.border);

    match active_tab {
        0 => {
            panel(20, 224, 292, 432);
            panel(328, 224, 292, 432);
            panel(20, 444, 600, 564);
        }
        1 => {
            panel(20, 224, 600, 400);
            panel(20, 412, 600, 564);
        }
        _ => {
            panel(20, 224, 600, 332);
            panel(20, 344, 600, 444);
            panel(20, 456, 600, 564);
        }
    }
    fill_solid(hdc, &rect(20, 576, 620, 577), pal.border);
    unsafe { EndPaint(hwnd, &ps) };
}

pub fn draw_main_item(lparam: LPARAM) -> Option<LRESULT> {
    let (pal, font, bold_font, active_tab, dpi) =
        with_main_data(|d| (d.pal, d.font, d.bold_font, d.ctrls.active_tab, d.dpi))?;
    if lparam == 0 {
        return None;
    }
    let dis = unsafe { &*(lparam as *const DRAWITEMSTRUCT) };
    if dis.CtlType != ODT_BUTTON {
        return handle_draw_combo_or_btn(lparam, &[], pal, font);
    }

    let txt = read_window_text(dis.hwndItem);
    let (checked, radio, effective_vi) = with_state(|st| {
        let cfg = st.engine.config();
        (
            MainUiCtrls::is_checkbox_checked(dis.CtlID, cfg),
            MainUiCtrls::is_radio_selected(dis.CtlID, cfg),
            st.effective_vietnamese(),
        )
    });
    if let Some(checked) = checked {
        fill_solid(dis.hDC, &dis.rcItem, pal.card_bg);
        draw_checkbox(dis.hDC, &dis.rcItem, &txt, checked, pal, font, dpi);
        draw_item_focus(dis, dpi);
        return Some(1);
    }
    if let Some(selected) = radio {
        fill_solid(dis.hDC, &dis.rcItem, pal.card_bg);
        draw_radio(dis.hDC, &dis.rcItem, &txt, selected, pal, font, dpi);
        draw_item_focus(dis, dpi);
        return Some(1);
    }

    let id = dis.CtlID as usize;
    let pressed = dis.itemState & ODS_SELECTED != 0;
    match id {
        IDC_VE_PILL => {
            let fill = match (effective_vi, pressed) {
                (true, false) => pal.danger_badge,
                (true, true) => rgb(151, 30, 45),
                (false, false) => rgb(0, 95, 178),
                (false, true) => rgb(0, 69, 132),
            };
            fill_solid(dis.hDC, &dis.rcItem, pal.window_bg);
            draw_round_box(dis.hDC, &dis.rcItem, scale_px(10, dpi), fill, fill);
            draw_centered_label(dis.hDC, &dis.rcItem, &txt, bold_font, rgb(255, 255, 255));
            draw_item_focus(dis, dpi);
        }
        IDC_TAB_BASIC..=IDC_TAB_SYSTEM => {
            let active = id - IDC_TAB_BASIC == active_tab;
            fill_solid(dis.hDC, &dis.rcItem, pal.tab_inactive_bg);
            if active || pressed {
                draw_round_box(
                    dis.hDC, &dis.rcItem, scale_px(8, dpi), pal.card_bg,
                    if active { pal.accent } else { pal.border },
                );
            }
            let (font, fg) = if active { (bold_font, pal.accent) } else { (font, pal.text) };
            draw_centered_label(dis.hDC, &dis.rcItem, &txt, font, fg);
            draw_item_focus(dis, dpi);
        }
        _ => {
            let in_card = id == IDC_BTN_CUSTOM_KEY || (1120..1400).contains(&id);
            fill_solid(dis.hDC, &dis.rcItem, if in_card { pal.card_bg } else { pal.window_bg });
            let primary = id == IDC_BTN_SAVE;
            draw_owner_button(dis, &txt, primary, pal, if primary { bold_font } else { font }, dpi);
        }
    }
    Some(1)
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

pub fn ctl_color_main(msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    let snapshot = with_main_data(|d| (d.pal, d.card_brush, d.bg_brush, d.in_brush));
    let (pal, card_brush, bg_brush, in_brush) = snapshot?;
    let hdc = wparam as HDC;
    if matches!(msg, WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX) {
        unsafe {
            SetTextColor(hdc, pal.text);
            SetBkColor(hdc, pal.input_bg);
        }
        return Some(in_brush as LRESULT);
    }
    let child = lparam as HWND;
    let id = unsafe { GetDlgCtrlID(child) } as usize;
    let in_card = matches!(
        id,
        IDC_LBL_SECTION
            | IDC_LBL_SUGGEST_TIP
            | IDC_LBL_SWITCH_CUSTOM
            | IDC_LBL_HOLD_EN
            | IDC_LBL_CONV_HOTKEY
            | IDC_LBL_UNDO_HOTKEY
            | IDC_LBL_CONV_SEL_HOTKEY
            | IDC_LBL_THEME
            | IDC_LBL_SYNC_PATH
            | IDC_LBL_CUR_APP
            | IDC_LBL_APP_HINT
    );
    let fg = if matches!(id, IDC_LBL_FOOTER_VER | IDC_LBL_ACTIVE_APP | IDC_LBL_PROFILE | IDC_LBL_SUGGEST_TIP | IDC_LBL_APP_HINT) {
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
