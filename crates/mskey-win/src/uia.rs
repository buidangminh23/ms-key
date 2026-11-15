use std::ffi::c_void;
use std::mem::transmute;
use std::ptr::null_mut;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, Thread};
use std::time::Duration;

use windows_sys::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows_sys::core::GUID;

const CLSID_CUI_AUTOMATION: GUID = GUID {
    data1: 0xff48dba4,
    data2: 0x60ef,
    data3: 0x4201,
    data4: [0xaa, 0x87, 0x54, 0x10, 0x3e, 0xef, 0x59, 0x4e],
};

const IID_IUI_AUTOMATION: GUID = GUID {
    data1: 0x30cbe57d,
    data2: 0xd9d0,
    data3: 0x452a,
    data4: [0xab, 0x13, 0x7a, 0xc5, 0xac, 0x48, 0x25, 0xee],
};

type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
type GetFocusedElementFn = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
type GetCurrentIsPasswordFn = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;

static IS_PASSWORD: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicBool = AtomicBool::new(false);
static WORKER: OnceLock<Thread> = OnceLock::new();

pub fn start() {
    if WORKER.get().is_none()
        && let Ok(handle) = thread::Builder::new().name("mskey-uia".to_string()).spawn(worker_loop)
    {
        let _ = WORKER.set(handle.thread().clone());
    }
}

pub fn notify_focus_check() {
    PENDING.store(true, Ordering::Release);
    if let Some(worker) = WORKER.get() {
        worker.unpark();
    }
}

pub fn reset_cached() {
    IS_PASSWORD.store(false, Ordering::Release);
    notify_focus_check();
}

pub fn is_password_focused() -> bool {
    IS_PASSWORD.load(Ordering::Acquire)
}

fn worker_loop() {
    let co_hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_MULTITHREADED as u32) };
    if co_hr < 0 {
        return;
    }

    let mut automation: *mut c_void = null_mut();

    loop {
        while !PENDING.swap(false, Ordering::AcqRel) {
            thread::park();
        }

        thread::sleep(Duration::from_millis(25));
        while PENDING.swap(false, Ordering::AcqRel) {
            thread::sleep(Duration::from_millis(20));
        }

        if automation.is_null() {
            automation = create_automation();
        }
        if !automation.is_null() {
            let is_pw = unsafe { query_focused_is_password(automation) };
            IS_PASSWORD.store(is_pw, Ordering::Release);
        }
    }
}

fn create_automation() -> *mut c_void {
    let mut ptr: *mut c_void = null_mut();
    let hr = unsafe {
        CoCreateInstance(
            &CLSID_CUI_AUTOMATION,
            null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IUI_AUTOMATION,
            &mut ptr,
        )
    };
    if hr >= 0 { ptr } else { null_mut() }
}

unsafe fn vtable_slot(obj: *mut c_void, slot: usize) -> *const c_void {
    unsafe {
        let vtbl = *(obj as *const *const *const c_void);
        *vtbl.add(slot)
    }
}

unsafe fn release_com(obj: *mut c_void) {
    if obj.is_null() {
        return;
    }
    unsafe {
        let func = transmute::<*const c_void, ReleaseFn>(vtable_slot(obj, 2));
        let _ = func(obj);
    }
}

unsafe fn query_focused_is_password(automation: *mut c_void) -> bool {
    let mut element: *mut c_void = null_mut();
    let hr = unsafe {
        let get_focused = transmute::<*const c_void, GetFocusedElementFn>(vtable_slot(automation, 8));
        get_focused(automation, &mut element)
    };
    if hr < 0 || element.is_null() {
        if !element.is_null() {
            unsafe { release_com(element) };
        }
        return false;
    }

    let mut is_password: i32 = 0;
    let pw_hr = unsafe {
        let get_pw = transmute::<*const c_void, GetCurrentIsPasswordFn>(vtable_slot(element, 35));
        get_pw(element, &mut is_password)
    };
    unsafe { release_com(element) };
    pw_hr >= 0 && is_password != 0
}
