use std::collections::HashSet;
use std::sync::LazyLock;

const VI_DIC_RAW: &str = include_str!("../data/vi.dic");
const KEEP_DIC_RAW: &str = include_str!("../data/keep.dic");
const EN_DIC_RAW: &str = include_str!("../data/english_dict_merged.txt");

static VI_WORDS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| VI_DIC_RAW.lines().skip(1).filter(|s| !s.is_empty()).collect());

static KEEP_WORDS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| KEEP_DIC_RAW.lines().skip(1).filter(|s| !s.is_empty()).collect());

static EN_WORDS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| EN_DIC_RAW.lines().filter(|s| !s.is_empty()).collect());

pub fn is_vietnamese_word(word: &str, allow_fjwz: bool) -> bool {
    if word.is_empty() {
        return false;
    }
    if !allow_fjwz {
        if let Some(first) = word.chars().next() {
            if matches!(first.to_ascii_lowercase(), 'f' | 'j' | 'w' | 'z') {
                return false;
            }
        }
    }
    let lower = word.to_lowercase();
    VI_WORDS.contains(lower.as_str())
}

pub fn is_english_word(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let lower = word.to_lowercase();
    EN_WORDS.contains(lower.as_str())
}

pub fn is_keep_word(word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let lower = word.to_lowercase();
    KEEP_WORDS.contains(lower.as_str())
}
