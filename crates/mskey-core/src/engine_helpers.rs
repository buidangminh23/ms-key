use crate::config::{Charset, Config};
use crate::converter::encode_char_for_charset;
use crate::dict;
use crate::engine_quick::expand_macro_variables;
use crate::syllable::{is_valid_complete_syllable, parse_syllable};
use crate::tables::{Tone, decode_vowel, strip_vowel_diacritic};

pub(crate) fn match_macro(
    config: &Config,
    display_str: &str,
    raw_str: &str,
    clock_offset_secs: i64,
) -> Option<String> {
    let (entry, word) = if let Some(e) = config
        .macros
        .iter()
        .find(|m| m.enabled && m.shortcut.eq_ignore_ascii_case(display_str))
    {
        (e, display_str)
    } else if raw_str != display_str {
        let e = config
            .macros
            .iter()
            .find(|m| m.enabled && m.shortcut.eq_ignore_ascii_case(raw_str))?;
        (e, raw_str)
    } else {
        return None;
    };
    let expanded = expand_macro_variables(&entry.expansion, clock_offset_secs);
    let mut chars = word.chars();
    let all_upper = word.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
    if all_upper && word.chars().any(|c| c.is_alphabetic()) {
        return Some(expanded.to_uppercase());
    }
    if chars.next().is_some_and(|c| c.is_uppercase()) {
        let mut exp_chars = expanded.chars();
        if let Some(first) = exp_chars.next() {
            let mut out: String = first.to_uppercase().collect();
            out.extend(exp_chars);
            return Some(out);
        }
    }
    Some(expanded)
}

pub(crate) fn should_auto_restore(
    config: &Config,
    display: &[char],
    display_str: &str,
    raw_str: &str,
    has_reverted: bool,
) -> bool {
    if display_str == raw_str {
        return false;
    }
    if has_reverted
        && !has_vietnamese_diacritics(display)
        && !dict::is_english_word(raw_str)
    {
        return false;
    }
    if dict::is_vietnamese_word_with_custom(
        display_str,
        config.allow_fjwz,
        &config.user_dict_words,
    ) {
        return false;
    }
    let Some(parsed) = parse_syllable(display, config.allow_fjwz) else {
        return true;
    };
    if !is_valid_complete_syllable(&parsed, config.allow_fjwz) {
        return true;
    }
    dict::is_english_word(raw_str) && !dict::is_keep_word(raw_str)
}

fn has_vietnamese_diacritics(display: &[char]) -> bool {
    display.iter().any(|&c| {
        matches!(c, 'đ' | 'Đ')
            || decode_vowel(c).is_some_and(|(b, t, _)| t != Tone::None || strip_vowel_diacritic(b) != b)
    })
}

pub(crate) fn encode_string(charset: Charset, text: &str) -> String {
    text.chars()
        .map(|c| encode_char_for_charset(c, charset))
        .collect()
}

pub(crate) fn encoded_len(charset: Charset, slice: &[char]) -> usize {
    slice
        .iter()
        .map(|&c| encode_char_for_charset(c, charset).chars().count())
        .sum()
}
