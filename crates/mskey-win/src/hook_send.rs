use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use mskey_core::{
    CaseTransform, Charset, Config, ConvertEncoding, EngineAction, Mechanism, convert_with_encoding,
    detect_convert_encoding,
};
use windows_sys::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput, VK_BACK,
    VK_CAPITAL, VK_INSERT, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL,
    VK_RETURN, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_TAB,
};

use crate::platform::write_clipboard_text;
use crate::platform_extra::write_clipboard_for_paste_with_restore;

pub const MSKEY_EXTRA_INFO: usize = 0x4D53_4B59;
const BATCH_MARKER: usize = 0x4D54_0000;
const MAX_BATCH_AGE: Duration = Duration::from_secs(1);

thread_local! {
    static NEXT_BATCH: Cell<u16> = const { Cell::new(0) };
    static PENDING_BATCHES: RefCell<Vec<(usize, Instant)>> = const { RefCell::new(Vec::new()) };
}

pub fn output_pending() -> bool {
    PENDING_BATCHES.with_borrow_mut(|batches| {
        batches.retain(|(_, start)| start.elapsed() < MAX_BATCH_AGE);
        !batches.is_empty()
    })
}

pub fn acknowledge_output(marker: usize) {
    if marker & !0xffff == BATCH_MARKER {
        PENDING_BATCHES.with_borrow_mut(|batches| batches.retain(|(id, _)| *id != marker));
    }
}

pub fn clear_pending_output() {
    PENDING_BATCHES.with_borrow_mut(Vec::clear);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    Complete,
    Rejected,
    Partial,
}

impl Delivery {
    fn from_counts(complete: bool, inserted: usize) -> Self {
        match (complete, inserted) {
            (true, _) => Self::Complete,
            (false, 0) => Self::Rejected,
            (false, _) => Self::Partial,
        }
    }
}

pub struct ReplacePayload {
    pub backspaces: usize,
    pub text: String,
    pub trailing_vk: Option<u16>,
    pub browser_suggest_1: bool,
    pub browser_suggest_2: bool,
    pub use_clipboard: bool,
    pub standard_key_send: bool,
    pub compatible_hook: bool,
    pub paste_via_shift_insert: bool,
    pub send_delay_ms: u32,
}

const MODIFIER_VKS: &[(u16, bool)] = &[
    (VK_LSHIFT, false),
    (VK_RSHIFT, false),
    (VK_LCONTROL, false),
    (VK_RCONTROL, true),
    (VK_LMENU, false),
    (VK_RMENU, true),
];

const ALL_MODIFIER_VKS: &[(u16, bool)] = &[
    (VK_LSHIFT, false),
    (VK_RSHIFT, false),
    (VK_LCONTROL, false),
    (VK_RCONTROL, true),
    (VK_LMENU, false),
    (VK_RMENU, true),
    (VK_LWIN, true),
    (VK_RWIN, true),
];

fn capture_held_modifiers(candidates: &[(u16, bool)]) -> Vec<(u16, bool)> {
    candidates
        .iter()
        .copied()
        .filter(|&(vk, _)| unsafe { GetAsyncKeyState(vk as i32) } < 0)
        .collect()
}

fn should_cycle_modifiers(
    p: &ReplacePayload,
    step_delay: u32,
    restore_before_trailing: bool,
) -> bool {
    step_delay == 0
        || p.use_clipboard
        || p.browser_suggest_2
        || restore_before_trailing
        || p.standard_key_send
}

pub fn dispatch_payload(p: ReplacePayload, cursor_left: usize) -> Delivery {
    dispatch_payload_with(p, cursor_left, send_slice)
}

pub(super) fn dispatch_payload_with(
    p: ReplacePayload,
    cursor_left: usize,
    mut submit: impl FnMut(&[INPUT]) -> usize,
) -> Delivery {
    let mut inserted = 0;
    let complete = write_payload(p, cursor_left, |inputs| {
        let count = send_ordered_with(inputs, &mut submit);
        inserted += count;
        count == inputs.len()
    });
    Delivery::from_counts(complete, inserted)
}

fn write_payload(
    p: ReplacePayload,
    cursor_left: usize,
    mut send: impl FnMut(&mut [INPUT]) -> bool,
) -> bool {
    let scan_mode = p.standard_key_send;
    let step_delay = effective_step_delay(&p);
    let restore_before_trailing = matches!(p.trailing_vk, Some(VK_RETURN | VK_TAB));
    let cycle_mods = should_cycle_modifiers(&p, step_delay, restore_before_trailing);
    let held_mods = if cycle_mods {
        capture_held_modifiers(MODIFIER_VKS)
    } else {
        Vec::new()
    };
    let mut pre = Vec::with_capacity(p.backspaces * 2 + held_mods.len() + 6);

    if p.backspaces > 0 || !p.text.is_empty() {
        for &(vk, extended) in &held_mods {
            push_vk(&mut pre, vk, false, extended, scan_mode);
        }
    }

    if p.backspaces > 0 {
        if p.browser_suggest_1 {
            push_unicode_char(&mut pre, '\u{200B}');
            push_vk_tap(&mut pre, VK_BACK, false, scan_mode);
        }
        if p.browser_suggest_2 {
            push_vk_tap(&mut pre, VK_RIGHT, true, scan_mode);
        }
        for _ in 0..p.backspaces {
            push_vk_tap(&mut pre, VK_BACK, false, scan_mode);
        }
    }

    let mut post = Vec::with_capacity(p.text.len() * 2 + held_mods.len() + 4);
    let mut did_clip_paste = false;
    if !p.text.is_empty()
        && p.use_clipboard
        && write_clipboard_for_paste_with_restore(&p.text)
    {
        if !send(&mut pre) {
            return false;
        }
        pre.clear();
        sleep_precise_ms(step_delay.clamp(6, 12));
        push_paste_combo(&mut pre, p.paste_via_shift_insert, scan_mode);
        if !send(&mut pre) {
            return false;
        }
        pre.clear();
        if p.trailing_vk.is_some() || !held_mods.is_empty() {
            sleep_precise_ms(8);
        }
        did_clip_paste = true;
    }

    if !did_clip_paste && !p.text.is_empty() {
        for unit in p.text.encode_utf16() {
            push_unicode_u16(&mut post, unit);
        }
    }

    if (p.backspaces > 0 || !p.text.is_empty()) && restore_before_trailing {
        for &(vk, extended) in &held_mods {
            push_vk(&mut post, vk, true, extended, scan_mode);
        }
    }
    if let Some(vk) = p.trailing_vk {
        push_vk_tap(&mut post, vk, false, scan_mode);
    }
    if (p.backspaces > 0 || !p.text.is_empty()) && !restore_before_trailing {
        for &(vk, extended) in &held_mods {
            push_vk(&mut post, vk, true, extended, scan_mode);
        }
    }

    pre.extend_from_slice(&post);
    for _ in 0..cursor_left {
        push_vk_tap(&mut pre, VK_LEFT, true, false);
    }
    send(&mut pre)
}

fn sleep_precise_ms(ms: u32) {
    if ms == 0 {
        return;
    }
    unsafe {
        timeBeginPeriod(1);
    }
    std::thread::sleep(Duration::from_millis(ms as u64));
    unsafe {
        timeEndPeriod(1);
    }
}

pub fn forward_typing_key(vk: u16, literal: Option<char>) -> Delivery {
    forward_typing_key_with(vk, literal, send_slice)
}

pub(super) fn forward_typing_key_with(
    vk: u16,
    literal: Option<char>,
    submit: impl FnMut(&[INPUT]) -> usize,
) -> Delivery {
    let mut inputs = Vec::with_capacity(2);
    if let Some(ch) = literal {
        push_unicode_char(&mut inputs, ch);
    } else {
        push_vk_tap(&mut inputs, vk, false, false);
    }
    let count = send_ordered_with(&mut inputs, submit);
    Delivery::from_counts(count == inputs.len(), count)
}

pub fn send_capslock_tap() {
    let mut inputs = Vec::with_capacity(2);
    push_vk_tap(&mut inputs, VK_CAPITAL, false, false);
    send_slice(&inputs);
}

pub fn send_ctrl_key(vk: u16) {
    let held_mods = capture_held_modifiers(ALL_MODIFIER_VKS);
    let mut inputs = Vec::with_capacity(held_mods.len() * 2 + 4);
    for &(m_vk, extended) in &held_mods {
        push_vk(&mut inputs, m_vk, false, extended, false);
    }
    push_vk(&mut inputs, VK_LCONTROL, true, false, false);
    push_vk_tap(&mut inputs, vk, false, false);
    push_vk(&mut inputs, VK_LCONTROL, false, false, false);
    for &(m_vk, extended) in &held_mods {
        push_vk(&mut inputs, m_vk, true, extended, false);
    }
    send_slice(&inputs);
}

fn effective_step_delay(p: &ReplacePayload) -> u32 {
    if p.compatible_hook {
        p.send_delay_ms.clamp(1, 10)
    } else if p.send_delay_ms > 2 {
        p.send_delay_ms.min(10)
    } else {
        0
    }
}

fn push_paste_combo(paste: &mut Vec<INPUT>, shift_insert: bool, scan_mode: bool) {
    if shift_insert {
        push_vk(paste, VK_LSHIFT, true, false, scan_mode);
        push_vk_tap(paste, VK_INSERT, true, scan_mode);
        push_vk(paste, VK_LSHIFT, false, false, scan_mode);
    } else {
        push_vk(paste, VK_LCONTROL, true, false, scan_mode);
        push_vk_tap(paste, b'V' as u16, false, scan_mode);
        push_vk(paste, VK_LCONTROL, false, false, scan_mode);
    }
}

fn send_ordered_with(inputs: &mut [INPUT], mut submit: impl FnMut(&[INPUT]) -> usize) -> usize {
    let Some(last) = inputs.last_mut() else {
        return 0;
    };
    let marker = NEXT_BATCH.with(|next| {
        next.set(next.get().wrapping_add(1));
        BATCH_MARKER | usize::from(next.get())
    });
    last.Anonymous.ki.dwExtraInfo = marker;
    PENDING_BATCHES.with_borrow_mut(|batches| {
        batches.retain(|(_, start)| start.elapsed() < MAX_BATCH_AGE);
        batches.push((marker, Instant::now()));
    });
    let mut inserted = 0;
    while inserted < inputs.len() {
        let count = submit(&inputs[inserted..]);
        if count == 0 {
            acknowledge_output(marker);
            break;
        }
        inserted += count;
    }
    inserted
}

fn send_slice(inputs: &[INPUT]) -> usize {
    if inputs.is_empty() {
        return 0;
    }
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        ) as usize
    }
}

fn push_vk(inputs: &mut Vec<INPUT>, vk: u16, down: bool, extended: bool, _scan_mode: bool) {
    let mut flags = if down { 0 } else { KEYEVENTF_KEYUP };
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u16 };
    inputs.push(INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MSKEY_EXTRA_INFO,
            },
        },
    });
}

fn push_vk_tap(inputs: &mut Vec<INPUT>, vk: u16, extended: bool, scan_mode: bool) {
    push_vk(inputs, vk, true, extended, scan_mode);
    push_vk(inputs, vk, false, extended, scan_mode);
}

fn push_unicode_char(inputs: &mut Vec<INPUT>, ch: char) {
    let mut buf = [0u16; 2];
    for &u in ch.encode_utf16(&mut buf).iter() {
        push_unicode_u16(inputs, u);
    }
}

fn push_unicode_u16(inputs: &mut Vec<INPUT>, unit: u16) {
    for flags in [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP] {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: 0,
                    wScan: unit,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: MSKEY_EXTRA_INFO,
                },
            },
        });
    }
}

pub fn is_browser_process(process_name: &str) -> bool {
    matches!(
        process_name.to_ascii_lowercase().as_str(),
        "chrome.exe"
            | "msedge.exe"
            | "brave.exe"
            | "opera.exe"
            | "vivaldi.exe"
            | "coccoc.exe"
            | "browser.exe"
            | "arc.exe"
            | "firefox.exe"
            | "waterfox.exe"
            | "floorp.exe"
            | "zen.exe"
    )
}

pub fn is_terminal_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "CASCADIA_HOSTING_WINDOW_CLASS" | "ConsoleWindowClass" | "mintty"
    ) || class_name.contains("Alacritty")
        || class_name.contains("wezterm")
}

pub fn build_replace_payload(
    act: EngineAction,
    trailing_vk: Option<u16>,
    cfg: &Config,
    fg_class: &str,
    fg_process: &str,
) -> Option<ReplacePayload> {
    match act {
        EngineAction::PassThrough => None,
        EngineAction::Replace { backspaces, mut text } => {
            let mut send_trailing_vk = None;
            if let Some(vk) = trailing_vk {
                let delim_char = match vk {
                    0x0D => '\n',
                    0x09 => '\t',
                    _ => ' ',
                };
                if text.ends_with(delim_char) {
                    text.pop();
                    send_trailing_vk = Some(vk);
                }
            }
            let browser_omnibox = is_browser_process(fg_process) && crate::uia::is_browser_omnibox();
            let terminal = is_terminal_class(fg_class);
            Some(ReplacePayload {
                backspaces,
                text,
                trailing_vk: send_trailing_vk,
                browser_suggest_1: browser_omnibox && cfg.fix_browser_suggest_1,
                browser_suggest_2: browser_omnibox && cfg.fix_browser_suggest_2,
                use_clipboard: cfg.use_clipboard_send,
                standard_key_send: cfg.standard_key_send || terminal,
                compatible_hook: cfg.mechanism == Mechanism::CompatibleHook || terminal,
                paste_via_shift_insert: cfg.paste_via_shift_insert || terminal,
                send_delay_ms: cfg.send_delay_ms as u32,
            })
        }
    }
}

pub fn convert_clipboard_now(target: Charset) {
    if let Some(src) = crate::platform::read_clipboard_text() {
        let from = detect_convert_encoding(&src);
        let target_enc = ConvertEncoding::from(target);
        let to = if from == target_enc {
            ConvertEncoding::Unicode
        } else {
            target_enc
        };
        let converted = convert_with_encoding(&src, from, to, CaseTransform::None);
        write_clipboard_text(&converted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_delivery_retries_only_the_unsent_suffix_and_releases_its_marker() {
        assert_eq!(
            forward_typing_key_with(0x42, Some('b'), |_| 0),
            Delivery::Rejected
        );
        assert!(!output_pending());

        let mut lengths = Vec::new();
        let result = forward_typing_key_with(0x42, Some('b'), |events| {
            lengths.push(events.len());
            usize::from(events.len() == 2)
        });
        assert_eq!(lengths, [2, 1]);
        assert_eq!(result, Delivery::Partial);
        assert!(!output_pending());

        let result = forward_typing_key_with(0x42, Some('b'), |events| {
            if events.len() == 1 {
                acknowledge_output(unsafe { events[0].Anonymous.ki.dwExtraInfo });
            }
            1
        });
        assert_eq!(result, Delivery::Complete);
        assert!(!output_pending());
    }

    #[test]
    fn browser_process_excludes_electron_ides_and_terminals() {
        assert!(is_browser_process("chrome.exe"));
        assert!(is_browser_process("msedge.exe"));
        assert!(is_browser_process("firefox.exe"));
        assert!(!is_browser_process("Antigravity.exe"));
        assert!(!is_browser_process("Code.exe"));
        assert!(!is_browser_process("Claude.exe"));
        assert!(!is_browser_process("WindowsTerminal.exe"));
    }

    #[test]
    fn non_browser_chrome_widget_never_triggers_suggest_zero_width_space() {
        let cfg = Config::default();
        let payload = build_replace_payload(
            EngineAction::Replace {
                backspaces: 2,
                text: "ào".to_string(),
            },
            None,
            &cfg,
            "Chrome_WidgetWin_1",
            "Antigravity.exe",
        )
        .expect("payload");
        assert!(!payload.browser_suggest_1);
        assert!(!payload.browser_suggest_2);
    }

    #[test]
    fn effective_delay_keeps_keyboard_hook_atomic_by_default() {
        let cfg = Config::default();
        let kb = build_replace_payload(
            EngineAction::Replace {
                backspaces: 2,
                text: "ào".to_string(),
            },
            None,
            &cfg,
            "Notepad",
            "notepad.exe",
        )
        .expect("payload");
        assert_eq!(effective_step_delay(&kb), 0);

        let mut compat_cfg = cfg;
        compat_cfg.mechanism = Mechanism::CompatibleHook;
        let compat = build_replace_payload(
            EngineAction::Replace {
                backspaces: 2,
                text: "ào".to_string(),
            },
            None,
            &compat_cfg,
            "Notepad",
            "notepad.exe",
        )
        .expect("payload");
        assert_eq!(effective_step_delay(&compat), 2);
    }

    #[test]
    fn compatible_hook_skips_modifier_cycling_when_no_special_path_is_used() {
        let compat_cfg = Config {
            mechanism: Mechanism::CompatibleHook,
            ..Config::default()
        };
        let compat = build_replace_payload(
            EngineAction::Replace {
                backspaces: 2,
                text: "ào".to_string(),
            },
            None,
            &compat_cfg,
            "Notepad",
            "notepad.exe",
        )
        .expect("payload");
        assert!(!should_cycle_modifiers(
            &compat,
            effective_step_delay(&compat),
            false,
        ));
        assert!(should_cycle_modifiers(
            &compat,
            effective_step_delay(&compat),
            true,
        ));

        let kb_cfg = Config::default();
        let kb = build_replace_payload(
            EngineAction::Replace {
                backspaces: 2,
                text: "ào".to_string(),
            },
            None,
            &kb_cfg,
            "Notepad",
            "notepad.exe",
        )
        .expect("payload");
        assert!(should_cycle_modifiers(&kb, effective_step_delay(&kb), false));
    }
}
