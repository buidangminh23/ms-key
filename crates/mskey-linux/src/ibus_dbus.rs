use std::cell::{Cell, RefCell};
use std::env;
use std::ffi::c_void;
use std::os::raw::{c_char, c_int};
use std::process::Command;
use std::ptr;
use std::time::Duration;

use mskey_core::{
    CaseTransform, Charset, Config, ConvertEncoding, InputMethod, Mechanism,
    convert_with_encoding, detect_convert_encoding,
};

use crate::config_store::{
    effective_config_mtime_nanos, load_config, resolve_ibus_address, save_config,
    save_last_esc_word,
};
use crate::gio_ffi::{
    G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT, G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
    GDBusConnection, GDBusInterfaceInfo, GDBusInterfaceVTable, GDBusMethodInvocation, GVariant,
    cstr, from_cstr, g_dbus_connection_call_sync, g_dbus_connection_new_for_address_sync,
    g_dbus_connection_register_object, g_dbus_method_invocation_return_value,
    g_dbus_node_info_lookup_interface, g_dbus_node_info_new_for_xml, g_dbus_node_info_unref,
    g_error_free, g_main_loop_new, g_main_loop_quit, g_main_loop_run, g_main_loop_unref,
    g_timeout_add, g_variant_get_child_value, g_variant_get_uint32, g_variant_new_boolean,
    g_variant_new_object_path, g_variant_new_string, g_variant_new_tuple, g_variant_new_uint32,
    g_variant_unref, gdk_beep,
};
use crate::ibus_logic::{IbusOp, IbusSession};
use crate::ibus_variant::{
    emit_commit_text, emit_delete_surrounding, emit_forward_key, emit_no_args, emit_preedit,
    extract_cursor_rect, extract_str_arg, extract_surrounding_args, extract_u32_arg,
    register_panel_properties,
};
use crate::keyval::KEY_ESCAPE;
use crate::linux_extra::{
    ActiveWindowInfo, apply_clipboard_paste_linux, detect_active_window, ensure_gtk_init,
    hide_linux_caret_hud, is_terminal_app, is_us_keyboard_layout, read_clipboard_text,
    show_linux_caret_hud, show_linux_switch_osd, trigger_copy_keystroke, write_clipboard_text,
};

const FACTORY_XML: &str = "<node><interface name='org.freedesktop.IBus.Factory'><method name='CreateEngine'><arg direction='in' type='s' name='name'/><arg direction='out' type='o'/></method><method name='Destroy'/></interface></node>";

const ENGINE_XML: &str = "\
<node><interface name='org.freedesktop.IBus.Engine'>\
<method name='ProcessKeyEvent'><arg direction='in' type='u' name='keyval'/><arg direction='in' type='u' name='keycode'/><arg direction='in' type='u' name='state'/><arg direction='out' type='b'/></method>\
<method name='SetCapabilities'><arg direction='in' type='u' name='caps'/></method>\
<method name='PropertyActivate'><arg direction='in' type='s' name='name'/><arg direction='in' type='u' name='state'/></method>\
<method name='PropertyShow'><arg direction='in' type='s' name='name'/></method>\
<method name='PropertyHide'><arg direction='in' type='s' name='name'/></method>\
<method name='SetCursorLocation'><arg direction='in' type='i' name='x'/><arg direction='in' type='i' name='y'/><arg direction='in' type='i' name='w'/><arg direction='in' type='i' name='h'/></method>\
<method name='SetSurroundingText'><arg direction='in' type='v' name='text'/><arg direction='in' type='u' name='cursor'/><arg direction='in' type='u' name='anchor'/></method>\
<method name='FocusIn'/><method name='FocusInId'><arg direction='in' type='s' name='object_path'/><arg direction='in' type='s' name='client'/></method>\
<method name='FocusOut'/><method name='FocusOutId'><arg direction='in' type='s' name='object_path'/></method>\
<method name='Reset'/><method name='Enable'/><method name='Disable'/>\
<method name='PageUp'/><method name='PageDown'/><method name='CursorUp'/><method name='CursorDown'/>\
<method name='CandidateClicked'><arg direction='in' type='u' name='index'/><arg direction='in' type='u' name='button'/><arg direction='in' type='u' name='state'/></method>\
</interface></node>";

struct DaemonState {
    conn: *mut GDBusConnection,
    engine_iface: *mut GDBusInterfaceInfo,
    engine_seq: u32,
    active_path: String,
    session: IbusSession,
    last_mtime: u128,
}

thread_local! {
    static DAEMON: RefCell<Option<DaemonState>> = const { RefCell::new(None) };
    static MAIN_LOOP: Cell<*mut c_void> = const { Cell::new(ptr::null_mut()) };
}

pub fn run_ibus_service() -> i32 {
    let Some(addr) = resolve_ibus_address() else {
        eprintln!("mskey-linux: cannot locate active IBus address");
        return 1;
    };
    let c_addr = cstr(&addr);
    let mut err: *mut c_void = ptr::null_mut();
    let conn = unsafe {
        g_dbus_connection_new_for_address_sync(
            c_addr.as_ptr(),
            G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT
                | G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut err,
        )
    };
    if conn.is_null() {
        if !err.is_null() {
            unsafe { g_error_free(err) };
        }
        eprintln!("mskey-linux: failed to connect to IBus socket {addr}");
        return 1;
    }

    request_ibus_name(conn);
    let _ = ensure_gtk_init();
    let f_xml = cstr(FACTORY_XML);
    let f_node = unsafe { g_dbus_node_info_new_for_xml(f_xml.as_ptr(), ptr::null_mut()) };
    let f_name = cstr("org.freedesktop.IBus.Factory");
    let f_iface = unsafe { g_dbus_node_info_lookup_interface(f_node, f_name.as_ptr()) };

    let e_xml = cstr(ENGINE_XML);
    let e_node = unsafe { g_dbus_node_info_new_for_xml(e_xml.as_ptr(), ptr::null_mut()) };
    let e_name = cstr("org.freedesktop.IBus.Engine");
    let e_iface = unsafe { g_dbus_node_info_lookup_interface(e_node, e_name.as_ptr()) };

    let mut session = IbusSession::new(load_config());
    session.load_persisted_app_memory();
    session.set_us_layout(is_us_keyboard_layout());
    let initial_mtime = effective_config_mtime_nanos(session.engine.config());
    DAEMON.with(|d| {
        *d.borrow_mut() = Some(DaemonState {
            conn,
            engine_iface: e_iface,
            engine_seq: 0,
            active_path: String::new(),
            session,
            last_mtime: initial_mtime,
        });
    });

    let vtable = GDBusInterfaceVTable {
        method_call: Some(factory_method_call),
        get_property: ptr::null(),
        set_property: ptr::null(),
        padding: [ptr::null_mut(); 8],
    };
    let path = cstr("/org/freedesktop/IBus/Factory");
    unsafe {
        g_dbus_connection_register_object(
            conn,
            path.as_ptr(),
            f_iface,
            &vtable,
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
        );
        g_timeout_add(500, Some(poll_config_reload), ptr::null_mut());
    }

    let ml = unsafe { g_main_loop_new(ptr::null_mut(), 0) };
    MAIN_LOOP.with(|c| c.set(ml));
    unsafe {
        g_main_loop_run(ml);
        g_main_loop_unref(ml);
        g_dbus_node_info_unref(f_node);
        g_dbus_node_info_unref(e_node);
    }
    0
}

fn request_ibus_name(conn: *mut GDBusConnection) {
    let dest = cstr("org.freedesktop.DBus");
    let path = cstr("/org/freedesktop/DBus");
    let iface = cstr("org.freedesktop.DBus");
    let meth = cstr("RequestName");
    let svc = cstr("org.freedesktop.IBus.MSKey");
    unsafe {
        let args = [g_variant_new_string(svc.as_ptr()), g_variant_new_uint32(0)];
        let params = g_variant_new_tuple(args.as_ptr(), 2);
        let res = g_dbus_connection_call_sync(
            conn,
            dest.as_ptr(),
            path.as_ptr(),
            iface.as_ptr(),
            meth.as_ptr(),
            params,
            ptr::null(),
            0,
            5000,
            ptr::null_mut(),
            ptr::null_mut(),
        );
        if !res.is_null() {
            g_variant_unref(res);
        }
    }
}

unsafe extern "C" fn poll_config_reload(_: *mut c_void) -> c_int {
    let cur = DAEMON.with(|d| {
        d.borrow()
            .as_ref()
            .map(|st| effective_config_mtime_nanos(st.session.engine.config()))
            .unwrap_or(0)
    });
    let changed = DAEMON.with(|d| {
        let mut b = d.borrow_mut();
        if let Some(st) = b.as_mut()
            && cur > 0
            && cur != st.last_mtime
        {
            let fresh = load_config();
            let differs = fresh.to_json_pretty().ok() != st.session.engine.config().to_json_pretty().ok();
            if differs {
                st.session.update_config(fresh);
                st.last_mtime = effective_config_mtime_nanos(st.session.engine.config());
                return Some((st.conn, st.active_path.clone(), st.session.engine.config().clone()));
            }
            st.last_mtime = cur;
        }
        None
    });
    if let Some((conn, path, cfg)) = changed
        && !path.is_empty()
    {
        register_panel_properties(conn, &path, &cfg);
    }
    1
}

unsafe extern "C" fn factory_method_call(
    conn: *mut GDBusConnection,
    _sender: *const c_char,
    _object_path: *const c_char,
    _interface_name: *const c_char,
    method_name: *const c_char,
    _parameters: *mut GVariant,
    invocation: *mut GDBusMethodInvocation,
    _user_data: *mut c_void,
) {
    let method = unsafe { from_cstr(method_name) };
    if method == "CreateEngine" {
        let (path_str, e_iface) = DAEMON.with(|d| {
            let mut b = d.borrow_mut();
            let st = b.as_mut().expect("daemon state");
            st.engine_seq += 1;
            let p = format!("/org/freedesktop/IBus/Engine/{}", st.engine_seq);
            st.active_path = p.clone();
            (p, st.engine_iface)
        });
        let c_path = cstr(&path_str);
        let vtable = GDBusInterfaceVTable {
            method_call: Some(engine_method_call),
            get_property: ptr::null(),
            set_property: ptr::null(),
            padding: [ptr::null_mut(); 8],
        };
        unsafe {
            g_dbus_connection_register_object(
                conn,
                c_path.as_ptr(),
                e_iface,
                &vtable,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
            );
            let opath = g_variant_new_object_path(c_path.as_ptr());
            let ret = g_variant_new_tuple(&opath, 1);
            g_dbus_method_invocation_return_value(invocation, ret);
        }
    } else if method == "Destroy" {
        let ml = MAIN_LOOP.with(|c| c.get());
        if !ml.is_null() {
            unsafe { g_main_loop_quit(ml) };
        }
    }
}

unsafe extern "C" fn engine_method_call(
    conn: *mut GDBusConnection,
    _sender: *const c_char,
    object_path: *const c_char,
    _interface_name: *const c_char,
    method_name: *const c_char,
    parameters: *mut GVariant,
    invocation: *mut GDBusMethodInvocation,
    _user_data: *mut c_void,
) {
    let method = unsafe { from_cstr(method_name) };
    let path = unsafe { from_cstr(object_path) };
    if method == "ProcessKeyEvent" {
        handle_process_key(conn, &path, parameters, invocation);
        return;
    }
    dispatch_non_key_method(conn, &path, &method, parameters);
    unsafe { g_dbus_method_invocation_return_value(invocation, ptr::null_mut()) };
}

fn handle_process_key(
    conn: *mut GDBusConnection,
    path: &str,
    parameters: *mut GVariant,
    invocation: *mut GDBusMethodInvocation,
) {
    let (kv, kc, st) = unsafe {
        let v0 = g_variant_get_child_value(parameters, 0);
        let v1 = g_variant_get_child_value(parameters, 1);
        let v2 = g_variant_get_child_value(parameters, 2);
        let res = (g_variant_get_uint32(v0), g_variant_get_uint32(v1), g_variant_get_uint32(v2));
        g_variant_unref(v0);
        g_variant_unref(v1);
        g_variant_unref(v2);
        res
    };
    let need_clip = DAEMON.with(|d| {
        d.borrow()
            .as_ref()
            .is_some_and(|st_d| st_d.session.needs_clipboard_for_key(kv, st))
    });
    let clip_opt = if need_clip { read_clipboard_text() } else { None };
    let (res, esc_word) = DAEMON.with(|d| {
        let mut b = d.borrow_mut();
        let st_d = b.as_mut().expect("daemon state");
        st_d.active_path = path.to_string();
        if need_clip {
            st_d.session.set_clipboard_text(clip_opt);
        }
        let r = st_d.session.process_key(kv, kc, st);
        let esc = if kv == KEY_ESCAPE && r.handled {
            st_d.session.engine.last_word_for_dict()
        } else {
            None
        };
        (r, esc)
    });
    if let Some(w) = esc_word {
        save_last_esc_word(Some(&w));
    }
    for op in res.ops {
        apply_dbus_op(conn, path, op);
    }
    unsafe {
        let bv = g_variant_new_boolean(if res.handled { 1 } else { 0 });
        let ret = g_variant_new_tuple(&bv, 1);
        g_dbus_method_invocation_return_value(invocation, ret);
    }
}

fn dispatch_non_key_method(
    conn: *mut GDBusConnection,
    path: &str,
    method: &str,
    parameters: *mut GVariant,
) {
    match method {
        "SetCapabilities" => {
            if let Some(caps) = unsafe { extract_u32_arg(parameters, 0) } {
                DAEMON.with(|d| {
                    if let Some(st) = d.borrow_mut().as_mut() {
                        st.session.set_capabilities(caps);
                    }
                });
            }
        }
        "SetCursorLocation" => {
            if let Some(rect) = unsafe { extract_cursor_rect(parameters) } {
                DAEMON.with(|d| {
                    if let Some(st) = d.borrow_mut().as_mut() {
                        st.session.set_cursor_location(rect.0, rect.1, rect.2, rect.3);
                    }
                });
            }
        }
        "SetSurroundingText" => {
            if let Some((txt, cur)) = unsafe { extract_surrounding_args(parameters) } {
                DAEMON.with(|d| {
                    if let Some(st) = d.borrow_mut().as_mut() {
                        st.session.set_surrounding_text(txt, cur);
                    }
                });
            }
        }
        "FocusIn" | "FocusInId" => {
            let client = if method == "FocusInId" {
                unsafe { extract_str_arg(parameters, 1) }.unwrap_or_default()
            } else {
                String::new()
            };
            let info = match detect_active_window() {
                Some(mut w) => {
                    if w.app.trim().is_empty() {
                        w.app = client;
                    }
                    w
                }
                None => ActiveWindowInfo {
                    app: client,
                    title: String::new(),
                },
            };
            let us = is_us_keyboard_layout();
            let ops = DAEMON.with(|d| {
                let mut b = d.borrow_mut();
                let st = b.as_mut().expect("daemon state");
                st.active_path = path.to_string();
                st.session.set_us_layout(us);
                st.session.focus_in_window(&info.app, &info.title)
            });
            for op in ops {
                apply_dbus_op(conn, path, op);
            }
        }
        "FocusOut" | "FocusOutId" | "Disable" => {
            hide_linux_caret_hud();
            let ops = DAEMON.with(|d| d.borrow_mut().as_mut().map(|s| s.session.focus_out()).unwrap_or_default());
            for op in ops {
                apply_dbus_op(conn, path, op);
            }
        }
        "Reset" => {
            let ops = DAEMON.with(|d| d.borrow_mut().as_mut().map(|s| s.session.reset()).unwrap_or_default());
            for op in ops {
                apply_dbus_op(conn, path, op);
            }
        }
        "Enable" => {
            let us = is_us_keyboard_layout();
            let cfg = DAEMON.with(|d| {
                let mut b = d.borrow_mut();
                let st = b.as_mut().expect("daemon state");
                st.active_path = path.to_string();
                st.session.set_us_layout(us);
                st.session.update_config(load_config());
                let cur = st.session.engine.config().clone();
                st.last_mtime = effective_config_mtime_nanos(&cur);
                cur
            });
            register_panel_properties(conn, path, &cfg);
        }
        "PropertyActivate" => {
            if let Some(name) = unsafe { extract_str_arg(parameters, 0) } {
                handle_property_activate(conn, path, &name);
            }
        }
        _ => {}
    }
}

type SwitchNotice = (bool, (i32, i32));

fn handle_property_activate(conn: *mut GDBusConnection, path: &str, name: &str) {
    if name == "OpenSetup" {
        spawn_self_arg("--setup");
        return;
    }
    if name == "OpenConvert" {
        spawn_self_arg("--convert");
        return;
    }
    let updated: Option<(Config, Option<SwitchNotice>)> = DAEMON.with(|d| {
        let mut b = d.borrow_mut();
        let st = b.as_mut()?;
        let mut cfg = st.session.engine.config().clone();
        match name {
            "InputMode" => {
                let next = !cfg.vietnamese_enabled;
                st.session.set_user_vietnamese(next);
                let c = st.session.engine.config().clone();
                save_config(&c);
                st.last_mtime = effective_config_mtime_nanos(&c);
                let rect = st.session.cursor_rect;
                return Some((c, Some((next, (rect.0, rect.1)))));
            }
            "Method.Telex" => cfg.input_method = InputMethod::Telex,
            "Method.Vni" => cfg.input_method = InputMethod::Vni,
            "Method.Viqr" => cfg.input_method = InputMethod::Viqr,
            "Method.Simple" => cfg.input_method = InputMethod::SimpleTelex,
            "Method.Custom" => cfg.input_method = InputMethod::Custom,
            "Charset.Unicode" => cfg.charset = Charset::Unicode,
            "Charset.Tcvn3" => cfg.charset = Charset::Tcvn3,
            "Charset.Vni" => cfg.charset = Charset::VniWindows,
            "Charset.Comp" => cfg.charset = Charset::UnicodeComposite,
            "Mech.Preedit" => cfg.mechanism = Mechanism::KeyboardHook,
            "Mech.Direct" => cfg.mechanism = Mechanism::CompatibleHook,
            _ => return None,
        }
        save_config(&cfg);
        st.last_mtime = effective_config_mtime_nanos(&cfg);
        st.session.update_config(cfg.clone());
        Some((cfg, None))
    });
    if let Some((cfg, toggled)) = updated {
        register_panel_properties(conn, path, &cfg);
        if let Some((next, anchor)) = toggled {
            if cfg.beep_on_switch {
                unsafe { gdk_beep() };
            }
            if cfg.show_switch_osd {
                show_linux_switch_osd(next, Some(anchor));
            }
            if cfg.show_caret_osd {
                show_linux_caret_hud(next, Some(anchor));
            }
        }
    }
}

fn spawn_self_arg(arg: &str) {
    let exe = env::current_exe().unwrap_or_else(|_| "mskey-linux".into());
    let _ = Command::new(exe).arg(arg).spawn();
}

fn daemon_cursor_anchor() -> Option<(i32, i32)> {
    DAEMON.with(|d| {
        d.borrow().as_ref().and_then(|s| {
            let (x, y, _, _) = s.session.cursor_rect;
            if x > 0 || y > 0 { Some((x, y)) } else { None }
        })
    })
}

fn handle_selection_convert() {
    let params = DAEMON.with(|d| {
        d.borrow().as_ref().map(|s| {
            let cfg = s.session.engine.config();
            (
                s.session.active_app.clone(),
                cfg.charset,
                cfg.paste_via_shift_insert,
                cfg.send_delay_ms as u32,
            )
        })
    });
    let Some((active_app, charset, shift_insert_cfg, delay_ms)) = params else {
        spawn_self_arg("--convert");
        return;
    };
    let terminal = is_terminal_app(&active_app);
    trigger_copy_keystroke(terminal);
    std::thread::sleep(Duration::from_millis(65));
    let Some(src) = read_clipboard_text() else {
        spawn_self_arg("--convert");
        return;
    };
    if src.trim().is_empty() {
        spawn_self_arg("--convert");
        return;
    }
    let from = detect_convert_encoding(&src);
    let to = ConvertEncoding::from(charset);
    let converted = convert_with_encoding(&src, from, to, CaseTransform::None);
    if converted != src {
        let _ = write_clipboard_text(&converted);
        apply_clipboard_paste_linux(&converted, shift_insert_cfg || terminal, delay_ms);
    }
}

fn apply_dbus_op(conn: *mut GDBusConnection, path: &str, op: IbusOp) {
    match op {
        IbusOp::UpdatePreedit(text) => emit_preedit(conn, path, &text),
        IbusOp::HidePreedit => emit_no_args(conn, path, "HidePreeditText"),
        IbusOp::CommitText(text) => emit_commit_text(conn, path, &text),
        IbusOp::DeleteSurrounding(offset, len) => emit_delete_surrounding(conn, path, offset, len),
        IbusOp::ForwardKey(kv, kc, st) => emit_forward_key(conn, path, kv, kc, st),
        IbusOp::SyncProperties => {
            if let Some(cfg) = DAEMON.with(|d| {
                d.borrow_mut().as_mut().map(|s| {
                    let c = s.session.engine.config().clone();
                    save_config(&c);
                    s.last_mtime = effective_config_mtime_nanos(&c);
                    c
                })
            }) {
                register_panel_properties(conn, path, &cfg);
            }
        }
        IbusOp::TriggerClipboardConvert => spawn_self_arg("--convert"),
        IbusOp::TriggerSelectionConvert => handle_selection_convert(),
        IbusOp::Beep => unsafe { gdk_beep() },
        IbusOp::ShowOsd(vi) => show_linux_switch_osd(vi, daemon_cursor_anchor()),
        IbusOp::ShowCaretHud(vi) => show_linux_caret_hud(vi, daemon_cursor_anchor()),
        IbusOp::Delay(ms) => std::thread::sleep(Duration::from_millis(ms as u64)),
        IbusOp::PasteViaClipboard(text, shift_insert) => {
            let delay = DAEMON.with(|d| {
                d.borrow()
                    .as_ref()
                    .map(|s| s.session.engine.config().send_delay_ms as u32)
                    .unwrap_or(0)
            });
            apply_clipboard_paste_linux(&text, shift_insert, delay);
        }
    }
}
