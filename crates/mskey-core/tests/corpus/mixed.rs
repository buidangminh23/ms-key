use super::{VI_DIC, sentence_to_keys, simulate};
use mskey_core::{Config, Engine, normalize_tone_style, strip_vietnamese_diacritics};

#[test]
fn all_vietnamese_dictionary_entries_commit_in_a_continuous_telex_session() {
    let mut engine = Engine::new(Config::default());
    for word in VI_DIC.lines().skip(1) {
        let mut keys =
            sentence_to_keys(word).unwrap_or_else(|| word.replace('Đ', "DD").replace('đ', "dd"));
        if strip_vietnamese_diacritics(word).contains("oo") {
            keys = keys.replace("oo", "ooo");
        }
        if word.bytes().all(|ch| ch.is_ascii_uppercase()) {
            keys.push('\x1b');
        }
        keys.push(' ');
        let actual = simulate(&mut engine, &keys);
        assert_eq!(
            normalize_tone_style(&actual, true),
            format!("{} ", normalize_tone_style(word, true)),
            "{word:?} from {keys:?}"
        );
    }
}

#[test]
fn english_mode_preserves_the_complete_dictionary_alphabet_and_multilingual_literals() {
    let mut engine = Engine::new(Config::default());
    engine.set_temporary_english(true);
    let dictionary = include_str!("../../data/english_dict_merged.txt");
    let samples = [
        "abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "aAbBcCdDeEfFgGhHiIjJkKlLmMnNoOpPqQrRsStTuUvVwWxXyYzZ",
        "C++: foo_bar(42); 3.14159; 0xFF; test+mac@example.org",
        "https://example.org/api/v2?platform=mac&speed=100",
        "Grüße, élève, mañana, русский, 日本語, 한국어, 😀.",
    ];
    for sample in dictionary.lines().chain(samples) {
        let expected = format!("{sample} ");
        assert_eq!(simulate(&mut engine, &expected), expected);
    }
    engine.set_temporary_english(false);
    assert_eq!(
        simulate(&mut engine, "cacs hoawcj ddaayf "),
        "các hoặc đầy "
    );
}

#[test]
fn mixed_telex_bursts_preserve_boundaries_punctuation_and_corrections() {
    let cases = [
        (
            "cacs hoawcj ddaayf; casc hoawjc ddaafy! ",
            "các hoặc đầy; các hoặc đầy! ",
        ),
        ("CACS HOAWCJ DDAAYF ", "CÁC HOẶC ĐẦY "),
        ("cacs \x08, hoawcj \x08; ddaayf. ", "các, hoặc; đầy. "),
        (
            "huyeets aps, tim machj, khowps, vieem, xoang. ",
            "huyết áp, tim mạch, khớp, viêm, xoang. ",
        ),
        (
            "API v2.4: cacs file JSON cofn ddaayf ddur. ",
            "API v2.4: các file JSON còn đầy đủ. ",
        ),
        (
            "CPU 95%, RAM 32GB; cacs tuwf Telex vieets ddungs. ",
            "CPU 95%, RAM 32GB; các từ Telex viết đúng. ",
        ),
        (
            "https://example.com/test?id=42 qa@example.com ",
            "https://example.com/test?id=42 qa@example.com ",
        ),
        (
            "0 1234567890 -12.75 (99%) [3/4] 2026-11-22 10:55 ",
            "0 1234567890 -12.75 (99%) [3/4] 2026-11-22 10:55 ",
        ),
    ];
    let mut engine = Engine::new(Config::default());
    let input = cases.map(|(input, _)| input).concat().repeat(12);
    let expected = cases.map(|(_, output)| output).concat().repeat(12);
    assert_eq!(simulate(&mut engine, &input), expected);
}

#[test]
fn separated_shape_keys_preserve_english_spelling_and_vietnamese_escapes() {
    let keys = "Amanda banana booongs xooong boook seeen toooi ddaayf ";
    let expected = "Amanda banana boóng xoong book seen tooi đầy ";
    assert_eq!(
        simulate(&mut Engine::new(Config::default()), keys),
        expected
    );
}
