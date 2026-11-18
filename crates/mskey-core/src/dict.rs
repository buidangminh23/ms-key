use std::collections::{BTreeSet, HashSet};
use std::sync::LazyLock;

const VI_DIC_RAW: &str = include_str!("../data/vi.dic");
const KEEP_DIC_RAW: &str = include_str!("../data/keep.dic");
const EN_DIC_RAW: &str = include_str!("../data/english_dict_merged.txt");

static VI_WORDS: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    let mut set = BTreeSet::new();
    for line in VI_DIC_RAW.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        let modern = crate::converter_extra::normalize_tone_style(&lower, true);
        let classic = crate::converter_extra::normalize_tone_style(&lower, false);
        set.insert(lower);
        set.insert(modern);
        set.insert(classic);
    }
    set
});

static KEEP_WORDS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| KEEP_DIC_RAW.lines().skip(1).filter(|s| !s.is_empty()).collect());

static EN_WORDS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| EN_DIC_RAW.lines().filter(|s| !s.is_empty()).collect());

pub fn is_vietnamese_word(word: &str, allow_fjwz: bool) -> bool {
    is_vietnamese_word_with_custom(word, allow_fjwz, &[])
}

fn allows_initial(word: &str, allow_fjwz: bool) -> bool {
    allow_fjwz || !word.starts_with(['f', 'j', 'w', 'z'])
}

pub(crate) fn has_vietnamese_completion(
    prefix: &str,
    allow_fjwz: bool,
    user_words: &[String],
) -> bool {
    let lower = prefix.to_lowercase();
    user_words.iter().any(|word| word.trim().to_lowercase().starts_with(&lower))
        || (allows_initial(&lower, allow_fjwz)
            && VI_WORDS.range(lower.clone()..).next().is_some_and(|word| word.starts_with(&lower)))
}

pub fn is_vietnamese_word_with_custom(
    word: &str,
    allow_fjwz: bool,
    user_words: &[String],
) -> bool {
    if word.is_empty() {
        return false;
    }
    if is_user_word(word, user_words) {
        return true;
    }
    let lower = word.to_lowercase();
    allows_initial(&lower, allow_fjwz) && VI_WORDS.contains(lower.as_str())
}

pub fn is_user_word(word: &str, user_words: &[String]) -> bool {
    if word.is_empty() || user_words.is_empty() {
        return false;
    }
    let lower = word.to_lowercase();
    user_words
        .iter()
        .any(|w| !w.trim().is_empty() && w.trim().to_lowercase() == lower)
}

pub fn is_english_word(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let lower = word.to_lowercase();
    if EN_WORDS.contains(lower.as_str()) {
        return true;
    }
    has_english_inflected_stem(&lower)
}

fn has_english_inflected_stem(lower: &str) -> bool {
    if lower.len() < 5 || !lower.is_ascii() {
        return false;
    }
    if let Some(stem) = lower.strip_suffix('s')
        && stem.len() >= 4
        && !stem.ends_with('s')
        && EN_WORDS.contains(stem)
    {
        return true;
    }
    for suffix in ["es", "ed", "ing"] {
        if let Some(stem) = lower.strip_suffix(suffix)
            && stem.len() >= 4
            && EN_WORDS.contains(stem)
        {
            return true;
        }
    }
    false
}

pub fn is_keep_word(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let lower = word.to_lowercase();
    KEEP_WORDS.contains(lower.as_str())
}
