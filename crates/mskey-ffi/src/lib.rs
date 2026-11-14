use std::ffi::{CStr, CString, c_char};
use std::panic::catch_unwind;
use std::ptr;
use std::sync::Mutex;

use mskey_core::{
    AppBehavior, CaseTransform, Charset, Config, Engine, EngineAction, convert_text,
};

pub struct MsKeyHandle {
    engine: Mutex<Engine>,
}

#[repr(C)]
pub struct MsKeyAction {
    pub kind: u8,
    pub backspaces: u32,
    pub text: *mut c_char,
}

impl MsKeyAction {
    fn pass_through() -> Self {
        Self {
            kind: 0,
            backspaces: 0,
            text: ptr::null_mut(),
        }
    }
}

fn to_c_string(s: &str) -> *mut c_char {
    let cleaned = if s.contains('\0') {
        s.replace('\0', "")
    } else {
        s.to_string()
    };
    CString::new(cleaned)
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

fn from_c_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .ok()
        .map(ToOwned::to_owned)
}

fn map_action(action: EngineAction) -> MsKeyAction {
    match action {
        EngineAction::PassThrough => MsKeyAction::pass_through(),
        EngineAction::Replace { backspaces, text } => MsKeyAction {
            kind: 1,
            backspaces: backspaces.min(u32::MAX as usize) as u32,
            text: to_c_string(&text),
        },
    }
}

fn parse_charset(code: u8) -> Charset {
    match code {
        1 => Charset::Tcvn3,
        2 => Charset::VniWindows,
        3 => Charset::UnicodeComposite,
        _ => Charset::Unicode,
    }
}

fn parse_case(code: u8) -> CaseTransform {
    match code {
        1 => CaseTransform::Upper,
        2 => CaseTransform::Lower,
        3 => CaseTransform::Sentence,
        4 => CaseTransform::Title,
        _ => CaseTransform::None,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_create(config_json: *const c_char) -> *mut MsKeyHandle {
    catch_unwind(|| {
        let cfg = from_c_str(config_json)
            .and_then(|raw| Config::from_json_str(&raw).ok())
            .unwrap_or_default();
        Box::into_raw(Box::new(MsKeyHandle {
            engine: Mutex::new(Engine::new(cfg)),
        }))
    })
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_destroy(handle: *mut MsKeyHandle) {
    let _ = catch_unwind(|| {
        if !handle.is_null() {
            unsafe {
                drop(Box::from_raw(handle));
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_default_config_json() -> *mut c_char {
    catch_unwind(|| {
        Config::default()
            .to_json_pretty()
            .map(|s| to_c_string(&s))
            .unwrap_or(ptr::null_mut())
    })
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_get_config_json(handle: *mut MsKeyHandle) -> *mut c_char {
    catch_unwind(|| {
        let Some(h) = (unsafe { handle.as_ref() }) else {
            return ptr::null_mut();
        };
        let guard = h.engine.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .config()
            .to_json_pretty()
            .map(|s| to_c_string(&s))
            .unwrap_or(ptr::null_mut())
    })
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_set_config_json(
    handle: *mut MsKeyHandle,
    config_json: *const c_char,
) -> bool {
    catch_unwind(|| {
        let Some(h) = (unsafe { handle.as_ref() }) else {
            return false;
        };
        let Some(raw) = from_c_str(config_json) else {
            return false;
        };
        let Ok(cfg) = Config::from_json_str(&raw) else {
            return false;
        };
        let mut guard = h.engine.lock().unwrap_or_else(|e| e.into_inner());
        guard.update_config(cfg);
        true
    })
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_import_evkey_tree(tree_text: *const c_char) -> *mut c_char {
    catch_unwind(|| {
        let Some(raw) = from_c_str(tree_text) else {
            return ptr::null_mut();
        };
        let cfg = Config::import_evkey_tree(&raw);
        cfg.to_json_pretty()
            .map(|s| to_c_string(&s))
            .unwrap_or(ptr::null_mut())
    })
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_set_enabled(handle: *mut MsKeyHandle, enabled: bool) {
    let _ = catch_unwind(|| {
        if let Some(h) = unsafe { handle.as_ref() } {
            let mut guard = h.engine.lock().unwrap_or_else(|e| e.into_inner());
            guard.set_vietnamese_enabled(enabled);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_is_enabled(handle: *mut MsKeyHandle) -> bool {
    catch_unwind(|| {
        let Some(h) = (unsafe { handle.as_ref() }) else {
            return false;
        };
        let guard = h.engine.lock().unwrap_or_else(|e| e.into_inner());
        guard.config().vietnamese_enabled
    })
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_reset_word(handle: *mut MsKeyHandle) {
    let _ = catch_unwind(|| {
        if let Some(h) = unsafe { handle.as_ref() } {
            let mut guard = h.engine.lock().unwrap_or_else(|e| e.into_inner());
            guard.reset_word();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_reset_all(handle: *mut MsKeyHandle) {
    let _ = catch_unwind(|| {
        if let Some(h) = unsafe { handle.as_ref() } {
            let mut guard = h.engine.lock().unwrap_or_else(|e| e.into_inner());
            guard.reset_all();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_process_key(handle: *mut MsKeyHandle, codepoint: u32) -> MsKeyAction {
    catch_unwind(|| {
        let Some(h) = (unsafe { handle.as_ref() }) else {
            return MsKeyAction::pass_through();
        };
        let Some(ch) = char::from_u32(codepoint) else {
            return MsKeyAction::pass_through();
        };
        let mut guard = h.engine.lock().unwrap_or_else(|e| e.into_inner());
        map_action(guard.process_key(ch))
    })
    .unwrap_or_else(|_| MsKeyAction::pass_through())
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_process_backspace(handle: *mut MsKeyHandle) -> MsKeyAction {
    catch_unwind(|| {
        let Some(h) = (unsafe { handle.as_ref() }) else {
            return MsKeyAction::pass_through();
        };
        let mut guard = h.engine.lock().unwrap_or_else(|e| e.into_inner());
        map_action(guard.process_backspace())
    })
    .unwrap_or_else(|_| MsKeyAction::pass_through())
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_process_escape(handle: *mut MsKeyHandle) -> MsKeyAction {
    catch_unwind(|| {
        let Some(h) = (unsafe { handle.as_ref() }) else {
            return MsKeyAction::pass_through();
        };
        let mut guard = h.engine.lock().unwrap_or_else(|e| e.into_inner());
        map_action(guard.process_escape())
    })
    .unwrap_or_else(|_| MsKeyAction::pass_through())
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_behavior_for_app(
    handle: *mut MsKeyHandle,
    process_name: *const c_char,
) -> u8 {
    catch_unwind(|| {
        let Some(h) = (unsafe { handle.as_ref() }) else {
            return 0;
        };
        let Some(name) = from_c_str(process_name) else {
            return 0;
        };
        let guard = h.engine.lock().unwrap_or_else(|e| e.into_inner());
        match guard.config().behavior_for_app(&name) {
            AppBehavior::Default => 0,
            AppBehavior::ForceEnglish => 1,
            AppBehavior::ForceVietnamese => 2,
            AppBehavior::Blacklist => 3,
        }
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_convert_text(
    input: *const c_char,
    from_charset: u8,
    to_charset: u8,
    case_mode: u8,
) -> *mut c_char {
    catch_unwind(|| {
        let Some(text) = from_c_str(input) else {
            return ptr::null_mut();
        };
        let out = convert_text(
            &text,
            parse_charset(from_charset),
            parse_charset(to_charset),
            parse_case(case_mode),
        );
        to_c_string(&out)
    })
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_free_string(ptr: *mut c_char) {
    let _ = catch_unwind(|| {
        if !ptr.is_null() {
            unsafe {
                drop(CString::from_raw(ptr));
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mskey_free_action(action: MsKeyAction) {
    mskey_free_string(action.text);
}
