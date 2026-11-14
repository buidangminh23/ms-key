use std::collections::HashMap;

use mskey_core::{AppBehavior, Config, Engine, EngineAction, Mechanism};

use crate::keyval::{
    HotkeySignal, HotkeyTracker, IBUS_CONTROL_MASK, IBUS_MOD1_MASK, IBUS_MOD4_MASK,
    IBUS_RELEASE_MASK, KEY_BACKSPACE, KEY_ESCAPE, is_modifier_keyval, is_navigation_keyval,
    keyval_to_char,
};

pub const IBUS_CAP_PREEDIT_TEXT: u32 = 1 << 0;
pub const IBUS_CAP_SURROUNDING_TEXT: u32 = 1 << 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IbusOp {
    UpdatePreedit(String),
    HidePreedit,
    CommitText(String),
    DeleteSurrounding(i32, u32),
    ForwardKey(u32, u32, u32),
    SyncProperties,
    TriggerClipboardConvert,
    Beep,
}

#[derive(Debug, Clone)]
pub struct IbusProcessResult {
    pub handled: bool,
    pub ops: Vec<IbusOp>,
}

pub struct IbusSession {
    pub engine: Engine,
    pub hotkeys: HotkeyTracker,
    pub user_preferred_ve: bool,
    pub active_app: String,
    pub app_ve_memory: HashMap<String, bool>,
    pub preedit: String,
    pub caps: u32,
}

impl IbusSession {
    pub fn new(config: Config) -> Self {
        let pref = config.vietnamese_enabled;
        Self {
            engine: Engine::new(config),
            hotkeys: HotkeyTracker::default(),
            user_preferred_ve: pref,
            active_app: String::new(),
            app_ve_memory: HashMap::new(),
            preedit: String::new(),
            caps: IBUS_CAP_PREEDIT_TEXT,
        }
    }

    pub fn update_config(&mut self, cfg: Config) {
        self.user_preferred_ve = cfg.vietnamese_enabled;
        self.engine.update_config(cfg);
        self.apply_app_policy();
    }

    pub fn set_user_vietnamese(&mut self, enabled: bool) {
        self.user_preferred_ve = enabled;
        if !self.active_app.is_empty() && self.engine.config().smart_app_switch {
            self.app_ve_memory.insert(self.active_app.clone(), enabled);
        }
        self.engine.set_vietnamese_enabled(enabled);
        self.preedit.clear();
    }

    pub fn set_capabilities(&mut self, caps: u32) {
        self.caps = caps;
    }

    pub fn focus_in(&mut self, app_name: &str) -> Vec<IbusOp> {
        let mut ops = Vec::new();
        self.flush_preedit(&mut ops);
        self.engine.reset_all();
        self.hotkeys.reset();
        let trimmed = app_name.trim().to_ascii_lowercase();
        if !trimmed.is_empty() {
            self.active_app = trimmed;
        }
        if self.apply_app_policy() {
            ops.push(IbusOp::SyncProperties);
        }
        ops
    }

    pub fn reset(&mut self) -> Vec<IbusOp> {
        let mut ops = Vec::new();
        self.flush_preedit(&mut ops);
        self.engine.reset_word();
        self.hotkeys.reset();
        ops
    }

    pub fn focus_out(&mut self) -> Vec<IbusOp> {
        let mut ops = Vec::new();
        self.flush_preedit(&mut ops);
        self.engine.reset_all();
        self.hotkeys.reset();
        ops
    }

    fn apply_app_policy(&mut self) -> bool {
        let cfg = self.engine.config().clone();
        let target = match cfg.behavior_for_app(&self.active_app) {
            AppBehavior::ForceEnglish | AppBehavior::Blacklist => false,
            AppBehavior::ForceVietnamese => true,
            AppBehavior::Default => {
                if cfg.smart_app_switch && !self.active_app.is_empty() {
                    *self.app_ve_memory.get(&self.active_app).unwrap_or(&self.user_preferred_ve)
                } else {
                    self.user_preferred_ve
                }
            }
        };
        if cfg.vietnamese_enabled != target {
            self.engine.set_vietnamese_enabled(target);
            self.preedit.clear();
            true
        } else {
            false
        }
    }

    fn flush_preedit(&mut self, ops: &mut Vec<IbusOp>) {
        if !self.preedit.is_empty() {
            let text = std::mem::take(&mut self.preedit);
            ops.push(IbusOp::HidePreedit);
            ops.push(IbusOp::CommitText(text));
        }
    }

    fn uses_preedit_mode(&self) -> bool {
        self.engine.config().mechanism == Mechanism::KeyboardHook
            && (self.caps & IBUS_CAP_PREEDIT_TEXT) != 0
    }

    pub fn process_key(&mut self, keyval: u32, keycode: u32, state: u32) -> IbusProcessResult {
        let cfg = self.engine.config();
        let sig = self.hotkeys.process_key(
            cfg.switch_preset,
            cfg.custom_switch_hotkey,
            cfg.clipboard_convert_hotkey,
            keyval,
            state,
        );
        match sig {
            HotkeySignal::ToggleVe => {
                let mut ops = Vec::new();
                self.flush_preedit(&mut ops);
                let next = !self.engine.config().vietnamese_enabled;
                self.set_user_vietnamese(next);
                ops.push(IbusOp::SyncProperties);
                if self.engine.config().beep_on_switch {
                    ops.push(IbusOp::Beep);
                }
                return IbusProcessResult { handled: true, ops };
            }
            HotkeySignal::ConvertClipboard => {
                return IbusProcessResult {
                    handled: true,
                    ops: vec![IbusOp::TriggerClipboardConvert],
                };
            }
            HotkeySignal::None => {}
        }

        if (state & IBUS_RELEASE_MASK) != 0 || is_modifier_keyval(keyval) {
            return IbusProcessResult { handled: false, ops: Vec::new() };
        }
        if !self.active_app.is_empty()
            && self.engine.config().behavior_for_app(&self.active_app) == AppBehavior::Blacklist
        {
            return IbusProcessResult { handled: false, ops: Vec::new() };
        }

        if (state & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_MOD4_MASK)) != 0 {
            let mut ops = Vec::new();
            self.flush_preedit(&mut ops);
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        }

        if self.uses_preedit_mode() {
            self.process_key_preedit(keyval)
        } else {
            self.process_key_direct(keyval, keycode, state)
        }
    }

    fn process_key_preedit(&mut self, keyval: u32) -> IbusProcessResult {
        let mut ops = Vec::new();
        if keyval == KEY_ESCAPE {
            if self.preedit.is_empty() {
                self.engine.reset_word();
                return IbusProcessResult { handled: false, ops };
            }
            let act = self.engine.process_escape();
            if let EngineAction::Replace { backspaces, text } = act {
                apply_replace_to_buf(&mut self.preedit, backspaces, &text);
                self.flush_preedit(&mut ops);
                return IbusProcessResult { handled: true, ops };
            }
            self.flush_preedit(&mut ops);
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        }

        if keyval == KEY_BACKSPACE {
            if self.preedit.is_empty() {
                self.engine.reset_word();
                return IbusProcessResult { handled: false, ops };
            }
            let _ = self.engine.process_backspace();
            self.preedit = self.engine.current_word();
            if self.preedit.is_empty() {
                ops.push(IbusOp::HidePreedit);
            } else {
                ops.push(IbusOp::UpdatePreedit(self.preedit.clone()));
            }
            return IbusProcessResult { handled: true, ops };
        }

        if is_navigation_keyval(keyval) {
            self.flush_preedit(&mut ops);
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        }

        let Some(ch) = keyval_to_char(keyval) else {
            self.flush_preedit(&mut ops);
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        };

        let cfg = self.engine.config();
        let active_typing = cfg.vietnamese_enabled || (cfg.macro_enabled && cfg.macro_when_english);
        if !active_typing {
            self.flush_preedit(&mut ops);
            return IbusProcessResult { handled: false, ops };
        }

        if Engine::is_word_delimiter(ch) {
            let had_preedit = !self.preedit.is_empty();
            let act = self.engine.process_key(ch);
            match act {
                EngineAction::Replace { backspaces, text } => {
                    apply_replace_to_buf(&mut self.preedit, backspaces, &text);
                    self.flush_preedit(&mut ops);
                    return IbusProcessResult { handled: true, ops };
                }
                EngineAction::PassThrough => {
                    if had_preedit {
                        self.preedit.push(ch);
                        self.flush_preedit(&mut ops);
                        return IbusProcessResult { handled: true, ops };
                    }
                    return IbusProcessResult { handled: false, ops };
                }
            }
        }

        let act = self.engine.process_key(ch);
        let cur = self.engine.current_word();
        if !cur.is_empty() {
            self.preedit = cur;
            ops.push(IbusOp::UpdatePreedit(self.preedit.clone()));
            return IbusProcessResult { handled: true, ops };
        }
        match act {
            EngineAction::Replace { backspaces, text } => {
                apply_replace_to_buf(&mut self.preedit, backspaces, &text);
                self.flush_preedit(&mut ops);
                IbusProcessResult { handled: true, ops }
            }
            EngineAction::PassThrough => {
                self.flush_preedit(&mut ops);
                IbusProcessResult { handled: false, ops }
            }
        }
    }

    fn process_key_direct(&mut self, keyval: u32, keycode: u32, state: u32) -> IbusProcessResult {
        let mut ops = Vec::new();
        self.flush_preedit(&mut ops);
        if keyval == KEY_ESCAPE {
            if let EngineAction::Replace { backspaces, text } = self.engine.process_escape() {
                self.emit_direct_replace(backspaces, &text, keycode, state, &mut ops);
                return IbusProcessResult { handled: true, ops };
            }
            return IbusProcessResult { handled: false, ops };
        }
        if keyval == KEY_BACKSPACE {
            if let EngineAction::Replace { backspaces, text } = self.engine.process_backspace() {
                self.emit_direct_replace(backspaces, &text, keycode, state, &mut ops);
                return IbusProcessResult { handled: true, ops };
            }
            return IbusProcessResult { handled: false, ops };
        }
        if is_navigation_keyval(keyval) {
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        }
        let Some(ch) = keyval_to_char(keyval) else {
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        };
        match self.engine.process_key(ch) {
            EngineAction::Replace { backspaces, text } => {
                self.emit_direct_replace(backspaces, &text, keycode, state, &mut ops);
                IbusProcessResult { handled: true, ops }
            }
            EngineAction::PassThrough => IbusProcessResult { handled: false, ops },
        }
    }

    fn emit_direct_replace(
        &self,
        backspaces: usize,
        text: &str,
        keycode: u32,
        state: u32,
        ops: &mut Vec<IbusOp>,
    ) {
        if backspaces > 0 {
            if (self.caps & IBUS_CAP_SURROUNDING_TEXT) != 0 {
                ops.push(IbusOp::DeleteSurrounding(-(backspaces as i32), backspaces as u32));
            } else {
                for _ in 0..backspaces {
                    ops.push(IbusOp::ForwardKey(KEY_BACKSPACE, 14, 0));
                }
            }
        }
        let trailing = text.chars().last().filter(|c| *c == '\n' || *c == '\t');
        let body = if trailing.is_some() { &text[..text.len() - 1] } else { text };
        if !body.is_empty() {
            ops.push(IbusOp::CommitText(body.to_string()));
        }
        if let Some(ch) = trailing {
            let kv = if ch == '\n' { 0xff0d } else { 0xff09 };
            ops.push(IbusOp::ForwardKey(kv, keycode, state & !IBUS_RELEASE_MASK));
        }
    }
}

fn apply_replace_to_buf(buf: &mut String, backspaces: usize, text: &str) {
    for _ in 0..backspaces {
        buf.pop();
    }
    buf.push_str(text);
}
