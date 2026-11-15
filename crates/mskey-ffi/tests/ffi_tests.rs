use std::ffi::{CStr, CString};
use std::ptr;

use mskey_ffi::{
    mskey_apply_profile, mskey_behavior_for_app, mskey_behavior_for_window, mskey_convert_text,
    mskey_convert_with_encoding, mskey_create, mskey_default_config_json, mskey_destroy,
    mskey_detect_charset, mskey_format_macro_text, mskey_free_action, mskey_free_string,
    mskey_get_config_json, mskey_hydrate_word, mskey_import_evkey_tree, mskey_is_active_delimiter,
    mskey_is_enabled, mskey_is_newer_version, mskey_last_word_for_dict,
    mskey_normalize_tone_style, mskey_parse_macro_text, mskey_process_backspace,
    mskey_process_escape, mskey_process_key, mskey_reset_all, mskey_reset_word,
    mskey_set_clipboard_text, mskey_set_clock_offset, mskey_set_config_json, mskey_set_enabled,
    mskey_set_temporary_english, mskey_take_pending_cursor_left, mskey_toggle_last_committed_word,
};

#[test]
fn ffi_lifecycle_typing_and_converter() {
    unsafe {
        let def_ptr = mskey_default_config_json();
        assert!(!def_ptr.is_null());
        mskey_free_string(def_ptr);

        let handle = mskey_create(ptr::null());
        assert!(!handle.is_null());
        assert!(mskey_is_enabled(handle));

        let mut screen = String::new();
        for ch in "tieesng".chars() {
            let act = mskey_process_key(handle, ch as u32);
            if act.kind == 0 {
                screen.push(ch);
            } else {
                for _ in 0..act.backspaces {
                    screen.pop();
                }
                if !act.text.is_null() {
                    let s = CStr::from_ptr(act.text).to_str().unwrap();
                    screen.push_str(s);
                }
            }
            mskey_free_action(act);
        }
        assert_eq!(screen, "tiếng");

        let esc = mskey_process_escape(handle);
        assert_eq!(esc.kind, 1);
        assert_eq!(esc.backspaces, 5);
        mskey_free_action(esc);

        let bs = mskey_process_backspace(handle);
        assert_eq!(bs.kind, 0);
        mskey_free_action(bs);

        mskey_reset_word(handle);
        mskey_set_enabled(handle, false);
        assert!(!mskey_is_enabled(handle));
        mskey_reset_all(handle);

        let tree = CString::new("blacklist{\nTerminal.app\n}\n").unwrap();
        let cfg_ptr = mskey_import_evkey_tree(tree.as_ptr());
        assert!(!cfg_ptr.is_null());
        assert!(mskey_set_config_json(handle, cfg_ptr));
        mskey_free_string(cfg_ptr);

        let app = CString::new("Terminal.app").unwrap();
        assert_eq!(mskey_behavior_for_app(handle, app.as_ptr()), 3);

        let fetched = mskey_get_config_json(handle);
        assert!(!fetched.is_null());
        mskey_free_string(fetched);

        let sample = CString::new("tiếng việt").unwrap();
        let titled = mskey_convert_text(sample.as_ptr(), 0, 0, 4);
        let titled_str = CStr::from_ptr(titled).to_str().unwrap().to_owned();
        mskey_free_string(titled);
        assert_eq!(titled_str, "Tiếng Việt");

        let stripped = mskey_convert_with_encoding(sample.as_ptr(), 0, 4, 5);
        let stripped_str = CStr::from_ptr(stripped).to_str().unwrap().to_owned();
        mskey_free_string(stripped);
        assert_eq!(stripped_str, "tieng viet");

        let viqr = CString::new("Tie^'ng Vie^.t").unwrap();
        assert_eq!(mskey_detect_charset(viqr.as_ptr()), 4);
        assert!(mskey_is_active_delimiter(handle, ' ' as u32));
        assert!(!mskey_is_active_delimiter(handle, '[' as u32));
        mskey_set_clock_offset(handle, 7 * 3600);

        let mraw = CString::new("ddn:Đà Nẵng\n").unwrap();
        let parsed_ptr = mskey_parse_macro_text(mraw.as_ptr());
        let parsed_str = CStr::from_ptr(parsed_ptr).to_str().unwrap().to_owned();
        mskey_free_string(parsed_ptr);
        assert!(parsed_str.contains("ddn"));

        let def2 = mskey_default_config_json();
        let fmt_ptr = mskey_format_macro_text(def2);
        assert!(!fmt_ptr.is_null());
        mskey_free_string(fmt_ptr);
        mskey_free_string(def2);

        mskey_destroy(handle);
        mskey_destroy(ptr::null_mut());
        mskey_free_string(ptr::null_mut());

        let h2 = mskey_create(ptr::null());
        mskey_set_clipboard_text(h2, CString::new("X").unwrap().as_ptr());
        let mut cfg_json = String::new();
        let cfg_p = mskey_get_config_json(h2);
        cfg_json.push_str(CStr::from_ptr(cfg_p).to_str().unwrap());
        mskey_free_string(cfg_p);
        let updated_cfg = cfg_json.replace("\"macros\": []", "\"macros\": [{\"shortcut\":\"tag\",\"expansion\":\"<a>{cursor}</a>\",\"enabled\":true}]");
        assert!(mskey_set_config_json(h2, CString::new(updated_cfg).unwrap().as_ptr()));
        for ch in "tag ".chars() {
            let a = mskey_process_key(h2, ch as u32);
            mskey_free_action(a);
        }
        assert_eq!(mskey_take_pending_cursor_left(h2), 5);
        mskey_set_temporary_english(h2, true);
        let passthrough = mskey_process_key(h2, 'a' as u32);
        assert_eq!(passthrough.kind, 0);
        mskey_free_action(passthrough);
        mskey_set_temporary_english(h2, false);
        mskey_hydrate_word(h2, CString::new("việt").unwrap().as_ptr());
        let tone_chg = mskey_process_key(h2, 's' as u32);
        assert_eq!(tone_chg.kind, 1);
        mskey_free_action(tone_chg);
        let esc2 = mskey_process_escape(h2);
        mskey_free_action(esc2);
        let last_dict = mskey_last_word_for_dict(h2);
        assert!(!last_dict.is_null());
        mskey_free_string(last_dict);
        let sp = mskey_process_key(h2, ' ' as u32);
        mskey_free_action(sp);
        let _ = mskey_behavior_for_window(
            h2,
            CString::new("code.exe").unwrap().as_ptr(),
            CString::new("Terminal").unwrap().as_ptr(),
        );
        for ch in "tooi ".chars() {
            let a = mskey_process_key(h2, ch as u32);
            mskey_free_action(a);
        }
        let undo_a = mskey_toggle_last_committed_word(h2);
        assert_eq!(undo_a.kind, 1);
        mskey_free_action(undo_a);
        assert!(mskey_apply_profile(h2, CString::new("Lập trình").unwrap().as_ptr()));
        mskey_destroy(h2);

        let norm = mskey_normalize_tone_style(CString::new("hòa").unwrap().as_ptr(), true);
        assert_eq!(CStr::from_ptr(norm).to_str().unwrap(), "hoà");
        mskey_free_string(norm);
        assert!(mskey_is_newer_version(
            CString::new("0.1.2").unwrap().as_ptr(),
            CString::new("v0.2.0").unwrap().as_ptr()
        ));
    }
}
