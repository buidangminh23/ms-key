use std::cell::{Cell, RefCell};
use std::env;
use std::ffi::c_void;
use std::os::raw::{c_char, c_int};
use std::process::Command;
use std::ptr;

use mskey_core::{Charset, Config, InputMethod, Mechanism};

use crate::config_store::{
    config_mtime_nanos, detect_active_x11_app, load_config, resolve_ibus_address, save_config,
};
use crate::gio_ffi::{
    G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT, G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
    GDBusConnection, GDBusInterfaceInfo, GDBusInterfaceVTable, GDBusMethodInvocation, GVariant,
    cstr, from_cstr, g_dbus_connection_call_sync, g_dbus_connection_new_for_address_sync,
    g_dbus_connection_register_object, g_dbus_method_invocation_return_value,
    g_dbus_node_info_lookup_interface, g_dbus_node_info_new_for_xml, g_dbus_node_info_unref,
    g_error_free, g_main_loop_new, g_main_loop_quit, g_main_loop_run, g_main_loop_unref,
    g_timeout_add, g_variant_get_child_value, g_variant_get_string, g_variant_get_uint32,
    g_variant_new_boolean, g_variant_new_object_path, g_variant_new_string, g_variant_new_tuple,
    g_variant_new_uint32, g_variant_unref, gdk_beep,
};
use crate::ibus_logic::{IbusOp, IbusSession};
use crate::ibus_variant::{
    emit_commit_text, emit_delete_surrounding, emit_forward_key, emit_no_args, emit_preedit,
    register_panel_properties,
};

const FACTORY_XML: &str = "\
<node>\
  <interface name='org.freedesktop.IBus.Factory'>\
    <method name='CreateEngine'>\
      <arg direction='in' type='s' name='name'/>\
      <arg direction='out' type='o'/>\
    </method>\
    <method name='Destroy'/>\
  </interface>\
</node>";

const ENGINE_XML: &str = "\
<node>\
  <interface name='org.freedesktop.IBus.Engine'>\
    <method name='ProcessKeyEvent'>\
      <arg direction='in' type='u' name='keyval'/>\
      <arg direction='in' type='u' name='keycode'/>\
      <arg direction='in' type='u' name='state'/>\
      <arg direction='out' type='b'/>\
    </method>\
    <method name='SetCapabilities'><arg direction='in' type='u' name='caps'/></method>\
    <method name='PropertyActivate'><arg direction='in' type='s' name='name'/><arg direction='in' type='u' name='state'/></method>\
    <method name='PropertyShow'><arg direction='in' type='s' name='name'/></method>\
    <method name='PropertyHide'><arg direction='in' type='s' name='name'/></method>\
    <method name='SetCursorLocation'><arg direction='in' type='i' name='x'/><arg direction='in' type='i' name='y'/><arg direction='in' type='i' name='w'/><arg direction='in' type='i' name='h'/></method>\
    <method name='SetSurroundingText'><arg direction='in' type='v' name='text'/><arg direction='in' type='u' name='cursor'/><arg direction='in' type='u' name='anchor'/></method>\
    <method name='FocusIn'/>\
    <method name='FocusInId'><arg direction='in' type='s' name='object_path'/><arg direction='in' type='s' name='client'/></method>\
    <method name='FocusOut'/>\
    <method name='FocusOutId'><arg direction='in' type='s' name='object_path'/></method>\
    <method name='Reset'/>\
    <method name='Enable'/>\
    <method name='Disable'/>\
    <method name='PageUp'/>\
    <method name='PageDown'/>\
    <method name='CursorUp'/>\
    <method name='CursorDown'/>\
    <method name='CandidateClicked'><arg direction='in' type='u' name='index'/><arg direction='in' type='u' name='button'/><arg direction='in' type='u' name='state'/></method>\
  </interface>\
</node>";

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
    let f_xml = cstr(FACTORY_XML);
    let f_node = unsafe { g_dbus_node_info_new_for_xml(f_xml.as_ptr(), ptr::null_mut()) };
    let f_name = cstr("org.freedesktop.IBus.Factory");
    let f_iface = unsafe { g_dbus_node_info_lookup_interface(f_node, f_name.as_ptr()) };

    let e_xml = cstr(ENGINE_XML);
    let e_node = unsafe { g_dbus_node_info_new_for_xml(e_xml.as_ptr(), ptr::null_mut()) };
    let e_name = cstr("org.freedesktop.IBus.Engine");
    let e_iface = unsafe { g_dbus_node_info_lookup_interface(e_node, e_name.as_ptr()) };

    DAEMON.with(|d| {
        *d.borrow_mut() = Some(DaemonState {
            conn,
            engine_iface: e_iface,
            engine_seq: 0,
            active_path: String::new(),
            session: IbusSession::new(load_config()),
            last_mtime: config_mtime_nanos(),
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
    let cur = config_mtime_nanos();
    let changed = DAEMON.with(|d| {
        let mut b = d.borrow_mut();
        if let Some(st) = b.as_mut() {
            if cur > 0 && cur != st.last_mtime {
                st.last_mtime = cur;
                st.session.update_config(load_config());
                return Some((st.conn, st.active_path.clone(), st.session.engine.config().clone()));
            }
        }
        None
    });
    if let Some((conn, path, cfg)) = changed {
        if !path.is_empty() {
            register_panel_properties(conn, &path, &cfg);
        }
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
    let res = DAEMON.with(|d| {
        let mut b = d.borrow_mut();
        let st_d = b.as_mut().expect("daemon state");
        st_d.active_path = path.to_string();
        st_d.session.process_key(kv, kc, st)
    });
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
            let caps = unsafe {
                let v0 = g_variant_get_child_value(parameters, 0);
                let c = g_variant_get_uint32(v0);
                g_variant_unref(v0);
                c
            };
            DAEMON.with(|d| {
                if let Some(st) = d.borrow_mut().as_mut() {
                    st.session.set_capabilities(caps);
                }
            });
        }
        "FocusIn" | "FocusInId" => {
            let client = if method == "FocusInId" {
                unsafe {
                    let v1 = g_variant_get_child_value(parameters, 1);
                    let s = from_cstr(g_variant_get_string(v1, ptr::null_mut()));
                    g_variant_unref(v1);
                    s
                }
            } else {
                String::new()
            };
            let app = detect_active_x11_app().unwrap_or(client);
            let ops = DAEMON.with(|d| {
                let mut b = d.borrow_mut();
                let st = b.as_mut().expect("daemon state");
                st.active_path = path.to_string();
                st.session.focus_in(&app)
            });
            for op in ops {
                apply_dbus_op(conn, path, op);
            }
        }
        "FocusOut" | "FocusOutId" | "Disable" => {
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
            let cfg = DAEMON.with(|d| {
                let mut b = d.borrow_mut();
                let st = b.as_mut().expect("daemon state");
                st.active_path = path.to_string();
                st.session.update_config(load_config());
                st.session.engine.config().clone()
            });
            register_panel_properties(conn, path, &cfg);
        }
        "PropertyActivate" => {
            let name = unsafe {
                let v0 = g_variant_get_child_value(parameters, 0);
                let s = from_cstr(g_variant_get_string(v0, ptr::null_mut()));
                g_variant_unref(v0);
                s
            };
            handle_property_activate(conn, path, &name);
        }
        _ => {}
    }
}

fn handle_property_activate(conn: *mut GDBusConnection, path: &str, name: &str) {
    if name == "OpenSetup" {
        spawn_self_arg("--setup");
        return;
    }
    if name == "OpenConvert" {
        spawn_self_arg("--convert");
        return;
    }
    let updated: Option<Config> = DAEMON.with(|d| {
        let mut b = d.borrow_mut();
        let st = b.as_mut()?;
        let mut cfg = st.session.engine.config().clone();
        match name {
            "InputMode" => {
                let next = !cfg.vietnamese_enabled;
                st.session.set_user_vietnamese(next);
                let c = st.session.engine.config().clone();
                save_config(&c);
                return Some(c);
            }
            "Method.Telex" => cfg.input_method = InputMethod::Telex,
            "Method.Vni" => cfg.input_method = InputMethod::Vni,
            "Method.Viqr" => cfg.input_method = InputMethod::Viqr,
            "Method.Simple" => cfg.input_method = InputMethod::SimpleTelex,
            "Charset.Unicode" => cfg.charset = Charset::Unicode,
            "Charset.Tcvn3" => cfg.charset = Charset::Tcvn3,
            "Charset.Vni" => cfg.charset = Charset::VniWindows,
            "Mech.Preedit" => cfg.mechanism = Mechanism::KeyboardHook,
            "Mech.Direct" => cfg.mechanism = Mechanism::CompatibleHook,
            _ => return None,
        }
        save_config(&cfg);
        st.session.update_config(cfg.clone());
        Some(cfg)
    });
    if let Some(cfg) = updated {
        register_panel_properties(conn, path, &cfg);
    }
}

fn spawn_self_arg(arg: &str) {
    let exe = env::current_exe().unwrap_or_else(|_| "mskey-linux".into());
    let _ = Command::new(exe).arg(arg).spawn();
}

fn apply_dbus_op(conn: *mut GDBusConnection, path: &str, op: IbusOp) {
    match op {
        IbusOp::UpdatePreedit(text) => emit_preedit(conn, path, &text),
        IbusOp::HidePreedit => emit_no_args(conn, path, "HidePreeditText"),
        IbusOp::CommitText(text) => emit_commit_text(conn, path, &text),
        IbusOp::DeleteSurrounding(offset, len) => emit_delete_surrounding(conn, path, offset, len),
        IbusOp::ForwardKey(kv, kc, st) => emit_forward_key(conn, path, kv, kc, st),
        IbusOp::SyncProperties => {
            if let Some(cfg) =
                DAEMON.with(|d| d.borrow().as_ref().map(|s| s.session.engine.config().clone()))
            {
                save_config(&cfg);
                register_panel_properties(conn, path, &cfg);
            }
        }
        IbusOp::TriggerClipboardConvert => spawn_self_arg("--convert"),
        IbusOp::Beep => unsafe { gdk_beep() },
    }
}
