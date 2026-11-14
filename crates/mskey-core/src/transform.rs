use crate::config::{Config, InputMethod};
use crate::syllable::{is_valid_typing_prefix, parse_syllable, tone_target_index};
use crate::tables::{
    BaseVowel, Tone, VowelFamily, base_family, decode_vowel, encode_vowel, is_d_or_d_stroke,
    to_d_stroke, to_plain_d, toggle_breve, toggle_circumflex, toggle_horn,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRole {
    Normal,
    Tone(Tone),
    RemoveTone,
    Circumflex(Option<VowelFamily>),
    Breve,
    Horn,
    HornOrBreve,
    DStroke,
    QuickOHornBracket,
    QuickUHornBracket,
}

pub fn classify_key(ch: char, config: &Config) -> KeyRole {
    let lower = ch.to_ascii_lowercase();
    match config.input_method {
        InputMethod::Telex => classify_telex(ch, lower, config.quick_brackets),
        InputMethod::SimpleTelex => classify_simple_telex(lower),
        InputMethod::Vni => classify_vni(ch),
        InputMethod::Viqr => classify_viqr(ch, lower),
        InputMethod::Custom => classify_custom(ch, lower, config),
    }
}

fn classify_telex(ch: char, lower: char, quick_brackets: bool) -> KeyRole {
    if quick_brackets {
        if ch == '[' || ch == '{' {
            return KeyRole::QuickOHornBracket;
        }
        if ch == ']' || ch == '}' {
            return KeyRole::QuickUHornBracket;
        }
    }
    match lower {
        's' => KeyRole::Tone(Tone::Acute),
        'f' => KeyRole::Tone(Tone::Grave),
        'r' => KeyRole::Tone(Tone::HookAbove),
        'x' => KeyRole::Tone(Tone::Tilde),
        'j' => KeyRole::Tone(Tone::DotBelow),
        'z' => KeyRole::RemoveTone,
        'a' => KeyRole::Circumflex(Some(VowelFamily::A)),
        'e' => KeyRole::Circumflex(Some(VowelFamily::E)),
        'o' => KeyRole::Circumflex(Some(VowelFamily::O)),
        'w' => KeyRole::HornOrBreve,
        'd' => KeyRole::DStroke,
        _ => KeyRole::Normal,
    }
}

fn classify_simple_telex(lower: char) -> KeyRole {
    match lower {
        's' => KeyRole::Tone(Tone::Acute),
        'f' => KeyRole::Tone(Tone::Grave),
        'r' => KeyRole::Tone(Tone::HookAbove),
        'x' => KeyRole::Tone(Tone::Tilde),
        'j' => KeyRole::Tone(Tone::DotBelow),
        'z' => KeyRole::RemoveTone,
        'a' => KeyRole::Circumflex(Some(VowelFamily::A)),
        'e' => KeyRole::Circumflex(Some(VowelFamily::E)),
        'o' => KeyRole::Circumflex(Some(VowelFamily::O)),
        'w' => KeyRole::HornOrBreve,
        'd' => KeyRole::DStroke,
        _ => KeyRole::Normal,
    }
}

fn classify_vni(ch: char) -> KeyRole {
    match ch {
        '1' => KeyRole::Tone(Tone::Acute),
        '2' => KeyRole::Tone(Tone::Grave),
        '3' => KeyRole::Tone(Tone::HookAbove),
        '4' => KeyRole::Tone(Tone::Tilde),
        '5' => KeyRole::Tone(Tone::DotBelow),
        '0' => KeyRole::RemoveTone,
        '6' => KeyRole::Circumflex(None),
        '7' => KeyRole::Horn,
        '8' => KeyRole::Breve,
        '9' => KeyRole::DStroke,
        _ => KeyRole::Normal,
    }
}

fn classify_viqr(ch: char, lower: char) -> KeyRole {
    match ch {
        '\'' => KeyRole::Tone(Tone::Acute),
        '`' => KeyRole::Tone(Tone::Grave),
        '?' => KeyRole::Tone(Tone::HookAbove),
        '~' => KeyRole::Tone(Tone::Tilde),
        '.' => KeyRole::Tone(Tone::DotBelow),
        '-' => KeyRole::RemoveTone,
        '^' => KeyRole::Circumflex(None),
        '+' => KeyRole::Horn,
        '(' => KeyRole::Breve,
        _ if lower == 'd' => KeyRole::DStroke,
        _ => KeyRole::Normal,
    }
}

fn classify_custom(ch: char, lower: char, config: &Config) -> KeyRole {
    let map = &config.custom_keymap;
    if config.quick_brackets {
        if ch == map.quick_o_horn_key {
            return KeyRole::QuickOHornBracket;
        }
        if ch == map.quick_u_horn_key {
            return KeyRole::QuickUHornBracket;
        }
    }
    if lower == map.acute.to_ascii_lowercase() {
        KeyRole::Tone(Tone::Acute)
    } else if lower == map.grave.to_ascii_lowercase() {
        KeyRole::Tone(Tone::Grave)
    } else if lower == map.hook.to_ascii_lowercase() {
        KeyRole::Tone(Tone::HookAbove)
    } else if lower == map.tilde.to_ascii_lowercase() {
        KeyRole::Tone(Tone::Tilde)
    } else if lower == map.dot.to_ascii_lowercase() {
        KeyRole::Tone(Tone::DotBelow)
    } else if lower == map.remove_tone.to_ascii_lowercase() {
        KeyRole::RemoveTone
    } else if lower == map.circumflex_a.to_ascii_lowercase() {
        KeyRole::Circumflex(Some(VowelFamily::A))
    } else if lower == map.circumflex_e.to_ascii_lowercase() {
        KeyRole::Circumflex(Some(VowelFamily::E))
    } else if lower == map.circumflex_o.to_ascii_lowercase() {
        KeyRole::Circumflex(Some(VowelFamily::O))
    } else if lower == map.horn_o.to_ascii_lowercase()
        && lower == map.breve_a.to_ascii_lowercase()
    {
        KeyRole::HornOrBreve
    } else if lower == map.horn_o.to_ascii_lowercase() || lower == map.horn_u.to_ascii_lowercase() {
        KeyRole::Horn
    } else if lower == map.breve_a.to_ascii_lowercase() {
        KeyRole::Breve
    } else if lower == map.d_stroke.to_ascii_lowercase() {
        KeyRole::DStroke
    } else {
        KeyRole::Normal
    }
}

pub enum TransformOutcome {
    Applied,
    Reverted,
    Ignored,
}

pub fn try_apply_d_stroke(
    chars: &mut Vec<char>,
    free_tone: bool,
    spelling_check: bool,
    allow_fjwz: bool,
) -> TransformOutcome {
    if chars.is_empty() || !is_d_or_d_stroke(chars[0]) {
        return TransformOutcome::Ignored;
    }
    if !free_tone && chars.len() > 1 {
        return TransformOutcome::Ignored;
    }
    let upper = chars[0].is_uppercase();
    if chars[0] == 'đ' || chars[0] == 'Đ' {
        chars[0] = to_plain_d(upper);
        return TransformOutcome::Reverted;
    }
    if spelling_check && chars.len() > 1 {
        let mut candidate = chars.clone();
        candidate[0] = to_d_stroke(upper);
        let Some(parsed) = parse_syllable(&candidate, allow_fjwz) else {
            return TransformOutcome::Ignored;
        };
        if !is_valid_typing_prefix(&parsed, allow_fjwz) {
            return TransformOutcome::Ignored;
        }
    }
    chars[0] = to_d_stroke(upper);
    TransformOutcome::Applied
}

pub fn try_apply_circumflex(
    chars: &mut Vec<char>,
    family_filter: Option<VowelFamily>,
    config: &Config,
) -> TransformOutcome {
    let mut candidate = chars.clone();
    let parsed = parse_syllable(&candidate, config.allow_fjwz);
    if config.spelling_check && parsed.is_none() {
        return TransformOutcome::Ignored;
    }
    let indices = vowel_scan_order(&candidate, parsed.as_ref().map(|p| p.vowel_start), config.free_tone);
    for idx in indices {
        let Some((base, tone, upper)) = decode_vowel(candidate[idx]) else {
            continue;
        };
        if let Some(target_fam) = family_filter {
            if base_family(base) != target_fam {
                continue;
            }
        }
        let Some((next_base, applied)) = toggle_circumflex(base) else {
            continue;
        };
        if !applied {
            candidate[idx] = encode_vowel(next_base, tone, upper);
            *chars = candidate;
            return TransformOutcome::Reverted;
        }
        candidate[idx] = encode_vowel(next_base, tone, upper);
        if validate_candidate(&candidate, config) {
            reanchor_tone(&mut candidate, config);
            *chars = candidate;
            return TransformOutcome::Applied;
        }
        candidate[idx] = chars[idx];
    }
    TransformOutcome::Ignored
}

pub fn try_apply_breve(chars: &mut Vec<char>, config: &Config) -> TransformOutcome {
    let mut candidate = chars.clone();
    let parsed = parse_syllable(&candidate, config.allow_fjwz);
    if config.spelling_check && parsed.is_none() {
        return TransformOutcome::Ignored;
    }
    let indices = vowel_scan_order(&candidate, parsed.as_ref().map(|p| p.vowel_start), config.free_tone);
    for idx in indices {
        let Some((base, tone, upper)) = decode_vowel(candidate[idx]) else {
            continue;
        };
        if base_family(base) != VowelFamily::A {
            continue;
        }
        let Some((next_base, applied)) = toggle_breve(base) else {
            continue;
        };
        if !applied {
            candidate[idx] = encode_vowel(next_base, tone, upper);
            *chars = candidate;
            return TransformOutcome::Reverted;
        }
        candidate[idx] = encode_vowel(next_base, tone, upper);
        if validate_candidate(&candidate, config) {
            reanchor_tone(&mut candidate, config);
            *chars = candidate;
            return TransformOutcome::Applied;
        }
        candidate[idx] = chars[idx];
    }
    TransformOutcome::Ignored
}

pub fn try_apply_horn(chars: &mut Vec<char>, config: &Config) -> TransformOutcome {
    if let Some(outcome) = try_apply_uo_compound_horn(chars, config) {
        return outcome;
    }
    let mut candidate = chars.clone();
    let parsed = parse_syllable(&candidate, config.allow_fjwz);
    if config.spelling_check && parsed.is_none() {
        return TransformOutcome::Ignored;
    }
    let indices = vowel_scan_order(&candidate, parsed.as_ref().map(|p| p.vowel_start), config.free_tone);
    for idx in indices {
        let Some((base, tone, upper)) = decode_vowel(candidate[idx]) else {
            continue;
        };
        if !matches!(base_family(base), VowelFamily::O | VowelFamily::U) {
            continue;
        }
        if idx == 1 && candidate[0].eq_ignore_ascii_case(&'q') && base_family(base) == VowelFamily::U {
            continue;
        }
        let Some((next_base, applied)) = toggle_horn(base) else {
            continue;
        };
        if !applied {
            candidate[idx] = encode_vowel(next_base, tone, upper);
            *chars = candidate;
            return TransformOutcome::Reverted;
        }
        candidate[idx] = encode_vowel(next_base, tone, upper);
        if validate_candidate(&candidate, config) {
            reanchor_tone(&mut candidate, config);
            *chars = candidate;
            return TransformOutcome::Applied;
        }
        candidate[idx] = chars[idx];
    }
    TransformOutcome::Ignored
}

fn try_apply_uo_compound_horn(chars: &mut Vec<char>, config: &Config) -> Option<TransformOutcome> {
    let parsed = parse_syllable(chars, config.allow_fjwz)?;
    if parsed.vowels.len() < 2 {
        return None;
    }
    let vs = parsed.vowel_start;
    for pos in 0..parsed.vowels.len() - 1 {
        let (b0, t0, u0) = parsed.vowels[pos];
        let (b1, t1, u1) = parsed.vowels[pos + 1];
        if base_family(b0) == VowelFamily::U && base_family(b1) == VowelFamily::O {
            let i0 = vs + pos;
            let i1 = vs + pos + 1;
            if !config.free_tone && i1 + 1 < chars.len() {
                continue;
            }
            let mut candidate = chars.clone();
            if b0 == BaseVowel::UHorn && b1 == BaseVowel::OHorn {
                candidate[i0] = encode_vowel(BaseVowel::U, t0, u0);
                candidate[i1] = encode_vowel(BaseVowel::O, t1, u1);
                *chars = candidate;
                return Some(TransformOutcome::Reverted);
            }
            candidate[i0] = encode_vowel(BaseVowel::UHorn, t0, u0);
            candidate[i1] = encode_vowel(BaseVowel::OHorn, t1, u1);
            if validate_candidate(&candidate, config) {
                reanchor_tone(&mut candidate, config);
                *chars = candidate;
                return Some(TransformOutcome::Applied);
            }
        }
    }
    None
}

pub fn try_apply_tone(chars: &mut Vec<char>, target_tone: Tone, config: &Config) -> TransformOutcome {
    let Some(parsed) = parse_syllable(chars, config.allow_fjwz) else {
        return TransformOutcome::Ignored;
    };
    if parsed.vowels.is_empty() {
        return TransformOutcome::Ignored;
    }
    if target_tone == Tone::None {
        if parsed.active_tone == Tone::None {
            return TransformOutcome::Ignored;
        }
        clear_all_tones(chars, &parsed);
        return TransformOutcome::Applied;
    }
    if parsed.active_tone == target_tone {
        clear_all_tones(chars, &parsed);
        return TransformOutcome::Reverted;
    }
    let Some(v_rel_idx) = tone_target_index(&parsed, config.modern_tone) else {
        return TransformOutcome::Ignored;
    };
    let mut candidate = chars.clone();
    clear_all_tones(&mut candidate, &parsed);
    let abs_idx = parsed.vowel_start + v_rel_idx;
    if let Some((base, _, upper)) = decode_vowel(candidate[abs_idx]) {
        candidate[abs_idx] = encode_vowel(base, target_tone, upper);
    }
    if !validate_candidate(&candidate, config) {
        return TransformOutcome::Ignored;
    }
    *chars = candidate;
    TransformOutcome::Applied
}

pub fn reanchor_tone(chars: &mut Vec<char>, config: &Config) {
    let Some(parsed) = parse_syllable(chars, config.allow_fjwz) else {
        return;
    };
    if parsed.active_tone == Tone::None || parsed.vowels.is_empty() {
        return;
    }
    let tone = parsed.active_tone;
    let Some(target_rel) = tone_target_index(&parsed, config.modern_tone) else {
        return;
    };
    clear_all_tones(chars, &parsed);
    let abs_idx = parsed.vowel_start + target_rel;
    if let Some((base, _, upper)) = decode_vowel(chars[abs_idx]) {
        chars[abs_idx] = encode_vowel(base, tone, upper);
    }
}

fn clear_all_tones(chars: &mut Vec<char>, parsed: &crate::syllable::ParsedSyllable) {
    for offset in 0..parsed.vowels.len() {
        let idx = parsed.vowel_start + offset;
        if let Some((base, _, upper)) = decode_vowel(chars[idx]) {
            chars[idx] = encode_vowel(base, Tone::None, upper);
        }
    }
}

fn validate_candidate(candidate: &[char], config: &Config) -> bool {
    if !config.spelling_check {
        return true;
    }
    let Some(parsed) = parse_syllable(candidate, config.allow_fjwz) else {
        return false;
    };
    is_valid_typing_prefix(&parsed, config.allow_fjwz)
}

fn vowel_scan_order(chars: &[char], vowel_start: Option<usize>, free_tone: bool) -> Vec<usize> {
    if chars.is_empty() {
        return Vec::new();
    }
    if free_tone {
        let start = vowel_start.unwrap_or(0);
        return (start..chars.len()).rev().collect();
    }
    let mut order = Vec::new();
    order.push(chars.len() - 1);
    if chars.len() >= 2 && decode_vowel(chars[chars.len() - 2]).is_some() {
        order.push(chars.len() - 2);
    }
    order
}
