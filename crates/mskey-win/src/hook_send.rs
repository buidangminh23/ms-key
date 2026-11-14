use mskey_core::{CaseTransform, Charset, convert_text};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput, VK_BACK, VK_LCONTROL, VK_LSHIFT, VK_RIGHT,
    VK_RSHIFT,
};

use crate::platform::{read_clipboard_text, write_clipboard_text};

pub const MSKEY_EXTRA_INFO: usize = 0x4D53_4B59;

pub struct ReplacePayload {
    pub backspaces: usize,
    pub text: String,
    pub trailing_vk: Option<u16>,
    pub browser_suggest_1: bool,
    pub browser_suggest_2: bool,
    pub use_clipboard: bool,
}

pub fn dispatch_payload(p: ReplacePayload) {
    let lshift = unsafe { GetAsyncKeyState(VK_LSHIFT as i32) } < 0;
    let rshift = unsafe { GetAsyncKeyState(VK_RSHIFT as i32) } < 0;
    let mut inputs: Vec<INPUT> = Vec::with_capacity(p.backspaces * 2 + p.text.len() * 2 + 8);

    if lshift {
        push_vk(&mut inputs, VK_LSHIFT, false, false);
    }
    if rshift {
        push_vk(&mut inputs, VK_RSHIFT, false, false);
    }

    if p.backspaces > 0 {
        if p.browser_suggest_1 {
            push_unicode_char(&mut inputs, '\u{200B}');
            push_vk_tap(&mut inputs, VK_BACK, false);
        } else if p.browser_suggest_2 {
            push_vk_tap(&mut inputs, VK_RIGHT, true);
        }
        for _ in 0..p.backspaces {
            push_vk_tap(&mut inputs, VK_BACK, false);
        }
    }

    if !p.text.is_empty() {
        if p.use_clipboard && p.trailing_vk.is_none() {
            if write_clipboard_text(&p.text) {
                send_slice(&inputs);
                inputs.clear();
                let mut paste = Vec::with_capacity(4);
                push_vk(&mut paste, VK_LCONTROL, true, false);
                push_vk_tap(&mut paste, b'V' as u16, false);
                push_vk(&mut paste, VK_LCONTROL, false, false);
                send_slice(&paste);
            }
        } else {
            for unit in p.text.encode_utf16() {
                push_unicode_u16(&mut inputs, unit);
            }
        }
    }

    if let Some(vk) = p.trailing_vk {
        push_vk_tap(&mut inputs, vk, false);
    }

    if lshift {
        push_vk(&mut inputs, VK_LSHIFT, true, false);
    }
    if rshift {
        push_vk(&mut inputs, VK_RSHIFT, true, false);
    }
    send_slice(&inputs);
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

fn push_vk(inputs: &mut Vec<INPUT>, vk: u16, down: bool, extended: bool) {
    let mut flags = if down { 0 } else { KEYEVENTF_KEYUP };
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    inputs.push(INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MSKEY_EXTRA_INFO,
            },
        },
    });
}

fn push_vk_tap(inputs: &mut Vec<INPUT>, vk: u16, extended: bool) {
    push_vk(inputs, vk, true, extended);
    push_vk(inputs, vk, false, extended);
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

pub fn convert_clipboard_now(target: Charset) {
    if let Some(src) = read_clipboard_text() {
        let from = detect_charset_guess(&src);
        let to = if from == target { Charset::Unicode } else { target };
        let converted = convert_text(&src, from, to, CaseTransform::None);
        write_clipboard_text(&converted);
    }
}

fn detect_charset_guess(text: &str) -> Charset {
    if text.contains("aù") || text.contains("aø") || text.contains("oû") {
        return Charset::VniWindows;
    }
    for ch in text.chars() {
        if matches!(ch as u32, 0x0300 | 0x0301 | 0x0309 | 0x0303 | 0x0323) {
            return Charset::UnicodeComposite;
        }
    }
    Charset::Unicode
}
