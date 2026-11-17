use std::cell::{Cell, RefCell};
use std::ptr;

use mskey_core::{Config, Engine, EngineAction, InputMethod};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, HBRUSH, HFONT, InvalidateRect,
    PAINTSTRUCT,
};
use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, EM_GETSEL, EM_REPLACESEL, EM_SETMARGINS, EM_SETSEL, EM_UNDO, ODT_BUTTON};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT,
    VK_NEXT, VK_PRIOR, VK_RIGHT, VK_UP,
};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CB_SETCURSEL, DefWindowProcW, DestroyWindow, EC_LEFTMARGIN, EC_RIGHTMARGIN, GetClientRect, LB_ADDSTRING, LB_GETCURSEL,
    LB_RESETCONTENT, LBS_NOTIFY, GetDlgItem, SendMessageW, WM_CHAR, WM_CLEAR, WM_CLOSE, WM_COMMAND,
    WM_CREATE, WM_CTLCOLOREDIT, WM_CUT, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_PASTE, WM_UNDO,
    WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DRAWITEM, WM_PAINT, WS_BORDER,
    WS_TABSTOP, WS_VSCROLL,
};

use crate::dialog_helpers::{
    combo_index, create_modal_window, current_palette, handle_ctl_color, handle_draw_combo_or_btn,
    make_child, make_combo, make_edit, make_owner_button, read_window_text, register_tool_class,
    run_modal_loop, run_modal_loop_with_focus, scale_px, set_cue_banner, window_dpi, write_window_text,
};
use crate::platform::{save_config, to_wide};
use crate::state::with_state;
use crate::theme::{create_ui_font, draw_checkbox, draw_item_focus, fill_solid};
use crate::ui_actions::{export_json_backup_dialog, import_json_backup_dialog};

const ID_EXT_QTELEX: usize = 6601;
const ID_EXT_QSTART: usize = 6602;
const ID_EXT_QEND: usize = 6603;
const ID_EXT_OSD: usize = 6604;
const ID_EXT_SHIFTINS: usize = 6605;
const ID_EXT_DELAY: usize = 6606;
const ID_EXT_EXP: usize = 6607;
const ID_EXT_IMP: usize = 6608;
const ID_EXT_CLOSE: usize = 6611;
const CBN_SELCHANGE_CODE: usize = 1;

const DELAY_OPTIONS: [u8; 5] = [0, 1, 2, 5, 10];

#[derive(Clone, Copy)]
struct ExtrasDlg {
    font: HFONT,
    bold: HFONT,
    bg_br: HBRUSH,
    in_br: HBRUSH,
    delay_cb: HWND,
}

thread_local! {
    static EXTRAS_STATE: RefCell<Option<ExtrasDlg>> = const { RefCell::new(None) };
    static DICT_STATE: RefCell<Option<DictDlg>> = const { RefCell::new(None) };
    static DICT_FILTER_IDX: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

pub fn show_extras_dialog(parent: HWND) {
    register_tool_class("MSKeyExtrasDlg", extras_wnd_proc);
    let modal = create_modal_window(parent, "MSKeyExtrasDlg", "Mở rộng và sao lưu", 496, 304);
    run_modal_loop(parent, modal);
}

fn delay_to_idx(ms: u8) -> usize {
    DELAY_OPTIONS.iter().position(|&v| v == ms).unwrap_or(0)
}

fn extras_checkbox_state(id: u32, cfg: &Config) -> Option<bool> {
    Some(match id as usize {
        ID_EXT_QTELEX => cfg.quick_telex,
        ID_EXT_QSTART => cfg.quick_start_consonant,
        ID_EXT_QEND => cfg.quick_end_consonant,
        ID_EXT_OSD => cfg.show_switch_osd,
        ID_EXT_SHIFTINS => cfg.paste_via_shift_insert,
        _ => return None,
    })
}

fn toggle_extras_id(id: usize, cfg: &mut Config) -> bool {
    match id {
        ID_EXT_QTELEX => cfg.quick_telex = !cfg.quick_telex,
        ID_EXT_QSTART => cfg.quick_start_consonant = !cfg.quick_start_consonant,
        ID_EXT_QEND => cfg.quick_end_consonant = !cfg.quick_end_consonant,
        ID_EXT_OSD => cfg.show_switch_osd = !cfg.show_switch_osd,
        ID_EXT_SHIFTINS => cfg.paste_via_shift_insert = !cfg.paste_via_shift_insert,
        _ => return false,
    }
    true
}

fn create_extras_controls(hwnd: HWND, font: HFONT, bold: HFONT, dpi: i32) -> HWND {
    make_child(hwnd, "STATIC", "Tùy chọn mở rộng", 0x0200, 20, 16, 456, 24, 0, bold, dpi);
    let items = [
        (ID_EXT_QTELEX, "Gõ nhanh phụ âm (Quick Telex: cc→ch, tt→th)"),
        (ID_EXT_QSTART, "Phụ âm đầu nhanh (f→ph, j→gi, w→qu)"),
        (ID_EXT_QEND, "Phụ âm cuối nhanh (-g→ng, -h→nh, -k→ch)"),
        (ID_EXT_OSD, "Hiện khung nổi OSD khi chuyển V/E"),
        (ID_EXT_SHIFTINS, "Dán bằng Shift+Insert (khi dùng Clipboard)"),
    ];
    for (idx, (id, label)) in items.iter().enumerate() {
        make_owner_button(hwnd, label, 20, 212 + (idx as i32) * 24, 456, 24, *id, font, dpi);
    }
    make_child(hwnd, "STATIC", "Độ trễ gửi phím", 0x0200, 20, 172, 132, 28, 0, font, dpi);
    let sel = with_state(|app| delay_to_idx(app.engine.config().send_delay_ms));
    let delay_cb = make_combo(
        hwnd, 160, 172, 180, 28, ID_EXT_DELAY,
        &["0 ms (Nhanh)", "1 ms", "2 ms", "5 ms", "10 ms"],
        sel, font, dpi,
    );
    make_child(hwnd, "STATIC", "Sao lưu JSON", 0x0200, 20, 208, 132, 28, 0, font, dpi);
    make_owner_button(hwnd, "Xuất JSON…", 160, 208, 116, 28, ID_EXT_EXP, font, dpi);
    make_owner_button(hwnd, "Nhập JSON…", 284, 208, 116, 28, ID_EXT_IMP, font, dpi);
    make_owner_button(hwnd, "Đóng", 380, 264, 96, 28, ID_EXT_CLOSE, font, dpi);
    delay_cb
}

fn handle_extras_cmd(hwnd: HWND, id: usize, code: usize, st: ExtrasDlg) {
    let mut changed = false;
    with_state(|app| {
        let mut cfg = app.engine.config().clone();
        if toggle_extras_id(id, &mut cfg) {
            changed = true;
        } else if id == ID_EXT_DELAY && code == CBN_SELCHANGE_CODE {
            let idx = combo_index(st.delay_cb);
            cfg.send_delay_ms = *DELAY_OPTIONS.get(idx).unwrap_or(&0);
            changed = true;
        }
        if changed {
            save_config(&cfg);
            app.engine.update_config(cfg);
        }
    });
    if changed {
        unsafe { InvalidateRect(hwnd, ptr::null(), 1) };
        return;
    }
    match id {
        ID_EXT_EXP => export_json_backup_dialog(hwnd),
        ID_EXT_IMP if import_json_backup_dialog(hwnd) => {
            let idx = with_state(|app| delay_to_idx(app.engine.config().send_delay_ms));
            unsafe {
                SendMessageW(st.delay_cb, CB_SETCURSEL, idx, 0);
                InvalidateRect(hwnd, ptr::null(), 1);
            }
        }
        ID_EXT_CLOSE => unsafe { DestroyWindow(hwnd); },
        _ => {}
    }
}

unsafe extern "system" fn extras_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let pal = current_palette();
    match msg {
        WM_CREATE => {
            let dpi = window_dpi(hwnd);
            let font = create_ui_font(scale_px(14, dpi), false);
            let bg_br = unsafe { CreateSolidBrush(pal.card_bg) };
            let in_br = unsafe { CreateSolidBrush(pal.input_bg) };
            let bold = create_ui_font(scale_px(14, dpi), true);
            let delay_cb = create_extras_controls(hwnd, font, bold, dpi);
            EXTRAS_STATE.with(|c| {
                *c.borrow_mut() = Some(ExtrasDlg { font, bold, bg_br, in_br, delay_cb });
            });
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            fill_solid(hdc, &rc, pal.card_bg);
            let s = |v| scale_px(v, window_dpi(hwnd));
            fill_solid(hdc, &RECT { left: s(20), top: s(252), right: s(476), bottom: s(252) + 1 }, pal.border);
            unsafe { EndPaint(hwnd, &ps) };
            0
        }
        WM_DRAWITEM if lparam != 0 => {
            let font = EXTRAS_STATE.with(|c| c.borrow().map(|s| s.font).unwrap_or(ptr::null_mut()));
            let dis = unsafe { &*(lparam as *const DRAWITEMSTRUCT) };
            if dis.CtlType == ODT_BUTTON {
                let cfg = with_state(|st| st.engine.config().clone());
                if let Some(checked) = extras_checkbox_state(dis.CtlID, &cfg) {
                    let txt = read_window_text(dis.hwndItem);
                    let dpi = window_dpi(hwnd);
                    fill_solid(dis.hDC, &dis.rcItem, pal.card_bg);
                    draw_checkbox(dis.hDC, &dis.rcItem, &txt, checked, pal, font, dpi);
                    draw_item_focus(dis, dpi);
                    return 1;
                }
            }
            handle_draw_combo_or_btn(lparam, &[], pal, font).unwrap_or(0)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let (b1, b2) = EXTRAS_STATE.with(|c| {
                c.borrow().map(|s| (s.bg_br, s.in_br)).unwrap_or((ptr::null_mut(), ptr::null_mut()))
            });
            handle_ctl_color(msg, wparam as _, pal, b1, b2)
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            let code = (wparam >> 16) & 0xFFFF;
            if let Some(st) = EXTRAS_STATE.with(|c| *c.borrow()) {
                handle_extras_cmd(hwnd, id, code, st);
            }
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            if let Some(st) = EXTRAS_STATE.with(|c| c.borrow_mut().take()) {
                unsafe {
                    DeleteObject(st.font);
                    DeleteObject(st.bold);
                    DeleteObject(st.bg_br);
                    DeleteObject(st.in_br);
                }
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

const ID_TRY_METHOD: usize = 6620;
const ID_TRY_MODE: usize = 6621;
const ID_TRY_SAMPLE: usize = 6622;
const ID_TRY_TEXT: usize = 6623;
const ID_TRY_CLEAR: usize = 6624;
const ID_TRY_CLOSE: usize = 6625;

const TRY_METHODS: [InputMethod; 5] = [
    InputMethod::Telex, InputMethod::Vni, InputMethod::Viqr, InputMethod::SimpleTelex, InputMethod::Custom,
];

struct TypingPreview {
    engine: Engine,
    font: HFONT,
    title_font: HFONT,
    bg_br: HBRUSH,
    in_br: HBRUSH,
}

thread_local! {
    static PREVIEW_WINDOW: Cell<usize> = const { Cell::new(0) };
    static PREVIEW_STATE: RefCell<Option<TypingPreview>> = const { RefCell::new(None) };
}

pub fn is_typing_preview(hwnd: usize) -> bool {
    hwnd != 0 && PREVIEW_WINDOW.get() == hwnd
}

pub fn show_typing_preview(parent: HWND) {
    register_tool_class("MSKeyTypingPreview", typing_preview_proc);
    let modal = create_modal_window(parent, "MSKeyTypingPreview", "Gõ thử · MS Key", 536, 352);
    if !modal.is_null() {
        let edit = unsafe { GetDlgItem(modal, ID_TRY_TEXT as i32) };
        run_modal_loop_with_focus(parent, modal, edit);
    }
}

fn preview_sample(method: InputMethod) -> &'static str {
    match method {
        InputMethod::Telex | InputMethod::SimpleTelex => "Gõ Tieengs Vieetj để thử chữ “Tiếng Việt”.",
        InputMethod::Vni => "Gõ Tie6ng1 Vie6t5 để thử chữ “Tiếng Việt”.",
        InputMethod::Viqr => "Gõ Tie^ng' Vie^t. để thử chữ “Tiếng Việt”.",
        InputMethod::Custom => "Gõ theo bảng phím tùy chỉnh đang dùng.",
    }
}

fn preview_mode_label(vietnamese: bool) -> &'static str {
    if vietnamese { "Tiếng Việt (V)" } else { "Tiếng Anh (E)" }
}

fn create_typing_preview(hwnd: HWND, pal: crate::theme::ThemePalette) {
    let dpi = window_dpi(hwnd);
    let font = create_ui_font(scale_px(14, dpi), false);
    let title_font = create_ui_font(scale_px(22, dpi), true);
    let cfg = with_state(|st| st.engine.config().clone());
    let method = cfg.input_method;
    let vietnamese = cfg.vietnamese_enabled;
    let method_idx = TRY_METHODS.iter().position(|value| *value == method).unwrap_or_default();
    let state = TypingPreview {
        engine: Engine::new(cfg),
        font,
        title_font,
        bg_br: unsafe { CreateSolidBrush(pal.card_bg) },
        in_br: unsafe { CreateSolidBrush(pal.input_bg) },
    };
    PREVIEW_WINDOW.set(hwnd as usize);
    PREVIEW_STATE.with(|slot| *slot.borrow_mut() = Some(state));
    make_child(hwnd, "STATIC", "Gõ thử", 0x0200, 20, 16, 232, 28, 0, title_font, dpi);
    make_child(hwnd, "STATIC", "Kiểu gõ thử", 0x0200, 20, 56, 96, 28, 0, font, dpi);
    make_combo(
        hwnd, 116, 56, 184, 28, ID_TRY_METHOD,
        &["Telex", "VNI", "VIQR", "Simple Telex", "Tự định nghĩa"], method_idx, font, dpi,
    );
    make_owner_button(hwnd, preview_mode_label(vietnamese), 400, 56, 116, 28, ID_TRY_MODE, font, dpi);
    make_child(hwnd, "STATIC", preview_sample(method), 0x0200, 20, 96, 496, 24, ID_TRY_SAMPLE, font, dpi);
    let edit = make_edit(hwnd, "", 20, 128, 496, 144, ID_TRY_TEXT, true, font, dpi);
    unsafe {
        let margin = scale_px(8, dpi);
        SendMessageW(
            edit, EM_SETMARGINS, (EC_LEFTMARGIN | EC_RIGHTMARGIN) as WPARAM,
            (margin | (margin << 16)) as LPARAM,
        );
        SetWindowSubclass(edit, Some(preview_edit_proc), 1, 0);
    }
    make_child(hwnd, "STATIC", "Các thay đổi chỉ áp dụng trong ô gõ thử.", 0x0200, 20, 296, 292, 32, 0, font, dpi);
    make_owner_button(hwnd, "Xóa chữ", 328, 296, 88, 32, ID_TRY_CLEAR, font, dpi);
    make_owner_button(hwnd, "Đóng", 424, 296, 92, 32, ID_TRY_CLOSE, font, dpi);
}

fn with_preview_engine<T>(f: impl FnOnce(&mut Engine) -> T) -> Option<T> {
    PREVIEW_STATE.with(|slot| slot.try_borrow_mut().ok()?.as_mut().map(|state| f(&mut state.engine)))
}

fn handle_try_command(hwnd: HWND, id: usize, code: usize) {
    let item = |id| unsafe { GetDlgItem(hwnd, id as i32) };
    match id {
        ID_TRY_METHOD if code == CBN_SELCHANGE_CODE => {
            if let Some(&method) = TRY_METHODS.get(combo_index(item(ID_TRY_METHOD))) {
                with_preview_engine(|engine| {
                    let mut cfg = engine.config().clone();
                    cfg.input_method = method;
                    engine.update_config(cfg);
                    engine.reset_all();
                });
                write_window_text(item(ID_TRY_SAMPLE), preview_sample(method));
            }
        }
        ID_TRY_MODE => {
            if let Some(vietnamese) = with_preview_engine(|engine| {
                let next = !engine.config().vietnamese_enabled;
                engine.set_vietnamese_enabled(next);
                next
            }) {
                write_window_text(item(ID_TRY_MODE), preview_mode_label(vietnamese));
                unsafe { InvalidateRect(item(ID_TRY_MODE), ptr::null(), 0) };
            }
        }
        ID_TRY_CLEAR => {
            write_window_text(item(ID_TRY_TEXT), "");
            with_preview_engine(Engine::reset_all);
            unsafe { SetFocus(item(ID_TRY_TEXT)) };
        }
        ID_TRY_CLOSE | 2 => { unsafe { DestroyWindow(hwnd) }; }
        _ => {}
    }
}

unsafe extern "system" fn typing_preview_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let pal = current_palette();
    match msg {
        WM_CREATE => { create_typing_preview(hwnd, pal); 0 }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            fill_solid(hdc, &rc, pal.card_bg);
            unsafe { EndPaint(hwnd, &ps) };
            0
        }
        WM_DRAWITEM => {
            let font = PREVIEW_STATE.with(|slot| slot.borrow().as_ref().map(|state| state.font));
            font.and_then(|font| handle_draw_combo_or_btn(lp, &[], pal, font)).unwrap_or(0)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            PREVIEW_STATE.with(|slot| slot.borrow().as_ref().map_or(0, |state| {
                handle_ctl_color(msg, wp as _, pal, state.bg_br, state.in_br)
            }))
        }
        WM_COMMAND => { handle_try_command(hwnd, wp & 0xFFFF, wp >> 16); 0 }
        WM_CLOSE => { unsafe { DestroyWindow(hwnd) }; 0 }
        WM_DESTROY => {
            PREVIEW_WINDOW.set(0);
            if let Some(state) = PREVIEW_STATE.with(|slot| slot.borrow_mut().take()) {
                unsafe {
                    DeleteObject(state.font);
                    DeleteObject(state.title_font);
                    DeleteObject(state.bg_br);
                    DeleteObject(state.in_br);
                }
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

unsafe extern "system" fn preview_edit_proc(
    hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _: usize, _: usize,
) -> LRESULT {
    if msg == WM_CHAR || (msg == WM_KEYDOWN && wp == VK_ESCAPE as usize) {
        if apply_preview_key(hwnd, wp) {
            return 0;
        }
    } else if matches!(msg, WM_LBUTTONDOWN | WM_KILLFOCUS | WM_PASTE | WM_CUT | WM_CLEAR | WM_UNDO | EM_UNDO)
        || (msg == WM_KEYDOWN && matches!(wp as u16, VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_DELETE))
    {
        with_preview_engine(Engine::reset_all);
    }
    unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
}

fn apply_preview_key(hwnd: HWND, key: usize) -> bool {
    let mut start = 0u32;
    let mut end = 0u32;
    unsafe { SendMessageW(hwnd, EM_GETSEL, &mut start as *mut u32 as WPARAM, &mut end as *mut u32 as LPARAM) };
    if unsafe { GetKeyState(VK_CONTROL as i32) } < 0 || start != end {
        with_preview_engine(Engine::reset_all);
        return false;
    }
    let action = with_preview_engine(|engine| match key {
        8 => engine.process_backspace(),
        13 => engine.process_delimiter('\n'),
        27 => engine.process_escape(),
        key => match char::from_u32(key as u32) {
            Some(ch) if !ch.is_control() => engine.process_key(ch),
            _ => { engine.reset_all(); EngineAction::PassThrough }
        },
    });
    let Some(EngineAction::Replace { backspaces, text }) = action else { return false };
    let current = read_window_text(hwnd);
    let before: Vec<u16> = current.encode_utf16().take(start as usize).collect();
    let mut from = before.len();
    for _ in 0..backspaces {
        if from == 0 {
            break;
        }
        from -= 1;
        if from > 0 && (0xDC00..=0xDFFF).contains(&before[from])
            && (0xD800..=0xDBFF).contains(&before[from - 1])
        {
            from -= 1;
        }
    }
    let text = to_wide(&text.replace('\n', "\r\n"));
    unsafe {
        SendMessageW(hwnd, EM_SETSEL, from, end as LPARAM);
        SendMessageW(hwnd, EM_REPLACESEL, 1, text.as_ptr() as LPARAM);
    }
    true
}

const ID_DICT_EDIT: usize = 6701;
const ID_DICT_ADD: usize = 6702;
const ID_DICT_DEL: usize = 6703;
const ID_DICT_CLOSE: usize = 6704;
const ID_DICT_LIST: usize = 6705;
const ID_DICT_SEARCH: usize = 6706;
const ID_DICT_IMPORT: usize = 6707;
const ID_DICT_EXPORT: usize = 6708;
const ID_DICT_ADD_ESC: usize = 6709;
const DICT_EN_CHANGE_CODE: usize = 0x0300;

#[derive(Clone, Copy)]
struct DictDlg {
    font: HFONT,
    bg_br: HBRUSH,
    in_br: HBRUSH,
    edit: HWND,
    search_edit: HWND,
    list: HWND,
}

pub fn show_user_dict_dialog(parent: HWND) {
    register_tool_class("MSKeyUserDictDlg", dict_wnd_proc);
    let modal = create_modal_window(
        parent,
        "MSKeyUserDictDlg",
        "Từ điển cá nhân (MS Key)",
        500,
        418,
    );
    run_modal_loop(parent, modal);
}

fn reload_dict_list(list: HWND, search_edit: HWND) {
    let needle = read_window_text(search_edit).trim().to_lowercase();
    unsafe {
        SendMessageW(list, LB_RESETCONTENT, 0, 0);
        let mut indices = Vec::new();
        with_state(|st| {
            for (idx, word) in st.engine.config().user_dict_words.iter().enumerate() {
                if !needle.is_empty() && !word.to_lowercase().contains(&needle) {
                    continue;
                }
                indices.push(idx);
                let w = to_wide(word);
                SendMessageW(list, LB_ADDSTRING, 0, w.as_ptr() as LPARAM);
            }
        });
        DICT_FILTER_IDX.with(|f| *f.borrow_mut() = indices);
    }
}

pub fn add_word_to_user_dict(word: &str) -> bool {
    let clean = word.trim();
    if clean.is_empty() {
        return false;
    }
    with_state(|app| {
        let mut cfg = app.engine.config().clone();
        if cfg
            .user_dict_words
            .iter()
            .any(|w| w.eq_ignore_ascii_case(clean))
        {
            return false;
        }
        cfg.user_dict_words.push(clean.to_string());
        cfg.user_dict_words.sort_by_key(|w| w.to_lowercase());
        save_config(&cfg);
        app.engine.update_config(cfg);
        true
    })
}

fn import_dict_txt(hwnd: HWND, list: HWND, search_edit: HWND) {
    let filter = crate::platform_extra::make_win_filter(&[
        ("Word List (*.txt)", "*.txt"),
        ("All Files (*.*)", "*.*"),
    ]);
    let Some(path) = crate::platform_extra::browse_file_dialog(hwnd, false, &filter, "txt") else {
        return;
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return;
    };
    let words = Config::parse_user_dict_text(&raw);
    if words.is_empty() {
        return;
    }
    with_state(|app| {
        let mut cfg = app.engine.config().clone();
        for w in words {
            if !cfg.user_dict_words.iter().any(|ex| ex.eq_ignore_ascii_case(&w)) {
                cfg.user_dict_words.push(w);
            }
        }
        cfg.user_dict_words.sort_by_key(|w| w.to_lowercase());
        save_config(&cfg);
        app.engine.update_config(cfg);
    });
    reload_dict_list(list, search_edit);
}

fn export_dict_txt(hwnd: HWND) {
    let filter = crate::platform_extra::make_win_filter(&[
        ("Word List (*.txt)", "*.txt"),
        ("All Files (*.*)", "*.*"),
    ]);
    let Some(path) = crate::platform_extra::browse_file_dialog(hwnd, true, &filter, "txt") else {
        return;
    };
    let txt = with_state(|app| app.engine.config().format_user_dict_text());
    let _ = std::fs::write(path, txt);
}

fn handle_dict_cmd(hwnd: HWND, id: usize, code: usize, st: DictDlg) {
    match id {
        ID_DICT_SEARCH if code == DICT_EN_CHANGE_CODE => {
            reload_dict_list(st.list, st.search_edit);
        }
        ID_DICT_ADD => {
            let word = read_window_text(st.edit);
            if add_word_to_user_dict(&word) {
                write_window_text(st.edit, "");
                reload_dict_list(st.list, st.search_edit);
            }
        }
        ID_DICT_ADD_ESC => {
            if let Some(word) = with_state(|app| app.engine.last_word_for_dict()) {
                let _ = add_word_to_user_dict(&word);
                write_window_text(st.edit, &word);
                reload_dict_list(st.list, st.search_edit);
            }
        }
        ID_DICT_DEL => {
            let sel = unsafe { SendMessageW(st.list, LB_GETCURSEL, 0, 0) };
            let real_idx = if sel >= 0 {
                DICT_FILTER_IDX.with(|f| f.borrow().get(sel as usize).copied())
            } else {
                None
            };
            if let Some(idx) = real_idx {
                with_state(|app| {
                    let mut cfg = app.engine.config().clone();
                    if idx < cfg.user_dict_words.len() {
                        cfg.user_dict_words.remove(idx);
                        save_config(&cfg);
                        app.engine.update_config(cfg);
                    }
                });
                reload_dict_list(st.list, st.search_edit);
            }
        }
        ID_DICT_IMPORT => import_dict_txt(hwnd, st.list, st.search_edit),
        ID_DICT_EXPORT => export_dict_txt(hwnd),
        ID_DICT_CLOSE => unsafe { DestroyWindow(hwnd); },
        _ => {}
    }
}

unsafe extern "system" fn dict_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let pal = current_palette();
    match msg {
        WM_CREATE => {
            let dpi = window_dpi(hwnd);
            let font = create_ui_font(scale_px(14, dpi), false);
            let bg_br = unsafe { CreateSolidBrush(pal.card_bg) };
            let in_br = unsafe { CreateSolidBrush(pal.input_bg) };
            make_child(hwnd, "STATIC", "Thêm từ không bị khôi phục khi gõ:", 0, 16, 14, 360, 22, 0, font, dpi);
            let edit = make_edit(hwnd, "", 16, 38, 368, 28, ID_DICT_EDIT, false, font, dpi);
            set_cue_banner(edit, "Nhập từ cần bảo vệ...");
            make_owner_button(hwnd, "Thêm từ", 392, 38, 92, 28, ID_DICT_ADD, font, dpi);
            make_owner_button(hwnd, "Thêm từ Esc gần nhất", 16, 74, 172, 30, ID_DICT_ADD_ESC, font, dpi);
            make_owner_button(hwnd, "Xóa mục chọn", 196, 74, 122, 30, ID_DICT_DEL, font, dpi);
            make_owner_button(hwnd, "Đóng", 400, 74, 84, 30, ID_DICT_CLOSE, font, dpi);
            make_owner_button(hwnd, "Nhập .txt...", 16, 112, 108, 28, ID_DICT_IMPORT, font, dpi);
            make_owner_button(hwnd, "Xuất .txt...", 132, 112, 108, 28, ID_DICT_EXPORT, font, dpi);
            let search_edit = make_edit(hwnd, "", 248, 112, 236, 28, ID_DICT_SEARCH, false, font, dpi);
            set_cue_banner(search_edit, "Tìm từ trong từ điển...");
            let list = make_child(
                hwnd,
                "LISTBOX",
                "",
                (LBS_NOTIFY as u32) | WS_BORDER | WS_VSCROLL | WS_TABSTOP,
                16,
                148,
                468,
                254,
                ID_DICT_LIST,
                font,
                dpi,
            );
            reload_dict_list(list, search_edit);
            DICT_STATE.with(|c| {
                *c.borrow_mut() = Some(DictDlg {
                    font,
                    bg_br,
                    in_br,
                    edit,
                    search_edit,
                    list,
                });
            });
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            let mut rc: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut rc) };
            fill_solid(hdc, &rc, pal.card_bg);
            unsafe { EndPaint(hwnd, &ps) };
            0
        }
        WM_DRAWITEM => {
            let font = DICT_STATE.with(|c| c.borrow().map(|s| s.font).unwrap_or(ptr::null_mut()));
            handle_draw_combo_or_btn(lparam, &[ID_DICT_ADD as u32], pal, font).unwrap_or(0)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let (b1, b2) = DICT_STATE.with(|c| {
                c.borrow().map(|s| (s.bg_br, s.in_br)).unwrap_or((ptr::null_mut(), ptr::null_mut()))
            });
            handle_ctl_color(msg, wparam as _, pal, b1, b2)
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            let code = (wparam >> 16) & 0xFFFF;
            if let Some(st) = DICT_STATE.with(|c| *c.borrow()) {
                handle_dict_cmd(hwnd, id, code, st);
            }
            0
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            if let Some(st) = DICT_STATE.with(|c| c.borrow_mut().take()) {
                unsafe {
                    DeleteObject(st.font);
                    DeleteObject(st.bg_br);
                    DeleteObject(st.in_br);
                }
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
