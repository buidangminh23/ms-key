use std::time::Duration;

use mskey_core::{
    CaseTransform, Charset, Config, ConvertEncoding, EngineAction, Mechanism, convert_with_encoding,
    detect_convert_encoding,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput, VK_BACK,
    VK_INSERT, VK_LCONTROL, VK_LSHIFT, VK_RIGHT, VK_RSHIFT,
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

pub fn dispatch_payload(p: ReplacePayload) {
    let lshift = unsafe { GetAsyncKeyState(VK_LSHIFT as i32) } < 0;
    let rshift = unsafe { GetAsyncKeyState(VK_RSHIFT as i32) } < 0;
    let scan_mode = p.standard_key_send;
    let step_delay = effective_step_delay(&p);
    let mut pre = Vec::with_capacity(p.backspaces * 2 + 12);

    if lshift {
        push_vk(&mut pre, VK_LSHIFT, false, false, scan_mode);
    }
    if rshift {
        push_vk(&mut pre, VK_RSHIFT, false, false, scan_mode);
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

    let mut did_clip_paste = false;
    if !p.text.is_empty() && p.use_clipboard {
        if write_clipboard_for_paste_with_restore(&p.text) {
            send_paced(&pre, step_delay);
            pre.clear();
            if step_delay > 0 {
                std::thread::sleep(Duration::from_millis(6));
            }
            send_paste_combo(p.paste_via_shift_insert, scan_mode);
            if p.trailing_vk.is_some() || lshift || rshift {
                std::thread::sleep(Duration::from_millis(8));
            }
            did_clip_paste = true;
        }
    }

    if !did_clip_paste && !p.text.is_empty() {
        for unit in p.text.encode_utf16() {
            push_unicode_u16(&mut pre, unit);
        }
    }

    if let Some(vk) = p.trailing_vk {
        push_vk_tap(&mut pre, vk, false, scan_mode);
    }
    if lshift {
        push_vk(&mut pre, VK_LSHIFT, true, false, scan_mode);
    }
    if rshift {
        push_vk(&mut pre, VK_RSHIFT, true, false, scan_mode);
    }
    send_paced(&pre, step_delay);
}

fn effective_step_delay(p: &ReplacePayload) -> u32 {
    if p.send_delay_ms > 0 {
        p.send_delay_ms.min(25)
    } else if p.compatible_hook {
        1
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

fn send_paced(inputs: &[INPUT], step_delay_ms: u32) {
    if inputs.is_empty() {
        return;
    }
    if step_delay_ms == 0 || inputs.len() <= 2 {
        send_slice(inputs);
        return;
    }
    let chunks = inputs.len().div_ceil(2) as u32;
    let gap = step_delay_ms.min((120 / chunks.max(1)).max(1));
    for pair in inputs.chunks(2) {
        send_slice(pair);
        std::thread::sleep(Duration::from_millis(gap as u64));
    }
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

fn push_vk(inputs: &mut Vec<INPUT>, vk: u16, down: bool, extended: bool, scan_mode: bool) {
    let mut flags = if down { 0 } else { KEYEVENTF_KEYUP };
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    let scan = if scan_mode {
        unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u16 }
    } else {
        0
    };
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

pub fn is_browser_class(class_name: &str) -> bool {
    class_name.starts_with("Chrome_WidgetWin")
        || class_name == "MozillaWindowClass"
        || class_name == "ApplicationFrameWindow"
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
            let browser = is_browser_class(fg_class);
            let terminal = is_terminal_class(fg_class);
            Some(ReplacePayload {
                backspaces,
                text,
                trailing_vk: send_trailing_vk,
                browser_suggest_1: browser && cfg.fix_browser_suggest_1,
                browser_suggest_2: browser && cfg.fix_browser_suggest_2,
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
