use std::ffi::{CStr, CString, c_void};
use std::os::raw::{c_char, c_int, c_uint};

pub type GVariant = c_void;
pub type GVariantType = c_void;
pub type GMainLoop = c_void;
pub type GDBusConnection = c_void;
pub type GDBusNodeInfo = c_void;
pub type GDBusInterfaceInfo = c_void;
pub type GDBusMethodInvocation = c_void;
pub type GError = c_void;

pub const G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT: u32 = 1 << 0;
pub const G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION: u32 = 1 << 3;

pub type GDBusMethodCallFunc = Option<
    unsafe extern "C" fn(
        conn: *mut GDBusConnection,
        sender: *const c_char,
        object_path: *const c_char,
        interface_name: *const c_char,
        method_name: *const c_char,
        parameters: *mut GVariant,
        invocation: *mut GDBusMethodInvocation,
        user_data: *mut c_void,
    ),
>;

#[repr(C)]
pub struct GDBusInterfaceVTable {
    pub method_call: GDBusMethodCallFunc,
    pub get_property: *const c_void,
    pub set_property: *const c_void,
    pub padding: [*mut c_void; 8],
}

unsafe extern "C" {
    pub fn g_main_loop_new(context: *mut c_void, is_running: c_int) -> *mut GMainLoop;
    pub fn g_main_loop_run(loop_ptr: *mut GMainLoop);
    pub fn g_main_loop_quit(loop_ptr: *mut GMainLoop);
    pub fn g_main_loop_unref(loop_ptr: *mut GMainLoop);
    pub fn g_timeout_add(
        interval: c_uint,
        function: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
        data: *mut c_void,
    ) -> c_uint;
    pub fn g_source_remove(tag: c_uint) -> c_int;
    pub fn g_idle_add(
        function: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
        data: *mut c_void,
    ) -> c_uint;
    pub fn g_free(mem: *mut c_void);
    pub fn g_error_free(err: *mut GError);

    pub fn g_variant_type_new(type_string: *const c_char) -> *mut GVariantType;
    pub fn g_variant_type_free(vt: *mut GVariantType);
    pub fn g_variant_new_string(s: *const c_char) -> *mut GVariant;
    pub fn g_variant_new_object_path(s: *const c_char) -> *mut GVariant;
    pub fn g_variant_new_uint32(val: u32) -> *mut GVariant;
    pub fn g_variant_new_int32(val: i32) -> *mut GVariant;
    pub fn g_variant_new_boolean(val: c_int) -> *mut GVariant;
    pub fn g_variant_new_variant(val: *mut GVariant) -> *mut GVariant;
    pub fn g_variant_new_array(
        child_type: *const GVariantType,
        children: *const *mut GVariant,
        n_children: usize,
    ) -> *mut GVariant;
    pub fn g_variant_new_tuple(children: *const *mut GVariant, n_children: usize) -> *mut GVariant;
    pub fn g_variant_get_child_value(val: *mut GVariant, index: usize) -> *mut GVariant;
    pub fn g_variant_get_variant(val: *mut GVariant) -> *mut GVariant;
    pub fn g_variant_n_children(val: *mut GVariant) -> usize;
    pub fn g_variant_get_uint32(val: *mut GVariant) -> u32;
    pub fn g_variant_get_int32(val: *mut GVariant) -> i32;
    pub fn g_variant_get_string(val: *mut GVariant, length: *mut usize) -> *const c_char;
    pub fn g_variant_unref(val: *mut GVariant);

    pub fn g_dbus_node_info_new_for_xml(
        xml: *const c_char,
        error: *mut *mut GError,
    ) -> *mut GDBusNodeInfo;
    pub fn g_dbus_node_info_lookup_interface(
        info: *mut GDBusNodeInfo,
        name: *const c_char,
    ) -> *mut GDBusInterfaceInfo;
    pub fn g_dbus_node_info_unref(info: *mut GDBusNodeInfo);

    pub fn g_dbus_connection_new_for_address_sync(
        address: *const c_char,
        flags: u32,
        observer: *mut c_void,
        cancellable: *mut c_void,
        error: *mut *mut GError,
    ) -> *mut GDBusConnection;
    pub fn g_dbus_connection_call_sync(
        conn: *mut GDBusConnection,
        bus_name: *const c_char,
        object_path: *const c_char,
        interface_name: *const c_char,
        method_name: *const c_char,
        parameters: *mut GVariant,
        reply_type: *const GVariantType,
        flags: u32,
        timeout_msec: c_int,
        cancellable: *mut c_void,
        error: *mut *mut GError,
    ) -> *mut GVariant;
    pub fn g_dbus_connection_register_object(
        conn: *mut GDBusConnection,
        object_path: *const c_char,
        interface_info: *mut GDBusInterfaceInfo,
        vtable: *const GDBusInterfaceVTable,
        user_data: *mut c_void,
        user_data_free_func: *const c_void,
        error: *mut *mut GError,
    ) -> c_uint;
    pub fn g_dbus_connection_emit_signal(
        conn: *mut GDBusConnection,
        dest_bus_name: *const c_char,
        object_path: *const c_char,
        interface_name: *const c_char,
        signal_name: *const c_char,
        parameters: *mut GVariant,
        error: *mut *mut GError,
    ) -> c_int;
    pub fn g_dbus_method_invocation_return_value(
        invocation: *mut GDBusMethodInvocation,
        parameters: *mut GVariant,
    );
    pub fn gdk_beep();
}

pub fn cstr(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| CString::new("").expect("empty"))
}

pub unsafe fn from_cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
    }
}
