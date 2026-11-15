use std::collections::VecDeque;

use crate::config::{Charset, Config, InputMethod};
use crate::engine_helpers::{
    encode_string, encoded_len, match_macro, should_auto_restore,
};
use crate::tables::{Tone, VowelFamily, base_family, decode_vowel};
use crate::transform::{KeyRole, classify_key, reanchor_tone};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineAction {
    PassThrough,
    Replace { backspaces: usize, text: String },
}

#[derive(Debug, Clone)]
struct CommittedWord {
    display: Vec<char>,
    raw: Vec<char>,
    char_raw_starts: Vec<usize>,
    had_transform: bool,
}

#[derive(Debug, Clone)]
pub struct Engine {
    pub(crate) config: Config,
    pub(crate) display: Vec<char>,
    pub(crate) raw: Vec<char>,
    pub(crate) char_raw_starts: Vec<usize>,
    pub(crate) had_transform: bool,
    pub(crate) reverted_circumflex: Option<VowelFamily>,
    pub(crate) reverted_breve: bool,
    pub(crate) reverted_horn: bool,
    pub(crate) reverted_tone: Option<Tone>,
    pub(crate) reverted_d: bool,
    pub(crate) reverted_w_standalone: bool,
    pub(crate) reverted_bracket: Option<char>,
    pub(crate) last_w_standalone: bool,
    pub(crate) last_bracket: Option<char>,
    pub(crate) auto_o_horn: bool,
    awaiting_sentence_space: bool,
    sentence_cap_pending: bool,
    can_pop_history: bool,
    clock_offset_secs: i64,
    history: VecDeque<CommittedWord>,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            display: Vec::new(),
            raw: Vec::new(),
            char_raw_starts: Vec::new(),
            had_transform: false,
            reverted_circumflex: None,
            reverted_breve: false,
            reverted_horn: false,
            reverted_tone: None,
            reverted_d: false,
            reverted_w_standalone: false,
            reverted_bracket: None,
            last_w_standalone: false,
            last_bracket: None,
            auto_o_horn: false,
            awaiting_sentence_space: false,
            sentence_cap_pending: false,
            can_pop_history: false,
            clock_offset_secs: 0,
            history: VecDeque::with_capacity(16),
        }
    }

    pub fn set_clock_offset_secs(&mut self, offset_secs: i64) {
        self.clock_offset_secs = offset_secs;
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn update_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn set_vietnamese_enabled(&mut self, enabled: bool) {
        self.config.vietnamese_enabled = enabled;
        self.clear_active_word();
    }

    pub fn reset_all(&mut self) {
        self.clear_active_word();
        self.history.clear();
        self.can_pop_history = false;
        self.awaiting_sentence_space = false;
        self.sentence_cap_pending = false;
    }

    pub fn reset_word(&mut self) {
        self.clear_active_word();
        self.can_pop_history = false;
    }

    pub fn current_word(&self) -> String {
        self.display.iter().collect()
    }

    pub fn raw_word(&self) -> String {
        self.raw.iter().collect()
    }

    pub fn is_word_delimiter(ch: char) -> bool {
        ch.is_whitespace()
            || matches!(
                ch,
                ',' | '.'
                    | ';'
                    | ':'
                    | '!'
                    | '?'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '/'
                    | '\\'
                    | '-'
                    | '+'
                    | '='
                    | '*'
                    | '@'
                    | '#'
                    | '$'
                    | '%'
                    | '^'
                    | '&'
                    | '_'
                    | '~'
                    | '<'
                    | '>'
                    | '|'
                    | '"'
                    | '\''
                    | '`'
                    | '\u{2013}'
                    | '\u{2014}'
                    | '\u{2018}'
                    | '\u{2019}'
                    | '\u{201C}'
                    | '\u{201D}'
                    | '\u{2026}'
            )
    }

    pub fn is_active_delimiter(&self, ch: char) -> bool {
        self.should_handle_as_delimiter(ch)
    }

    pub fn process_key(&mut self, ch: char) -> EngineAction {
        if self.should_handle_as_delimiter(ch) {
            return self.process_delimiter(ch);
        }
        if !self.config.vietnamese_enabled {
            return self.process_key_when_disabled(ch);
        }
        self.can_pop_history = false;
        let effective_ch = self.apply_auto_capitalize(ch);
        let old_display = self.display.clone();

        if let Some(action) = self.try_bracket_shortcut(effective_ch, &old_display) {
            return action;
        }

        self.raw.push(effective_ch);
        let quick_handled = (self.config.quick_start_consonant
            && crate::engine_quick::try_apply_quick_start(
                &mut self.display,
                self.raw.len(),
                &mut self.char_raw_starts,
                effective_ch,
            ))
            || (self.config.quick_telex
                && crate::engine_quick::try_apply_quick_telex(
                    &mut self.display,
                    self.raw.len(),
                    &mut self.char_raw_starts,
                    effective_ch,
                ));
        if quick_handled {
            self.had_transform = true;
            return self.diff_action_for_key(&old_display, Some(ch));
        }

        let role = classify_key(effective_ch, &self.config);
        let mut handled = self.apply_key_role(effective_ch, role);
        if !handled && self.config.quick_end_consonant
            && crate::engine_quick::try_apply_quick_end_consonant(
                &mut self.display,
                self.raw.len(),
                &mut self.char_raw_starts,
                effective_ch,
                &self.config,
            ) {
                self.had_transform = true;
                handled = true;
            }
        if !handled {
            self.append_normal_char(effective_ch);
        } else if self.display == old_display {
            return EngineAction::Replace {
                backspaces: 0,
                text: String::new(),
            };
        }
        while self.char_raw_starts.len() < self.display.len() {
            self.char_raw_starts.push(self.raw.len().saturating_sub(1));
        }
        self.char_raw_starts.truncate(self.display.len());
        self.diff_action_for_key(&old_display, Some(ch))
    }

    pub fn process_backspace(&mut self) -> EngineAction {
        if self.display.is_empty() {
            if self.can_pop_history {
                self.can_pop_history = false;
                if let Some(prev) = self.history.pop_back() {
                    self.display = prev.display;
                    self.raw = prev.raw;
                    self.char_raw_starts = prev.char_raw_starts;
                    self.had_transform = prev.had_transform;
                    self.reset_revert_flags();
                }
            }
            return EngineAction::PassThrough;
        }
        let old_display = self.display.clone();
        self.display.pop();
        if let Some(raw_start) = self.char_raw_starts.pop() {
            self.raw.truncate(raw_start);
        } else if !self.raw.is_empty() {
            self.raw.pop();
        }
        self.last_w_standalone = false;
        self.last_bracket = None;
        self.auto_o_horn = false;
        if self.display.is_empty() {
            self.clear_active_word();
        } else {
            self.had_transform = self.display != self.raw;
            reanchor_tone(&mut self.display, &self.config);
        }
        let action = self.diff_action(&old_display);
        match &action {
            EngineAction::Replace { backspaces: 1, text } if text.is_empty() => {
                EngineAction::PassThrough
            }
            _ => action,
        }
    }

    pub fn process_escape(&mut self) -> EngineAction {
        if !self.config.vietnamese_enabled
            || !self.config.esc_restore
            || !self.had_transform
            || self.display.is_empty()
        {
            return EngineAction::PassThrough;
        }
        let backspaces = encoded_len(self.config.charset, &self.display);
        let text: String = self.raw.iter().collect();
        self.display = self.raw.clone();
        self.char_raw_starts = (0..self.raw.len()).collect();
        self.had_transform = false;
        self.reverted_d = true;
        self.reverted_breve = true;
        self.reverted_horn = true;
        self.reverted_w_standalone = true;
        self.auto_o_horn = false;
        EngineAction::Replace { backspaces, text }
    }

    pub fn process_delimiter(&mut self, delimiter: char) -> EngineAction {
        if self.display.is_empty() {
            self.can_pop_history = false;
            self.update_sentence_state(delimiter);
            return EngineAction::PassThrough;
        }
        let display_str = self.current_word();
        let raw_str = self.raw_word();
        let allow_macro = self.config.macro_enabled
            && (self.config.vietnamese_enabled || self.config.macro_when_english);

        if allow_macro
            && let Some(expanded) = match_macro(
                &self.config,
                &display_str,
                &raw_str,
                self.clock_offset_secs,
            ) {
                let backspaces = encoded_len(self.config.charset, &self.display);
                let mut out = encode_string(self.config.charset, &expanded);
                out.push(delimiter);
                self.clear_active_word();
                self.can_pop_history = false;
                self.update_sentence_state(delimiter);
                return EngineAction::Replace {
                    backspaces,
                    text: out,
                };
            }

        if self.config.vietnamese_enabled
            && self.config.auto_restore_wrong_word
            && self.had_transform
            && should_auto_restore(
                &self.config,
                &self.display,
                &display_str,
                &raw_str,
                self.has_any_revert(),
            )
        {
            let backspaces = encoded_len(self.config.charset, &self.display);
            let mut out = raw_str;
            out.push(delimiter);
            self.display = self.raw.clone();
            self.char_raw_starts = (0..self.raw.len()).collect();
            self.had_transform = false;
            self.push_history();
            self.clear_active_word();
            self.can_pop_history = true;
            self.update_sentence_state(delimiter);
            return EngineAction::Replace {
                backspaces,
                text: out,
            };
        }

        self.push_history();
        self.clear_active_word();
        self.can_pop_history = true;
        self.update_sentence_state(delimiter);
        EngineAction::PassThrough
    }

    fn should_handle_as_delimiter(&self, ch: char) -> bool {
        if !Self::is_word_delimiter(ch) {
            return false;
        }
        if self.config.vietnamese_enabled && self.config.quick_brackets {
            let supports_brackets = matches!(
                self.config.input_method,
                InputMethod::Telex | InputMethod::Custom
            );
            if supports_brackets && matches!(ch, '[' | ']' | '{' | '}') {
                return false;
            }
        }
        if self.config.vietnamese_enabled && self.config.input_method == InputMethod::Viqr
            && matches!(ch, '\'' | '`' | '?' | '~' | '.' | '-' | '^' | '+' | '(') {
                return false;
            }
        if self.config.vietnamese_enabled && self.config.input_method == InputMethod::Custom
            && classify_key(ch, &self.config) != KeyRole::Normal {
                return false;
            }
        true
    }

    fn process_key_when_disabled(&mut self, ch: char) -> EngineAction {
        if self.config.macro_enabled && self.config.macro_when_english {
            self.raw.push(ch);
            self.display.push(ch);
            self.char_raw_starts.push(self.raw.len() - 1);
        } else {
            self.clear_active_word();
        }
        EngineAction::PassThrough
    }

    fn apply_auto_capitalize(&mut self, ch: char) -> char {
        self.awaiting_sentence_space = false;
        if self.config.auto_capitalize_sentence
            && self.sentence_cap_pending
            && self.display.is_empty()
            && ch.is_ascii_alphabetic()
        {
            self.sentence_cap_pending = false;
            return ch.to_ascii_uppercase();
        }
        if ch.is_ascii_alphanumeric() {
            self.sentence_cap_pending = false;
        }
        ch
    }

    pub(crate) fn diff_action(&self, old_display: &[char]) -> EngineAction {
        self.diff_action_for_key(old_display, None)
    }

    fn diff_action_for_key(&self, old_display: &[char], typed_ch: Option<char>) -> EngineAction {
        let common = old_display
            .iter()
            .zip(self.display.iter())
            .take_while(|(a, b)| a == b)
            .count();
        let backspaces = encoded_len(self.config.charset, &old_display[common..]);
        let tail: String = self.display[common..].iter().collect();
        if backspaces == 0 && tail.len() == 1 && self.config.charset == Charset::Unicode
            && let Some(ch) = tail.chars().next()
                && ch.is_ascii() && (typed_ch.is_none() || typed_ch == Some(ch)) {
                    return EngineAction::PassThrough;
                }
        let encoded = encode_string(self.config.charset, &tail);
        if backspaces == 0 && encoded.is_empty() {
            return EngineAction::PassThrough;
        }
        EngineAction::Replace {
            backspaces,
            text: encoded,
        }
    }

    pub(crate) fn last_vowel_family(&self) -> Option<VowelFamily> {
        self.display
            .iter()
            .rev()
            .find_map(|&c| decode_vowel(c).map(|(b, _, _)| base_family(b)))
    }

    fn push_history(&mut self) {
        if self.history.len() >= 16 {
            self.history.pop_front();
        }
        self.history.push_back(CommittedWord {
            display: self.display.clone(),
            raw: self.raw.clone(),
            char_raw_starts: self.char_raw_starts.clone(),
            had_transform: self.had_transform,
        });
    }

    fn clear_active_word(&mut self) {
        self.display.clear();
        self.raw.clear();
        self.char_raw_starts.clear();
        self.had_transform = false;
        self.reset_revert_flags();
    }

    fn reset_revert_flags(&mut self) {
        self.reverted_circumflex = None;
        self.reverted_breve = false;
        self.reverted_horn = false;
        self.reverted_tone = None;
        self.reverted_d = false;
        self.reverted_w_standalone = false;
        self.reverted_bracket = None;
        self.last_w_standalone = false;
        self.last_bracket = None;
        self.auto_o_horn = false;
    }

    fn has_any_revert(&self) -> bool {
        self.reverted_circumflex.is_some()
            || self.reverted_breve
            || self.reverted_horn
            || self.reverted_tone.is_some()
            || self.reverted_d
            || self.reverted_w_standalone
            || self.reverted_bracket.is_some()
    }

    fn update_sentence_state(&mut self, delimiter: char) {
        if matches!(delimiter, '\n' | '\r') {
            self.sentence_cap_pending = true;
            self.awaiting_sentence_space = false;
        } else if matches!(delimiter, '.' | '!' | '?') {
            self.awaiting_sentence_space = true;
            self.sentence_cap_pending = false;
        } else if delimiter.is_whitespace() {
            if self.awaiting_sentence_space {
                self.sentence_cap_pending = true;
                self.awaiting_sentence_space = false;
            }
        } else {
            self.awaiting_sentence_space = false;
            self.sentence_cap_pending = false;
        }
    }
}
