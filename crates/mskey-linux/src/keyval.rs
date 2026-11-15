use std::time::Instant;

use mskey_core::{HoldEnglishKey, HotkeyCombo, SwitchPreset};

pub const IBUS_SHIFT_MASK: u32 = 1 << 0;
pub const IBUS_LOCK_MASK: u32 = 1 << 1;
pub const IBUS_CONTROL_MASK: u32 = 1 << 2;
pub const IBUS_MOD1_MASK: u32 = 1 << 3;
pub const IBUS_MOD4_MASK: u32 = 1 << 26;
pub const IBUS_RELEASE_MASK: u32 = 1 << 30;

pub const KEY_BACKSPACE: u32 = 0xff08;
pub const KEY_TAB: u32 = 0xff09;
pub const KEY_RETURN: u32 = 0xff0d;
pub const KEY_ESCAPE: u32 = 0xff1b;
pub const KEY_DELETE: u32 = 0xffff;
pub const KEY_ISO_LEFT_TAB: u32 = 0xfe20;
pub const KEY_KP_SPACE: u32 = 0xff80;
pub const KEY_KP_TAB: u32 = 0xff89;
pub const KEY_KP_ENTER: u32 = 0xff8d;

pub const KEY_HOME: u32 = 0xff50;
pub const KEY_LEFT: u32 = 0xff51;
pub const KEY_UP: u32 = 0xff52;
pub const KEY_RIGHT: u32 = 0xff53;
pub const KEY_DOWN: u32 = 0xff54;
pub const KEY_PAGE_UP: u32 = 0xff55;
pub const KEY_PAGE_DOWN: u32 = 0xff56;
pub const KEY_END: u32 = 0xff57;
pub const KEY_BEGIN: u32 = 0xff58;

pub const KEY_SHIFT_L: u32 = 0xffe1;
pub const KEY_SHIFT_R: u32 = 0xffe2;
pub const KEY_CONTROL_L: u32 = 0xffe3;
pub const KEY_CONTROL_R: u32 = 0xffe4;
pub const KEY_CAPS_LOCK: u32 = 0xffe5;
pub const KEY_META_L: u32 = 0xffe7;
pub const KEY_META_R: u32 = 0xffe8;
pub const KEY_ALT_L: u32 = 0xffe9;
pub const KEY_ALT_R: u32 = 0xffea;
pub const KEY_SUPER_L: u32 = 0xffeb;
pub const KEY_SUPER_R: u32 = 0xffec;
pub const KEY_ISO_NEXT_GROUP: u32 = 0xfe08;
pub const KEY_ISO_PREV_GROUP: u32 = 0xfe0c;

pub fn is_modifier_keyval(kv: u32) -> bool {
    matches!(
        kv,
        KEY_SHIFT_L
            | KEY_SHIFT_R
            | KEY_CONTROL_L
            | KEY_CONTROL_R
            | KEY_CAPS_LOCK
            | KEY_META_L
            | KEY_META_R
            | KEY_ALT_L
            | KEY_ALT_R
            | KEY_SUPER_L
            | KEY_SUPER_R
            | KEY_ISO_NEXT_GROUP
            | KEY_ISO_PREV_GROUP
    )
}

pub fn is_navigation_keyval(kv: u32) -> bool {
    matches!(
        kv,
        KEY_HOME..=KEY_BEGIN
            | KEY_RETURN
            | KEY_KP_ENTER
            | KEY_TAB
            | KEY_KP_TAB
            | KEY_ISO_LEFT_TAB
            | KEY_DELETE
    )
}

pub fn keyval_to_char(kv: u32) -> Option<char> {
    if (0x0020..=0x007e).contains(&kv) {
        return char::from_u32(kv);
    }
    match kv {
        KEY_KP_SPACE => Some(' '),
        0xffaa => Some('*'),
        0xffab => Some('+'),
        0xffac => Some(','),
        0xffad => Some('-'),
        0xffae => Some('.'),
        0xffaf => Some('/'),
        0xffbd => Some('='),
        0xffb0..=0xffb9 => char::from_u32(b'0' as u32 + (kv - 0xffb0)),
        _ => None,
    }
}

const PUNCT_VK_MAP: &[(u32, u8, u8)] = &[
    (0xBA, b';', b':'),
    (0xBB, b'=', b'+'),
    (0xBC, b',', b'<'),
    (0xBD, b'-', b'_'),
    (0xBE, b'.', b'>'),
    (0xBF, b'/', b'?'),
    (0xC0, b'`', b'~'),
    (0xDB, b'[', b'{'),
    (0xDC, b'\\', b'|'),
    (0xDD, b']', b'}'),
    (0xDE, b'\'', b'"'),
];

pub fn gdk_keyval_matches_vk(vk: u32, keyval: u32) -> bool {
    match vk {
        0x30..=0x39 => keyval == vk,
        0x41..=0x5A => {
            let lower = vk + 32;
            keyval == vk || keyval == lower
        }
        0x70..=0x7B => keyval == 0xffbe + (vk - 0x70),
        0x20 => keyval == 0x0020 || keyval == KEY_KP_SPACE,
        0x0D => keyval == KEY_RETURN || keyval == KEY_KP_ENTER,
        0x09 => keyval == KEY_TAB || keyval == KEY_KP_TAB || keyval == KEY_ISO_LEFT_TAB,
        _ => {
            if let Some(&(_, unshifted, shifted)) = PUNCT_VK_MAP.iter().find(|&&(v, _, _)| v == vk) {
                keyval == unshifted as u32 || keyval == shifted as u32
            } else {
                keyval == vk
            }
        }
    }
}

pub fn gdk_keyval_to_vk(keyval: u32) -> u32 {
    match keyval {
        0x0061..=0x007a => keyval - 32,
        0x0041..=0x005a => keyval,
        0x0030..=0x0039 => keyval,
        0xffbe..=0xffc9 => 0x70 + (keyval - 0xffbe),
        0x0020 | KEY_KP_SPACE => 0x20,
        KEY_RETURN | KEY_KP_ENTER => 0x0D,
        KEY_TAB | KEY_KP_TAB | KEY_ISO_LEFT_TAB => 0x09,
        _ => {
            if let Some(&(vk, _, _)) = PUNCT_VK_MAP
                .iter()
                .find(|&&(_, u, s)| keyval == u as u32 || keyval == s as u32)
            {
                vk
            } else {
                keyval
            }
        }
    }
}

pub fn hold_english_keyval(mode: HoldEnglishKey) -> Option<u32> {
    match mode {
        HoldEnglishKey::None => None,
        HoldEnglishKey::RightShift => Some(KEY_SHIFT_R),
        HoldEnglishKey::LeftShift => Some(KEY_SHIFT_L),
        HoldEnglishKey::RightAlt => Some(KEY_ALT_R),
        HoldEnglishKey::RightCtrl => Some(KEY_CONTROL_R),
        HoldEnglishKey::CapsLock => Some(KEY_CAPS_LOCK),
    }
}

pub fn format_hotkey_linux(combo: HotkeyCombo) -> String {
    if combo.key_code == 0 {
        return "Không đặt".to_string();
    }
    let mut parts = Vec::new();
    if combo.ctrl {
        parts.push("Ctrl".to_string());
    }
    if combo.shift {
        parts.push("Shift".to_string());
    }
    if combo.alt {
        parts.push("Alt".to_string());
    }
    if combo.win_cmd {
        parts.push("Super".to_string());
    }
    let key = match combo.key_code {
        0x70..=0x7B => format!("F{}", combo.key_code - 0x6F),
        0x30..=0x5A => char::from_u32(combo.key_code).map(|c| c.to_string()).unwrap_or_default(),
        0x20 => "Space".to_string(),
        0x0D => "Enter".to_string(),
        0x09 => "Tab".to_string(),
        other => {
            if let Some(&(_, unshifted, _)) = PUNCT_VK_MAP.iter().find(|&&(v, _, _)| v == other) {
                (unshifted as char).to_string()
            } else {
                format!("0x{other:X}")
            }
        }
    };
    parts.push(key);
    parts.join(" + ")
}

pub fn matches_combo(combo: HotkeyCombo, keyval: u32, state: u32) -> bool {
    if combo.key_code == 0 {
        return false;
    }
    let ctrl = (state & IBUS_CONTROL_MASK) != 0;
    let shift = (state & IBUS_SHIFT_MASK) != 0;
    let alt = (state & IBUS_MOD1_MASK) != 0;
    let super_held = (state & IBUS_MOD4_MASK) != 0;
    ctrl == combo.ctrl
        && alt == combo.alt
        && shift == combo.shift
        && super_held == combo.win_cmd
        && gdk_keyval_matches_vk(combo.key_code, keyval)
}

#[derive(Debug, Default, Clone)]
pub struct HotkeyTracker {
    ctrl_shift_primed: bool,
    shift_press_at: Option<Instant>,
    last_shift_tap_at: Option<Instant>,
    shift_tainted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeySignal {
    None,
    ToggleVe,
    ConvertClipboard,
    ConvertSelection,
    UndoLastWord,
}

impl HotkeyTracker {
    pub fn process_key(
        &mut self,
        preset: SwitchPreset,
        custom_switch: HotkeyCombo,
        convert_hotkey: HotkeyCombo,
        keyval: u32,
        state: u32,
    ) -> HotkeySignal {
        let disabled = HotkeyCombo {
            ctrl: false,
            alt: false,
            shift: false,
            win_cmd: false,
            key_code: 0,
        };
        self.process_key_full(
            preset,
            custom_switch,
            convert_hotkey,
            disabled,
            disabled,
            false,
            HoldEnglishKey::None,
            keyval,
            state,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process_key_full(
        &mut self,
        preset: SwitchPreset,
        custom_switch: HotkeyCombo,
        convert_hotkey: HotkeyCombo,
        convert_selection_hotkey: HotkeyCombo,
        undo_hotkey: HotkeyCombo,
        quick_capslock_switch: bool,
        hold_english_key: HoldEnglishKey,
        keyval: u32,
        state: u32,
    ) -> HotkeySignal {
        let is_release = (state & IBUS_RELEASE_MASK) != 0;
        if is_modifier_keyval(keyval) {
            if quick_capslock_switch
                && !is_release
                && keyval == KEY_CAPS_LOCK
                && hold_english_key != HoldEnglishKey::CapsLock
                && (state & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_MOD4_MASK | IBUS_SHIFT_MASK)) == 0
            {
                self.reset();
                return HotkeySignal::ToggleVe;
            }
            return self.handle_modifier(preset, keyval, state, is_release);
        }

        self.ctrl_shift_primed = false;
        self.shift_press_at = None;
        self.last_shift_tap_at = None;
        self.shift_tainted = !is_release;
        if is_release {
            return HotkeySignal::None;
        }
        if matches_combo(undo_hotkey, keyval, state) {
            return HotkeySignal::UndoLastWord;
        }
        if matches_combo(convert_hotkey, keyval, state) {
            return HotkeySignal::ConvertClipboard;
        }
        if matches_combo(convert_selection_hotkey, keyval, state) {
            return HotkeySignal::ConvertSelection;
        }
        match preset {
            SwitchPreset::AltZ => {
                let alt = (state & IBUS_MOD1_MASK) != 0;
                let ctrl = (state & IBUS_CONTROL_MASK) != 0;
                if alt && !ctrl && matches!(keyval, 0x5A | 0x7A) {
                    return HotkeySignal::ToggleVe;
                }
            }
            SwitchPreset::CtrlSpace => {
                let ctrl = (state & IBUS_CONTROL_MASK) != 0;
                let alt = (state & IBUS_MOD1_MASK) != 0;
                let shift = (state & IBUS_SHIFT_MASK) != 0;
                let super_held = (state & IBUS_MOD4_MASK) != 0;
                if ctrl && !alt && !shift && !super_held && (keyval == 0x0020 || keyval == KEY_KP_SPACE) {
                    return HotkeySignal::ToggleVe;
                }
            }
            SwitchPreset::Custom => {
                if matches_combo(custom_switch, keyval, state) {
                    return HotkeySignal::ToggleVe;
                }
            }
            SwitchPreset::CtrlShift | SwitchPreset::ShiftShift => {}
        }
        HotkeySignal::None
    }

    fn handle_modifier(
        &mut self,
        preset: SwitchPreset,
        keyval: u32,
        state: u32,
        is_release: bool,
    ) -> HotkeySignal {
        let is_ctrl = matches!(keyval, KEY_CONTROL_L | KEY_CONTROL_R);
        let is_shift = matches!(keyval, KEY_SHIFT_L | KEY_SHIFT_R);
        let is_group_shift = matches!(keyval, KEY_ISO_NEXT_GROUP | KEY_ISO_PREV_GROUP);
        let other_mods = (state & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_MOD4_MASK)) != 0;

        if preset == SwitchPreset::CtrlShift {
            let ctrl_held = (state & IBUS_CONTROL_MASK) != 0 || (!is_release && is_ctrl);
            let shift_held = (state & IBUS_SHIFT_MASK) != 0 || (!is_release && (is_shift || is_group_shift));
            if !is_release && ctrl_held && shift_held {
                self.ctrl_shift_primed = true;
            } else if is_release && (is_ctrl || is_shift || is_group_shift) && self.ctrl_shift_primed {
                self.ctrl_shift_primed = false;
                return HotkeySignal::ToggleVe;
            } else if !ctrl_held && !shift_held {
                self.ctrl_shift_primed = false;
            }
        }

        if preset == SwitchPreset::ShiftShift {
            if is_shift {
                let now = Instant::now();
                if !is_release {
                    if other_mods {
                        self.shift_tainted = true;
                        self.shift_press_at = None;
                        self.last_shift_tap_at = None;
                    } else if self.shift_press_at.is_none() {
                        self.shift_press_at = Some(now);
                        self.shift_tainted = false;
                    }
                } else {
                    let valid = !self.shift_tainted
                        && !other_mods
                        && self
                            .shift_press_at
                            .take()
                            .is_some_and(|t| now.saturating_duration_since(t).as_millis() <= 350);
                    self.shift_tainted = false;
                    if valid {
                        if self
                            .last_shift_tap_at
                            .is_some_and(|prev| now.saturating_duration_since(prev).as_millis() <= 400)
                        {
                            self.last_shift_tap_at = None;
                            return HotkeySignal::ToggleVe;
                        }
                        self.last_shift_tap_at = Some(now);
                    } else {
                        self.last_shift_tap_at = None;
                    }
                }
            } else {
                self.shift_press_at = None;
                self.last_shift_tap_at = None;
                self.shift_tainted = !is_release;
            }
        }

        HotkeySignal::None
    }

    pub fn reset(&mut self) {
        self.ctrl_shift_primed = false;
        self.shift_press_at = None;
        self.last_shift_tap_at = None;
        self.shift_tainted = false;
    }
}
