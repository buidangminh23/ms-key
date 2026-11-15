use mskey_core::{
    AppBehavior, CaseTransform, Charset, Config, ConvertEncoding, CustomKeyMap, Engine,
    EngineAction, InputMethod, SwitchPreset, convert_text, convert_with_encoding,
    detect_convert_encoding, strip_vietnamese_diacritics,
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

    let mut vni_cfg = Config::default();
    vni_cfg.charset = Charset::VniWindows;
    let mut engine = Engine::new(vni_cfg);
    let screen = simulate(&mut engine, "has\x08o ");
    assert_eq!(screen, "ho ");

    let mut nfd_cfg = Config::default();
    nfd_cfg.charset = Charset::UnicodeComposite;
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
    let mut cfg = Config::default();
    cfg.macros = Config::parse_macro_text("# cmt\nddn:Đà Nẵng\ndt:{date}\nday:{time}\\nOk\n");
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
