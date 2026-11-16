use crate::config::{Charset, Config, InputMethod};
use crate::converter::encode_char_for_charset;
use crate::dict;
use crate::engine_quick::{MacroExpansion, expand_macro_variables_with_context};
use crate::syllable::{is_valid_complete_syllable, parse_syllable};
use crate::tables::{BaseVowel, Tone, decode_vowel, strip_vowel_diacritic};

pub(crate) fn match_macro(
    config: &Config,
    display_str: &str,
    raw_str: &str,
    clock_offset_secs: i64,
    clipboard: Option<&str>,
) -> Option<MacroExpansion> {
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
    let expanded =
        expand_macro_variables_with_context(&entry.expansion, clock_offset_secs, clipboard);
    let mut chars = word.chars();
    let all_upper = word.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
    if all_upper && word.chars().any(|c| c.is_alphabetic()) {
        if let Some(pos) = expanded.cursor_char_idx {
            let before: String = expanded.text.chars().take(pos).collect();
            let after: String = expanded.text.chars().skip(pos).collect();
            let upper_before = before.to_uppercase();
            let upper_after = after.to_uppercase();
            let new_pos = upper_before.chars().count();
            return Some(MacroExpansion {
                text: format!("{upper_before}{upper_after}"),
                cursor_char_idx: Some(new_pos),
            });
        }
        return Some(MacroExpansion {
            text: expanded.text.to_uppercase(),
            cursor_char_idx: None,
        });
    }
    if chars.next().is_some_and(|c| c.is_uppercase()) {
        let mut exp_chars = expanded.text.chars();
        if let Some(first) = exp_chars.next() {
            let first_upper: String = first.to_uppercase().collect();
            let delta = first_upper.chars().count().saturating_sub(1);
            let mut out = first_upper;
            out.extend(exp_chars);
            let adjusted_pos = expanded
                .cursor_char_idx
                .map(|p| if p == 0 { 0 } else { p + delta });
            return Some(MacroExpansion {
                text: out,
                cursor_char_idx: adjusted_pos,
            });
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
    reverted_circumflex: Option<crate::tables::VowelFamily>,
    delimiter: char,
) -> bool {
    if display_str == raw_str {
        return false;
    }
    if is_standalone_marked_letter(display) && raw_str.chars().count() <= 2 {
        return false;
    }
    if delimiter == '\'' && is_english_contraction_stem(raw_str) {
        return true;
    }
    if has_reverted
        && !has_vietnamese_diacritics(display)
        && !dict::is_english_word(raw_str)
    {
        return false;
    }
    if let Some(fam) = reverted_circumflex
        && !dict::is_english_word(raw_str)
        && has_reverted_circumflex_pair(display, fam)
    {
        return false;
    }
    if dict::is_user_word(display_str, &config.user_dict_words) {
        return false;
    }
    if dict::is_user_word(raw_str, &config.user_dict_words) {
        return true;
    }
    if dict::is_vietnamese_word_with_custom(
        display_str,
        config.allow_fjwz,
        &config.user_dict_words,
    ) {
        return is_non_canonical_english_collision(config, display, raw_str);
    }
    let Some(parsed) = parse_syllable(display, config.allow_fjwz) else {
        return true;
    };
    if matches!(parsed.onset.as_str(), "f" | "j" | "w" | "z") {
        return true;
    }
    if !is_valid_complete_syllable(&parsed, config.allow_fjwz) {
        return true;
    }
    dict::is_english_word(raw_str) && !dict::is_keep_word(raw_str)
}

fn is_english_contraction_stem(raw_str: &str) -> bool {
    let lower = raw_str.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "didn"
            | "doesn"
            | "don"
            | "hadn"
            | "hasn"
            | "haven"
            | "isn"
            | "aren"
            | "wasn"
            | "weren"
            | "wouldn"
            | "couldn"
            | "shouldn"
            | "mustn"
            | "needn"
            | "ain"
    )
}

fn has_reverted_circumflex_pair(display: &[char], fam: crate::tables::VowelFamily) -> bool {
    let target = match fam {
        crate::tables::VowelFamily::A => 'a',
        crate::tables::VowelFamily::E => 'e',
        crate::tables::VowelFamily::O => 'o',
        _ => return false,
    };
    display
        .windows(2)
        .any(|w| w[0].to_ascii_lowercase() == target && w[1].to_ascii_lowercase() == target)
}

fn is_non_canonical_english_collision(
    config: &Config,
    display: &[char],
    raw_str: &str,
) -> bool {
    if !matches!(
        config.input_method,
        InputMethod::Telex | InputMethod::SimpleTelex
    ) || !raw_str.is_ascii()
        || !dict::is_english_word(raw_str)
        || dict::is_keep_word(raw_str)
    {
        return false;
    }
    let lower = raw_str.to_ascii_lowercase();
    let starts_u_horn = display
        .first()
        .is_some_and(|&c| decode_vowel(c).is_some_and(|(b, _, _)| b == BaseVowel::UHorn));
    if lower.len() >= 2 && lower.starts_with('w') && starts_u_horn {
        return true;
    }
    let bytes = lower.as_bytes();
    let Some(first_vowel) = bytes
        .iter()
        .position(|b| matches!(b, b'a' | b'e' | b'i' | b'o' | b'u' | b'y'))
    else {
        return false;
    };
    let is_tone_key = |b: u8| matches!(b, b's' | b'f' | b'r' | b'x' | b'j');
    let is_vowel_key = |b: u8| matches!(b, b'a' | b'e' | b'i' | b'o' | b'u' | b'y');
    let mut tone_count = 0usize;
    for (idx, &b) in bytes.iter().enumerate().skip(first_vowel + 1) {
        if is_tone_key(b) {
            tone_count += 1;
            if bytes[idx + 1..].iter().any(|&next| is_vowel_key(next)) {
                return true;
            }
        }
    }
    if tone_count >= 2 {
        return true;
    }
    let has_d_stroke = display.iter().any(|&c| matches!(c, 'đ' | 'Đ'));
    if has_d_stroke && !lower.contains("dd") {
        return true;
    }
    let has_circumflex = |target: BaseVowel| {
        display
            .iter()
            .any(|&c| decode_vowel(c).is_some_and(|(base, _, _)| base == target))
    };
    (has_circumflex(BaseVowel::ACircumflex) && !lower.contains("aa"))
        || (has_circumflex(BaseVowel::ECircumflex) && !lower.contains("ee"))
        || (has_circumflex(BaseVowel::OCircumflex) && !lower.contains("oo"))
}

fn is_standalone_marked_letter(display: &[char]) -> bool {
    let [c] = display else {
        return false;
    };
    matches!(c, 'đ' | 'Đ')
        || decode_vowel(*c).is_some_and(|(b, t, _)| t == Tone::None && strip_vowel_diacritic(b) != b)
}

pub(crate) fn has_vietnamese_diacritics(display: &[char]) -> bool {
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
