use mskey_core::{
    AppBehavior, AppRule, CaseTransform, Charset, Config, ConvertEncoding, CustomKeyMap, Engine,
    EngineAction, InputMethod, SwitchPreset, convert_text, convert_with_encoding,
    detect_convert_encoding, normalize_tone_style, strip_vietnamese_diacritics, update,
};

fn simulate(engine: &mut Engine, input: &str) -> String {
    let mut screen = String::new();
    for ch in input.chars() {
        match ch {
            '\x08' => match engine.process_backspace() {
                EngineAction::PassThrough => {
                    screen.pop();
                }
                act @ EngineAction::Replace { .. } => apply_action(&mut screen, act, None),
            },
            '\x1B' => apply_action(&mut screen, engine.process_escape(), None),
            _ => apply_action(&mut screen, engine.process_key(ch), Some(ch)),
        }
    }
    screen
}

fn apply_action(screen: &mut String, action: EngineAction, passthrough_ch: Option<char>) {
    match action {
        EngineAction::PassThrough => {
            if let Some(ch) = passthrough_ch {
                screen.push(ch);
            }
        }
        EngineAction::Replace { backspaces, text } => {
            for _ in 0..backspaces {
                screen.pop();
            }
            screen.push_str(&text);
        }
    }
}

fn type_with(cfg_fn: impl FnOnce(&mut Config), input: &str) -> String {
    let mut cfg = Config::default();
    cfg_fn(&mut cfg);
    let mut engine = Engine::new(cfg);
    simulate(&mut engine, input)
}

#[test]
fn uo_open_and_closed_syllables_and_gieng() {
    assert_eq!(type_with(|_| {}, "thuowr "), "thuở ");
    assert_eq!(type_with(|_| {}, "thuorw "), "thuở ");
    assert_eq!(type_with(|_| {}, "huow "), "huơ ");
    assert_eq!(type_with(|_| {}, "tuowi "), "tươi ");
    assert_eq!(type_with(|_| {}, "thuowng "), "thương ");
    assert_eq!(type_with(|_| {}, "gieeng gieengf gieengs gieecs "), "giêng giềng giếng giếc ");
    assert_eq!(type_with(|_| {}, "quyts quytj "), "quýt quỵt ");
    assert_eq!(type_with(|c| c.input_method = InputMethod::Vni, "quyt5 quyt2 quyt52 "), "quỵt quyt2 quyt52 ");
}

#[test]
fn user_dict_and_ethnic_words_preserved_on_auto_restore() {
    let dak_lak = type_with(|_| {}, "DDawsk Lawsk ");
    assert_eq!(dak_lak, "Đắk Lắk ");
    let custom = type_with(
        |c| {
            c.spelling_check = false;
            c.user_dict_words = vec!["brô".into(), "krông".into()];
        },
        "broo kroong ",
    );
    assert_eq!(custom, "brô krông ");
}

#[test]
fn backspace_syncs_raw_and_multi_unit_charsets() {
    assert_eq!(type_with(|_| {}, "haf\x08ello "), "hello ");
    assert_eq!(type_with(|_| {}, "how\x08ello "), "hello ");
    assert_eq!(type_with(|_| {}, "cas\x08ane "), "cane ");
    assert_eq!(type_with(|c| c.modern_tone = false, "toasn\x08 "), "tóa ");

    let vni_cfg = Config {
        charset: Charset::VniWindows,
        ..Config::default()
    };
    let mut engine = Engine::new(vni_cfg);
    let screen = simulate(&mut engine, "has\x08o ");
    assert_eq!(screen, "ho ");

    let nfd_cfg = Config {
        charset: Charset::UnicodeComposite,
        ..Config::default()
    };
    let mut nfd_engine = Engine::new(nfd_cfg);
    let nfd_screen = simulate(&mut nfd_engine, "bas\x08a ");
    assert_eq!(nfd_screen, "ba ");
}

#[test]
fn quick_telex_start_and_end_consonants() {
    let qt = type_with(
        |c| c.quick_telex = true,
        "ccao ggaf kko nnayf ppoox qqa ttao ",
    );
    assert_eq!(qt, "chao già kho ngày phỗ qua thao ");

    let qs = type_with(|c| c.quick_start_consonant = true, "faf jaf waf free ");
    assert_eq!(qs, "phà già quà free ");

    let qe = type_with(
        |c| c.quick_end_consonant = true,
        "tieegs tihs khaks book ",
    );
    assert_eq!(qe, "tiếng tính khách book ");
}

#[test]
fn revert_guards_and_custom_keymap_options() {
    assert_eq!(type_with(|c| c.input_method = InputMethod::Vni, "a666 a888 o777 "), "a66 a88 o77 ");
    assert_eq!(type_with(|c| c.input_method = InputMethod::Viqr, "a^^^ "), "a^^ ");
    let custom = type_with(
        |c| {
            c.input_method = InputMethod::Custom;
            c.quick_w_to_u_horn = true;
            c.quick_brackets = true;
            c.custom_keymap = CustomKeyMap {
                horn_o: ']',
                horn_u: 'w',
                quick_u_horn: 'v',
                quick_o_horn_key: ';',
                quick_u_horn_key: '\'',
                ..CustomKeyMap::default()
            };
        },
        "v ; ' tuw to] ",
    );
    assert_eq!(custom, "ư ơ ư tư tơ ");
}

#[test]
fn sentence_autocap_ignores_urls_and_requires_whitespace() {
    let out = type_with(
        |c| c.auto_capitalize_sentence = true,
        "xem gonhanh.org nhes. tooi vui! ok",
    );
    assert_eq!(out, "xem gonhanh.org nhé. Tôi vui! Ok");
}

#[test]
fn macro_raw_match_dynamic_tokens_and_serialization() {
    let cfg = Config {
        macros: Config::parse_macro_text("# cmt\nddn:Đà Nẵng\ndt:{date}\nday:{time}\\nOk\n"),
        ..Config::default()
    };
    let mut engine = Engine::new(cfg.clone());
    engine.set_clock_offset_secs(7 * 3_600);
    assert_eq!(simulate(&mut engine, "ddn "), "Đà Nẵng ");
    let dt_out = simulate(&mut engine, "dt ");
    assert_eq!(dt_out.trim().len(), 10);
    assert!(dt_out.contains('/'));
    let multiline = simulate(&mut engine, "day ");
    assert!(multiline.contains('\n') && multiline.ends_with("Ok "));
    let exported = cfg.format_macro_text();
    assert!(exported.contains("day:{time}\\nOk"));
}

#[test]
fn encoding_detect_viqr_strip_title_and_evkey_import() {
    assert_eq!(strip_vietnamese_diacritics("Tiếng Việt Đắk Lắk"), "Tieng Viet Dak Lak");
    assert_eq!(
        convert_text("don't stop o'clock", Charset::Unicode, Charset::Unicode, CaseTransform::Title),
        "Don't Stop O'clock"
    );
    assert_eq!(
        convert_text("Tiếng Việt", Charset::Unicode, Charset::Unicode, CaseTransform::StripDiacritics),
        "Tieng Viet"
    );

    let viqr = convert_with_encoding(
        "Tiếng Việt Đắk",
        ConvertEncoding::Unicode,
        ConvertEncoding::Viqr,
        CaseTransform::None,
    );
    assert_eq!(viqr, "Tie^'ng Vie^.t DDa('k");
    let back = convert_with_encoding(&viqr, ConvertEncoding::Viqr, ConvertEncoding::Unicode, CaseTransform::None);
    assert_eq!(back, "Tiếng Việt Đắk");
    assert_eq!(detect_convert_encoding(&viqr), ConvertEncoding::Viqr);

    let tcvn = convert_text("Tiếng Việt", Charset::Unicode, Charset::Tcvn3, CaseTransform::None);
    let vni = convert_text("Tiếng Việt", Charset::Unicode, Charset::VniWindows, CaseTransform::None);
    let nfd = convert_text("Tiếng Việt", Charset::Unicode, Charset::UnicodeComposite, CaseTransform::None);
    assert_eq!(detect_convert_encoding(&tcvn), ConvertEncoding::Tcvn3);
    assert_eq!(detect_convert_encoding(&vni), ConvertEncoding::VniWindows);
    assert_eq!(detect_convert_encoding(&nfd), ConvertEncoding::UnicodeComposite);
    assert_eq!(detect_convert_encoding("Con mèo"), ConvertEncoding::Unicode);

    let ini = "setting{\nQTelex=1\nSStartConsonant=1\nSEndConsonant=1\nSKey=4\nShowOSD=0\n}\n[macro]\nddn:Đà Nẵng\\nVN\non_vn{\n*chrome*\n}\n";
    let imported = Config::import_evkey_tree(ini);
    assert!(imported.quick_telex && imported.quick_start_consonant && imported.quick_end_consonant);
    assert_eq!(imported.switch_preset, SwitchPreset::ShiftShift);
    assert!(!imported.show_switch_osd);
    assert_eq!(imported.behavior_for_app("Google-chrome.exe"), AppBehavior::ForceVietnamese);
    assert_eq!(imported.macros.len(), 1);
    assert_eq!(imported.macros[0].expansion, "Đà Nẵng\nVN");
}

#[test]
fn smart_code_bypass_handles_code_urls_paths_and_emails() {
    let cfg = Config {
        smart_code_bypass: true,
        quick_w_to_u_horn: true,
        ..Default::default()
    };
    let mut engine = Engine::new(cfg.clone());

    assert_eq!(simulate(&mut engine, "getElementById "), "getElementById ");
    assert_eq!(simulate(&mut engine, "addEventListener "), "addEventListener ");
    assert_eq!(simulate(&mut engine, "iOS "), "iOS ");
    assert_eq!(simulate(&mut engine, "Tooi "), "Tôi ");
    assert_eq!(simulate(&mut engine, "if_else max_size is_valid as_str or_else "), "if_else max_size is_valid as_str or_else ");
    assert_eq!(simulate(&mut engine, "too_many __init__ "), "too_many __init__ ");
    assert_eq!(simulate(&mut engine, "v2s sha256 foo2 "), "v2s sha256 foo2 ");
    assert_eq!(simulate(&mut engine, "https://example.com/tooi "), "https://example.com/tooi ");
    assert_eq!(simulate(&mut engine, "wwww.google.com/tooi "), "www.google.com/tooi ");
    assert_eq!(simulate(&mut engine, "foo@bar.vn "), "foo@bar.vn ");
    assert_eq!(simulate(&mut engine, "C:\\Users\\tooi\\file "), "C:\\Users\\tooi\\file ");
    assert_eq!(simulate(&mut engine, "./src/tooi ../docs/vi ~/tooi "), "./src/tooi ../docs/vi ~/tooi ");
    assert_eq!(simulate(&mut engine, "tooi "), "tôi ");

    let simple_cfg = Config {
        quick_w_to_u_horn: false,
        smart_code_bypass: true,
        ..Default::default()
    };
    let mut simple_engine = Engine::new(simple_cfg);
    assert_eq!(simulate(&mut simple_engine, "www.google.com/tooi "), "www.google.com/tooi ");

    let vni_cfg = Config {
        input_method: InputMethod::Vni,
        smart_code_bypass: true,
        ..Default::default()
    };
    let mut vni = Engine::new(vni_cfg);
    assert_eq!(simulate(&mut vni, "quyt5 "), "quỵt ");
    assert_eq!(simulate(&mut vni, "quyt1 "), "quýt ");
}

#[test]
fn temporary_english_undo_last_word_and_hydrate_reedit() {
    let mut engine = Engine::new(Config::default());
    assert_eq!(simulate(&mut engine, "vieetj "), "việt ");
    engine.set_temporary_english(true);
    assert!(engine.config().vietnamese_enabled);
    assert!(engine.is_temporary_english());
    assert_eq!(simulate(&mut engine, "vieetj "), "vieetj ");
    engine.set_temporary_english(false);
    assert_eq!(simulate(&mut engine, "vieetj "), "việt ");

    let mut screen = String::from("việt ");
    let act1 = engine.toggle_last_committed_word();
    apply_action(&mut screen, act1, None);
    assert_eq!(screen, "vieetj ");
    let act2 = engine.toggle_last_committed_word();
    apply_action(&mut screen, act2, None);
    assert_eq!(screen, "việt ");

    let mut reedit = Engine::new(Config::default());
    reedit.hydrate_word("xin tiếng");
    assert_eq!(reedit.current_word(), "tiếng");
    let mut reedit_screen = String::from("xin tiếng");
    apply_action(&mut reedit_screen, reedit.process_key('f'), Some('f'));
    assert_eq!(reedit_screen, "xin tiềng");
}

#[test]
fn macro_cursor_clipboard_dates_tab_only_and_esc_user_dict() {
    let cfg = Config {
        macros: Config::parse_macro_text(
            "tag:<b>{cursor}</b>\ncb:Hi {clipboard}\ndt:{year}-{month}-{day} {date:YYYYMMDD} {weekday}\n",
        ),
        macro_trigger_tab_only: true,
        ..Default::default()
    };
    let mut engine = Engine::new(cfg.clone());
    engine.set_clock_offset_secs(0);
    engine.set_clipboard_text(Some("{date} $| raw".to_string()));

    assert_eq!(simulate(&mut engine, "tag "), "tag ");
    assert_eq!(engine.take_pending_cursor_left(), 0);

    let tab_tag = simulate(&mut engine, "tag\t");
    assert_eq!(tab_tag, "<b></b>");
    assert_eq!(engine.take_pending_cursor_left(), 4);

    let cb_out = simulate(&mut engine, "cb\t");
    assert_eq!(cb_out, "Hi {date} $| raw");
    assert_eq!(engine.take_pending_cursor_left(), 0);

    let dt_out = simulate(&mut engine, "dt\t");
    let dt_parts: Vec<&str> = dt_out.split_whitespace().collect();
    assert!(dt_parts.len() >= 3);
    assert_eq!(dt_parts[0].replace('-', ""), dt_parts[1]);
    assert!(dt_out.contains("Thứ ") || dt_out.contains("Chủ Nhật"));

    let mut esc_cfg = Config {
        auto_restore_wrong_word: true,
        ..Default::default()
    };
    let mut esc_engine = Engine::new(esc_cfg.clone());
    let s = simulate(&mut esc_engine, "boo\x1B ");
    assert_eq!(s, "boo ");
    assert_eq!(esc_engine.last_esc_word(), Some("boo"));
    assert_eq!(esc_engine.last_word_for_dict().as_deref(), Some("boo"));

    esc_cfg.user_dict_words = Config::parse_user_dict_text(&format!(
        "{}\n{}",
        esc_cfg.format_user_dict_text(),
        "boo"
    ));
    let mut dict_engine = Engine::new(esc_cfg);
    assert_eq!(simulate(&mut dict_engine, "boo "), "boo ");
}

#[test]
fn tone_normalization_window_rules_profiles_and_update_helpers() {
    assert_eq!(normalize_tone_style("hòa khóa thúy English", true), "hoà khoá thuý English");
    assert_eq!(normalize_tone_style("hoà khoá thuý English", false), "hòa khóa thúy English");
    assert_eq!(
        convert_text("hòa", Charset::Unicode, Charset::Unicode, CaseTransform::NormalizeModernTone),
        "hoà"
    );

    let mut cfg = Config::default();
    cfg.app_rules.push(AppRule {
        process_name: "code.exe".to_string(),
        title_pattern: "*terminal*".to_string(),
        behavior: AppBehavior::ForceEnglish,
    });
    cfg.set_app_rule("code.exe", AppBehavior::ForceVietnamese);
    assert_eq!(
        cfg.behavior_for_window("code.exe", "bash - Terminal"),
        AppBehavior::ForceEnglish
    );
    assert_eq!(
        cfg.behavior_for_window("code.exe", "README.md"),
        AppBehavior::ForceVietnamese
    );

    assert!(cfg.apply_profile("Lập trình"));
    assert!(cfg.smart_code_bypass);
    assert!(cfg.macro_trigger_tab_only);
    cfg.modern_tone = false;
    cfg.save_current_as_profile("CustomTest");
    assert!(cfg.apply_profile("Soạn thảo"));
    assert!(cfg.auto_capitalize_sentence);
    assert!(cfg.apply_profile("CustomTest"));
    assert!(!cfg.modern_tone);
    assert!(update::is_newer_version("0.1.2", "v0.1.3"));
}
