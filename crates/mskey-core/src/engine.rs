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
    transformed: Vec<char>,
    raw: Vec<char>,
    char_raw_starts: Vec<usize>,
    had_transform: bool,
    delimiter: char,
    showing_raw: bool,
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
    temporary_english: bool,
    bypass_until_whitespace: bool,
    word_bypassed: bool,
    last_delim: Option<char>,
    clock_offset_secs: i64,
    clipboard_text: Option<String>,
    pending_cursor_left: usize,
    last_esc_word: Option<String>,
    stashed_undo_display: Option<Vec<char>>,
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
            temporary_english: false,
            bypass_until_whitespace: false,
            word_bypassed: false,
            last_delim: None,
            clock_offset_secs: 0,
            clipboard_text: None,
            pending_cursor_left: 0,
            last_esc_word: None,
            stashed_undo_display: None,
            history: VecDeque::with_capacity(16),
        }
    }

    pub fn set_clock_offset_secs(&mut self, offset_secs: i64) {
        self.clock_offset_secs = offset_secs;
    }

    pub fn set_clipboard_text(&mut self, text: Option<String>) {
        self.clipboard_text = text;
    }

    pub fn take_pending_cursor_left(&mut self) -> usize {
        let n = self.pending_cursor_left;
        self.pending_cursor_left = 0;
        n
    }

    pub fn last_esc_word(&self) -> Option<&str> {
        self.last_esc_word.as_deref()
    }

    pub fn last_word_for_dict(&self) -> Option<String> {
        if let Some(w) = &self.last_esc_word
            && !w.trim().is_empty()
        {
            return Some(w.clone());
        }
        if !self.display.is_empty() {
            return Some(self.current_word());
        }
        self.history
            .back()
            .map(|h| {
                let slice = if h.showing_raw { &h.raw } else { &h.transformed };
                slice.iter().collect()
            })
            .filter(|s: &String| !s.trim().is_empty())
    }

    pub fn set_temporary_english(&mut self, enabled: bool) {
        if self.temporary_english != enabled {
            self.temporary_english = enabled;
            self.clear_active_word();
            self.bypass_until_whitespace = false;
            self.last_delim = None;
        }
    }

    pub fn is_temporary_english(&self) -> bool {
        self.temporary_english
    }

    pub fn is_vietnamese_active(&self) -> bool {
        self.config.vietnamese_enabled && !self.temporary_english
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
        self.bypass_until_whitespace = false;
        self.last_delim = None;
    }

    pub fn reset_all(&mut self) {
        self.clear_active_word();
        self.history.clear();
        self.can_pop_history = false;
        self.awaiting_sentence_space = false;
        self.sentence_cap_pending = false;
        self.bypass_until_whitespace = false;
        self.last_delim = None;
        self.pending_cursor_left = 0;
    }

    pub fn reset_word(&mut self) {
        self.clear_active_word();
        self.can_pop_history = false;
        self.bypass_until_whitespace = false;
        self.last_delim = None;
    }

    pub fn hydrate_word(&mut self, surrounding_before_cursor: &str) {
        self.clear_active_word();
        self.can_pop_history = false;
        self.bypass_until_whitespace = false;
        self.last_delim = None;
        if !self.config.reedit_surrounding_word || surrounding_before_cursor.is_empty() {
            return;
        }
        let mut word_rev = Vec::new();
        for ch in surrounding_before_cursor.chars().rev() {
            if Self::is_word_delimiter(ch) {
                break;
            }
            word_rev.push(ch);
        }
        if word_rev.is_empty() {
            return;
        }
        word_rev.reverse();
        self.raw = word_rev.clone();
        self.display = word_rev;
        self.char_raw_starts = (0..self.display.len()).collect();
        self.had_transform = false;
        if self.config.smart_code_bypass && Self::looks_like_code_word(&self.raw) {
            self.word_bypassed = true;
        }
    }

    pub fn toggle_last_committed_word(&mut self) -> EngineAction {
        if !self.display.is_empty() {
            let old_display = self.display.clone();
            if let Some(stashed) = self.stashed_undo_display.take() {
                self.display = stashed;
                self.had_transform = self.display != self.raw;
                return self.diff_action(&old_display);
            }
            if self.display != self.raw {
                self.stashed_undo_display = Some(old_display.clone());
                self.display = self.raw.clone();
                self.char_raw_starts = (0..self.raw.len()).collect();
                self.had_transform = false;
                return self.diff_action(&old_display);
            }
            return EngineAction::PassThrough;
        }
        if !self.can_pop_history {
            return EngineAction::PassThrough;
        }
        let Some(last) = self.history.back_mut() else {
            return EngineAction::PassThrough;
        };
        if last.transformed == last.raw {
            return EngineAction::PassThrough;
        }
        let old_chars = if last.showing_raw {
            &last.raw
        } else {
            &last.transformed
        };
        let new_chars = if last.showing_raw {
            &last.transformed
        } else {
            &last.raw
        };
        let backspaces = encoded_len(self.config.charset, old_chars) + 1;
        let mut text = if last.showing_raw {
            let s: String = new_chars.iter().collect();
            encode_string(self.config.charset, &s)
        } else {
            new_chars.iter().collect()
        };
        text.push(last.delimiter);
        last.showing_raw = !last.showing_raw;
        EngineAction::Replace { backspaces, text }
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
        self.last_delim = None;
        self.stashed_undo_display = None;
        if !self.is_vietnamese_active() {
            return self.process_key_when_disabled(ch);
        }
        self.can_pop_history = false;
        if self.bypass_until_whitespace || self.word_bypassed {
            let old_display = self.display.clone();
            self.raw.push(ch);
            self.display.push(ch);
            self.char_raw_starts.push(self.raw.len() - 1);
            return self.diff_action_for_key(&old_display, Some(ch));
        }
        let effective_ch = self.apply_auto_capitalize(ch);
        let old_display = self.display.clone();
        if self.config.smart_code_bypass && self.should_trigger_word_bypass(effective_ch) {
            self.word_bypassed = true;
            if self.raw.iter().all(|c| c.is_ascii()) {
                self.display = self.raw.clone();
                self.char_raw_starts = (0..self.raw.len()).collect();
                self.had_transform = false;
            }
            self.raw.push(effective_ch);
            self.display.push(effective_ch);
            self.char_raw_starts.push(self.raw.len() - 1);
            return self.diff_action_for_key(&old_display, Some(ch));
        }

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
        self.stashed_undo_display = None;
        if self.display.is_empty() {
            self.last_delim = None;
            if self.can_pop_history {
                self.can_pop_history = false;
                self.bypass_until_whitespace = false;
                if let Some(prev) = self.history.pop_back() {
                    self.display = if prev.showing_raw {
                        prev.raw.clone()
                    } else {
                        prev.transformed
                    };
                    self.raw = prev.raw;
                    self.char_raw_starts = if prev.showing_raw {
                        (0..self.raw.len()).collect()
                    } else {
                        prev.char_raw_starts
                    };
                    self.had_transform = !prev.showing_raw && prev.had_transform;
                    self.reset_revert_flags();
                    self.word_bypassed = self.config.smart_code_bypass
                        && Self::looks_like_code_word(&self.raw);
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
        self.reset_revert_flags();
        if self.display.is_empty() {
            self.clear_active_word();
        } else if self.word_bypassed {
            self.word_bypassed = self.config.smart_code_bypass
                && Self::looks_like_code_word(&self.raw);
            self.had_transform = self.display != self.raw;
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
        if !self.is_vietnamese_active()
            || !self.config.esc_restore
            || !self.had_transform
            || self.display.is_empty()
        {
            return EngineAction::PassThrough;
        }
        let backspaces = encoded_len(self.config.charset, &self.display);
        let text: String = self.raw.iter().collect();
        self.last_esc_word = Some(text.clone());
        self.stashed_undo_display = None;
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
        self.stashed_undo_display = None;
        let active_vietnamese = self.is_vietnamese_active();
        let triggers_code_bypass = active_vietnamese
            && self.config.smart_code_bypass
            && self.should_trigger_delimiter_bypass(delimiter);
        if triggers_code_bypass {
            self.bypass_until_whitespace = true;
        }

        if self.display.is_empty() {
            self.can_pop_history = false;
            self.last_delim = Some(delimiter);
            if delimiter.is_whitespace() {
                self.bypass_until_whitespace = false;
            }
            self.update_sentence_state(delimiter);
            return EngineAction::PassThrough;
        }

        let mut display_str = self.current_word();
        let raw_str = self.raw_word();
        let tab_ok = !self.config.macro_trigger_tab_only || delimiter == '\t';
        let allow_macro = self.config.macro_enabled
            && (active_vietnamese || self.config.macro_when_english)
            && !self.bypass_until_whitespace
            && !self.word_bypassed
            && !triggers_code_bypass
            && tab_ok;

        if allow_macro
            && let Some(expanded) = match_macro(
                &self.config,
                &display_str,
                &raw_str,
                self.clock_offset_secs,
                self.clipboard_text.as_deref(),
            )
        {
            let backspaces = encoded_len(self.config.charset, &self.display);
            let consume_delim = self.config.macro_trigger_tab_only && delimiter == '\t';
            let out = if let Some(pos) = expanded.cursor_char_idx {
                let before: String = expanded.text.chars().take(pos).collect();
                let after: String = expanded.text.chars().skip(pos).collect();
                let before_enc = encode_string(self.config.charset, &before);
                let after_enc = encode_string(self.config.charset, &after);
                let mut left_steps = after_enc.chars().count();
                let mut combined = format!("{before_enc}{after_enc}");
                if !consume_delim {
                    combined.push(delimiter);
                    left_steps += 1;
                }
                self.pending_cursor_left = left_steps;
                combined
            } else {
                self.pending_cursor_left = 0;
                let mut combined = encode_string(self.config.charset, &expanded.text);
                if !consume_delim {
                    combined.push(delimiter);
                }
                combined
            };
            self.clear_active_word();
            self.can_pop_history = false;
            self.last_delim = Some(delimiter);
            if delimiter.is_whitespace() {
                self.bypass_until_whitespace = false;
            }
            self.update_sentence_state(delimiter);
            return EngineAction::Replace {
                backspaces,
                text: out,
            };
        }

        if triggers_code_bypass && self.display != self.raw && self.raw.iter().all(|c| c.is_ascii())
        {
            let backspaces = encoded_len(self.config.charset, &self.display);
            let mut out = raw_str;
            out.push(delimiter);
            self.display = self.raw.clone();
            self.char_raw_starts = (0..self.raw.len()).collect();
            self.had_transform = false;
            self.push_history(delimiter);
            self.clear_active_word();
            self.can_pop_history = true;
            self.last_delim = Some(delimiter);
            if delimiter.is_whitespace() {
                self.bypass_until_whitespace = false;
            }
            self.update_sentence_state(delimiter);
            return EngineAction::Replace {
                backspaces,
                text: out,
            };
        }

        let pre_delim_display = self.display.clone();
        if active_vietnamese
            && !self.bypass_until_whitespace
            && !self.word_bypassed
            && self.config.spelling_check
            && self.had_transform
            && self.normalize_open_u_ohorn_word(&raw_str)
        {
            display_str = self.current_word();
        }

        if active_vietnamese
            && !self.bypass_until_whitespace
            && !self.word_bypassed
            && self.config.auto_restore_wrong_word
            && self.had_transform
            && should_auto_restore(
                &self.config,
                &self.display,
                &display_str,
                &raw_str,
                self.has_any_revert(),
                self.reverted_circumflex,
                delimiter,
            )
        {
            let backspaces = encoded_len(self.config.charset, &pre_delim_display);
            let mut out = raw_str;
            out.push(delimiter);
            self.display = self.raw.clone();
            self.char_raw_starts = (0..self.raw.len()).collect();
            self.had_transform = false;
            self.push_history(delimiter);
            self.clear_active_word();
            self.can_pop_history = true;
            self.last_delim = Some(delimiter);
            if delimiter.is_whitespace() {
                self.bypass_until_whitespace = false;
            }
            self.update_sentence_state(delimiter);
            return EngineAction::Replace {
                backspaces,
                text: out,
            };
        }

        let norm_action = if self.display != pre_delim_display {
            match self.diff_action(&pre_delim_display) {
                EngineAction::Replace { backspaces, mut text } => {
                    text.push(delimiter);
                    EngineAction::Replace { backspaces, text }
                }
                EngineAction::PassThrough => EngineAction::PassThrough,
            }
        } else {
            EngineAction::PassThrough
        };

        self.push_history(delimiter);
        self.clear_active_word();
        self.can_pop_history = true;
        self.last_delim = Some(delimiter);
        if delimiter.is_whitespace() {
            self.bypass_until_whitespace = false;
        }
        self.update_sentence_state(delimiter);
        norm_action
    }

    fn should_handle_as_delimiter(&self, ch: char) -> bool {
        if !Self::is_word_delimiter(ch) {
            return false;
        }
        if self.bypass_until_whitespace {
            return true;
        }
        let active_vietnamese = self.is_vietnamese_active();
        if active_vietnamese && self.config.quick_brackets {
            let supports_brackets = matches!(
                self.config.input_method,
                InputMethod::Telex | InputMethod::Custom
            );
            if supports_brackets && matches!(ch, '[' | ']' | '{' | '}') {
                return false;
            }
        }
        if active_vietnamese && self.config.input_method == InputMethod::Viqr
            && matches!(ch, '\'' | '`' | '?' | '~' | '.' | '-' | '^' | '+' | '(') {
                return false;
            }
        if active_vietnamese && self.config.input_method == InputMethod::Custom
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

    fn should_trigger_word_bypass(&self, ch: char) -> bool {
        if self.raw.is_empty() {
            return false;
        }
        if ch.is_ascii_uppercase() && self.raw.iter().any(|c| c.is_ascii_lowercase()) {
            return true;
        }
        if matches!(
            self.config.input_method,
            InputMethod::Vni | InputMethod::Custom
        ) {
            return false;
        }
        (ch.is_ascii_digit() && self.raw.iter().any(|c| c.is_ascii_alphabetic()))
            || (ch.is_ascii_alphabetic() && self.raw.iter().any(|c| c.is_ascii_digit()))
    }

    fn looks_like_code_word(raw: &[char]) -> bool {
        let has_lower = raw.iter().any(|c| c.is_ascii_lowercase());
        let has_upper_after_lower = raw
            .iter()
            .skip_while(|c| !c.is_ascii_lowercase())
            .any(|c| c.is_ascii_uppercase());
        let has_alpha = raw.iter().any(|c| c.is_ascii_alphabetic());
        let has_digit = raw.iter().any(|c| c.is_ascii_digit());
        (has_lower && has_upper_after_lower) || (has_alpha && has_digit)
    }

    fn should_trigger_delimiter_bypass(&self, delimiter: char) -> bool {
        match delimiter {
            '_' | '@' => true,
            '\\' => {
                self.display.is_empty()
                    || self.last_delim == Some(':')
                    || self.last_delim == Some('\\')
                    || self.raw.iter().all(|c| c.is_ascii())
            }
            '/' => {
                self.display.is_empty()
                    && matches!(self.last_delim, Some(':') | Some('.') | Some('~') | Some('/'))
            }
            '.' => {
                self.raw_word().eq_ignore_ascii_case("www")
                    && self.last_delim.is_none_or(|d| d.is_whitespace())
            }
            _ => false,
        }
    }

    fn push_history(&mut self, delimiter: char) {
        if self.history.len() >= 16 {
            self.history.pop_front();
        }
        let showing_raw = self.display == self.raw;
        self.history.push_back(CommittedWord {
            transformed: self.display.clone(),
            raw: self.raw.clone(),
            char_raw_starts: self.char_raw_starts.clone(),
            had_transform: self.had_transform,
            delimiter,
            showing_raw,
        });
    }

    fn clear_active_word(&mut self) {
        self.display.clear();
        self.raw.clear();
        self.char_raw_starts.clear();
        self.had_transform = false;
        self.word_bypassed = false;
        self.stashed_undo_display = None;
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

    fn normalize_open_u_ohorn_word(&mut self, raw_str: &str) -> bool {
        if raw_str.to_ascii_lowercase().starts_with('w') || crate::dict::is_english_word(raw_str) {
            return false;
        }
        let Some(parsed) = crate::syllable::parse_syllable(&self.display, self.config.allow_fjwz) else {
            return false;
        };
        if !parsed.coda.is_empty() || parsed.vowels.len() != 2 {
            return false;
        }
        let (b0, t0, u0) = parsed.vowels[0];
        let (b1, _, _) = parsed.vowels[1];
        if b0 != crate::tables::BaseVowel::UHorn || b1 != crate::tables::BaseVowel::OHorn {
            return false;
        }
        let mut candidate = self.display.clone();
        candidate[parsed.vowel_start] =
            crate::tables::encode_vowel(crate::tables::BaseVowel::U, t0, u0);
        let cand_str: String = candidate.iter().collect();
        if crate::dict::is_vietnamese_word_with_custom(
            &cand_str,
            self.config.allow_fjwz,
            &self.config.user_dict_words,
        ) {
            self.display = candidate;
            return true;
        }
        false
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
