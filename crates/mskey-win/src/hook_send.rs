use std::time::Duration;

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

pub fn dispatch_payload(p: ReplacePayload) {
    let held_mods = capture_held_modifiers(MODIFIER_VKS);
    let scan_mode = p.standard_key_send;
    let step_delay = effective_step_delay(&p);
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
        send_slice(&pre);
        pre.clear();
        sleep_precise_ms(step_delay.clamp(6, 12));
        send_paste_combo(p.paste_via_shift_insert, scan_mode);
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

    let restore_before_trailing = matches!(p.trailing_vk, Some(VK_RETURN | VK_TAB));
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

    if step_delay == 0 || pre.is_empty() || post.is_empty() {
        pre.extend_from_slice(&post);
        send_slice(&pre);
    } else {
        send_slice(&pre);
        sleep_precise_ms(step_delay.min(5));
        send_slice(&post);
    }
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

pub fn send_cursor_left(steps: usize) {
    if steps == 0 {
        return;
    }
    let mut inputs = Vec::with_capacity(steps * 2);
    for _ in 0..steps {
        push_vk_tap(&mut inputs, VK_LEFT, true, false);
    }
    send_slice(&inputs);
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

fn send_paste_combo(shift_insert: bool, scan_mode: bool) {
    let mut paste = Vec::with_capacity(4);
    if shift_insert {
        push_vk(&mut paste, VK_LSHIFT, true, false, scan_mode);
        push_vk_tap(&mut paste, VK_INSERT, true, scan_mode);
        push_vk(&mut paste, VK_LSHIFT, false, false, scan_mode);
    } else {
        push_vk(&mut paste, VK_LCONTROL, true, false, scan_mode);
        push_vk_tap(&mut paste, b'V' as u16, false, scan_mode);
        push_vk(&mut paste, VK_LCONTROL, false, false, scan_mode);
    }
    send_slice(&paste);
}

fn send_slice(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
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
}
