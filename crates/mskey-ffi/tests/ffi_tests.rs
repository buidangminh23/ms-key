use std::ffi::{CStr, CString};
use std::ptr;

use mskey_ffi::{
    mskey_behavior_for_app, mskey_convert_text, mskey_convert_with_encoding, mskey_create,
    mskey_default_config_json, mskey_destroy, mskey_detect_charset, mskey_format_macro_text,
    mskey_free_action, mskey_free_string, mskey_get_config_json, mskey_import_evkey_tree,
    mskey_is_active_delimiter, mskey_is_enabled, mskey_parse_macro_text, mskey_process_backspace,
    mskey_process_escape, mskey_process_key, mskey_reset_all, mskey_reset_word,
    mskey_set_clock_offset, mskey_set_config_json, mskey_set_enabled,
};

#[test]
fn ffi_lifecycle_typing_and_converter() {
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
                let s = unsafe { CStr::from_ptr(act.text) }.to_str().unwrap();
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
    let titled_str = unsafe { CStr::from_ptr(titled) }.to_str().unwrap().to_owned();
    mskey_free_string(titled);
    assert_eq!(titled_str, "Tiếng Việt");

    let stripped = mskey_convert_with_encoding(sample.as_ptr(), 0, 4, 5);
    let stripped_str = unsafe { CStr::from_ptr(stripped) }.to_str().unwrap().to_owned();
    mskey_free_string(stripped);
    assert_eq!(stripped_str, "tieng viet");

    let viqr = CString::new("Tie^'ng Vie^.t").unwrap();
    assert_eq!(mskey_detect_charset(viqr.as_ptr()), 4);
    assert!(mskey_is_active_delimiter(handle, ' ' as u32));
    assert!(!mskey_is_active_delimiter(handle, '[' as u32));
    mskey_set_clock_offset(handle, 7 * 3600);

    let mraw = CString::new("ddn:Đà Nẵng\n").unwrap();
    let parsed_ptr = mskey_parse_macro_text(mraw.as_ptr());
    let parsed_str = unsafe { CStr::from_ptr(parsed_ptr) }.to_str().unwrap().to_owned();
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
}
