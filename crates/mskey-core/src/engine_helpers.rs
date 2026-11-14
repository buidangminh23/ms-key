use crate::config::{Charset, Config};
use crate::converter::encode_char_for_charset;
use crate::dict;
use crate::syllable::{is_valid_complete_syllable, parse_syllable};

pub(crate) fn match_macro(config: &Config, word: &str) -> Option<String> {
    let entry = config
        .macros
        .iter()
        .find(|m| m.enabled && m.shortcut.eq_ignore_ascii_case(word))?;
    let mut chars = word.chars();
    let all_upper = word.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
    if all_upper && word.chars().any(|c| c.is_alphabetic()) {
        return Some(entry.expansion.to_uppercase());
    }
    if chars.next().is_some_and(|c| c.is_uppercase()) {
        let mut exp_chars = entry.expansion.chars();
        if let Some(first) = exp_chars.next() {
            let mut out: String = first.to_uppercase().collect();
            out.extend(exp_chars);
            return Some(out);
        }
    }
    Some(entry.expansion.clone())
}

pub(crate) fn should_auto_restore(
    config: &Config,
    display: &[char],
    display_str: &str,
    raw_str: &str,
    has_reverted_circumflex: bool,
) -> bool {
    if display_str == raw_str {
        return false;
    }
    if has_reverted_circumflex
        && dict::is_english_word(display_str)
        && !dict::is_english_word(raw_str)
    {
        return false;
    }
    let Some(parsed) = parse_syllable(display, config.allow_fjwz) else {
        return true;
    };
    if !is_valid_complete_syllable(&parsed, config.allow_fjwz) {
        return true;
    }
    !dict::is_vietnamese_word(display_str, config.allow_fjwz)
        && dict::is_english_word(raw_str)
        && !dict::is_keep_word(raw_str)
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

pub(crate) fn old_display_plus(old: &[char], ch: char) -> Vec<char> {
    let mut v = old.to_vec();
    v.push(ch);
    v
}
