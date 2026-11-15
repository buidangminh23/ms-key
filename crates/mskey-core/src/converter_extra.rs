use crate::config::Charset;
use crate::converter::{
    TCVN3_LOWER, VNI_TABLE, apply_case_transform, convert_text, from_tcvn3, from_vni,
    normalize_nfd_to_nfc,
};
use crate::dict;
use crate::tables::{BaseVowel, Tone, base_family, decode_vowel, encode_vowel, strip_vowel_diacritic, VowelFamily};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertEncoding {
    Unicode,
    Tcvn3,
    VniWindows,
    UnicodeComposite,
    Viqr,
}

impl From<Charset> for ConvertEncoding {
    fn from(value: Charset) -> Self {
        match value {
            Charset::Unicode => Self::Unicode,
            Charset::Tcvn3 => Self::Tcvn3,
            Charset::VniWindows => Self::VniWindows,
            Charset::UnicodeComposite => Self::UnicodeComposite,
        }
    }
}

impl ConvertEncoding {
    pub fn to_charset(self) -> Option<Charset> {
        match self {
            Self::Unicode => Some(Charset::Unicode),
            Self::Tcvn3 => Some(Charset::Tcvn3),
            Self::VniWindows => Some(Charset::VniWindows),
            Self::UnicodeComposite => Some(Charset::UnicodeComposite),
            Self::Viqr => None,
        }
    }
}

pub fn strip_vietnamese_diacritics(input: &str) -> String {
    let nfc = normalize_nfd_to_nfc(input);
    let mut out = String::with_capacity(nfc.len());
    for ch in nfc.chars() {
        match ch {
            'đ' => out.push('d'),
            'Đ' => out.push('D'),
            '\u{0300}'..='\u{036F}' => {}
            _ => {
                if let Some((base, _, upper)) = decode_vowel(ch) {
                    out.push(encode_vowel(strip_vowel_diacritic(base), Tone::None, upper));
                } else {
                    out.push(ch);
                }
            }
        }
    }
    out
}

pub fn detect_charset(input: &str) -> Charset {
    match detect_convert_encoding(input) {
        ConvertEncoding::Tcvn3 => Charset::Tcvn3,
        ConvertEncoding::VniWindows => Charset::VniWindows,
        ConvertEncoding::UnicodeComposite => Charset::UnicodeComposite,
        ConvertEncoding::Unicode | ConvertEncoding::Viqr => Charset::Unicode,
    }
}

pub fn detect_convert_encoding(input: &str) -> ConvertEncoding {
    let mut has_non_ascii_latin1 = false;
    for ch in input.chars() {
        if is_combining_vi_mark(ch) {
            return ConvertEncoding::UnicodeComposite;
        }
        if (ch as u32) > 0x00FF {
            return ConvertEncoding::Unicode;
        }
        if !ch.is_ascii() {
            has_non_ascii_latin1 = true;
        }
    }
    if has_non_ascii_latin1 {
        return detect_latin1_encoding(input);
    }
    if count_viqr_cues(input) > 0 {
        ConvertEncoding::Viqr
    } else {
        ConvertEncoding::Unicode
    }
}

pub fn convert_with_encoding(
    input: &str,
    from: ConvertEncoding,
    to: ConvertEncoding,
    case_mode: crate::converter::CaseTransform,
) -> String {
    let unicode = match from {
        ConvertEncoding::Viqr => from_viqr(input),
        ConvertEncoding::Unicode => input.to_string(),
        ConvertEncoding::UnicodeComposite => normalize_nfd_to_nfc(input),
        ConvertEncoding::Tcvn3 => from_tcvn3(input),
        ConvertEncoding::VniWindows => from_vni(input),
    };
    let cased = apply_case_transform(&unicode, case_mode);
    match to.to_charset() {
        Some(cs) => convert_text(&cased, Charset::Unicode, cs, crate::converter::CaseTransform::None),
        None => to_viqr(&cased),
    }
}

pub fn to_viqr(unicode: &str) -> String {
    let mut out = String::with_capacity(unicode.len());
    for ch in unicode.chars() {
        match ch {
            'đ' => out.push_str("dd"),
            'Đ' => out.push_str("DD"),
            _ => {
                if let Some((base, tone, upper)) = decode_vowel(ch) {
                    out.push(encode_vowel(strip_vowel_diacritic(base), Tone::None, upper));
                    if let Some(m) = viqr_modifier_char(base) {
                        out.push(m);
                    }
                    if let Some(t) = viqr_tone_char(tone) {
                        out.push(t);
                    }
                } else {
                    out.push(ch);
                }
            }
        }
    }
    out
}

pub fn from_viqr(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            if matches!(next, '^' | '(' | '+' | '\'' | '`' | '?' | '~' | '.' | '\\') {
                out.push(next);
                i += 2;
                continue;
            }
        }
        if matches!(ch, 'd' | 'D')
            && i + 1 < chars.len()
            && matches!(chars[i + 1], 'd' | 'D')
        {
            out.push(if ch == 'D' { 'Đ' } else { 'đ' });
            i += 2;
            continue;
        }
        if let Some((mut base, mut tone, upper)) = decode_vowel(ch) {
            if tone == Tone::None && strip_vowel_diacritic(base) == base {
                if i + 1 < chars.len() {
                    if let Some(mod_base) = apply_viqr_modifier(base, chars[i + 1]) {
                        base = mod_base;
                        i += 1;
                    }
                }
                if i + 1 < chars.len() {
                    if let Some(t) = parse_viqr_tone(chars[i + 1]) {
                        tone = t;
                        i += 1;
                    }
                }
                out.push(encode_vowel(base, tone, upper));
                i += 1;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn is_combining_vi_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}' | '\u{0301}' | '\u{0302}' | '\u{0303}' | '\u{0306}' | '\u{0309}' | '\u{031B}' | '\u{0323}'
    )
}

fn detect_latin1_encoding(input: &str) -> ConvertEncoding {
    let chars: Vec<char> = input.chars().collect();
    let mut vni_score: i32 = 0;
    let mut tcvn_score: i32 = 0;
    let mut uni_score: i32 = 0;
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && is_vni_two_char(chars[i], chars[i + 1]) {
            vni_score += 5;
            i += 2;
            continue;
        }
        if !chars[i].is_ascii() {
            if is_tcvn3_exclusive(chars[i]) {
                tcvn_score += 5;
            } else if chars[i] == 'à' || chars[i] == 'À' {
                uni_score += 2;
            }
        }
        i += 1;
    }
    for word in input.split(|c: char| !c.is_alphanumeric()) {
        if word.chars().all(|c| c.is_ascii()) {
            continue;
        }
        if dict::is_vietnamese_word(word, true) {
            uni_score += 4;
        }
        let dt = from_tcvn3(word);
        if dt != word && dict::is_vietnamese_word(&dt, true) {
            tcvn_score += 6;
        }
        let dv = from_vni(word);
        if dv != word && dict::is_vietnamese_word(&dv, true) {
            vni_score += 6;
        }
    }
    if tcvn_score > vni_score && tcvn_score > uni_score {
        ConvertEncoding::Tcvn3
    } else if vni_score > tcvn_score && vni_score > uni_score {
        ConvertEncoding::VniWindows
    } else {
        ConvertEncoding::Unicode
    }
}

fn is_vni_two_char(a: char, b: char) -> bool {
    VNI_TABLE.iter().any(|&(uni, vni)| {
        let mut it = vni.chars();
        it.next() == Some(a) && it.next() == Some(b) && it.next().is_none() && !uni.is_ascii()
    })
}

fn is_tcvn3_exclusive(ch: char) -> bool {
    if matches!(
        ch,
        'à' | 'á'
            | 'â'
            | 'ã'
            | 'è'
            | 'é'
            | 'ê'
            | 'ì'
            | 'í'
            | 'ò'
            | 'ó'
            | 'ô'
            | 'õ'
            | 'ù'
            | 'ú'
            | 'ý'
    ) {
        return false;
    }
    let in_tcvn3 = TCVN3_LOWER
        .iter()
        .any(|&(uni, tcvn)| tcvn == ch && !uni.is_ascii());
    if !in_tcvn3 {
        return false;
    }
    !VNI_TABLE
        .iter()
        .any(|&(uni, vni)| !uni.is_ascii() && vni.chars().count() == 1 && vni.chars().next() == Some(ch))
}

fn count_viqr_cues(input: &str) -> usize {
    let chars: Vec<char> = input.chars().collect();
    let mut cues = 0;
    for i in 0..chars.len() {
        let ch = chars[i];
        let at_word_start = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
        if at_word_start
            && matches!(ch, 'd' | 'D')
            && i + 2 < chars.len()
            && matches!(chars[i + 1], 'd' | 'D')
            && chars[i + 2].is_ascii_alphabetic()
        {
            cues += 1;
        }
        let lower = ch.to_ascii_lowercase();
        if matches!(lower, 'a' | 'e' | 'o' | 'u') && i + 1 < chars.len() {
            let next = chars[i + 1];
            let mod_hit = (next == '^' && matches!(lower, 'a' | 'e' | 'o'))
                || (next == '(' && lower == 'a')
                || (next == '+' && matches!(lower, 'o' | 'u'));
            if mod_hit {
                cues += 1;
            }
        }
        if matches!(lower, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
            && i + 2 < chars.len()
            && matches!(chars[i + 1], '`' | '~' | '?')
            && chars[i + 2].is_ascii_alphabetic()
        {
            cues += 1;
        }
    }
    cues
}

fn viqr_modifier_char(base: BaseVowel) -> Option<char> {
    match base {
        BaseVowel::ABreve => Some('('),
        BaseVowel::ACircumflex | BaseVowel::ECircumflex | BaseVowel::OCircumflex => Some('^'),
        BaseVowel::OHorn | BaseVowel::UHorn => Some('+'),
        _ => None,
    }
}

fn viqr_tone_char(tone: Tone) -> Option<char> {
    match tone {
        Tone::None => None,
        Tone::Acute => Some('\''),
        Tone::Grave => Some('`'),
        Tone::HookAbove => Some('?'),
        Tone::Tilde => Some('~'),
        Tone::DotBelow => Some('.'),
    }
}

fn apply_viqr_modifier(base: BaseVowel, ch: char) -> Option<BaseVowel> {
    let fam = base_family(base);
    match ch {
        '^' => match fam {
            VowelFamily::A => Some(BaseVowel::ACircumflex),
            VowelFamily::E => Some(BaseVowel::ECircumflex),
            VowelFamily::O => Some(BaseVowel::OCircumflex),
            _ => None,
        },
        '(' if fam == VowelFamily::A => Some(BaseVowel::ABreve),
        '+' => match fam {
            VowelFamily::O => Some(BaseVowel::OHorn),
            VowelFamily::U => Some(BaseVowel::UHorn),
            _ => None,
        },
        _ => None,
    }
}

fn parse_viqr_tone(ch: char) -> Option<Tone> {
    match ch {
        '\'' => Some(Tone::Acute),
        '`' => Some(Tone::Grave),
        '?' => Some(Tone::HookAbove),
        '~' => Some(Tone::Tilde),
        '.' => Some(Tone::DotBelow),
        _ => None,
    }
}
