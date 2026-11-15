use crate::syllable_rimes::{CODAS_QU_Y, RIMES, RimeRule};
use crate::tables::{BaseVowel, Tone, VowelFamily, base_family, decode_vowel, strip_vowel_diacritic};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSyllable {
    pub onset: String,
    pub onset_len: usize,
    pub vowels: Vec<(BaseVowel, Tone, bool)>,
    pub vowel_start: usize,
    pub coda: String,
    pub active_tone: Tone,
}

pub fn parse_syllable(chars: &[char], allow_fjwz: bool) -> Option<ParsedSyllable> {
    if chars.is_empty() {
        return None;
    }
    let lowered: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    if lowered.len() != chars.len() {
        return None;
    }
    let onset_len = detect_onset_len(&lowered, allow_fjwz)?;
    let onset: String = lowered[..onset_len].iter().collect();

    let mut vowels = Vec::new();
    let mut idx = onset_len;
    while idx < lowered.len() {
        if let Some((base, tone, _)) = decode_vowel(chars[idx]) {
            vowels.push((base, tone, chars[idx].is_uppercase()));
            idx += 1;
        } else {
            break;
        }
    }
    if vowels.len() > 3 {
        return None;
    }
    for &c in &lowered[idx..] {
        if decode_vowel(c).is_some() || !c.is_ascii_alphabetic() {
            return None;
        }
    }
    let coda: String = lowered[idx..].iter().collect();
    let active_tone = vowels
        .iter()
        .find(|(_, t, _)| *t != Tone::None)
        .map(|(_, t, _)| *t)
        .unwrap_or(Tone::None);

    Some(ParsedSyllable {
        onset,
        onset_len,
        vowels,
        vowel_start: onset_len,
        coda,
        active_tone,
    })
}

fn detect_onset_len(lowered: &[char], allow_fjwz: bool) -> Option<usize> {
    if lowered.is_empty() {
        return Some(0);
    }
    if decode_vowel(lowered[0]).is_some() {
        return Some(0);
    }
    if lowered.len() >= 3 && lowered[0] == 'n' && lowered[1] == 'g' && lowered[2] == 'h' {
        return Some(3);
    }
    if lowered.len() >= 2 {
        let pair = [lowered[0], lowered[1]];
        if matches!(
            pair,
            ['c', 'h']
                | ['g', 'h']
                | ['k', 'h']
                | ['n', 'g']
                | ['n', 'h']
                | ['p', 'h']
                | ['t', 'h']
                | ['t', 'r']
        ) {
            return Some(2);
        }
        if pair == ['q', 'u'] {
            return Some(2);
        }
        if lowered[0] == 'g'
            && decode_vowel(lowered[1]).is_some_and(|(b, _, _)| b == BaseVowel::I)
            && lowered.get(2).is_some_and(|c| decode_vowel(*c).is_some())
        {
            return Some(2);
        }
    }
    let first = lowered[0];
    let standard = matches!(
        first,
        'b' | 'c'
            | 'd'
            | 'đ'
            | 'g'
            | 'h'
            | 'k'
            | 'l'
            | 'm'
            | 'n'
            | 'p'
            | 'q'
            | 'r'
            | 's'
            | 't'
            | 'v'
            | 'x'
    );
    if standard || (allow_fjwz && matches!(first, 'f' | 'j' | 'w' | 'z')) {
        return Some(1);
    }
    None
}

pub fn tone_target_index(syllable: &ParsedSyllable, modern_tone: bool) -> Option<usize> {
    if syllable.vowels.is_empty() {
        return None;
    }
    if syllable.vowels.len() == 1 {
        return Some(0);
    }
    let bases: Vec<BaseVowel> = syllable.vowels.iter().map(|(b, _, _)| *b).collect();
    if let Some(rule) = find_exact_rime(&bases) {
        let idx = if !syllable.coda.is_empty() {
            rule.tone_closed
        } else if modern_tone {
            rule.tone_open_modern
        } else {
            rule.tone_open_classic
        };
        return Some(idx.min(bases.len() - 1));
    }
    for (idx, b) in bases.iter().enumerate() {
        if matches!(
            b,
            BaseVowel::ABreve
                | BaseVowel::ACircumflex
                | BaseVowel::ECircumflex
                | BaseVowel::OCircumflex
                | BaseVowel::OHorn
                | BaseVowel::UHorn
        ) {
            return Some(idx);
        }
    }
    if bases.len() >= 3 {
        return Some(1);
    }
    let f0 = base_family(bases[0]);
    let f1 = base_family(bases[1]);
    if syllable.coda.is_empty() {
        if matches!(
            (f0, f1),
            (VowelFamily::O, VowelFamily::A)
                | (VowelFamily::O, VowelFamily::E)
                | (VowelFamily::U, VowelFamily::Y)
        ) {
            return Some(if modern_tone { 1 } else { 0 });
        }
        return Some(0);
    }
    Some(1)
}

pub fn is_valid_complete_syllable(syllable: &ParsedSyllable, allow_fjwz: bool) -> bool {
    if syllable.vowels.is_empty() {
        return false;
    }
    if !onset_compatible(syllable, allow_fjwz) {
        return false;
    }
    let bases: Vec<BaseVowel> = syllable.vowels.iter().map(|(b, _, _)| *b).collect();
    let Some(rule) = find_exact_rime(&bases) else {
        return false;
    };
    if syllable.coda.is_empty() {
        if !rule.open_allowed {
            return false;
        }
    } else if syllable.onset == "qu" && bases == [BaseVowel::Y] {
        if !CODAS_QU_Y.contains(&syllable.coda.as_str()) {
            return false;
        }
    } else if syllable.onset == "gi" && bases == [BaseVowel::ECircumflex] {
        if !rule.codas.contains(&syllable.coda.as_str())
            && !matches!(syllable.coda.as_str(), "c" | "ng")
        {
            return false;
        }
    } else if !rule.codas.contains(&syllable.coda.as_str()) {
        return false;
    }
    !(is_checked_coda(&syllable.coda) && !matches!(syllable.active_tone, Tone::Acute | Tone::DotBelow))
}

pub fn is_valid_typing_prefix(syllable: &ParsedSyllable, allow_fjwz: bool) -> bool {
    if !onset_compatible(syllable, allow_fjwz) {
        return false;
    }
    if syllable.vowels.is_empty() {
        return true;
    }
    let bases: Vec<BaseVowel> = syllable.vowels.iter().map(|(b, _, _)| *b).collect();
    if is_checked_coda(&syllable.coda)
        && !matches!(
            syllable.active_tone,
            Tone::None | Tone::Acute | Tone::DotBelow
        )
    {
        return false;
    }
    let rime_match = find_exact_rime(&bases).or_else(|| find_stripped_rime(&bases));
    let Some(rule) = rime_match else {
        return false;
    };
    if syllable.coda.is_empty() {
        return true;
    }
    if syllable.onset == "gi"
        && matches!(bases.as_slice(), [BaseVowel::E] | [BaseVowel::ECircumflex])
        && matches!(syllable.coda.as_str(), "c" | "n" | "ng")
    {
        return true;
    }
    rule.codas
        .iter()
        .any(|valid| valid.starts_with(syllable.coda.as_str()))
}

pub fn is_checked_coda(coda: &str) -> bool {
    matches!(coda, "c" | "ch" | "p" | "t")
}

fn onset_compatible(syllable: &ParsedSyllable, allow_fjwz: bool) -> bool {
    if syllable.onset.is_empty() {
        return true;
    }
    if !allow_fjwz && matches!(syllable.onset.as_str(), "f" | "j" | "w" | "z") {
        return false;
    }
    if syllable.vowels.is_empty() {
        return true;
    }
    let first_base = syllable.vowels[0].0;
    let first_fam = base_family(first_base);
    match syllable.onset.as_str() {
        "c" => matches!(first_fam, VowelFamily::A | VowelFamily::O | VowelFamily::U),
        "k" => matches!(first_fam, VowelFamily::E | VowelFamily::I | VowelFamily::Y),
        "g" => matches!(
            first_fam,
            VowelFamily::A | VowelFamily::I | VowelFamily::O | VowelFamily::U
        ),
        "gh" => matches!(first_fam, VowelFamily::E | VowelFamily::I),
        "ng" => matches!(first_fam, VowelFamily::A | VowelFamily::O | VowelFamily::U),
        "ngh" => matches!(first_fam, VowelFamily::E | VowelFamily::I),
        "qu" => !matches!(first_base, BaseVowel::U | BaseVowel::UHorn),
        "gi" => !matches!(first_fam, VowelFamily::I | VowelFamily::Y),
        _ => true,
    }
}

fn find_exact_rime(bases: &[BaseVowel]) -> Option<&'static RimeRule> {
    RIMES.iter().find(|r| r.vowels == bases)
}

fn find_stripped_rime(bases: &[BaseVowel]) -> Option<&'static RimeRule> {
    let stripped: Vec<BaseVowel> = bases.iter().map(|b| strip_vowel_diacritic(*b)).collect();
    RIMES.iter().find(|r| {
        if r.vowels.len() != stripped.len() {
            return false;
        }
        r.vowels
            .iter()
            .zip(stripped.iter())
            .all(|(rv, sv)| strip_vowel_diacritic(*rv) == *sv)
    })
}
