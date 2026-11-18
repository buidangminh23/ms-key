use mskey_core::{Config, Engine, EngineAction, InputMethod, normalize_tone_style};

const VI_DIC: &str = include_str!("../data/vi.dic");

fn simulate(engine: &mut Engine, input: &str) -> String {
    let mut screen = String::new();
    for ch in input.chars() {
        match engine.process_key(ch) {
            EngineAction::PassThrough => screen.push(ch),
            EngineAction::Replace { backspaces, text } => {
                for _ in 0..backspaces {
                    assert!(
                        screen.pop().is_some(),
                        "backspace before start of text on {ch:?}"
                    );
                }
                screen.push_str(&text);
            }
        }
    }
    screen
}

const VOWEL_ROWS: &[(&str, char, &str)] = &[
    ("aàáảãạ", 'a', ""),
    ("ăằắẳẵặ", 'a', "w"),
    ("âầấẩẫậ", 'a', "a"),
    ("eèéẻẽẹ", 'e', ""),
    ("êềếểễệ", 'e', "e"),
    ("iìíỉĩị", 'i', ""),
    ("oòóỏõọ", 'o', ""),
    ("ôồốổỗộ", 'o', "o"),
    ("ơờớởỡợ", 'o', "w"),
    ("uùúủũụ", 'u', ""),
    ("ưừứửữự", 'u', "w"),
    ("yỳýỷỹỵ", 'y', ""),
];

fn split_char(c: char) -> Option<(char, &'static str, Option<char>)> {
    let tone_keys = [None, Some('f'), Some('s'), Some('r'), Some('x'), Some('j')];
    for (set, base, m) in VOWEL_ROWS {
        if let Some(i) = set.chars().position(|x| x == c) {
            return Some((*base, m, tone_keys[i]));
        }
    }
    None
}

#[derive(Clone, Copy)]
enum Style {
    InlineToneEnd,
    InlineToneAfterVowel,
    InlineToneMid,
    HornDeferred,
}

fn to_telex(word: &str, style: Style) -> Option<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut tone = None;
    let mut parts: Vec<(String, bool)> = Vec::new();
    let mut horn = false;
    for &c in &chars {
        if c == 'đ' {
            parts.push(("dd".into(), false));
            continue;
        }
        if let Some((base, m, t)) = split_char(c) {
            if t.is_some() {
                tone = t;
            }
            let mut s = base.to_string();
            if m == "w" {
                horn = true;
                if matches!(style, Style::HornDeferred) {
                } else {
                    s.push('w');
                }
            } else if !m.is_empty() {
                s.push_str(m);
            }
            parts.push((s, true));
        } else if c.is_ascii_lowercase() {
            parts.push((c.to_string(), false));
        } else {
            return None;
        }
    }
    let last_vowel = parts.iter().rposition(|p| p.1)?;
    let first_vowel = parts.iter().position(|p| p.1)?;
    let mut out = String::new();
    for (i, (s, _)) in parts.iter().enumerate() {
        out.push_str(s);
        if let Some(t) = tone {
            if matches!(style, Style::InlineToneAfterVowel) && i == last_vowel {
                out.push(t);
            }
            if matches!(style, Style::InlineToneMid) && i == first_vowel {
                out.push(t);
            }
        }
    }
    if matches!(style, Style::HornDeferred) {
        if !horn {
            return None;
        }
        out.push('w');
    }
    if let Some(t) = tone
        && matches!(style, Style::InlineToneEnd | Style::HornDeferred)
    {
        out.push(t);
    }
    Some(out)
}

fn words() -> Vec<&'static str> {
    VI_DIC
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|w| !w.is_empty() && w.chars().all(|c| c.is_lowercase()) && !w.is_ascii())
        .filter(|w| !is_ambiguous_spelling(w))
        .collect()
}

fn is_ambiguous_spelling(word: &str) -> bool {
    let base = mskey_core::strip_vietnamese_diacritics(word);
    base.contains("oo") || matches!(word, "palăng" | "gỵa")
}

fn run(style: Style, label: &str) -> Vec<String> {
    let mut fails = Vec::new();
    let mut engine = Engine::new(Config::default());
    for w in words() {
        let Some(keys) = to_telex(w, style) else {
            continue;
        };
        if matches!(style, Style::InlineToneMid)
            && (w.starts_with("qu") || mskey_core::dict::is_english_word(&keys))
        {
            continue;
        }
        let got = simulate(&mut engine, &format!("{keys} "));
        let want = format!("{w} ");
        if got != want && normalize_tone_style(&got, true) != normalize_tone_style(&want, true) {
            fails.push(format!("{label}: {keys} -> {got:?} want {want:?}"));
        }
    }
    fails
}

#[test]
fn corpus_every_dictionary_syllable_types_correctly() {
    let mut all = Vec::new();
    all.extend(run(Style::InlineToneEnd, "tone-end"));
    all.extend(run(Style::InlineToneAfterVowel, "tone-after-vowel"));
    all.extend(run(Style::InlineToneMid, "tone-mid"));
    all.extend(run(Style::HornDeferred, "w-deferred"));
    for f in all.iter().take(400) {
        eprintln!("{f}");
    }
    assert!(all.is_empty(), "{} failures", all.len());
}

fn user_config() -> Config {
    Config {
        mechanism: mskey_core::Mechanism::CompatibleHook,
        macro_trigger_tab_only: true,
        smart_code_bypass: true,
        quick_w_to_u_horn: true,
        ..Config::default()
    }
}

fn case_keys(keys: &str, style: u8) -> String {
    match style {
        1 => {
            let mut cs = keys.chars();
            match cs.next() {
                Some(f) => f.to_ascii_uppercase().to_string() + cs.as_str(),
                None => String::new(),
            }
        }
        2 => keys.to_ascii_uppercase(),
        _ => keys.to_string(),
    }
}

fn case_word(w: &str, style: u8) -> String {
    match style {
        1 => {
            let mut cs = w.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
                None => String::new(),
            }
        }
        2 => w.to_uppercase(),
        _ => w.to_string(),
    }
}

#[test]
fn corpus_capitalized_and_uppercase() {
    let mut fails = Vec::new();
    for w in words() {
        let Some(keys) = to_telex(w, Style::InlineToneEnd) else {
            continue;
        };
        for style in [1u8, 2] {
            let mut e = Engine::new(user_config());
            let got = simulate(&mut e, &format!("{} ", case_keys(&keys, style)));
            let want = format!("{} ", case_word(w, style));
            if normalize_tone_style(&got, true) != normalize_tone_style(&want, true) {
                fails.push(format!(
                    "case{style}: {} -> {got:?} want {want:?}",
                    case_keys(&keys, style)
                ));
            }
        }
    }
    for f in fails.iter().take(200) {
        eprintln!("{f}");
    }
    assert!(fails.is_empty(), "{} failures", fails.len());
}

const SENTENCES: &[&str] = &[
    "Hôm nay trời đẹp quá, chúng ta đi chơi nhé.",
    "Tôi muốn xem lại báo cáo tài chính của công ty trước khi gửi cho khách hàng.",
    "Người Việt Nam rất thương yêu nhau, luôn giúp đỡ những người khó khăn.",
    "Nước sông Hồng chảy qua Hà Nội, quê hương của nhiều thế hệ.",
    "Anh ấy đã thuyết phục được ban giám đốc phê duyệt dự án xây dựng trạm biến áp.",
    "Khuya rồi, sao em vẫn chưa ngủ? Mai còn phải đi học sớm đấy!",
    "Hệ thống quản lý công trình cần được kiểm tra kỹ lưỡng trước khi triển khai.",
    "Những ngày tháng ấy thật đáng nhớ, giường chiếu, giếng nước, gốc đa.",
    "Quyển sách này khuyến khích người đọc suy nghĩ độc lập và sáng tạo.",
    "Trường đại học tổ chức hội thảo về trí tuệ nhân tạo vào tuần sau.",
    "Mẹ nấu canh chua cá lóc, thơm ngon và rất đậm đà hương vị miền Tây.",
    "Bạn có thể gửi lại tài liệu hướng dẫn sử dụng phần mềm được không?",
    "Đường về quê ngoằn ngoèo, khúc khuỷu nhưng phong cảnh tuyệt đẹp.",
    "Chiều qua, ông bà ngoại ra vườn tưới rau, nhổ cỏ và bắt sâu.",
    "Cuộc họp sẽ diễn ra lúc chín giờ sáng thứ hai tại phòng khách.",
    "Thuở ấy, ở làng tôi, người ta hay khoe nhau chiếc xe đạp mới.",
    "Hướng dẫn viên du lịch giới thiệu về lịch sử của thành phố cổ kính.",
    "Huyện Yên Thế nổi tiếng với cuộc khởi nghĩa của nghĩa quân Hoàng Hoa Thám.",
    "Ước mơ của tôi là trở thành kỹ sư phần mềm giỏi, được mọi người tin tưởng.",
    "Xin lỗi, tôi không hiểu ý anh. Anh nói lại được không ạ?",
];

fn sentence_to_keys(s: &str) -> Option<String> {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| -> Option<()> {
        if word.is_empty() {
            return Some(());
        }
        let lower = word.to_lowercase();
        let keys = if lower.is_ascii() {
            lower.clone()
        } else {
            to_telex(&lower, Style::InlineToneEnd)?
        };
        let first_upper = word.chars().next().is_some_and(char::is_uppercase);
        let all_upper = word.chars().count() > 1 && word.chars().all(|c| !c.is_lowercase());
        let style = if all_upper {
            2
        } else if first_upper {
            1
        } else {
            0
        };
        out.push_str(&case_keys(&keys, style));
        word.clear();
        Some(())
    };
    for c in s.chars() {
        if c.is_alphabetic() {
            word.push(c);
        } else {
            flush(&mut word, &mut out)?;
            out.push(c);
        }
    }
    flush(&mut word, &mut out)?;
    Some(out)
}

#[test]
fn corpus_sentences() {
    let mut fails = Vec::new();
    for cfg in [Config::default(), user_config()] {
        for s in SENTENCES {
            let keys = sentence_to_keys(s).unwrap();
            let mut e = Engine::new(cfg.clone());
            let got = simulate(&mut e, &format!("{keys} "));
            let want = format!("{s} ");
            if normalize_tone_style(&got, true) != normalize_tone_style(&want, true) {
                fails.push(format!("{keys}\n  got  {got}\n  want {want}"));
            }
        }
    }
    for f in &fails {
        eprintln!("{f}");
    }
    assert!(fails.is_empty(), "{} failures", fails.len());
}

#[test]
fn abbreviations_with_d_stroke() {
    let cases = [
        ("hdd ", "hđ "),
        ("ddc ", "đc "),
        ("ddk ", "đk "),
        ("vdd ", "vđ "),
        ("ddt ", "đt "),
        ("ddb ", "đb "),
        ("tdd ", "tđ "),
        ("dd ", "đ "),
        ("Hdd ", "Hđ "),
        ("DDc ", "Đc "),
        ("hddd ", "hdd "),
        ("dddc ", "ddc "),
        ("hdd, ", "hđ, "),
        ("ddc.", "đc."),
    ];
    let mut fails = Vec::new();
    for cfg in [Config::default(), user_config()] {
        for (k, want) in cases {
            let mut e = Engine::new(cfg.clone());
            let got = simulate(&mut e, k);
            if got != want {
                fails.push(format!("{k:?} -> {got:?} want {want:?}"));
            }
        }
    }
    for f in &fails {
        eprintln!("{f}");
    }
    assert!(fails.is_empty(), "{} failures", fails.len());
}

#[test]
fn every_vietnamese_letter_and_toned_vowel_in_both_cases() {
    for method in [
        InputMethod::Telex,
        InputMethod::SimpleTelex,
        InputMethod::Vni,
        InputMethod::Viqr,
    ] {
        let config = Config {
            input_method: method,
            ..Config::default()
        };
        let check = |keys: &str, expected: char| {
            for (input, output) in [
                (keys.to_string(), expected.to_string()),
                (
                    keys.to_ascii_uppercase(),
                    expected.to_uppercase().to_string(),
                ),
            ] {
                let composed = simulate(&mut Engine::new(config.clone()), &input);
                assert_eq!(composed, output, "{method:?} {input:?} before delimiter");
                let literal_config = Config {
                    auto_restore_wrong_word: false,
                    ..config.clone()
                };
                let committed = simulate(&mut Engine::new(literal_config), &format!("{input} "));
                assert_eq!(
                    committed,
                    format!("{output} "),
                    "{method:?} {input:?} after space"
                );
            }
        };
        for &(row, base, telex_shape) in VOWEL_ROWS {
            let (shape, tones) = match (method, telex_shape, base) {
                (InputMethod::Vni, "", _) => ("", " 21345"),
                (InputMethod::Vni, "w", 'a') => ("8", " 21345"),
                (InputMethod::Vni, "w", _) => ("7", " 21345"),
                (InputMethod::Vni, _, _) => ("6", " 21345"),
                (InputMethod::Viqr, "", _) => ("", " `'?~."),
                (InputMethod::Viqr, "w", 'a') => ("(", " `'?~."),
                (InputMethod::Viqr, "w", _) => ("+", " `'?~."),
                (InputMethod::Viqr, _, _) => ("^", " `'?~."),
                _ => (telex_shape, " fsrxj"),
            };
            for (vowel, tone) in row.chars().zip(tones.chars()) {
                let mut keys = format!("{base}{shape}");
                if tone != ' ' {
                    keys.push(tone);
                }
                check(&keys, vowel);
            }
        }
        for consonant in "bcdghklmnpqrstvx".chars() {
            check(&consonant.to_string(), consonant);
        }
        check(
            if method == InputMethod::Vni {
                "d9"
            } else {
                "dd"
            },
            'đ',
        );
    }
}

#[test]
fn continuous_sentences_preserve_words_and_boundaries_with_early_and_late_tones() {
    let expected =
        "và VÀ Và, tiếng Việt vẫn đủ chữ!\nGõ nhanh: trường học, ngõ nhỏ và chiếc thuyền!\t";
    let telex_early = "vaf VAF Vaf, tieesng Vieejt vaaxn ddur chuwx!\nGox nhanh: truwowfng hojc, ngox nhor vaf chieesc thuyeefn!\t";
    let telex_late = "vaf VAF Vaf, tieengs Vieetj vaanx ddur chuwx!\nGox nhanh: truwowngf hocj, ngox nhor vaf chieecs thuyeenf!\t";
    let cases = [
        (InputMethod::Telex, telex_early, telex_late),
        (InputMethod::SimpleTelex, telex_early, telex_late),
        (
            InputMethod::Vni,
            "va2 VA2 Va2, tie61ng Vie65t va64n d9u3 chu74!\nGo4 nhanh: tru7o72ng ho5c, ngo4 nho3 va2 chie61c thuye62n!\t",
            "va2 VA2 Va2, tie6ng1 Vie6t5 va6n4 d9u3 chu74!\nGo4 nhanh: tru7o7ng2 hoc5, ngo4 nho3 va2 chie6c1 thuye6n2!\t",
        ),
        (
            InputMethod::Viqr,
            "va` VA` Va`, tie^'ng Vie^.t va^~n ddu? chu+~!\nGo~ nhanh: tru+o+`ng ho.c, ngo~ nho? va` chie^'c thuye^`n!\t",
            "va` VA` Va`, tie^ng' Vie^t. va^n~ ddu? chu+~!\nGo~ nhanh: tru+o+ng` hoc., ngo~ nho? va` chie^c' thuye^n`!\t",
        ),
    ];
    for (method, early, late) in cases {
        let mut engine = Engine::new(Config {
            input_method: method,
            ..Config::default()
        });
        let input = format!("{early}{late}").repeat(32);
        let output = simulate(&mut engine, &input);
        assert_eq!(output, expected.repeat(64), "{method:?}");
    }
}
