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

fn early_telex_cases() -> impl Iterator<Item = (&'static str, &'static str)> {
    include_str!("telex-regressions.tsv")
        .lines()
        .map(|line| line.split_once('\t').unwrap())
}

const ENGLISH_COLLISIONS: [&str; 14] = [
    "chose", "gary", "hero", "lose", "mary", "ore", "sofa", "taxi", "usee", "visa", "vary", "usa",
    "ira", "uri",
];

#[test]
fn early_telex_words_compose_before_and_after_each_boundary() {
    let mut failures = Vec::new();
    for (keys, word) in early_telex_cases() {
        for boundary in ["", " ", ",", "!", "\t", "\n"] {
            let actual = simulate(
                &mut Engine::new(Config::default()),
                &format!("{keys}{boundary}"),
            );
            if actual != format!("{word}{boundary}") {
                failures.push(format!(
                    "{keys:?} + {boundary:?}: {actual:?}, expected {word:?}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn continuous_early_telex_preserves_words_and_separators_without_resets() {
    let mut input = String::new();
    let mut expected = String::new();
    let boundaries = [" ", ", ", "! ", "\t", "\n"];
    for (index, (keys, word)) in early_telex_cases().enumerate() {
        let separator = boundaries[index % boundaries.len()];
        input.push_str(keys);
        input.push_str(separator);
        expected.push_str(word);
        expected.push_str(separator);
    }
    assert_eq!(
        simulate(&mut Engine::new(Config::default()), &input.repeat(3)),
        expected.repeat(3)
    );
}

#[test]
fn early_telex_respects_orthography_english_escape_and_user_dictionary() {
    let mut classic = Engine::new(Config {
        modern_tone: false,
        ..Config::default()
    });
    assert_eq!(
        simulate(&mut classic, "chose lose ore sofa "),
        "chóe lóe ỏe sòa "
    );

    let literal = ENGLISH_COLLISIONS.join(" ") + " ";
    let mut english = Engine::new(Config::default());
    english.set_temporary_english(true);
    assert_eq!(simulate(&mut english, &literal), literal);

    let escaped = ENGLISH_COLLISIONS.join("\x1b ") + "\x1b ";
    assert_eq!(
        simulate(&mut Engine::new(Config::default()), &escaped),
        literal
    );

    let mut overridden = Engine::new(Config {
        user_dict_words: ENGLISH_COLLISIONS
            .iter()
            .map(|word| word.to_string())
            .collect(),
        ..Config::default()
    });
    assert_eq!(simulate(&mut overridden, &literal), literal);

    let unaffected = "software hardware browser server example ";
    assert_eq!(
        simulate(&mut Engine::new(Config::default()), unaffected),
        unaffected
    );
}
