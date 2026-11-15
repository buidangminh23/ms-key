use mskey_core::{
    AppBehavior, CaseTransform, Charset, Config, CustomKeyMap, Engine, EngineAction, InputMethod,
    MacroEntry, convert_text,
};

fn simulate(engine: &mut Engine, input: &str) -> String {
    let mut screen = String::new();
    for ch in input.chars() {
        match ch {
            '\x08' => {
                let act = engine.process_backspace();
                if matches!(act, EngineAction::PassThrough) {
                    screen.pop();
                } else {
                    apply_action(&mut screen, act, None);
                }
            }
            '\x1B' => {
                apply_action(&mut screen, engine.process_escape(), None);
            }
            _ => {
                apply_action(&mut screen, engine.process_key(ch), Some(ch));
            }
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
fn telex_basic_vietnamese_sentences() {
    let out = type_with(|_| {}, "Tieesng Vieejt raast tuyeejt vowfi!");
    assert_eq!(out, "Tiếng Việt rất tuyệt vời!");
}

#[test]
fn telex_free_tone_and_uo_cluster() {
    let out = type_with(
        |_| {},
        "truwowngf truwongf hojc, nguwoif, nguowif, nguoiwf, dduowcj, muoiwfs ",
    );
    assert_eq!(
        out,
        "trường trường học, người, người, người, được, mưới "
    );
    let free = type_with(|_| {}, "hoafng quoosc gia ");
    assert_eq!(free, "hoàng quốc gia ");
}

#[test]
fn telex_d_stroke_and_dd_toggle() {
    assert_eq!(type_with(|_| {}, "ddi ddoong"), "đi đông");
    assert_eq!(type_with(|_| {}, "DDi DDoong DDAA"), "Đi Đông ĐÂ");
    assert_eq!(type_with(|_| {}, "dddi"), "ddi");
}

#[test]
fn telex_gi_and_qu_tone_placement() {
    assert_eq!(
        type_with(|_| {}, "giaf gif gioosng quoosc quaas"),
        "già gì giống quốc quấ"
    );
    assert_eq!(type_with(|_| {}, "giuwax quar quyeets"), "giữa quả quyết");
}

#[test]
fn telex_modern_vs_classic_tone_placement() {
    assert_eq!(
        type_with(|c| c.modern_tone = true, "hoaf tuys uyas"),
        "hoà tuý uýa"
    );
    assert_eq!(
        type_with(|c| c.modern_tone = false, "hoaf tuys oas"),
        "hòa túy óa"
    );
}

#[test]
fn telex_triple_o_and_double_key_revert() {
    assert_eq!(type_with(|_| {}, "booongs xooong "), "boóng xoong ");
    assert_eq!(type_with(|_| {}, "boook seeen "), "book seen ");
    assert_eq!(type_with(|_| {}, "toooi"), "tooi");
    assert_eq!(type_with(|_| {}, "toiss"), "tois");
}

#[test]
fn telex_w_and_bracket_shortcuts() {
    assert_eq!(type_with(|_| {}, "w ww "), "ư w ");
    assert_eq!(type_with(|_| {}, "[ ] [[ ]] "), "ơ ư [ ] ");
    assert_eq!(type_with(|_| {}, "[[hello] "), "[hello] ");
    assert_eq!(
        type_with(
            |c| {
                c.quick_w_to_u_horn = false;
                c.quick_brackets = false;
            },
            "w [ ]"
        ),
        "w [ ]"
    );
}

#[test]
fn telex_esc_restore() {
    assert_eq!(type_with(|_| {}, "tieesng\x1B"), "tieesng");
    assert_eq!(type_with(|_| {}, "hello\x1B"), "hello");
}

#[test]
fn telex_backspace_and_word_boundary() {
    assert_eq!(type_with(|_| {}, "tieesn\x08ng"), "tiếng");
    assert_eq!(type_with(|_| {}, "con mefo\x08ss"), "con mes");
    assert_eq!(type_with(|_| {}, "con mefo\x08sss"), "con mess");
    assert_eq!(type_with(|_| {}, "con mefo\x08s"), "con mé");
    assert_eq!(type_with(|_| {}, "meo \x08\x08\x08\x08cho"), "cho");
    assert_eq!(type_with(|_| {}, "meo  \x08f"), "meo f");
}

#[test]
fn vni_input_method() {
    let out = type_with(
        |c| c.input_method = InputMethod::Vni,
        "Tie6ng1 Vie6t5 d9e6m nay ra6t1 d9e5p!",
    );
    assert_eq!(out, "Tiếng Việt đêm nay rất đẹp!");
    let uo = type_with(
        |c| c.input_method = InputMethod::Vni,
        "truong72 nguoi72 boong1 ",
    );
    assert_eq!(uo, "trường người boóng ");
}

#[test]
fn viqr_input_method() {
    let out = type_with(
        |c| c.input_method = InputMethod::Viqr,
        "Tie^'ng Vie^.t dde^m nay ra^'t dde.p!",
    );
    assert_eq!(out, "Tiếng Việt đêm nay rất đẹp!");
}

#[test]
fn simple_telex_no_w_standalone_or_brackets() {
    let out = type_with(
        |c| c.input_method = InputMethod::SimpleTelex,
        "tieesng vieejt [w] ",
    );
    assert_eq!(out, "tiếng việt [w] ");
}

#[test]
fn custom_keymap_input_method() {
    let out = type_with(
        |c| {
            c.input_method = InputMethod::Custom;
            c.custom_keymap = CustomKeyMap {
                acute: '1',
                grave: '2',
                circumflex_a: 'a',
                circumflex_e: 'e',
                d_stroke: 'd',
                ..CustomKeyMap::default()
            };
        },
        "tiee1ng ddee2m ",
    );
    assert_eq!(out, "tiếng đềm ");
}

#[test]
fn english_word_auto_restore() {
    let words = "window keep export push server word text their looks async await software format world cheese session less address ";
    let out = type_with(|c| c.auto_restore_wrong_word = true, words);
    assert_eq!(out, words);
}

#[test]
fn macro_expansion_enabled_and_when_english() {
    let out = type_with(
        |c| {
            c.macros = vec![MacroEntry {
                shortcut: "ms".into(),
                expansion: "MS Key".into(),
                enabled: true,
            }];
            c.macro_enabled = true;
        },
        "ms ",
    );
    assert_eq!(out, "MS Key ");

    let en = type_with(
        |c| {
            c.vietnamese_enabled = false;
            c.macro_enabled = true;
            c.macro_when_english = true;
            c.macros = vec![MacroEntry {
                shortcut: "vn".into(),
                expansion: "Việt Nam".into(),
                enabled: true,
            }];
        },
        "vn ",
    );
    assert_eq!(en, "Việt Nam ");
}

#[test]
fn auto_capitalize_after_sentence_end() {
    let out = type_with(
        |c| c.auto_capitalize_sentence = true,
        "chao. tieesng vieejt! mooix ngayf? vui ",
    );
    assert_eq!(out, "chao. Tiếng việt! Mỗi ngày? Vui ");
}

#[test]
fn evkey_tree_import_and_clipboard_converters() {
    let tree = "\
setting{
  Codec=4
  Type=1
  SModern=1
  SCheck=1
  SConso=1
}
on_vn{
  code.exe
}
off_vn{
  terminal.exe
}
blacklist{
  game.exe
}
";
    let cfg = Config::import_evkey_tree(tree);
    assert_eq!(cfg.behavior_for_app("code.exe"), AppBehavior::ForceVietnamese);
    assert_eq!(cfg.behavior_for_app("terminal.exe"), AppBehavior::ForceEnglish);
    assert_eq!(cfg.behavior_for_app("game.exe"), AppBehavior::Blacklist);
    assert!(cfg.modern_tone);
    assert!(cfg.spelling_check);

    let sample_tcvn = "tiếng việt đẹp ăn ở ươm ĐĂÂÊÔƠƯ";
    let tcvn3 = convert_text(sample_tcvn, Charset::Unicode, Charset::Tcvn3, CaseTransform::None);
    let back_tcvn3 = convert_text(&tcvn3, Charset::Tcvn3, Charset::Unicode, CaseTransform::None);
    assert_eq!(back_tcvn3, sample_tcvn);

    let sample = "Tiếng Việt Đẹp Ăn Ở Ươm";
    let vni = convert_text(sample, Charset::Unicode, Charset::VniWindows, CaseTransform::None);
    let back_vni = convert_text(&vni, Charset::VniWindows, Charset::Unicode, CaseTransform::None);
    assert_eq!(back_vni, sample);

    let nfd = convert_text(sample, Charset::Unicode, Charset::UnicodeComposite, CaseTransform::None);
    assert_ne!(nfd, sample);
    let nfc = convert_text(&nfd, Charset::UnicodeComposite, Charset::Unicode, CaseTransform::None);
    assert_eq!(nfc, sample);

    let titled = convert_text(
        "tiếng việt nam",
        Charset::Unicode,
        Charset::Unicode,
        CaseTransform::Title,
    );
    assert_eq!(titled, "Tiếng Việt Nam");
}

#[test]
fn standalone_marked_letters_survive_auto_restore() {
    assert_eq!(type_with(|_| {}, "dd "), "đ ");
    assert_eq!(type_with(|_| {}, "DD "), "Đ ");
    assert_eq!(type_with(|_| {}, "aa "), "â ");
    assert_eq!(type_with(|_| {}, "ee "), "ê ");
    assert_eq!(type_with(|_| {}, "oo "), "ô ");
    assert_eq!(type_with(|_| {}, "aw "), "ă ");
    assert_eq!(type_with(|_| {}, "ow "), "ơ ");
    assert_eq!(type_with(|_| {}, "uw "), "ư ");
    assert_eq!(type_with(|_| {}, "500k dd "), "500k đ ");
    assert_eq!(
        type_with(|c| c.input_method = InputMethod::Vni, "d9 a6 "),
        "đ â "
    );
}

#[test]
fn english_short_words_still_restore() {
    assert_eq!(type_with(|_| {}, "add odd ass "), "add odd ass ");
    assert_eq!(type_with(|_| {}, "ass "), "ass ");
    assert_eq!(type_with(|_| {}, "dddi "), "ddi ");
}
