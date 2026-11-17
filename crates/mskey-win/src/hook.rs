use std::ptr;
use std::sync::Mutex;
use std::time::Instant;

use mskey_core::{EngineAction, HoldEnglishKey, SwitchPreset};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_BACK, VK_CAPITAL,
    VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_F6, VK_F24, VK_HOME, VK_INSERT,
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
    MSKEY_EXTRA_INFO, ReplacePayload, convert_clipboard_now, dispatch_payload, send_capslock_tap,
    send_cursor_left,
};
use crate::platform::{
    play_switch_beep, query_foreground_info, query_window_title, save_config,
};
use crate::state::{
    AppState, WM_APP_CONVERT_SELECTION, WM_APP_SYNC_UI, WM_APP_UPDATE_CARET_OSD, with_state,
};

const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const EVENT_OBJECT_NAMECHANGE: u32 = 0x800C;
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;

struct CapsTapState {
    down_at: Option<Instant>,
    had_other_key: bool,
    toggled_os: bool,
}

impl CapsTapState {
    const fn new() -> Self {
        Self {
            down_at: None,
            had_other_key: false,
            toggled_os: false,
        }
    }

    fn reset(&mut self) {
        self.down_at = None;
        self.had_other_key = false;
        self.toggled_os = false;
    }
}

static CAPS_TAP: Mutex<CapsTapState> = Mutex::new(CapsTapState::new());

pub struct HookGuard {
    kb_hook: HHOOK,
    mouse_hook: HHOOK,
    win_event: HWINEVENTHOOK,
    name_event: HWINEVENTHOOK,
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
            if !self.name_event.is_null() {
                UnhookWinEvent(self.name_event);
            }
        }
    }
}

pub fn install_hooks() -> HookGuard {
    let kb_hook =
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), ptr::null_mut(), 0) };
    let mouse_hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), ptr::null_mut(), 0) };
    let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
    let win_event = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            ptr::null_mut(),
            Some(win_event_proc),
            0,
            0,
            flags,
        )
    };
    let name_event = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_NAMECHANGE,
            EVENT_OBJECT_NAMECHANGE,
            ptr::null_mut(),
            Some(win_event_proc),
            0,
            0,
            flags,
        )
    };
    refresh_foreground();
    HookGuard {
        kb_hook,
        mouse_hook,
        win_event,
        name_event,
    }
}

fn post_caret_osd_if_enabled(hwnd: HWND, is_vi: bool, force_show: bool, caret_enabled: bool) {
    if !hwnd.is_null() && (caret_enabled || force_show) {
        unsafe {
            PostMessageW(
                hwnd,
                WM_APP_UPDATE_CARET_OSD,
                if is_vi { 1 } else { 0 },
                if force_show { 1 } else { 0 },
            );
        }
    }
}

fn refresh_foreground() {
    let (hwnd, proc_name, class_name, title, is_us) = query_foreground_info();
    crate::uia::reset_cached();
    if let Ok(mut caps) = CAPS_TAP.lock() {
        caps.reset();
    }
    let (notify_hw, changed_ve, is_vi, caret_on) = with_state(|st| {
        let before_active = st.effective_vietnamese();
        let before_blacklisted = st.is_blacklisted();
        st.engine.set_temporary_english(false);
        st.update_foreground(hwnd as usize, proc_name, class_name, title, is_us);
        let after_active = st.effective_vietnamese();
        let after_blacklisted = st.is_blacklisted();
        (
            st.main_window(),
            after_active != before_active || after_blacklisted != before_blacklisted,
            after_active,
            st.engine.config().show_caret_osd,
        )
    });
    if !notify_hw.is_null() {
        if changed_ve {
            unsafe {
                PostMessageW(notify_hw, WM_APP_SYNC_UI, 0, 0);
            }
        }
        post_caret_osd_if_enabled(notify_hw, is_vi, false, caret_on);
    }
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if event == EVENT_OBJECT_NAMECHANGE {
        if id_object != 0 || id_child != 0 || hwnd.is_null() {
            return;
        }
        let fg = unsafe { GetForegroundWindow() };
        if hwnd != fg {
            return;
        }
        let title = query_window_title(hwnd);
        let notify = with_state(|st| {
            if st.fg_hwnd == hwnd as usize && st.update_foreground_title(title) {
                Some((
                    st.main_window(),
                    st.effective_vietnamese(),
                    st.engine.config().show_caret_osd,
                ))
            } else {
                None
            }
        });
        if let Some((hw, is_vi, caret_on)) = notify
            && !hw.is_null()
        {
            unsafe {
                PostMessageW(hw, WM_APP_SYNC_UI, 0, 0);
            }
            post_caret_osd_if_enabled(hw, is_vi, false, caret_on);
        }
        return;
    }
    refresh_foreground();
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0
        && matches!(
            wparam as u32,
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN
        )
    {
        crate::uia::notify_focus_check();
        let caret_info = with_state(|st| {
            st.engine.reset_all();
            if st.engine.config().show_caret_osd {
                Some((st.main_window(), st.effective_vietnamese()))
            } else {
                None
            }
        });
        if let Some((hw, is_vi)) = caret_info {
            post_caret_osd_if_enabled(hw, is_vi, false, true);
        }
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
            let steps = with_state(|st| st.engine.take_pending_cursor_left());
            if steps > 0 {
                send_cursor_left(steps);
            }
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

struct KeySideEffects {
    notify_hwnd: HWND,
    sync_ui: bool,
    should_beep: bool,
    should_save: bool,
    osd_next: Option<bool>,
    caret_osd: Option<(bool, bool)>,
    post_convert_selection: Option<usize>,
    send_os_capslock: bool,
}

impl KeySideEffects {
    fn new() -> Self {
        Self {
            notify_hwnd: ptr::null_mut(),
            sync_ui: false,
            should_beep: false,
            should_save: false,
            osd_next: None,
            caret_osd: None,
            post_convert_selection: None,
            send_os_capslock: false,
        }
    }
}

fn hold_english_vk(key: HoldEnglishKey) -> Option<u16> {
    match key {
        HoldEnglishKey::None => None,
        HoldEnglishKey::RightShift => Some(VK_RSHIFT),
        HoldEnglishKey::LeftShift => Some(VK_LSHIFT),
        HoldEnglishKey::RightAlt => Some(VK_RMENU),
        HoldEnglishKey::RightCtrl => Some(VK_RCONTROL),
        HoldEnglishKey::CapsLock => Some(VK_CAPITAL),
    }
}

fn is_modifier_swallowed_for_hold(hold_key: HoldEnglishKey, vk: u16) -> bool {
    matches!(
        (hold_key, vk),
        (HoldEnglishKey::RightCtrl, VK_RCONTROL) | (HoldEnglishKey::RightAlt, VK_RMENU)
    )
}

fn update_modifier_flags(st: &mut AppState, hold_key: HoldEnglishKey, vk: u16, is_down: bool) {
    if is_modifier_swallowed_for_hold(hold_key, vk) {
        return;
    }
    match vk {
        VK_LCONTROL => st.l_ctrl_down = is_down,
        VK_RCONTROL => st.r_ctrl_down = is_down,
        VK_CONTROL => {
            st.l_ctrl_down = is_down;
            st.r_ctrl_down = is_down;
        }
        VK_LSHIFT => st.l_shift_down = is_down,
        VK_RSHIFT => st.r_shift_down = is_down,
        VK_SHIFT => {
            st.l_shift_down = is_down;
            st.r_shift_down = is_down;
        }
        VK_LMENU => st.l_alt_down = is_down,
        VK_RMENU => st.r_alt_down = is_down,
        VK_MENU => {
            st.l_alt_down = is_down;
            st.r_alt_down = is_down;
        }
        VK_LWIN => st.l_win_down = is_down,
        VK_RWIN => st.r_win_down = is_down,
        _ if is_down => {
            if st.l_ctrl_down && unsafe { GetAsyncKeyState(VK_LCONTROL as i32) } >= 0 {
                st.l_ctrl_down = false;
            }
            if st.r_ctrl_down && unsafe { GetAsyncKeyState(VK_RCONTROL as i32) } >= 0 {
                st.r_ctrl_down = false;
            }
            if st.l_shift_down && unsafe { GetAsyncKeyState(VK_LSHIFT as i32) } >= 0 {
                st.l_shift_down = false;
            }
            if st.r_shift_down && unsafe { GetAsyncKeyState(VK_RSHIFT as i32) } >= 0 {
                st.r_shift_down = false;
            }
            if st.l_alt_down && unsafe { GetAsyncKeyState(VK_LMENU as i32) } >= 0 {
                st.l_alt_down = false;
            }
            if st.r_alt_down && unsafe { GetAsyncKeyState(VK_RMENU as i32) } >= 0 {
                st.r_alt_down = false;
            }
            if st.l_win_down && unsafe { GetAsyncKeyState(VK_LWIN as i32) } >= 0 {
                st.l_win_down = false;
            }
            if st.r_win_down && unsafe { GetAsyncKeyState(VK_RWIN as i32) } >= 0 {
                st.r_win_down = false;
            }
        }
        _ => {}
    }
    st.ctrl_down = st.l_ctrl_down || st.r_ctrl_down;
    st.shift_down = st.l_shift_down || st.r_shift_down;
    st.alt_down = st.l_alt_down || st.r_alt_down;
    st.win_down = st.l_win_down || st.r_win_down;
}

fn trigger_ve_toggle(st: &mut AppState, next: bool, fx: &mut KeySideEffects) {
    let cfg = st.engine.config().clone();
    st.set_user_vietnamese(next);
    fx.notify_hwnd = st.main_window();
    fx.sync_ui = true;
    fx.should_beep = cfg.beep_on_switch;
    fx.should_save = true;
    if cfg.show_switch_osd {
        fx.osd_next = Some(next);
    }
    if cfg.show_caret_osd {
        fx.caret_osd = Some((st.effective_vietnamese(), st.engine.is_temporary_english()));
    }
}

fn handle_hold_english(
    st: &mut AppState,
    hold_key: HoldEnglishKey,
    vk: u16,
    is_down: bool,
    is_up: bool,
    fx: &mut KeySideEffects,
) -> Option<KeyDecision> {
    let target_vk = hold_english_vk(hold_key)?;
    if vk != target_vk {
        return None;
    }
    let swallow = matches!(
        hold_key,
        HoldEnglishKey::RightCtrl | HoldEnglishKey::RightAlt | HoldEnglishKey::CapsLock
    );
    if is_down {
        let mods_ok = match hold_key {
            HoldEnglishKey::LeftShift | HoldEnglishKey::RightShift => {
                !st.ctrl_down && !st.alt_down && !st.win_down
            }
            HoldEnglishKey::RightCtrl
            | HoldEnglishKey::RightAlt
            | HoldEnglishKey::CapsLock => {
                !st.shift_down && !st.ctrl_down && !st.alt_down && !st.win_down
            }
            HoldEnglishKey::None => false,
        };
        if mods_ok {
            if !st.engine.is_temporary_english() {
                st.engine.set_temporary_english(true);
                fx.notify_hwnd = st.main_window();
                fx.sync_ui = true;
                fx.caret_osd = Some((st.effective_vietnamese(), true));
            }
            if swallow {
                return Some(KeyDecision::Consume);
            }
        }
    } else if is_up {
        let was_temp = st.engine.is_temporary_english();
        if was_temp {
            st.engine.set_temporary_english(false);
            fx.notify_hwnd = st.main_window();
            fx.sync_ui = true;
            fx.caret_osd = Some((st.effective_vietnamese(), false));
        }
        if swallow && (was_temp || hold_key != HoldEnglishKey::CapsLock) {
            return Some(KeyDecision::Consume);
        }
    }
    None
}

fn handle_quick_capslock(
    st: &mut AppState,
    vk: u16,
    is_down: bool,
    is_up: bool,
    fx: &mut KeySideEffects,
) -> Option<KeyDecision> {
    let cfg = st.engine.config().clone();
    if !cfg.quick_capslock_switch || cfg.hold_english_key == HoldEnglishKey::CapsLock {
        return None;
    }
    let Ok(mut caps) = CAPS_TAP.lock() else {
        return None;
    };
    if vk != VK_CAPITAL {
        if is_down && caps.down_at.is_some() && !is_modifier_swallowed_for_hold(cfg.hold_english_key, vk)
            && !is_modifier_vk(vk)
        {
            caps.had_other_key = true;
        }
        return None;
    }
    if is_down {
        if caps.down_at.is_none() {
            if st.shift_down || st.ctrl_down || st.alt_down || st.win_down {
                return None;
            }
            caps.down_at = Some(Instant::now());
            caps.had_other_key = false;
            caps.toggled_os = false;
            return Some(KeyDecision::Consume);
        }
        if !caps.toggled_os
            && !caps.had_other_key
            && caps.down_at.is_some_and(|t| t.elapsed().as_millis() >= 250)
        {
            caps.toggled_os = true;
            fx.send_os_capslock = true;
            st.caps_lock_on = !st.caps_lock_on;
        }
        return Some(KeyDecision::Consume);
    }
    if is_up && let Some(down_at) = caps.down_at.take() {
        let had_other = caps.had_other_key;
        let already_os = caps.toggled_os;
        caps.had_other_key = false;
        caps.toggled_os = false;
        drop(caps);
        if !already_os {
            if !had_other && !st.shift_down && down_at.elapsed().as_millis() < 250 {
                trigger_ve_toggle(st, !cfg.vietnamese_enabled, fx);
            } else if !had_other {
                fx.send_os_capslock = true;
                st.caps_lock_on = !st.caps_lock_on;
            }
        }
        return Some(KeyDecision::Consume);
    }
    None
}

fn check_hotkey_decision(
    st: &mut AppState,
    vk: u16,
    is_down: bool,
    is_up: bool,
    fx: &mut KeySideEffects,
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
        trigger_ve_toggle(st, !cfg.vietnamese_enabled, fx);
        return Some(KeyDecision::Pass);
    }
    if cfg.switch_preset == SwitchPreset::CtrlShift {
        if is_down
            && is_modifier_vk(vk)
            && st.ctrl_down
            && st.shift_down
            && !st.alt_down
            && !st.win_down
        {
            st.pending_ctrl_shift = true;
        } else if is_down && !is_modifier_vk(vk) {
            st.pending_ctrl_shift = false;
        } else if is_up && st.pending_ctrl_shift && is_modifier_vk(vk) {
            st.pending_ctrl_shift = false;
            trigger_ve_toggle(st, !cfg.vietnamese_enabled, fx);
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
            trigger_ve_toggle(st, !cfg.vietnamese_enabled, fx);
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
        && combo_matches(
            &cfg.custom_switch_hotkey,
            st.ctrl_down,
            st.alt_down,
            st.shift_down,
            st.win_down,
            vk as u32,
        );
    if alt_z || custom_sw {
        trigger_ve_toggle(st, !cfg.vietnamese_enabled, fx);
        return Some(KeyDecision::Consume);
    }
    if combo_matches(
        &cfg.clipboard_convert_hotkey,
        st.ctrl_down,
        st.alt_down,
        st.shift_down,
        st.win_down,
        vk as u32,
    ) {
        convert_clipboard_now(cfg.charset);
        fx.should_beep = cfg.beep_on_switch;
        return Some(KeyDecision::Consume);
    }
    if combo_matches(
        &cfg.undo_last_word_hotkey,
        st.ctrl_down,
        st.alt_down,
        st.shift_down,
        st.win_down,
        vk as u32,
    ) {
        let act = st.engine.toggle_last_committed_word();
        if let Some(payload) = crate::hook_send::build_replace_payload(
            act,
            None,
            &cfg,
            &st.fg_class,
            &st.fg_process,
        ) {
            return Some(KeyDecision::Inject(payload));
        }
        return Some(KeyDecision::Pass);
    }
    if combo_matches(
        &cfg.convert_selection_hotkey,
        st.ctrl_down,
        st.alt_down,
        st.shift_down,
        st.win_down,
        vk as u32,
    ) {
        fx.notify_hwnd = st.main_window();
        fx.post_convert_selection = Some(st.fg_hwnd);
        return Some(KeyDecision::Consume);
    }
    None
}

fn maybe_preload_macro_clipboard(st: &mut AppState) {
    let cfg = st.engine.config();
    if cfg.macro_enabled
        && !st.engine.raw_word().is_empty()
        && cfg
            .macros
            .iter()
            .any(|m| m.enabled && m.expansion.contains("{clipboard}"))
    {
        st.engine
            .set_clipboard_text(crate::platform_extra::read_clipboard_text());
    }
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
        return map_action(
            st.engine.process_backspace(),
            None,
            &cfg,
            &st.fg_class,
            &st.fg_process,
        );
    }
    if vk == VK_ESCAPE {
        return map_action(
            st.engine.process_escape(),
            None,
            &cfg,
            &st.fg_class,
            &st.fg_process,
        );
    }
    if is_reset_navigation_vk(vk) {
        st.engine.reset_all();
        return KeyDecision::Pass;
    }
    if vk == VK_RETURN {
        maybe_preload_macro_clipboard(st);
        return map_action(
            st.engine.process_delimiter('\n'),
            Some(VK_RETURN),
            &cfg,
            &st.fg_class,
            &st.fg_process,
        );
    }
    if vk == VK_TAB {
        maybe_preload_macro_clipboard(st);
        return map_action(
            st.engine.process_delimiter('\t'),
            Some(VK_TAB),
            &cfg,
            &st.fg_class,
            &st.fg_process,
        );
    }
    let Some(ch) = translate_vk_to_char(vk, scan, st.shift_down, st.caps_lock_on) else {
        return KeyDecision::Pass;
    };
    if st.engine.is_active_delimiter(ch) {
        maybe_preload_macro_clipboard(st);
    }
    let trailing = if ch == ' ' { Some(VK_SPACE) } else { None };
    map_action(
        st.engine.process_key(ch),
        trailing,
        &cfg,
        &st.fg_class,
        &st.fg_process,
    )
}

fn handle_key_event(vk: u16, scan: u32, is_down: bool, is_up: bool) -> KeyDecision {
    let fg_now = unsafe { GetForegroundWindow() } as usize;
    if crate::ui_popup::is_popup_hwnd(fg_now) {
        return KeyDecision::Pass;
    }
    let cached_fg = with_state(|st| st.fg_hwnd);
    if fg_now != 0 && fg_now != cached_fg {
        refresh_foreground();
    }

    let mut fx = KeySideEffects::new();

    let decision = with_state(|st| {
        let hold_key = st.engine.config().hold_english_key;
        update_modifier_flags(st, hold_key, vk, is_down);
        if is_down
            && (matches!(vk, VK_TAB | VK_RETURN | VK_F6)
                || ((st.ctrl_down || st.alt_down) && !is_modifier_vk(vk)))
        {
            crate::uia::notify_focus_check();
        }
        if st.suspend_typing_for_hotkey {
            return KeyDecision::Pass;
        }
        if let Some(d) = handle_hold_english(st, hold_key, vk, is_down, is_up, &mut fx) {
            return d;
        }
        if let Some(d) = handle_quick_capslock(st, vk, is_down, is_up, &mut fx) {
            return d;
        }
        if vk == VK_CAPITAL {
            if is_down && !st.caps_key_down {
                st.caps_key_down = true;
                if !st.ctrl_down && !st.alt_down && !st.win_down {
                    st.caps_lock_on = !st.caps_lock_on;
                }
            } else if is_up {
                st.caps_key_down = false;
            }
        }
        if is_down && matches!(vk, VK_TAB | VK_RETURN) && st.engine.config().show_caret_osd {
            fx.notify_hwnd = st.main_window();
            if fx.caret_osd.is_none() {
                fx.caret_osd = Some((st.effective_vietnamese(), st.engine.is_temporary_english()));
            }
        }
        if let Some(d) = check_hotkey_decision(st, vk, is_down, is_up, &mut fx) {
            return d;
        }
        dispatch_typing_key(st, vk, scan)
    });

    if fx.send_os_capslock {
        send_capslock_tap();
    }
    if fx.should_beep {
        play_switch_beep();
    }
    if fx.should_save {
        with_state(|st| save_config(st.engine.config()));
    }
    if !fx.notify_hwnd.is_null() {
        unsafe {
            if fx.sync_ui {
                PostMessageW(fx.notify_hwnd, WM_APP_SYNC_UI, 0, 0);
            }
            if let Some(is_vi) = fx.osd_next {
                PostMessageW(
                    fx.notify_hwnd,
                    crate::state::WM_APP_SHOW_OSD,
                    if is_vi { 1 } else { 0 },
                    0,
                );
            }
            if let Some((is_vi, force_show)) = fx.caret_osd {
                PostMessageW(
                    fx.notify_hwnd,
                    WM_APP_UPDATE_CARET_OSD,
                    if is_vi { 1 } else { 0 },
                    if force_show { 1 } else { 0 },
                );
            }
            if let Some(target_fg) = fx.post_convert_selection {
                PostMessageW(
                    fx.notify_hwnd,
                    WM_APP_CONVERT_SELECTION,
                    target_fg,
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
    ) || (VK_F1..=VK_F24).contains(&vk)
}

fn map_action(
    act: EngineAction,
    trailing_vk: Option<u16>,
    cfg: &mskey_core::Config,
    fg_class: &str,
    fg_process: &str,
) -> KeyDecision {
    match crate::hook_send::build_replace_payload(act, trailing_vk, cfg, fg_class, fg_process) {
        Some(p) => KeyDecision::Inject(p),
        None => KeyDecision::Pass,
    }
}

fn translate_vk_to_char(vk: u16, scan: u32, shift_down: bool, caps_on: bool) -> Option<char> {
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
    state[VK_CAPITAL as usize] = if caps_on { 0x01 } else { 0 };
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
