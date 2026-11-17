
use mskey_core::{AppBehavior, Config, Engine, EngineAction, Mechanism};

use crate::config_store::local_utc_offset_secs;
use crate::keyval::{
    HotkeySignal, HotkeyTracker, IBUS_CONTROL_MASK, IBUS_MOD1_MASK, IBUS_MOD4_MASK,
    IBUS_RELEASE_MASK, KEY_BACKSPACE, KEY_ESCAPE, KEY_KP_ENTER, KEY_KP_TAB, KEY_LEFT,
    KEY_RETURN, KEY_RIGHT, KEY_TAB, hold_english_keyval, is_modifier_keyval,
    is_navigation_keyval, keyval_to_char,
};
use crate::linux_extra::{is_browser_app, is_terminal_app};

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
    TriggerSelectionConvert,
    Beep,
    ShowOsd(bool),
    ShowCaretHud(bool),
    Delay(u32),
    PasteViaClipboard(String, bool),
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
    pub active_title: String,
    pub preedit: String,
    pub caps: u32,
    pub us_layout: bool,
    pub cursor_rect: (i32, i32, i32, i32),
    pub surrounding_text: String,
    pub surrounding_cursor: u32,
    pub hold_english_active: bool,
    pub suppress_surrounding_count: u32,
    pub expected_surrounding_tail: Option<String>,
    pub hydrated_surrounding_len: usize,
}

impl IbusSession {
    pub fn new(config: Config) -> Self {
        let pref = config.vietnamese_enabled;
        let mut engine = Engine::new(config);
        engine.set_clock_offset_secs(local_utc_offset_secs() as i64);
        Self {
            engine,
            hotkeys: HotkeyTracker::default(),
            user_preferred_ve: pref,
            active_app: String::new(),
            active_title: String::new(),
            preedit: String::new(),
            caps: IBUS_CAP_PREEDIT_TEXT,
            us_layout: true,
            cursor_rect: (0, 0, 0, 0),
            surrounding_text: String::new(),
            surrounding_cursor: 0,
            hold_english_active: false,
            suppress_surrounding_count: 0,
            expected_surrounding_tail: None,
            hydrated_surrounding_len: 0,
        }
    }

    pub fn set_us_layout(&mut self, us: bool) {
        self.us_layout = us;
    }

    pub fn set_cursor_location(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.cursor_rect = (x, y, w, h);
    }

    pub fn set_clipboard_text(&mut self, text: Option<String>) {
        self.engine.set_clipboard_text(text);
    }

    pub fn needs_clipboard_for_key(&self, keyval: u32, state: u32) -> bool {
        if (state & (IBUS_RELEASE_MASK | IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_MOD4_MASK)) != 0 {
            return false;
        }
        let Some(ch) = macro_delimiter_char(keyval) else {
            return false;
        };
        let cfg = self.engine.config();
        cfg.macro_enabled
            && self.engine.is_active_delimiter(ch)
            && (!cfg.macro_trigger_tab_only || ch == '\t')
            && !self.engine.current_word().is_empty()
            && cfg.macros.iter().any(|m| m.enabled && m.expansion.contains("{clipboard}"))
    }

    pub fn set_surrounding_text(&mut self, text: String, cursor: u32) {
        let before_cursor: String = text.chars().take(cursor as usize).collect();
        self.surrounding_text = text;
        self.surrounding_cursor = cursor;
        if self.suppress_surrounding_count > 0 {
            self.suppress_surrounding_count -= 1;
            if let Some(ref tail) = self.expected_surrounding_tail
                && before_cursor.ends_with(tail)
            {
                self.suppress_surrounding_count = 0;
                self.expected_surrounding_tail = None;
            }
            return;
        }
        if !self.preedit.is_empty() || !self.engine.config().reedit_surrounding_word {
            return;
        }
        self.engine.hydrate_word(&before_cursor);
        let word_len = self.engine.current_word().chars().count();
        self.hydrated_surrounding_len = if self.uses_preedit_mode() { word_len } else { 0 };
    }

    pub fn update_config(&mut self, cfg: Config) {
        self.user_preferred_ve = cfg.vietnamese_enabled;
        self.engine.update_config(cfg);
        self.apply_app_policy();
    }

    pub fn set_user_vietnamese(&mut self, enabled: bool) {
        self.user_preferred_ve = enabled;
        self.engine.set_vietnamese_enabled(enabled);
        self.preedit.clear();
    }

    pub fn set_capabilities(&mut self, caps: u32) {
        self.caps = caps;
    }

    pub fn focus_in(&mut self, app_name: &str) -> Vec<IbusOp> {
        self.focus_in_window(app_name, "")
    }

    pub fn focus_in_window(&mut self, app_name: &str, window_title: &str) -> Vec<IbusOp> {
        let mut ops = Vec::new();
        self.flush_preedit(&mut ops);
        self.engine.reset_all();
        self.engine.set_temporary_english(false);
        self.hotkeys.reset();
        self.hold_english_active = false;
        self.hydrated_surrounding_len = 0;
        self.suppress_surrounding_count = 0;
        self.expected_surrounding_tail = None;
        let trimmed = app_name.trim().to_ascii_lowercase();
        if !trimmed.is_empty() {
            self.active_app = trimmed;
            self.active_title = window_title.trim().to_string();
        } else if !window_title.trim().is_empty() {
            self.active_title = window_title.trim().to_string();
        }
        if self.apply_app_policy() {
            ops.push(IbusOp::SyncProperties);
        }
        let cfg = self.engine.config();
        let behavior = cfg.behavior_for_window(&self.active_app, &self.active_title);
        if cfg.show_caret_osd && behavior != AppBehavior::Blacklist {
            ops.push(IbusOp::ShowCaretHud(self.engine.is_vietnamese_active()));
        }
        ops
    }

    pub fn reset(&mut self) -> Vec<IbusOp> {
        let mut ops = Vec::new();
        self.flush_preedit(&mut ops);
        self.engine.reset_word();
        self.engine.set_temporary_english(false);
        self.hotkeys.reset();
        self.hold_english_active = false;
        self.hydrated_surrounding_len = 0;
        self.record_self_surrounding_edits(&ops);
        ops
    }

    pub fn focus_out(&mut self) -> Vec<IbusOp> {
        let mut ops = Vec::new();
        self.flush_preedit(&mut ops);
        self.engine.reset_all();
        self.engine.set_temporary_english(false);
        self.hotkeys.reset();
        self.hold_english_active = false;
        self.hydrated_surrounding_len = 0;
        self.record_self_surrounding_edits(&ops);
        ops
    }

    fn apply_app_policy(&mut self) -> bool {
        let cfg = self.engine.config().clone();
        let target = match cfg.behavior_for_window(&self.active_app, &self.active_title) {
            AppBehavior::ForceEnglish | AppBehavior::Blacklist => false,
            AppBehavior::ForceVietnamese => true,
            AppBehavior::Default => self.user_preferred_ve,
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

    fn record_self_surrounding_edits(&mut self, ops: &[IbusOp]) {
        let edit_ops = ops
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    IbusOp::DeleteSurrounding(..)
                        | IbusOp::CommitText(_)
                        | IbusOp::PasteViaClipboard(..)
                )
            })
            .count() as u32;
        let has_bs_replace = edit_ops == 0
            && ops
                .iter()
                .any(|op| matches!(op, IbusOp::ForwardKey(kv, _, _) if *kv == KEY_BACKSPACE));
        let count = if edit_ops > 0 {
            edit_ops
        } else if has_bs_replace {
            1
        } else {
            0
        };
        if count > 0 {
            self.suppress_surrounding_count = self.suppress_surrounding_count.saturating_add(count);
            let word = self.engine.current_word();
            if !word.is_empty() {
                self.expected_surrounding_tail = Some(word);
            } else if let Some(last_commit) = ops.iter().rev().find_map(|op| match op {
                IbusOp::CommitText(t) | IbusOp::PasteViaClipboard(t, _) => Some(t.clone()),
                _ => None,
            }) {
                self.expected_surrounding_tail = Some(last_commit);
            } else {
                self.expected_surrounding_tail = None;
            }
        }
    }

    fn push_pending_cursor_left(&mut self, ops: &mut Vec<IbusOp>) {
        let steps = self.engine.take_pending_cursor_left();
        for _ in 0..steps {
            ops.push(IbusOp::ForwardKey(KEY_LEFT, 113, 0));
        }
    }

    fn delete_hydrated_surrounding(&mut self, count: usize, ops: &mut Vec<IbusOp>) {
        if count == 0 {
            return;
        }
        if (self.caps & IBUS_CAP_SURROUNDING_TEXT) != 0 {
            ops.push(IbusOp::DeleteSurrounding(-(count as i32), count as u32));
        } else {
            for _ in 0..count {
                ops.push(IbusOp::ForwardKey(KEY_BACKSPACE, 14, 0));
            }
        }
    }

    pub fn process_key(&mut self, keyval: u32, keycode: u32, state: u32) -> IbusProcessResult {
        let is_release = (state & IBUS_RELEASE_MASK) != 0;
        let cfg = self.engine.config().clone();
        if let Some(hold_kv) = hold_english_keyval(cfg.hold_english_key)
            && keyval == hold_kv
        {
            let mut ops = Vec::new();
            if !is_release {
                if !self.hold_english_active {
                    self.hold_english_active = true;
                    self.flush_preedit(&mut ops);
                    self.engine.set_temporary_english(true);
                    self.hydrated_surrounding_len = 0;
                    self.record_self_surrounding_edits(&ops);
                    if cfg.show_caret_osd {
                        ops.push(IbusOp::ShowCaretHud(false));
                    }
                }
            } else if self.hold_english_active {
                self.hold_english_active = false;
                self.engine.set_temporary_english(false);
                if cfg.show_caret_osd {
                    ops.push(IbusOp::ShowCaretHud(self.engine.is_vietnamese_active()));
                }
            }
            return IbusProcessResult { handled: false, ops };
        }

        let sig = self.hotkeys.process_key_full(
            cfg.switch_preset,
            cfg.custom_switch_hotkey,
            cfg.clipboard_convert_hotkey,
            cfg.convert_selection_hotkey,
            cfg.undo_last_word_hotkey,
            cfg.quick_capslock_switch,
            cfg.hold_english_key,
            keyval,
            state,
        );
        match sig {
            HotkeySignal::ToggleVe => {
                let mut ops = Vec::new();
                self.flush_preedit(&mut ops);
                self.hydrated_surrounding_len = 0;
                let next = !self.engine.config().vietnamese_enabled;
                self.set_user_vietnamese(next);
                self.record_self_surrounding_edits(&ops);
                ops.push(IbusOp::SyncProperties);
                if self.engine.config().beep_on_switch {
                    ops.push(IbusOp::Beep);
                }
                if self.engine.config().show_switch_osd {
                    ops.push(IbusOp::ShowOsd(next));
                }
                if self.engine.config().show_caret_osd {
                    ops.push(IbusOp::ShowCaretHud(self.engine.is_vietnamese_active()));
                }
                return IbusProcessResult { handled: true, ops };
            }
            HotkeySignal::ConvertClipboard => {
                return IbusProcessResult {
                    handled: true,
                    ops: vec![IbusOp::TriggerClipboardConvert],
                };
            }
            HotkeySignal::ConvertSelection => {
                let mut ops = Vec::new();
                self.flush_preedit(&mut ops);
                self.hydrated_surrounding_len = 0;
                self.record_self_surrounding_edits(&ops);
                ops.push(IbusOp::TriggerSelectionConvert);
                return IbusProcessResult { handled: true, ops };
            }
            HotkeySignal::UndoLastWord => {
                if !self.active_app.is_empty()
                    && cfg.behavior_for_window(&self.active_app, &self.active_title)
                        == AppBehavior::Blacklist
                {
                    return IbusProcessResult { handled: false, ops: Vec::new() };
                }
                return self.process_undo_last_word(keycode, state);
            }
            HotkeySignal::None => {}
        }

        if is_release || is_modifier_keyval(keyval) {
            return IbusProcessResult { handled: false, ops: Vec::new() };
        }
        if !self.active_app.is_empty()
            && self.engine.config().behavior_for_window(&self.active_app, &self.active_title)
                == AppBehavior::Blacklist
        {
            return IbusProcessResult { handled: false, ops: Vec::new() };
        }
        if self.engine.config().disable_on_non_us_layout && !self.us_layout {
            return IbusProcessResult { handled: false, ops: Vec::new() };
        }

        if (state & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_MOD4_MASK)) != 0 {
            let mut ops = Vec::new();
            self.flush_preedit(&mut ops);
            self.engine.reset_word();
            self.hydrated_surrounding_len = 0;
            self.record_self_surrounding_edits(&ops);
            return IbusProcessResult { handled: false, ops };
        }

        let res = if self.uses_preedit_mode() {
            self.process_key_preedit(keyval, keycode, state)
        } else {
            self.process_key_direct(keyval, keycode, state)
        };
        self.record_self_surrounding_edits(&res.ops);
        res
    }

    fn process_undo_last_word(&mut self, keycode: u32, state: u32) -> IbusProcessResult {
        let mut ops = Vec::new();
        if self.uses_preedit_mode() && !self.preedit.is_empty() {
            match self.engine.toggle_last_committed_word() {
                EngineAction::Replace { backspaces, text } => {
                    apply_replace_to_buf(&mut self.preedit, backspaces, &text);
                    if self.preedit.is_empty() {
                        ops.push(IbusOp::HidePreedit);
                    } else {
                        ops.push(IbusOp::UpdatePreedit(self.preedit.clone()));
                    }
                    self.push_pending_cursor_left(&mut ops);
                    self.record_self_surrounding_edits(&ops);
                    return IbusProcessResult { handled: true, ops };
                }
                EngineAction::PassThrough => {
                    return IbusProcessResult { handled: false, ops };
                }
            }
        }
        match self.engine.toggle_last_committed_word() {
            EngineAction::Replace { backspaces, text } => {
                self.hydrated_surrounding_len = 0;
                self.emit_direct_replace(backspaces, &text, keycode, state, &mut ops);
                self.push_pending_cursor_left(&mut ops);
                self.record_self_surrounding_edits(&ops);
                IbusProcessResult { handled: true, ops }
            }
            EngineAction::PassThrough => IbusProcessResult { handled: false, ops },
        }
    }

    fn process_key_preedit(&mut self, keyval: u32, keycode: u32, state: u32) -> IbusProcessResult {
        let mut ops = Vec::new();
        if keyval == KEY_ESCAPE {
            self.hydrated_surrounding_len = 0;
            if self.preedit.is_empty() {
                self.engine.reset_word();
                return IbusProcessResult { handled: false, ops };
            }
            let act = self.engine.process_escape();
            if let EngineAction::Replace { backspaces, text } = act {
                apply_replace_to_buf(&mut self.preedit, backspaces, &text);
                self.flush_preedit(&mut ops);
                self.push_pending_cursor_left(&mut ops);
                return IbusProcessResult { handled: true, ops };
            }
            self.flush_preedit(&mut ops);
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        }

        if keyval == KEY_BACKSPACE {
            if self.preedit.is_empty() {
                if self.hydrated_surrounding_len > 0 {
                    self.hydrated_surrounding_len -= 1;
                    let _ = self.engine.process_backspace();
                    self.suppress_surrounding_count = self.suppress_surrounding_count.saturating_add(1);
                    let word = self.engine.current_word();
                    self.expected_surrounding_tail = if word.is_empty() { None } else { Some(word) };
                } else {
                    self.engine.reset_word();
                }
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

        if matches!(keyval, KEY_TAB | KEY_KP_TAB | KEY_RETURN | KEY_KP_ENTER) {
            let delim = if matches!(keyval, KEY_TAB | KEY_KP_TAB) { '\t' } else { '\n' };
            let cfg = self.engine.config();
            let active_typing = (self.engine.is_vietnamese_active()
                || (cfg.macro_enabled && cfg.macro_when_english))
                && !self.engine.is_temporary_english();
            if active_typing && (!self.preedit.is_empty() || self.hydrated_surrounding_len > 0) {
                let hydrated = std::mem::take(&mut self.hydrated_surrounding_len);
                if let EngineAction::Replace { backspaces, text } = self.engine.process_key(delim) {
                    if !self.preedit.is_empty() {
                        let trailing = text.chars().last().filter(|c| *c == '\n' || *c == '\t');
                        let body = if trailing.is_some() { &text[..text.len() - 1] } else { &text };
                        apply_replace_to_buf(&mut self.preedit, backspaces, body);
                        self.flush_preedit(&mut ops);
                        if let Some(ch) = trailing {
                            let kv = if ch == '\n' { KEY_RETURN } else { KEY_TAB };
                            ops.push(IbusOp::ForwardKey(kv, keycode, state & !IBUS_RELEASE_MASK));
                        }
                    } else {
                        let del = if hydrated > 0 { backspaces.min(hydrated) } else { backspaces };
                        self.emit_direct_replace(del, &text, keycode, state, &mut ops);
                    }
                    self.push_pending_cursor_left(&mut ops);
                    return IbusProcessResult { handled: true, ops };
                }
            }
            self.hydrated_surrounding_len = 0;
            self.flush_preedit(&mut ops);
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        }

        if is_navigation_keyval(keyval) {
            self.hydrated_surrounding_len = 0;
            self.flush_preedit(&mut ops);
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        }

        let Some(ch) = keyval_to_char(keyval) else {
            self.hydrated_surrounding_len = 0;
            self.flush_preedit(&mut ops);
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        };

        let cfg = self.engine.config();
        let active_typing = (self.engine.is_vietnamese_active()
            || (cfg.macro_enabled && cfg.macro_when_english))
            && !self.engine.is_temporary_english();
        if !active_typing {
            self.hydrated_surrounding_len = 0;
            self.flush_preedit(&mut ops);
            return IbusProcessResult { handled: false, ops };
        }

        if self.engine.is_active_delimiter(ch) {
            let hydrated = std::mem::take(&mut self.hydrated_surrounding_len);
            let had_preedit = !self.preedit.is_empty();
            let act = self.engine.process_key(ch);
            match act {
                EngineAction::Replace { backspaces, text } => {
                    if had_preedit {
                        apply_replace_to_buf(&mut self.preedit, backspaces, &text);
                        self.flush_preedit(&mut ops);
                    } else {
                        let del = if hydrated > 0 { backspaces.min(hydrated) } else { backspaces };
                        self.emit_direct_replace(del, &text, keycode, state, &mut ops);
                    }
                    self.push_pending_cursor_left(&mut ops);
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

        let hydrated = std::mem::take(&mut self.hydrated_surrounding_len);
        let act = self.engine.process_key(ch);
        let cur = self.engine.current_word();
        if hydrated > 0 {
            if let EngineAction::Replace { backspaces, text } = act {
                self.emit_direct_replace(backspaces, &text, keycode, state, &mut ops);
                self.push_pending_cursor_left(&mut ops);
                return IbusProcessResult { handled: true, ops };
            }
            if !cur.is_empty() {
                self.delete_hydrated_surrounding(hydrated, &mut ops);
                self.preedit = cur;
                ops.push(IbusOp::UpdatePreedit(self.preedit.clone()));
                return IbusProcessResult { handled: true, ops };
            }
            return IbusProcessResult { handled: false, ops };
        }
        if !cur.is_empty() {
            self.preedit = cur;
            ops.push(IbusOp::UpdatePreedit(self.preedit.clone()));
            return IbusProcessResult { handled: true, ops };
        }
        match act {
            EngineAction::Replace { backspaces, text } => {
                apply_replace_to_buf(&mut self.preedit, backspaces, &text);
                self.flush_preedit(&mut ops);
                self.push_pending_cursor_left(&mut ops);
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
        self.hydrated_surrounding_len = 0;
        if self.engine.is_temporary_english() {
            return IbusProcessResult { handled: false, ops };
        }
        if keyval == KEY_ESCAPE {
            if let EngineAction::Replace { backspaces, text } = self.engine.process_escape() {
                self.emit_direct_replace(backspaces, &text, keycode, state, &mut ops);
                self.push_pending_cursor_left(&mut ops);
                return IbusProcessResult { handled: true, ops };
            }
            return IbusProcessResult { handled: false, ops };
        }
        if keyval == KEY_BACKSPACE {
            if let EngineAction::Replace { backspaces, text } = self.engine.process_backspace() {
                self.emit_direct_replace(backspaces, &text, keycode, state, &mut ops);
                self.push_pending_cursor_left(&mut ops);
                return IbusProcessResult { handled: true, ops };
            }
            return IbusProcessResult { handled: false, ops };
        }
        if matches!(keyval, KEY_TAB | KEY_KP_TAB | KEY_RETURN | KEY_KP_ENTER) {
            let delim = if matches!(keyval, KEY_TAB | KEY_KP_TAB) { '\t' } else { '\n' };
            if let EngineAction::Replace { backspaces, text } = self.engine.process_key(delim) {
                self.emit_direct_replace(backspaces, &text, keycode, state, &mut ops);
                self.push_pending_cursor_left(&mut ops);
                return IbusProcessResult { handled: true, ops };
            }
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        }
        if is_navigation_keyval(keyval) {
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        }
        let Some(ch) = macro_delimiter_char(keyval) else {
            self.engine.reset_word();
            return IbusProcessResult { handled: false, ops };
        };
        match self.engine.process_key(ch) {
            EngineAction::Replace { backspaces, text } => {
                self.emit_direct_replace(backspaces, &text, keycode, state, &mut ops);
                self.push_pending_cursor_left(&mut ops);
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
        let cfg = self.engine.config();
        let browser = is_browser_app(&self.active_app);
        let terminal = is_terminal_app(&self.active_app);
        let force_forward = cfg.standard_key_send || terminal;
        let use_clip = cfg.use_clipboard_send;
        let shift_insert = cfg.paste_via_shift_insert || terminal;
        let delay = cfg.send_delay_ms.min(25) as u32;
        let push_delay = |ops: &mut Vec<IbusOp>| {
            if delay > 0 {
                ops.push(IbusOp::Delay(delay));
            }
        };

        if backspaces > 0 {
            if browser && cfg.fix_browser_suggest_1 {
                ops.push(IbusOp::CommitText("\u{200B}".to_string()));
                ops.push(IbusOp::ForwardKey(KEY_BACKSPACE, 14, 0));
                push_delay(ops);
            }
            if browser && cfg.fix_browser_suggest_2 {
                ops.push(IbusOp::ForwardKey(KEY_RIGHT, 114, 0));
                push_delay(ops);
            }
            if (self.caps & IBUS_CAP_SURROUNDING_TEXT) != 0 && !force_forward && !use_clip {
                ops.push(IbusOp::DeleteSurrounding(-(backspaces as i32), backspaces as u32));
                push_delay(ops);
            } else {
                for _ in 0..backspaces {
                    ops.push(IbusOp::ForwardKey(KEY_BACKSPACE, 14, 0));
                    push_delay(ops);
                }
            }
        }
        let trailing = text.chars().last().filter(|c| *c == '\n' || *c == '\t');
        let body = if trailing.is_some() { &text[..text.len() - 1] } else { text };
        if !body.is_empty() {
            if use_clip {
                ops.push(IbusOp::PasteViaClipboard(body.to_string(), shift_insert));
            } else {
                ops.push(IbusOp::CommitText(body.to_string()));
            }
            push_delay(ops);
        }
        if let Some(ch) = trailing {
            let kv = if ch == '\n' { KEY_RETURN } else { KEY_TAB };
            ops.push(IbusOp::ForwardKey(kv, keycode, state & !IBUS_RELEASE_MASK));
        }
    }
}

fn macro_delimiter_char(kv: u32) -> Option<char> {
    match kv {
        KEY_TAB | KEY_KP_TAB => Some('\t'),
        KEY_RETURN | KEY_KP_ENTER => Some('\n'),
        _ => keyval_to_char(kv),
    }
}

fn apply_replace_to_buf(buf: &mut String, backspaces: usize, text: &str) {
    for _ in 0..backspaces {
        buf.pop();
    }
    buf.push_str(text);
}
