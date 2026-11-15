use mskey_core::{AppBehavior, Config, HotkeyCombo, InputMethod, Mechanism, SwitchPreset};
use mskey_linux::config_store::{parse_ibus_bus_file, parse_tz_offset, render_ibus_component_xml};
use mskey_linux::ibus_logic::{
    IBUS_CAP_PREEDIT_TEXT, IBUS_CAP_SURROUNDING_TEXT, IbusOp, IbusSession,
};
use mskey_linux::keyval::{
    HotkeySignal, HotkeyTracker, IBUS_CONTROL_MASK, IBUS_MOD1_MASK, IBUS_RELEASE_MASK,
    IBUS_SHIFT_MASK, KEY_BACKSPACE, KEY_CONTROL_L, KEY_ESCAPE, KEY_KP_SPACE, KEY_RETURN,
    KEY_RIGHT, KEY_SHIFT_L, format_hotkey_linux, gdk_keyval_matches_vk, gdk_keyval_to_vk,
    keyval_to_char,
};
use mskey_linux::linux_extra::{
    is_browser_app, is_terminal_app, is_us_xkb_layout, parse_gnome_eval_class,
    parse_gnome_mru_xkb, parse_hypr_active_class, parse_setxkbmap_layout, parse_sway_focused_app,
};

fn type_preedit_string(sess: &mut IbusSession, s: &str) -> Vec<String> {
    let mut committed = Vec::new();
    for ch in s.chars() {
        let res = sess.process_key(ch as u32, 0, 0);
        for op in res.ops {
            if let IbusOp::CommitText(t) = op {
                committed.push(t);
            }
        }
    }
    committed
}

#[test]
fn preedit_typing_tieng_viet_and_english_restore() {
    let cfg = Config {
        auto_restore_wrong_word: true,
        ..Config::default()
    };
    let mut sess = IbusSession::new(cfg);
    sess.set_capabilities(IBUS_CAP_PREEDIT_TEXT);

    let committed = type_preedit_string(&mut sess, "tieengs Vieejt ");
    assert_eq!(committed.concat(), "tiếng Việt ");

    let res_w = sess.process_key('w' as u32, 0, 0);
    assert!(res_w.handled);
    assert_eq!(res_w.ops, vec![IbusOp::UpdatePreedit("ư".to_string())]);

    let eng = type_preedit_string(&mut sess, "indow ");
    assert_eq!(eng.concat(), "window ");
}

#[test]
fn preedit_backspace_and_escape_restore() {
    let mut sess = IbusSession::new(Config::default());
    sess.set_capabilities(IBUS_CAP_PREEDIT_TEXT);
    for ch in "vieejt".chars() {
        let _ = sess.process_key(ch as u32, 0, 0);
    }
    assert_eq!(sess.preedit, "việt");

    let bs = sess.process_key(KEY_BACKSPACE, 14, 0);
    assert!(bs.handled);
    assert_eq!(sess.preedit, "việ");
    assert_eq!(bs.ops, vec![IbusOp::UpdatePreedit("việ".to_string())]);

    let esc = sess.process_key(KEY_ESCAPE, 1, 0);
    assert!(esc.handled);
    assert!(sess.preedit.is_empty());
    assert!(esc.ops.contains(&IbusOp::HidePreedit));
    assert!(esc.ops.iter().any(|op| matches!(op, IbusOp::CommitText(s) if s == "vieej")));
}

#[test]
fn direct_surrounding_and_forward_key_fallback() {
    let cfg = Config {
        mechanism: Mechanism::CompatibleHook,
        send_delay_ms: 0,
        ..Config::default()
    };
    let mut sess = IbusSession::new(cfg.clone());
    sess.set_capabilities(IBUS_CAP_SURROUNDING_TEXT);

    assert!(!sess.process_key('a' as u32, 0, 0).handled);
    let r = sess.process_key('s' as u32, 0, 0);
    assert!(r.handled);
    assert_eq!(
        r.ops,
        vec![IbusOp::DeleteSurrounding(-1, 1), IbusOp::CommitText("á".to_string())]
    );

    let mut fallback = IbusSession::new(cfg);
    fallback.set_capabilities(0);
    assert!(!fallback.process_key('a' as u32, 0, 0).handled);
    let r2 = fallback.process_key('s' as u32, 0, 0);
    assert!(r2.handled);
    assert_eq!(
        r2.ops,
        vec![IbusOp::ForwardKey(KEY_BACKSPACE, 14, 0), IbusOp::CommitText("á".to_string())]
    );
}

#[test]
fn hotkey_tracker_ctrl_shift_alt_z_and_convert() {
    let mut tracker = HotkeyTracker::default();
    let conv = HotkeyCombo { ctrl: true, shift: true, alt: false, win_cmd: false, key_code: 0x75 };
    let custom = HotkeyCombo::default();

    assert_eq!(
        tracker.process_key(SwitchPreset::CtrlShift, custom, conv, KEY_CONTROL_L, 0),
        HotkeySignal::None
    );
    assert_eq!(
        tracker.process_key(SwitchPreset::CtrlShift, custom, conv, KEY_SHIFT_L, IBUS_CONTROL_MASK),
        HotkeySignal::None
    );
    assert_eq!(
        tracker.process_key(
            SwitchPreset::CtrlShift,
            custom,
            conv,
            KEY_SHIFT_L,
            IBUS_CONTROL_MASK | IBUS_SHIFT_MASK | IBUS_RELEASE_MASK,
        ),
        HotkeySignal::ToggleVe
    );

    assert_eq!(
        tracker.process_key(SwitchPreset::AltZ, custom, conv, b'z' as u32, IBUS_MOD1_MASK),
        HotkeySignal::ToggleVe
    );
    assert_eq!(
        tracker.process_key(
            SwitchPreset::CtrlShift,
            custom,
            conv,
            0xffc3,
            IBUS_CONTROL_MASK | IBUS_SHIFT_MASK,
        ),
        HotkeySignal::ConvertClipboard
    );
    assert_eq!(format_hotkey_linux(conv), "Ctrl + Shift + F6");
    assert!(gdk_keyval_matches_vk(0x0D, KEY_RETURN));
    assert_eq!(gdk_keyval_to_vk(b'a' as u32), 0x41);
    assert_eq!(keyval_to_char(0xffb5), Some('5'));
}

#[test]
fn focus_in_app_rules_and_bus_xml_parsing() {
    let mut cfg = Config::default();
    cfg.set_app_rule("kitty", AppBehavior::ForceEnglish);
    cfg.set_app_rule("obs", AppBehavior::Blacklist);
    let mut sess = IbusSession::new(cfg);

    let ops_kitty = sess.focus_in("kitty");
    assert!(ops_kitty.contains(&IbusOp::SyncProperties));
    assert!(!sess.engine.config().vietnamese_enabled);

    let ops_gedit = sess.focus_in("gedit");
    assert!(ops_gedit.contains(&IbusOp::SyncProperties));
    assert!(sess.engine.config().vietnamese_enabled);

    let _ = sess.focus_in("obs");
    assert!(!sess.process_key('a' as u32, 0, 0).handled);

    let sample = "# IBus bus info\nIBUS_ADDRESS='unix:path=/run/user/1000/ibus.sock'\nIBUS_PID=42\n";
    assert_eq!(parse_ibus_bus_file(sample).as_deref(), Some("unix:path=/run/user/1000/ibus.sock"));
    let xml = render_ibus_component_xml("/usr/local/bin/mskey-linux", "/usr/local/bin/mskey-linux");
    assert!(xml.contains("<exec>/usr/local/bin/mskey-linux --ibus</exec>"));
    assert!(xml.contains("<name>mskey</name>"));
    assert!(xml.contains("<rank>99</rank>"));
    assert!(!xml.contains("AppData"));
}

#[test]
fn hotkey_tracker_ctrl_space_shift_shift_and_punctuation() {
    let mut tracker = HotkeyTracker::default();
    let custom = HotkeyCombo::default();
    let conv = HotkeyCombo::default();

    assert_eq!(
        tracker.process_key(SwitchPreset::CtrlSpace, custom, conv, b' ' as u32, IBUS_CONTROL_MASK),
        HotkeySignal::ToggleVe
    );
    assert_eq!(
        tracker.process_key(SwitchPreset::CtrlSpace, custom, conv, KEY_KP_SPACE, IBUS_CONTROL_MASK),
        HotkeySignal::ToggleVe
    );
    assert_eq!(
        tracker.process_key(
            SwitchPreset::CtrlSpace,
            custom,
            conv,
            b' ' as u32,
            IBUS_CONTROL_MASK | IBUS_MOD1_MASK
        ),
        HotkeySignal::None
    );

    assert_eq!(tracker.process_key(SwitchPreset::ShiftShift, custom, conv, KEY_SHIFT_L, 0), HotkeySignal::None);
    assert_eq!(
        tracker.process_key(
            SwitchPreset::ShiftShift,
            custom,
            conv,
            KEY_SHIFT_L,
            IBUS_SHIFT_MASK | IBUS_RELEASE_MASK
        ),
        HotkeySignal::None
    );
    assert_eq!(tracker.process_key(SwitchPreset::ShiftShift, custom, conv, KEY_SHIFT_L, 0), HotkeySignal::None);
    assert_eq!(
        tracker.process_key(
            SwitchPreset::ShiftShift,
            custom,
            conv,
            KEY_SHIFT_L,
            IBUS_SHIFT_MASK | IBUS_RELEASE_MASK
        ),
        HotkeySignal::ToggleVe
    );

    assert_eq!(tracker.process_key(SwitchPreset::ShiftShift, custom, conv, KEY_SHIFT_L, 0), HotkeySignal::None);
    assert_eq!(
        tracker.process_key(SwitchPreset::ShiftShift, custom, conv, b'A' as u32, IBUS_SHIFT_MASK),
        HotkeySignal::None
    );
    assert_eq!(
        tracker.process_key(
            SwitchPreset::ShiftShift,
            custom,
            conv,
            KEY_SHIFT_L,
            IBUS_SHIFT_MASK | IBUS_RELEASE_MASK
        ),
        HotkeySignal::None
    );

    assert_eq!(gdk_keyval_to_vk(b'[' as u32), 0xDB);
    assert!(gdk_keyval_matches_vk(0xBA, b';' as u32));
    assert_eq!(keyval_to_char(KEY_KP_SPACE), Some(' '));
    let semi = HotkeyCombo { ctrl: true, shift: false, alt: false, win_cmd: false, key_code: 0xBA };
    assert_eq!(format_hotkey_linux(semi), "Ctrl + ;");
}

#[test]
fn config_store_tz_and_wayland_parsers() {
    assert_eq!(parse_tz_offset("+0700"), Some(25_200));
    assert_eq!(parse_tz_offset("-05:30"), Some(-19_800));
    assert_eq!(parse_tz_offset("bad"), None);

    assert_eq!(parse_hypr_active_class(r#"{"class":"firefox","title":"Docs"}"#).as_deref(), Some("firefox"));
    assert_eq!(
        parse_sway_focused_app(r#"{"nodes":[{"focused":false,"nodes":[{"focused":true,"app_id":"foot"}]}]}"#).as_deref(),
        Some("foot")
    );
    assert_eq!(parse_gnome_eval_class(r#"(true, '"Alacritty"')"#).as_deref(), Some("Alacritty"));
    assert_eq!(parse_setxkbmap_layout("rules: evdev\nlayout: us,vn\n").as_deref(), Some("us"));
    assert_eq!(parse_gnome_mru_xkb("[('xkb', 'vn'), ('ibus', 'mskey')]").as_deref(), Some("vn"));
    assert!(is_us_xkb_layout("us"));
    assert!(!is_us_xkb_layout("vn"));
    assert!(is_browser_app("firefox"));
    assert!(is_terminal_app("Alacritty"));
}

#[test]
fn preedit_active_delimiter_and_direct_ops() {
    let mut cfg = Config {
        quick_brackets: true,
        ..Config::default()
    };
    let mut pre = IbusSession::new(cfg.clone());
    pre.set_capabilities(IBUS_CAP_PREEDIT_TEXT);
    assert!(pre.process_key('[' as u32, 0, 0).ops.contains(&IbusOp::UpdatePreedit("ơ".to_string())));

    cfg.input_method = InputMethod::Viqr;
    cfg.quick_brackets = false;
    let mut viqr = IbusSession::new(cfg.clone());
    viqr.set_capabilities(IBUS_CAP_PREEDIT_TEXT);
    let _ = viqr.process_key('a' as u32, 0, 0);
    assert!(viqr.process_key('^' as u32, 0, 0).ops.contains(&IbusOp::UpdatePreedit("â".to_string())));

    let dcfg = Config {
        mechanism: Mechanism::CompatibleHook,
        disable_on_non_us_layout: true,
        fix_browser_suggest_1: true,
        fix_browser_suggest_2: true,
        use_clipboard_send: true,
        show_switch_osd: true,
        ..Config::default()
    };

    let mut sess = IbusSession::new(dcfg);
    sess.set_capabilities(IBUS_CAP_SURROUNDING_TEXT);
    sess.set_us_layout(false);
    let _ = sess.process_key('a' as u32, 0, 0);
    assert!(!sess.process_key('s' as u32, 0, 0).handled);

    sess.set_us_layout(true);
    let _ = sess.focus_in("firefox");
    let _ = sess.process_key('a' as u32, 0, 0);
    let r = sess.process_key('s' as u32, 0, 0);
    assert!(r.handled);
    assert!(r.ops.contains(&IbusOp::CommitText("\u{200B}".to_string())));
    assert!(r.ops.contains(&IbusOp::ForwardKey(KEY_RIGHT, 114, 0)));
    assert!(r.ops.contains(&IbusOp::PasteViaClipboard("á".to_string(), false)));
    assert!(r.ops.contains(&IbusOp::Delay(2)));

    let osd_cfg = Config {
        switch_preset: SwitchPreset::AltZ,
        show_switch_osd: true,
        ..Config::default()
    };
    let mut osd_sess = IbusSession::new(osd_cfg);
    let t2 = osd_sess.process_key(b'z' as u32, 0, IBUS_MOD1_MASK);
    assert!(t2.ops.contains(&IbusOp::ShowOsd(false)));
}
