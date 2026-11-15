use std::ptr;

use mskey_core::{EngineAction, SwitchPreset};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_BACK, VK_CAPITAL,
    VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_F24, VK_HOME, VK_INSERT,
    VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RCONTROL,
    VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetWindowThreadProcessId, HHOOK, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN,
    WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::hook_send::{
    MSKEY_EXTRA_INFO, ReplacePayload, convert_clipboard_now, dispatch_payload,
};
use crate::platform::{play_switch_beep, query_foreground_info, save_config};
use crate::state::{AppState, WM_APP_SYNC_UI, with_state};

const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;

pub struct HookGuard {
    kb_hook: HHOOK,
    mouse_hook: HHOOK,
    win_event: HWINEVENTHOOK,
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        unsafe {
            if !self.kb_hook.is_null() {
                UnhookWindowsHookEx(self.kb_hook);
            }
            if !self.mouse_hook.is_null() {
                UnhookWindowsHookEx(self.mouse_hook);
            }
            if !self.win_event.is_null() {
                UnhookWinEvent(self.win_event);
            }
        }
    }
}

pub fn install_hooks() -> HookGuard {
    let kb_hook =
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), ptr::null_mut(), 0) };
    let mouse_hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), ptr::null_mut(), 0) };
    let win_event = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            ptr::null_mut(),
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    };
    refresh_foreground();
    HookGuard {
        kb_hook,
        mouse_hook,
        win_event,
    }
}

fn refresh_foreground() {
    let (hwnd, proc_name, class_name, is_us) = query_foreground_info();
    let changed_ve = with_state(|st| {
        let before = st.engine.config().vietnamese_enabled;
        st.update_foreground(hwnd as usize, proc_name, class_name, is_us);
        (st.main_window(), st.engine.config().vietnamese_enabled != before)
    });
    if changed_ve.1 && !changed_ve.0.is_null() {
        unsafe {
            PostMessageW(changed_ve.0, WM_APP_SYNC_UI, 0, 0);
        }
    }
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    refresh_foreground();
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0
        && matches!(
            wparam as u32,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
        )
    {
        with_state(|st| st.engine.reset_all());
    }
    unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 || lparam == 0 {
        return unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) };
    }
    let kb = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
    if (kb.flags & LLKHF_INJECTED) != 0 || kb.dwExtraInfo == MSKEY_EXTRA_INFO {
        return unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) };
    }
    let msg = wparam as u32;
    let is_down = matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN);
    let is_up = matches!(msg, WM_KEYUP | WM_SYSKEYUP);
    if !is_down && !is_up {
        return unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) };
    }

    let vk = kb.vkCode as u16;
    match handle_key_event(vk, kb.scanCode, is_down, is_up) {
        KeyDecision::Pass => unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) },
        KeyDecision::Consume => 1,
        KeyDecision::Inject(payload) => {
            dispatch_payload(payload);
            1
        }
    }
}

enum KeyDecision {
    Pass,
    Consume,
    Inject(ReplacePayload),
}

fn is_modifier_vk(vk: u16) -> bool {
    matches!(
        vk,
        VK_CONTROL
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_SHIFT
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_MENU
            | VK_LMENU
            | VK_RMENU
            | VK_LWIN
            | VK_RWIN
    )
}

fn update_modifier_flags(st: &mut AppState, vk: u16, is_down: bool) {
    if vk == VK_CONTROL || vk == VK_LCONTROL || vk == VK_RCONTROL {
        st.ctrl_down = is_down;
    } else if vk == VK_SHIFT || vk == VK_LSHIFT || vk == VK_RSHIFT {
        st.shift_down = is_down;
    } else if vk == VK_MENU || vk == VK_LMENU || vk == VK_RMENU {
        st.alt_down = is_down;
    } else if vk == VK_LWIN || vk == VK_RWIN {
        st.win_down = is_down;
    }
}

fn trigger_ve_toggle(
    st: &mut AppState,
    next: bool,
    notify_hwnd: &mut HWND,
    should_beep: &mut bool,
    should_save: &mut bool,
    osd_next: &mut Option<bool>,
) {
    let cfg = st.engine.config().clone();
    st.set_user_vietnamese(next);
    if cfg.smart_app_switch && !st.fg_process.is_empty() {
        crate::platform_extra::record_app_ve_memory(&st.fg_process, next);
    }
    *notify_hwnd = st.main_window();
    *should_beep = cfg.beep_on_switch;
    *should_save = true;
    if cfg.show_switch_osd {
        *osd_next = Some(next);
    }
}

fn check_hotkey_decision(
    st: &mut AppState,
    vk: u16,
    is_down: bool,
    is_up: bool,
    notify_hwnd: &mut HWND,
    should_beep: &mut bool,
    should_save: &mut bool,
    osd_next: &mut Option<bool>,
) -> Option<KeyDecision> {
    let cfg = st.engine.config().clone();
    let double_shift = crate::platform_extra::update_shift_tap_state(
        vk,
        is_down,
        is_up,
        st.ctrl_down,
        st.alt_down,
        st.win_down,
    );
    if cfg.switch_preset == SwitchPreset::ShiftShift && double_shift {
        trigger_ve_toggle(st, !cfg.vietnamese_enabled, notify_hwnd, should_beep, should_save, osd_next);
        return Some(KeyDecision::Pass);
    }
    if cfg.switch_preset == SwitchPreset::CtrlShift {
        if is_down && is_modifier_vk(vk) && st.ctrl_down && st.shift_down && !st.alt_down && !st.win_down {
            st.pending_ctrl_shift = true;
        } else if is_down && !is_modifier_vk(vk) {
            st.pending_ctrl_shift = false;
        } else if is_up && st.pending_ctrl_shift && is_modifier_vk(vk) {
            st.pending_ctrl_shift = false;
            trigger_ve_toggle(st, !cfg.vietnamese_enabled, notify_hwnd, should_beep, should_save, osd_next);
            return Some(KeyDecision::Pass);
        }
    }
    if !is_down {
        return Some(KeyDecision::Pass);
    }
    let ctrl_space = cfg.switch_preset == SwitchPreset::CtrlSpace
        && st.ctrl_down
        && !st.alt_down
        && !st.shift_down
        && !st.win_down
        && vk == 0x20;
    if ctrl_space {
        if crate::platform_extra::take_fresh_ctrl_space_press() {
            trigger_ve_toggle(st, !cfg.vietnamese_enabled, notify_hwnd, should_beep, should_save, osd_next);
        }
        return Some(KeyDecision::Consume);
    }
    let alt_z = cfg.switch_preset == SwitchPreset::AltZ
        && st.alt_down
        && !st.ctrl_down
        && !st.shift_down
        && !st.win_down
        && vk == b'Z' as u16;
    let custom_sw = cfg.switch_preset == SwitchPreset::Custom
        && combo_matches(&cfg.custom_switch_hotkey, st.ctrl_down, st.alt_down, st.shift_down, st.win_down, vk as u32);
    if alt_z || custom_sw {
        trigger_ve_toggle(st, !cfg.vietnamese_enabled, notify_hwnd, should_beep, should_save, osd_next);
        return Some(KeyDecision::Consume);
    }
    if combo_matches(&cfg.clipboard_convert_hotkey, st.ctrl_down, st.alt_down, st.shift_down, st.win_down, vk as u32) {
        convert_clipboard_now(cfg.charset);
        *should_beep = cfg.beep_on_switch;
        return Some(KeyDecision::Consume);
    }
    None
}

fn dispatch_typing_key(st: &mut AppState, vk: u16, scan: u32) -> KeyDecision {
    let cfg = st.engine.config().clone();
    let us_layout = if cfg.disable_on_non_us_layout {
        let live = crate::platform_extra::is_foreground_layout_us();
        st.fg_is_us = live;
        live
    } else {
        true
    };
    if st.is_blacklisted() || (cfg.disable_on_non_us_layout && !us_layout) {
        return KeyDecision::Pass;
    }
    if st.ctrl_down || st.alt_down || st.win_down {
        if !is_modifier_vk(vk) {
            st.engine.reset_word();
        }
        return KeyDecision::Pass;
    }
    if crate::platform_extra::is_focused_password_field() {
        st.engine.reset_all();
        return KeyDecision::Pass;
    }
    if vk == VK_BACK {
        return map_action(st.engine.process_backspace(), None, &cfg, &st.fg_class);
    }
    if vk == VK_ESCAPE {
        return map_action(st.engine.process_escape(), None, &cfg, &st.fg_class);
    }
    if is_reset_navigation_vk(vk) {
        st.engine.reset_all();
        return KeyDecision::Pass;
    }
    if vk == VK_RETURN {
        return map_action(st.engine.process_delimiter('\n'), Some(VK_RETURN), &cfg, &st.fg_class);
    }
    if vk == VK_TAB {
        return map_action(st.engine.process_delimiter('\t'), Some(VK_TAB), &cfg, &st.fg_class);
    }
    let Some(ch) = translate_vk_to_char(vk, scan, st.shift_down) else {
        return KeyDecision::Pass;
    };
    let trailing = if ch == ' ' { Some(VK_SPACE) } else { None };
    map_action(st.engine.process_key(ch), trailing, &cfg, &st.fg_class)
}

fn handle_key_event(vk: u16, scan: u32, is_down: bool, is_up: bool) -> KeyDecision {
    let fg_now = unsafe { GetForegroundWindow() } as usize;
    let cached_fg = with_state(|st| st.fg_hwnd);
    if fg_now != 0 && fg_now != cached_fg {
        refresh_foreground();
        let sync_hw = with_state(|st| {
            let cfg = st.engine.config().clone();
            if cfg.smart_app_switch
                && !st.fg_process.is_empty()
                && cfg.behavior_for_app(&st.fg_process)
                    == cfg.behavior_for_app("__mskey_no_rule__")
            {
                if let Some(mem_ve) =
                    crate::platform_extra::recalled_app_ve_memory(&st.fg_process)
                {
                    if cfg.vietnamese_enabled != mem_ve {
                        let mut next_cfg = cfg;
                        next_cfg.vietnamese_enabled = mem_ve;
                        st.engine.update_config(next_cfg);
                        return Some(st.main_window());
                    }
                }
            }
            None
        });
        if let Some(hw) = sync_hw {
            if !hw.is_null() {
                unsafe {
                    PostMessageW(hw, WM_APP_SYNC_UI, 0, 0);
                }
            }
        }
    }

    let mut notify_hwnd: HWND = ptr::null_mut();
    let mut should_beep = false;
    let mut should_save = false;
    let mut osd_next: Option<bool> = None;

    let decision = with_state(|st| {
        update_modifier_flags(st, vk, is_down);
        if st.suspend_typing_for_hotkey {
            return KeyDecision::Pass;
        }
        if let Some(d) = check_hotkey_decision(
            st,
            vk,
            is_down,
            is_up,
            &mut notify_hwnd,
            &mut should_beep,
            &mut should_save,
            &mut osd_next,
        ) {
            return d;
        }
        dispatch_typing_key(st, vk, scan)
    });

    if should_beep {
        play_switch_beep();
    }
    if should_save {
        with_state(|st| save_config(st.engine.config()));
    }
    if !notify_hwnd.is_null() {
        unsafe {
            PostMessageW(notify_hwnd, WM_APP_SYNC_UI, 0, 0);
            if let Some(is_vi) = osd_next {
                PostMessageW(
                    notify_hwnd,
                    crate::state::WM_APP_SHOW_OSD,
                    if is_vi { 1 } else { 0 },
                    0,
                );
            }
        }
    }
    decision
}

fn combo_matches(
    c: &mskey_core::HotkeyCombo,
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    vk: u32,
) -> bool {
    c.key_code != 0
        && c.ctrl == ctrl
        && c.alt == alt
        && c.shift == shift
        && c.win_cmd == win
        && c.key_code == vk
}

fn is_reset_navigation_vk(vk: u16) -> bool {
    matches!(
        vk,
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_DELETE
            | VK_INSERT
    ) || (vk >= VK_F1 && vk <= VK_F24)
}

fn map_action(
    act: EngineAction,
    trailing_vk: Option<u16>,
    cfg: &mskey_core::Config,
    fg_class: &str,
) -> KeyDecision {
    match crate::hook_send::build_replace_payload(act, trailing_vk, cfg, fg_class) {
        Some(p) => KeyDecision::Inject(p),
        None => KeyDecision::Pass,
    }
}

fn translate_vk_to_char(vk: u16, scan: u32, shift_down: bool) -> Option<char> {
    let mut state = [0u8; 256];
    let _ = unsafe { GetKeyboardState(state.as_mut_ptr()) };
    if shift_down || unsafe { GetAsyncKeyState(VK_SHIFT as i32) } < 0 {
        state[VK_SHIFT as usize] = 0x80;
    } else {
        state[VK_SHIFT as usize] = 0;
    }
    state[VK_CONTROL as usize] = 0;
    state[VK_LCONTROL as usize] = 0;
    state[VK_RCONTROL as usize] = 0;
    state[VK_MENU as usize] = 0;
    state[VK_LMENU as usize] = 0;
    state[VK_RMENU as usize] = 0;
    if (unsafe { GetAsyncKeyState(VK_CAPITAL as i32) } & 1) != 0 {
        state[VK_CAPITAL as usize] |= 0x01;
    }
    let fg = unsafe { GetForegroundWindow() };
    let tid = if fg.is_null() {
        0
    } else {
        unsafe { GetWindowThreadProcessId(fg, ptr::null_mut()) }
    };
    let hkl = unsafe { GetKeyboardLayout(tid) };
    let mut out = [0u16; 4];
    let count = unsafe {
        ToUnicodeEx(
            vk as u32,
            scan,
            state.as_ptr(),
            out.as_mut_ptr(),
            out.len() as i32,
            0x04,
            hkl,
        )
    };
    if count != 1 {
        return None;
    }
    char::from_u32(out[0] as u32).filter(|c| !c.is_control())
}
