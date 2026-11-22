use crate::config::InputMethod;
use crate::engine::Engine;
use crate::engine_quick::promote_u_ohorn_for_suffix;
use crate::syllable::{is_valid_typing_prefix, parse_syllable};
use crate::tables::{BaseVowel, Tone, VowelFamily, decode_vowel, is_vietnamese_vowel};
use crate::transform::{
    KeyRole, TransformOutcome, classify_key, reanchor_tone, try_apply_breve, try_apply_circumflex,
    try_apply_d_stroke, try_apply_horn_family, try_apply_tone,
};

impl Engine {
    pub(crate) fn try_bracket_shortcut(
        &mut self,
        ch: char,
        old_display: &[char],
    ) -> Option<crate::engine::EngineAction> {
        let supports = matches!(
            self.config.input_method,
            InputMethod::Telex | InputMethod::Custom
        );
        if !supports || !self.config.quick_brackets {
            return None;
        }
        let target = self.bracket_target_char(ch)?;
        if self.last_bracket == Some(ch) && self.display.last().copied() == Some(target) {
            self.display.pop();
            self.display.push(ch);
            self.last_bracket = None;
            self.reverted_bracket = Some(ch);
            self.raw.push(ch);
            if self.display.len() == 1 {
                self.raw = self.display.clone();
                self.char_raw_starts = vec![0];
                self.had_transform = false;
            }
            return Some(self.diff_action(old_display));
        }
        if self.reverted_bracket == Some(ch) {
            self.display.push(ch);
            self.raw.push(ch);
            self.char_raw_starts.push(self.raw.len() - 1);
            self.last_bracket = None;
            return Some(crate::engine::EngineAction::PassThrough);
        }
        self.raw.push(ch);
        self.char_raw_starts.push(self.raw.len() - 1);
        self.display.push(target);
        self.had_transform = true;
        self.last_bracket = Some(ch);
        self.last_w_standalone = false;
        self.auto_o_horn = false;
        reanchor_tone(&mut self.display, &self.config);
        Some(self.diff_action(old_display))
    }

    fn bracket_target_char(&self, ch: char) -> Option<char> {
        if self.config.input_method == InputMethod::Custom {
            let map = &self.config.custom_keymap;
            if ch == map.quick_o_horn_key {
                return Some(if ch.is_uppercase() { 'Ơ' } else { 'ơ' });
            }
            if map.quick_o_horn_key == '[' && ch == '{' {
                return Some('Ơ');
            }
            if map.quick_o_horn_key.is_ascii_lowercase()
                && ch == map.quick_o_horn_key.to_ascii_uppercase()
            {
                return Some('Ơ');
            }
            if ch == map.quick_u_horn_key {
                return Some(if ch.is_uppercase() { 'Ư' } else { 'ư' });
            }
            if map.quick_u_horn_key == ']' && ch == '}' {
                return Some('Ư');
            }
            if map.quick_u_horn_key.is_ascii_lowercase()
                && ch == map.quick_u_horn_key.to_ascii_uppercase()
            {
                return Some('Ư');
            }
            return None;
        }
        match ch {
            '[' => Some('ơ'),
            '{' => Some('Ơ'),
            ']' => Some('ư'),
            '}' => Some('Ư'),
            _ => None,
        }
    }

    pub(crate) fn apply_key_role(&mut self, ch: char, role: KeyRole) -> bool {
        self.last_bracket = None;
        if !matches!(
            role,
            KeyRole::Normal
                | KeyRole::HornOrBreve(_)
                | KeyRole::Horn(_)
                | KeyRole::Tone(_)
                | KeyRole::RemoveTone
        ) {
            self.auto_o_horn = false;
        }
        match role {
            KeyRole::Normal => false,
            KeyRole::DStroke => self.apply_d_stroke(ch),
            KeyRole::Circumflex(family) => self.apply_circumflex_role(ch, family),
            KeyRole::Breve => self.apply_breve_role(ch),
            KeyRole::Horn(family) => self.apply_horn_role(ch, family),
            KeyRole::HornOrBreve(family) => self.apply_w_key(ch, family),
            KeyRole::QuickUHornStandalone => self.try_standalone_u_horn(ch),
            KeyRole::Tone(tone) => self.apply_tone_role(ch, tone),
            KeyRole::RemoveTone => matches!(
                try_apply_tone(&mut self.display, Tone::None, &self.config),
                TransformOutcome::Applied
            ),
            KeyRole::QuickOHornBracket | KeyRole::QuickUHornBracket => false,
        }
    }

    fn sync_reverted_plain_ascii(&mut self) {
        let explicit_escape = self
            .raw
            .last_chunk::<2>()
            .is_some_and(|[first, second]| first.eq_ignore_ascii_case(second));
        if explicit_escape && self.display.iter().all(char::is_ascii) {
            self.raw = self.display.clone();
            self.char_raw_starts = (0..self.display.len()).collect();
            self.had_transform = false;
        }
    }

    fn apply_d_stroke(&mut self, ch: char) -> bool {
        if self.reverted_d {
            return false;
        }
        match try_apply_d_stroke(
            &mut self.display,
            self.config.free_tone,
            self.config.spelling_check,
            self.config.allow_fjwz,
        ) {
            TransformOutcome::Applied => {
                self.had_transform = true;
                true
            }
            TransformOutcome::Reverted => {
                self.reverted_d = true;
                self.display.push(ch);
                self.sync_reverted_plain_ascii();
                true
            }
            TransformOutcome::Ignored => false,
        }
    }

    fn apply_circumflex_role(&mut self, ch: char, family: Option<VowelFamily>) -> bool {
        let check_fam = family.or_else(|| self.last_vowel_family());
        if check_fam.is_some() && self.reverted_circumflex == check_fam {
            return false;
        }
        match try_apply_circumflex(&mut self.display, family, &self.config) {
            TransformOutcome::Applied => {
                self.had_transform = true;
                true
            }
            TransformOutcome::Reverted => {
                self.reverted_circumflex = family.or_else(|| self.last_vowel_family());
                self.display.push(ch);
                self.sync_reverted_plain_ascii();
                true
            }
            TransformOutcome::Ignored => false,
        }
    }

    fn apply_breve_role(&mut self, ch: char) -> bool {
        if self.reverted_breve {
            return false;
        }
        match try_apply_breve(&mut self.display, &self.config) {
            TransformOutcome::Applied => {
                self.had_transform = true;
                true
            }
            TransformOutcome::Reverted => {
                self.reverted_breve = true;
                self.display.push(ch);
                self.sync_reverted_plain_ascii();
                true
            }
            TransformOutcome::Ignored => false,
        }
    }

    fn apply_horn_role(&mut self, ch: char, family: Option<VowelFamily>) -> bool {
        if self.auto_o_horn {
            self.auto_o_horn = false;
            return true;
        }
        if self.reverted_horn {
            return false;
        }
        match try_apply_horn_family(&mut self.display, family, &self.config) {
            TransformOutcome::Applied => {
                self.had_transform = true;
                true
            }
            TransformOutcome::Reverted => {
                self.reverted_horn = true;
                self.display.push(ch);
                self.sync_reverted_plain_ascii();
                true
            }
            TransformOutcome::Ignored => false,
        }
    }

    fn apply_tone_role(&mut self, ch: char, tone: Tone) -> bool {
        if self.reverted_tone == Some(tone) {
            return false;
        }
        match try_apply_tone(&mut self.display, tone, &self.config) {
            TransformOutcome::Applied => {
                self.had_transform = true;
                true
            }
            TransformOutcome::Reverted => {
                self.reverted_tone = Some(tone);
                self.display.push(ch);
                true
            }
            TransformOutcome::Ignored => false,
        }
    }

    fn apply_w_key(&mut self, ch: char, horn_family: Option<VowelFamily>) -> bool {
        if self.auto_o_horn {
            self.auto_o_horn = false;
            self.last_w_standalone = false;
            return true;
        }
        if self.try_revert_standalone_u_horn(ch) {
            return true;
        }
        if !self.reverted_horn {
            match try_apply_horn_family(&mut self.display, horn_family, &self.config) {
                TransformOutcome::Applied => {
                    self.had_transform = true;
                    self.last_w_standalone = false;
                    return true;
                }
                TransformOutcome::Reverted => {
                    self.reverted_horn = true;
                    self.display.push(ch);
                    self.last_w_standalone = false;
                    self.sync_reverted_plain_ascii();
                    return true;
                }
                TransformOutcome::Ignored => {}
            }
        }
        if !self.reverted_breve {
            match try_apply_breve(&mut self.display, &self.config) {
                TransformOutcome::Applied => {
                    self.had_transform = true;
                    self.last_w_standalone = false;
                    return true;
                }
                TransformOutcome::Reverted => {
                    self.reverted_breve = true;
                    self.display.push(ch);
                    self.last_w_standalone = false;
                    self.sync_reverted_plain_ascii();
                    return true;
                }
                TransformOutcome::Ignored => {}
            }
        }
        self.try_standalone_u_horn(ch)
    }

    fn try_revert_standalone_u_horn(&mut self, ch: char) -> bool {
        if !(self.last_w_standalone && matches!(self.display.last(), Some('ư' | 'Ư'))) {
            return false;
        }
        let upper = self.display.pop().is_some_and(|c| c.is_uppercase());
        self.display.push(if upper {
            ch.to_ascii_uppercase()
        } else {
            ch.to_ascii_lowercase()
        });
        self.last_w_standalone = false;
        self.reverted_w_standalone = true;
        self.sync_reverted_plain_ascii();
        true
    }

    fn try_standalone_u_horn(&mut self, ch: char) -> bool {
        if self.try_revert_standalone_u_horn(ch) {
            return true;
        }
        let allow_custom = self.config.input_method == InputMethod::Custom
            && ch.eq_ignore_ascii_case(&self.config.custom_keymap.quick_u_horn);
        let allow_telex = self.config.input_method == InputMethod::Telex;
        let allow_standalone =
            (allow_telex || allow_custom) && self.config.quick_w_to_u_horn && !self.reverted_w_standalone;
        if allow_standalone && self.can_insert_w_as_u_horn() {
            let target = if ch.is_uppercase() { 'Ư' } else { 'ư' };
            self.display.push(target);
            self.had_transform = true;
            self.last_w_standalone = true;
            reanchor_tone(&mut self.display, &self.config);
            return true;
        }
        self.last_w_standalone = false;
        false
    }

    fn can_insert_w_as_u_horn(&self) -> bool {
        if self.display.is_empty() || !self.config.spelling_check {
            return true;
        }
        let mut candidate = self.display.clone();
        candidate.push('ư');
        parse_syllable(&candidate, self.config.allow_fjwz).is_some_and(|parsed| {
            parsed.vowels.len() == 1
                && parsed.onset != "p"
                && is_valid_typing_prefix(&parsed, self.config.allow_fjwz)
        })
    }

    pub(crate) fn append_normal_char(&mut self, ch: char) {
        self.last_w_standalone = false;
        if let [q, u, key] = self.display.as_slice()
            && q.eq_ignore_ascii_case(&'q')
            && u.eq_ignore_ascii_case(&'u')
            && is_vietnamese_vowel(ch)
            && matches!(self.config.input_method, InputMethod::Telex | InputMethod::SimpleTelex)
            && let KeyRole::Tone(tone) = classify_key(*key, &self.config)
        {
            let mut candidate = vec![*q, *u, ch];
            if matches!(
                try_apply_tone(&mut candidate, tone, &self.config),
                TransformOutcome::Applied
            ) {
                self.display = candidate;
                self.had_transform = true;
                return;
            }
        }
        if is_vietnamese_vowel(ch) && !matches!(ch.to_ascii_lowercase(), 'i' | 'u') {
            self.auto_o_horn = false;
        }
        if self.config.vietnamese_enabled
            && matches!(ch, 'o' | 'O')
            && !self.reverted_horn
            && self
                .display
                .last()
                .is_some_and(|&prev| decode_vowel(prev).is_some_and(|(b, _, _)| b == BaseVowel::UHorn))
        {
            self.display.push(if ch == 'O' { 'Ơ' } else { 'ơ' });
            self.had_transform = true;
            self.auto_o_horn = true;
            reanchor_tone(&mut self.display, &self.config);
            return;
        }
        let closes_ng = ch.eq_ignore_ascii_case(&'g')
            && self.display.last().is_some_and(|c| c.eq_ignore_ascii_case(&'n'));
        if self.config.vietnamese_enabled
            && (closes_ng || matches!(ch.to_ascii_lowercase(), 'n' | 'm' | 'c' | 't' | 'p' | 'i' | 'u'))
        {
            promote_u_ohorn_for_suffix(&mut self.display, &[ch], &self.config);
        }
        self.display.push(ch);
        if is_vietnamese_vowel(ch) {
            reanchor_tone(&mut self.display, &self.config);
        }
    }
}
