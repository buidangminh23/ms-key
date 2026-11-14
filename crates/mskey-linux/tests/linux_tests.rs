use mskey_core::{AppBehavior, Config, HotkeyCombo, Mechanism, SwitchPreset};
use mskey_linux::config_store::{parse_ibus_bus_file, render_ibus_component_xml};
use mskey_linux::ibus_logic::{
    IBUS_CAP_PREEDIT_TEXT, IBUS_CAP_SURROUNDING_TEXT, IbusOp, IbusSession,
};
use mskey_linux::keyval::{
    HotkeySignal, HotkeyTracker, IBUS_CONTROL_MASK, IBUS_MOD1_MASK, IBUS_RELEASE_MASK,
    IBUS_SHIFT_MASK, KEY_BACKSPACE, KEY_CONTROL_L, KEY_ESCAPE, KEY_RETURN, KEY_SHIFT_L,
    format_hotkey_linux, gdk_keyval_matches_vk, gdk_keyval_to_vk, keyval_to_char,
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
    let mut cfg = Config::default();
    cfg.auto_restore_wrong_word = true;
    let mut sess = IbusSession::new(cfg);
    sess.set_capabilities(IBUS_CAP_PREEDIT_TEXT);

    let committed = type_preedit_string(&mut sess, "tieengs Vieejt ");
    assert_eq!(committed.concat(), "tiếng Việt ");

    let res_t = sess.process_key('t' as u32, 0, 0);
    assert!(res_t.handled);
    assert_eq!(res_t.ops, vec![IbusOp::UpdatePreedit("t".to_string())]);

    let eng = type_preedit_string(&mut sess, "esst ");
    assert_eq!(eng.concat(), "tesst ");
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
    let mut cfg = Config::default();
    cfg.mechanism = Mechanism::CompatibleHook;
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
