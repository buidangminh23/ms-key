use std::ptr;

use mskey_core::{Charset, Config, InputMethod, Mechanism};

use crate::gio_ffi::{
    GDBusConnection, GVariant, cstr, from_cstr, g_dbus_connection_emit_signal,
    g_variant_get_child_value, g_variant_get_int32, g_variant_get_string, g_variant_get_uint32,
    g_variant_get_variant, g_variant_n_children, g_variant_new_array, g_variant_new_boolean,
    g_variant_new_int32, g_variant_new_string, g_variant_new_tuple, g_variant_new_uint32,
    g_variant_new_variant, g_variant_type_free, g_variant_type_new, g_variant_unref,
};

pub unsafe fn extract_u32_arg(params: *mut GVariant, idx: usize) -> Option<u32> {
    if unsafe { g_variant_n_children(params) } <= idx {
        return None;
    }
    unsafe {
        let v = g_variant_get_child_value(params, idx);
        let val = g_variant_get_uint32(v);
        g_variant_unref(v);
        Some(val)
    }
}

pub unsafe fn extract_str_arg(params: *mut GVariant, idx: usize) -> Option<String> {
    if unsafe { g_variant_n_children(params) } <= idx {
        return None;
    }
    unsafe {
        let v = g_variant_get_child_value(params, idx);
        let s = from_cstr(g_variant_get_string(v, ptr::null_mut()));
        g_variant_unref(v);
        Some(s)
    }
}

pub unsafe fn extract_cursor_rect(params: *mut GVariant) -> Option<(i32, i32, i32, i32)> {
    if unsafe { g_variant_n_children(params) } < 4 {
        return None;
    }
    unsafe {
        let v0 = g_variant_get_child_value(params, 0);
        let v1 = g_variant_get_child_value(params, 1);
        let v2 = g_variant_get_child_value(params, 2);
        let v3 = g_variant_get_child_value(params, 3);
        let r = (
            g_variant_get_int32(v0),
            g_variant_get_int32(v1),
            g_variant_get_int32(v2),
            g_variant_get_int32(v3),
        );
        g_variant_unref(v0);
        g_variant_unref(v1);
        g_variant_unref(v2);
        g_variant_unref(v3);
        Some(r)
    }
}

pub unsafe fn extract_surrounding_args(params: *mut GVariant) -> Option<(String, u32)> {
    if unsafe { g_variant_n_children(params) } < 2 {
        return None;
    }
    unsafe {
        let v0 = g_variant_get_child_value(params, 0);
        let v1 = g_variant_get_child_value(params, 1);
        let inner = g_variant_get_variant(v0);
        let cursor = g_variant_get_uint32(v1);
        let text = if !inner.is_null() && g_variant_n_children(inner) >= 3 {
            let s_val = g_variant_get_child_value(inner, 2);
            let s = from_cstr(g_variant_get_string(s_val, ptr::null_mut()));
            g_variant_unref(s_val);
            s
        } else {
            String::new()
        };
        if !inner.is_null() {
            g_variant_unref(inner);
        }
        g_variant_unref(v0);
        g_variant_unref(v1);
        Some((text, cursor))
    }
}

pub fn emit_signal(conn: *mut GDBusConnection, path: &str, name: &str, params: *mut GVariant) {
    let c_path = cstr(path);
    let c_iface = cstr("org.freedesktop.IBus.Engine");
    let c_name = cstr(name);
    unsafe {
        g_dbus_connection_emit_signal(
            conn,
            ptr::null(),
            c_path.as_ptr(),
            c_iface.as_ptr(),
            c_name.as_ptr(),
            params,
            ptr::null_mut(),
        );
    }
}

pub fn emit_no_args(conn: *mut GDBusConnection, path: &str, name: &str) {
    emit_signal(conn, path, name, ptr::null_mut());
}

pub fn emit_commit_text(conn: *mut GDBusConnection, path: &str, text: &str) {
    unsafe {
        let vt = build_ibus_text(text, false);
        let tuple = g_variant_new_tuple(&vt, 1);
        emit_signal(conn, path, "CommitText", tuple);
    }
}

pub fn emit_preedit(conn: *mut GDBusConnection, path: &str, text: &str) {
    let char_len = text.chars().count() as u32;
    unsafe {
        let vt = build_ibus_text(text, true);
        let args = [
            vt,
            g_variant_new_uint32(char_len),
            g_variant_new_boolean(1),
            g_variant_new_uint32(0),
        ];
        let tuple = g_variant_new_tuple(args.as_ptr(), 4);
        emit_signal(conn, path, "UpdatePreeditText", tuple);
    }
}

pub fn emit_delete_surrounding(conn: *mut GDBusConnection, path: &str, offset: i32, nchars: u32) {
    unsafe {
        let args = [g_variant_new_int32(offset), g_variant_new_uint32(nchars)];
        let tuple = g_variant_new_tuple(args.as_ptr(), 2);
        emit_signal(conn, path, "DeleteSurroundingText", tuple);
    }
}

pub fn emit_forward_key(conn: *mut GDBusConnection, path: &str, kv: u32, kc: u32, st: u32) {
    unsafe {
        let args = [g_variant_new_uint32(kv), g_variant_new_uint32(kc), g_variant_new_uint32(st)];
        let tuple = g_variant_new_tuple(args.as_ptr(), 3);
        emit_signal(conn, path, "ForwardKeyEvent", tuple);
    }
}

pub fn register_panel_properties(conn: *mut GDBusConnection, path: &str, cfg: &Config) {
    let mode_sym = if cfg.vietnamese_enabled { "V" } else { "E" };
    let mode_lbl = if cfg.vietnamese_enabled {
        "Chế độ: Tiếng Việt (V)"
    } else {
        "Chế độ: Tiếng Anh (E)"
    };
    unsafe {
        let items = [
            build_property("InputMode", 0, mode_lbl, mode_sym, false, build_prop_list(&[])),
            build_property("Method.Telex", 2, "Telex", "", cfg.input_method == InputMethod::Telex, build_prop_list(&[])),
            build_property("Method.Vni", 2, "VNI", "", cfg.input_method == InputMethod::Vni, build_prop_list(&[])),
            build_property("Method.Viqr", 2, "VIQR", "", cfg.input_method == InputMethod::Viqr, build_prop_list(&[])),
            build_property("Method.Simple", 2, "Simple Telex", "", cfg.input_method == InputMethod::SimpleTelex, build_prop_list(&[])),
            build_property("Method.Custom", 2, "Tự định nghĩa", "", cfg.input_method == InputMethod::Custom, build_prop_list(&[])),
            build_property("Sep1", 4, "", "", false, build_prop_list(&[])),
            build_property("Charset.Unicode", 2, "Bảng mã: Unicode", "", cfg.charset == Charset::Unicode, build_prop_list(&[])),
            build_property("Charset.Tcvn3", 2, "Bảng mã: TCVN3 (ABC)", "", cfg.charset == Charset::Tcvn3, build_prop_list(&[])),
            build_property("Charset.Vni", 2, "Bảng mã: VNI Windows", "", cfg.charset == Charset::VniWindows, build_prop_list(&[])),
            build_property("Charset.Comp", 2, "Bảng mã: Unicode tổ hợp", "", cfg.charset == Charset::UnicodeComposite, build_prop_list(&[])),
            build_property("Sep2", 4, "", "", false, build_prop_list(&[])),
            build_property("Mech.Preedit", 2, "Cơ chế: Preedit (Wayland/GTK)", "", cfg.mechanism == Mechanism::KeyboardHook, build_prop_list(&[])),
            build_property("Mech.Direct", 2, "Cơ chế: Tương thích trực tiếp", "", cfg.mechanism == Mechanism::CompatibleHook, build_prop_list(&[])),
            build_property("Sep3", 4, "", "", false, build_prop_list(&[])),
            build_property("OpenConvert", 0, "Chuyển mã Clipboard...", "", false, build_prop_list(&[])),
            build_property("OpenSetup", 0, "Cài đặt MS Key...", "", false, build_prop_list(&[])),
        ];
        let plist = build_prop_list(&items);
        let tuple = g_variant_new_tuple(&plist, 1);
        emit_signal(conn, path, "RegisterProperties", tuple);
    }
}

unsafe fn empty_sv_dict() -> *mut GVariant {
    let t = cstr("{sv}");
    let vt = unsafe { g_variant_type_new(t.as_ptr()) };
    let d = unsafe { g_variant_new_array(vt, ptr::null(), 0) };
    unsafe { g_variant_type_free(vt) };
    d
}

unsafe fn build_ibus_attr_list(char_len: u32, underline: bool) -> *mut GVariant {
    let name = cstr("IBusAttrList");
    let tv = cstr("v");
    let vtype = unsafe { g_variant_type_new(tv.as_ptr()) };
    let attrs_arr = if underline && char_len > 0 {
        let aname = cstr("IBusAttribute");
        let aflds = [
            unsafe { g_variant_new_string(aname.as_ptr()) },
            unsafe { empty_sv_dict() },
            unsafe { g_variant_new_uint32(1) },
            unsafe { g_variant_new_uint32(1) },
            unsafe { g_variant_new_uint32(0) },
            unsafe { g_variant_new_uint32(char_len) },
        ];
        let attr_var = unsafe { g_variant_new_variant(g_variant_new_tuple(aflds.as_ptr(), 6)) };
        unsafe { g_variant_new_array(vtype, &attr_var, 1) }
    } else {
        unsafe { g_variant_new_array(vtype, ptr::null(), 0) }
    };
    unsafe { g_variant_type_free(vtype) };
    let flds = [unsafe { g_variant_new_string(name.as_ptr()) }, unsafe { empty_sv_dict() }, attrs_arr];
    unsafe { g_variant_new_variant(g_variant_new_tuple(flds.as_ptr(), 3)) }
}

unsafe fn build_ibus_text(text: &str, underline: bool) -> *mut GVariant {
    let name = cstr("IBusText");
    let ctxt = cstr(text);
    let len = text.chars().count() as u32;
    let flds = [
        unsafe { g_variant_new_string(name.as_ptr()) },
        unsafe { empty_sv_dict() },
        unsafe { g_variant_new_string(ctxt.as_ptr()) },
        unsafe { build_ibus_attr_list(len, underline) },
    ];
    unsafe { g_variant_new_variant(g_variant_new_tuple(flds.as_ptr(), 4)) }
}

unsafe fn build_prop_list(items: &[*mut GVariant]) -> *mut GVariant {
    let name = cstr("IBusPropList");
    let tv = cstr("v");
    let vtype = unsafe { g_variant_type_new(tv.as_ptr()) };
    let arr = unsafe {
        if items.is_empty() {
            g_variant_new_array(vtype, ptr::null(), 0)
        } else {
            g_variant_new_array(vtype, items.as_ptr(), items.len())
        }
    };
    unsafe { g_variant_type_free(vtype) };
    let flds = [unsafe { g_variant_new_string(name.as_ptr()) }, unsafe { empty_sv_dict() }, arr];
    unsafe { g_variant_new_variant(g_variant_new_tuple(flds.as_ptr(), 3)) }
}

unsafe fn build_property(
    key: &str,
    prop_type: u32,
    label: &str,
    symbol: &str,
    checked: bool,
    sub_props: *mut GVariant,
) -> *mut GVariant {
    let name = cstr("IBusProperty");
    let ckey = cstr(key);
    let cicon = cstr("");
    let flds = [
        unsafe { g_variant_new_string(name.as_ptr()) },
        unsafe { empty_sv_dict() },
        unsafe { g_variant_new_string(ckey.as_ptr()) },
        unsafe { g_variant_new_uint32(prop_type) },
        unsafe { build_ibus_text(label, false) },
        unsafe { g_variant_new_string(cicon.as_ptr()) },
        unsafe { build_ibus_text("", false) },
        unsafe { g_variant_new_boolean(1) },
        unsafe { g_variant_new_boolean(1) },
        unsafe { g_variant_new_uint32(if checked { 1 } else { 0 }) },
        sub_props,
        unsafe { build_ibus_text(symbol, false) },
    ];
    unsafe { g_variant_new_variant(g_variant_new_tuple(flds.as_ptr(), 12)) }
}
