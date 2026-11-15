use crate::config::Charset;
use crate::tables::{BaseVowel, Tone, decode_vowel, encode_vowel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseTransform {
    None,
    Upper,
    Lower,
    Sentence,
    Title,
    StripDiacritics,
}

const COMBINING_TONE: [Option<char>; 6] = [
    None,
    Some('\u{0301}'),
    Some('\u{0300}'),
    Some('\u{0309}'),
    Some('\u{0303}'),
    Some('\u{0323}'),
];

pub(crate) const TCVN3_LOWER: &[(&str, char)] = &[
    ("a", 'a'),
    ("á", '¸'),
    ("à", 'µ'),
    ("ả", '¶'),
    ("ã", '·'),
    ("ạ", '¹'),
    ("ă", '¨'),
    ("ắ", '¾'),
    ("ằ", '»'),
    ("ẳ", '¼'),
    ("ẵ", '½'),
    ("ặ", 'Æ'),
    ("â", '©'),
    ("ấ", 'Ê'),
    ("ầ", 'Ç'),
    ("ẩ", 'È'),
    ("ẫ", 'É'),
    ("ậ", 'Ë'),
    ("e", 'e'),
    ("é", 'Ð'),
    ("è", 'Ì'),
    ("ẻ", 'Î'),
    ("ẽ", 'Ï'),
    ("ẹ", 'Ñ'),
    ("ê", 'ª'),
    ("ế", 'Õ'),
    ("ề", 'Ò'),
    ("ể", 'Ó'),
    ("ễ", 'Ô'),
    ("ệ", 'Ö'),
    ("i", 'i'),
    ("í", 'Ý'),
    ("ì", '×'),
    ("ỉ", 'Ø'),
    ("ĩ", 'Ü'),
    ("ị", 'Þ'),
    ("o", 'o'),
    ("ó", 'ã'),
    ("ò", 'ß'),
    ("ỏ", 'á'),
    ("õ", 'â'),
    ("ọ", 'ä'),
    ("ô", '«'),
    ("ố", 'è'),
    ("ồ", 'å'),
    ("ổ", 'æ'),
    ("ỗ", 'ç'),
    ("ộ", 'é'),
    ("ơ", '¬'),
    ("ớ", 'í'),
    ("ờ", 'ê'),
    ("ở", 'ë'),
    ("ỡ", 'ì'),
    ("ợ", 'î'),
    ("u", 'u'),
    ("ú", 'ó'),
    ("ù", 'ï'),
    ("ủ", 'ñ'),
    ("ũ", 'ò'),
    ("ụ", 'ô'),
    ("ư", '\u{AD}'),
    ("ứ", 'ø'),
    ("ừ", 'õ'),
    ("ử", 'ö'),
    ("ữ", '÷'),
    ("ự", 'ù'),
    ("y", 'y'),
    ("ý", 'ý'),
    ("ỳ", 'ú'),
    ("ỷ", 'û'),
    ("ỹ", 'ü'),
    ("ỵ", 'þ'),
    ("đ", '®'),
    ("Đ", '§'),
    ("Ă", '¡'),
    ("Â", '¢'),
    ("Ê", '£'),
    ("Ô", '¤'),
    ("Ơ", '¥'),
    ("Ư", '¦'),
];

pub(crate) const VNI_TABLE: &[(&str, &str)] = &[
    ("a", "a"),
    ("á", "aù"),
    ("à", "aø"),
    ("ả", "aû"),
    ("ã", "aõ"),
    ("ạ", "aï"),
    ("ă", "aê"),
    ("ắ", "aé"),
    ("ằ", "aè"),
    ("ẳ", "aú"),
    ("ẵ", "aü"),
    ("ặ", "aë"),
    ("â", "aâ"),
    ("ấ", "aá"),
    ("ầ", "aà"),
    ("ẩ", "aå"),
    ("ẫ", "aã"),
    ("ậ", "aä"),
    ("e", "e"),
    ("é", "eù"),
    ("è", "eø"),
    ("ẻ", "eû"),
    ("ẽ", "eõ"),
    ("ẹ", "eï"),
    ("ê", "eâ"),
    ("ế", "eá"),
    ("ề", "eà"),
    ("ể", "eå"),
    ("ễ", "eã"),
    ("ệ", "eä"),
    ("i", "i"),
    ("í", "í"),
    ("ì", "ì"),
    ("ỉ", "æ"),
    ("ĩ", "ó"),
    ("ị", "ò"),
    ("o", "o"),
    ("ó", "où"),
    ("ò", "oø"),
    ("ỏ", "oû"),
    ("õ", "oõ"),
    ("ọ", "oï"),
    ("ô", "oâ"),
    ("ố", "oá"),
    ("ồ", "oà"),
    ("ổ", "oå"),
    ("ỗ", "oã"),
    ("ộ", "oä"),
    ("ơ", "ô"),
    ("ớ", "ôù"),
    ("ờ", "ôø"),
    ("ở", "ôû"),
    ("ỡ", "ôõ"),
    ("ợ", "ôï"),
    ("u", "u"),
    ("ú", "uù"),
    ("ù", "uø"),
    ("ủ", "uû"),
    ("ũ", "uõ"),
    ("ụ", "uï"),
    ("ư", "ö"),
    ("ứ", "öù"),
    ("ừ", "öø"),
    ("ử", "öû"),
    ("ữ", "öõ"),
    ("ự", "öï"),
    ("y", "y"),
    ("ý", "yù"),
    ("ỳ", "yø"),
    ("ỷ", "yû"),
    ("ỹ", "yõ"),
    ("ỵ", "î"),
    ("đ", "ñ"),
    ("Đ", "Ñ"),
    ("Á", "AÙ"),
    ("À", "AØ"),
    ("Ả", "AÛ"),
    ("Ã", "AÕ"),
    ("Ạ", "AÏ"),
    ("Ă", "AÊ"),
    ("Ắ", "AÉ"),
    ("Ằ", "AÈ"),
    ("Ẳ", "AÚ"),
    ("Ẵ", "AÜ"),
    ("Ặ", "AË"),
    ("Â", "AÂ"),
    ("Ấ", "AÁ"),
    ("Ầ", "AÀ"),
    ("Ẩ", "AÅ"),
    ("Ẫ", "AÃ"),
    ("Ậ", "AÄ"),
    ("É", "EÙ"),
    ("È", "EØ"),
    ("Ẻ", "EÛ"),
    ("Ẽ", "EÕ"),
    ("Ẹ", "EÏ"),
    ("Ê", "EÂ"),
    ("Ế", "EÁ"),
    ("Ề", "EÀ"),
    ("Ể", "EÅ"),
    ("Ễ", "EÃ"),
    ("Ệ", "EÄ"),
    ("Í", "Í"),
    ("Ì", "Ì"),
    ("Ỉ", "Æ"),
    ("Ĩ", "Ó"),
    ("Ị", "Ò"),
    ("Ó", "OÙ"),
    ("Ò", "OØ"),
    ("Ỏ", "OÛ"),
    ("Õ", "OÕ"),
    ("Ọ", "OÏ"),
    ("Ô", "OÂ"),
    ("Ố", "OÁ"),
    ("Ồ", "OÀ"),
    ("Ổ", "OÅ"),
    ("Ỗ", "OÃ"),
    ("Ộ", "OÄ"),
    ("Ơ", "Ô"),
    ("Ớ", "ÔÙ"),
    ("Ờ", "ÔØ"),
    ("Ở", "ÔÛ"),
    ("Ỡ", "ÔÕ"),
    ("Ợ", "ÔÏ"),
    ("Ú", "UÙ"),
    ("Ù", "UØ"),
    ("Ủ", "UÛ"),
    ("Ũ", "UÕ"),
    ("Ụ", "UÏ"),
    ("Ư", "Ö"),
    ("Ứ", "ÖÙ"),
    ("Ừ", "ÖØ"),
    ("Ử", "ÖÛ"),
    ("Ữ", "ÖÕ"),
    ("Ự", "ÖÏ"),
    ("Ý", "YÙ"),
    ("Ỳ", "YØ"),
    ("Ỷ", "YÛ"),
    ("Ỹ", "YÕ"),
    ("Ỵ", "Î"),
];

pub fn encode_char_for_charset(ch: char, charset: Charset) -> String {
    match charset {
        Charset::Unicode => ch.to_string(),
        Charset::UnicodeComposite => to_composite(ch),
        Charset::Tcvn3 => to_tcvn3_char(ch),
        Charset::VniWindows => to_vni_char(ch),
    }
}

pub fn convert_text(
    input: &str,
    from: Charset,
    to: Charset,
    case_mode: CaseTransform,
) -> String {
    let unicode = match from {
        Charset::Unicode => input.to_string(),
        Charset::UnicodeComposite => normalize_nfd_to_nfc(input),
        Charset::Tcvn3 => from_tcvn3(input),
        Charset::VniWindows => from_vni(input),
    };
    let cased = apply_case_transform(&unicode, case_mode);
    match to {
        Charset::Unicode => cased,
        Charset::UnicodeComposite => cased.chars().map(to_composite).collect(),
        Charset::Tcvn3 => cased.chars().map(to_tcvn3_char).collect(),
        Charset::VniWindows => cased.chars().map(to_vni_char).collect(),
    }
}

pub fn apply_case_transform(input: &str, mode: CaseTransform) -> String {
    match mode {
        CaseTransform::None => input.to_string(),
        CaseTransform::Upper => input.to_uppercase(),
        CaseTransform::Lower => input.to_lowercase(),
        CaseTransform::Sentence => to_sentence_case(input),
        CaseTransform::Title => to_title_case(input),
        CaseTransform::StripDiacritics => crate::converter_extra::strip_vietnamese_diacritics(input),
    }
}

fn to_sentence_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut cap_next = true;
    for ch in input.chars() {
        if cap_next && ch.is_alphabetic() {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            cap_next = false;
        } else {
            for l in ch.to_lowercase() {
                out.push(l);
            }
            if matches!(ch, '.' | '!' | '?' | '\n') {
                cap_next = true;
            }
        }
    }
    out
}

fn to_title_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut cap_next = true;
    let mut prev_alpha = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_alphabetic() {
            if cap_next {
                for u in ch.to_uppercase() {
                    out.push(u);
                }
                cap_next = false;
            } else {
                for l in ch.to_lowercase() {
                    out.push(l);
                }
            }
            prev_alpha = true;
        } else {
            out.push(ch);
            let keep_joined = prev_alpha
                && matches!(ch, '\'' | '\u{2019}')
                && chars.peek().is_some_and(|n| n.is_alphabetic());
            if !keep_joined {
                cap_next = true;
            }
            prev_alpha = false;
        }
    }
    out
}

fn to_composite(ch: char) -> String {
    if let Some((base, tone, upper)) = decode_vowel(ch) {
        let base_no_tone = encode_vowel(base, Tone::None, upper);
        let mut out = String::new();
        out.push(base_no_tone);
        if let Some(mark) = COMBINING_TONE[tone as usize] {
            out.push(mark);
        }
        return out;
    }
    ch.to_string()
}

pub fn normalize_nfd_to_nfc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if let Some((mut base, mut tone, upper)) = decode_vowel(ch) {
            let mut j = i + 1;
            while j < chars.len() {
                match chars[j] {
                    '\u{0306}' if base == BaseVowel::A => base = BaseVowel::ABreve,
                    '\u{0302}' if base == BaseVowel::A => base = BaseVowel::ACircumflex,
                    '\u{0302}' if base == BaseVowel::E => base = BaseVowel::ECircumflex,
                    '\u{0302}' if base == BaseVowel::O => base = BaseVowel::OCircumflex,
                    '\u{031B}' if base == BaseVowel::O => base = BaseVowel::OHorn,
                    '\u{031B}' if base == BaseVowel::U => base = BaseVowel::UHorn,
                    '\u{0301}' => tone = Tone::Acute,
                    '\u{0300}' => tone = Tone::Grave,
                    '\u{0309}' => tone = Tone::HookAbove,
                    '\u{0303}' => tone = Tone::Tilde,
                    '\u{0323}' => tone = Tone::DotBelow,
                    _ => break,
                }
                j += 1;
            }
            out.push(encode_vowel(base, tone, upper));
            i = j;
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

fn to_tcvn3_char(ch: char) -> String {
    let mut buf = [0u8; 4];
    let s = ch.encode_utf8(&mut buf);
    for &(uni, tcvn) in TCVN3_LOWER {
        if uni == s {
            return tcvn.to_string();
        }
    }
    let upper_s = ch.to_lowercase().to_string();
    for &(uni, tcvn) in TCVN3_LOWER {
        if uni == upper_s && !matches!(uni, "đ" | "ă" | "â" | "ê" | "ô" | "ơ" | "ư") {
            return tcvn.to_string();
        }
    }
    ch.to_string()
}

pub(crate) fn from_tcvn3(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        let mut matched = None;
        for &(uni, tcvn) in TCVN3_LOWER {
            if tcvn == ch && !uni.is_ascii() {
                matched = Some(uni);
                break;
            }
        }
        if let Some(u) = matched {
            out.push_str(u);
        } else {
            out.push(ch);
        }
    }
    out
}

fn to_vni_char(ch: char) -> String {
    let mut buf = [0u8; 4];
    let s = ch.encode_utf8(&mut buf);
    for &(uni, vni) in VNI_TABLE {
        if uni == s {
            return vni.to_string();
        }
    }
    ch.to_string()
}

pub(crate) fn from_vni(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() {
            let pair: String = [chars[i], chars[i + 1]].iter().collect();
            if let Some((uni, _)) = VNI_TABLE
                .iter()
                .find(|&&(uni, vni)| vni == pair && uni.chars().count() == 1 && !uni.is_ascii())
            {
                out.push_str(uni);
                i += 2;
                continue;
            }
        }
        let single = chars[i].to_string();
        if let Some((uni, _)) = VNI_TABLE
            .iter()
            .find(|&&(uni, vni)| vni == single && !uni.is_ascii())
        {
            out.push_str(uni);
        } else {
            out.push(chars[i]);
        }
        i += 1;
    }
    out
}
