use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use mskey_core::Config;
use windows_sys::Win32::Foundation::{FILETIME, HWND, SYSTEMTIME};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows_sys::Win32::System::SystemInformation::{GetLocalTime, GetSystemTime};
use windows_sys::Win32::System::Time::SystemTimeToFileTime;
use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY,
    OFN_OVERWRITEPROMPT, OPENFILENAMEW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GUITHREADINFO, GWL_STYLE, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo,
    GetWindowLongW, GetWindowThreadProcessId, SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW,
};

use crate::platform::{config_file_path, to_wide};

static APP_MEMORY: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
static CLIP_SNAPSHOT: Mutex<Option<Vec<(u32, Vec<u8>)>>> = Mutex::new(None);
static CLIP_GEN: AtomicU64 = AtomicU64::new(0);

fn ft_to_i64(ft: FILETIME) -> i64 {
    (((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)) as i64
}

pub fn local_utc_offset_secs() -> i64 {
    unsafe {
        let mut utc: SYSTEMTIME = std::mem::zeroed();
        let mut local: SYSTEMTIME = std::mem::zeroed();
        GetSystemTime(&mut utc);
        GetLocalTime(&mut local);
        let mut utc_ft: FILETIME = std::mem::zeroed();
        let mut local_ft: FILETIME = std::mem::zeroed();
        if SystemTimeToFileTime(&utc, &mut utc_ft) != 0
            && SystemTimeToFileTime(&local, &mut local_ft) != 0
        {
            return (ft_to_i64(local_ft) - ft_to_i64(utc_ft)) / 10_000_000;
        }
    }
    0
}

pub fn app_state_file_path() -> PathBuf {
    let cfg = config_file_path();
    cfg.parent()
        .map(|p| p.join("app_state.json"))
        .unwrap_or_else(|| PathBuf::from("app_state.json"))
}

pub fn load_app_state() -> HashMap<String, bool> {
    let path = app_state_file_path();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_app_state(map: &HashMap<String, bool>) {
    let path = app_state_file_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = std::fs::write(path, json);
    }
}

fn memory_store() -> &'static Mutex<HashMap<String, bool>> {
    APP_MEMORY.get_or_init(|| Mutex::new(load_app_state()))
}

pub fn recalled_app_ve_memory(proc_name: &str) -> Option<bool> {
    if proc_name.is_empty() {
        return None;
    }
    let key = proc_name.to_ascii_lowercase();
    memory_store().lock().ok().and_then(|m| m.get(&key).copied())
}

pub fn record_app_ve_memory(proc_name: &str, enabled: bool) {
    if proc_name.is_empty() {
        return;
    }
    let key = proc_name.to_ascii_lowercase();
    if let Ok(mut guard) = memory_store().lock() {
        guard.insert(key, enabled);
        save_app_state(&guard);
    }
}

pub fn export_config_json(cfg: &Config, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn import_config_json(path: &Path) -> Result<Config, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn browse_file_dialog(owner: HWND, save: bool, filter: &[u16], def_ext: &str) -> Option<PathBuf> {
    let mut file_buf = vec![0u16; 1024];
    let ext_w = to_wide(def_ext);
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.nFilterIndex = 1;
    ofn.lpstrFile = file_buf.as_mut_ptr();
    ofn.nMaxFile = file_buf.len() as u32;
    ofn.lpstrDefExt = ext_w.as_ptr();
    ofn.Flags = if save {
        OFN_EXPLORER | OFN_OVERWRITEPROMPT
    } else {
        OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_HIDEREADONLY
    };
    let ok = unsafe {
        if save {
            GetSaveFileNameW(&mut ofn)
        } else {
            GetOpenFileNameW(&mut ofn)
        }
    };
    if ok == 0 {
        return None;
    }
    let end = file_buf.iter().position(|&c| c == 0).unwrap_or(file_buf.len());
    Some(PathBuf::from(String::from_utf16_lossy(&file_buf[..end])))
}

pub fn make_win_filter(pairs: &[(&str, &str)]) -> Vec<u16> {
    let mut out = Vec::new();
    for (label, pattern) in pairs {
        out.extend(label.encode_utf16());
        out.push(0);
        out.extend(pattern.encode_utf16());
        out.push(0);
    }
    out.push(0);
    out
}

fn open_clipboard_retry() -> bool {
    for _ in 0..4 {
        if unsafe { OpenClipboard(ptr::null_mut()) } != 0 {
            return true;
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    false
}

fn is_safe_clipboard_format(fmt: u32) -> bool {
    !matches!(fmt, 0 | 2 | 3 | 4 | 9 | 14 | 0x0081 | 0x0082 | 0x0083 | 0x008E)
        && !(0x0200..=0x03FF).contains(&fmt)
}

fn snapshot_current_clipboard_locked() -> Vec<(u32, Vec<u8>)> {
    let mut saved = Vec::new();
    let mut fmt = 0u32;
    loop {
        fmt = unsafe { EnumClipboardFormats(fmt) };
        if fmt == 0 {
            break;
        }
        if !is_safe_clipboard_format(fmt) {
            continue;
        }
        let h = unsafe { GetClipboardData(fmt) };
        if h.is_null() {
            continue;
        }
        let sz = unsafe { GlobalSize(h) };
        if sz == 0 || sz > 4 * 1024 * 1024 {
            continue;
        }
        let ptr_mem = unsafe { GlobalLock(h) } as *const u8;
        if ptr_mem.is_null() {
            continue;
        }
        let bytes = unsafe { std::slice::from_raw_parts(ptr_mem, sz) }.to_vec();
        unsafe {
            GlobalUnlock(h);
        }
        saved.push((fmt, bytes));
    }
    saved
}

fn write_unicode_clipboard_locked(text: &str) -> bool {
    let mut w = to_wide(text);
    let bytes = w.len() * 2;
    unsafe {
        if EmptyClipboard() == 0 {
            return false;
        }
        let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if hmem.is_null() {
            return false;
        }
        let dst = GlobalLock(hmem) as *mut u16;
        if dst.is_null() {
            return false;
        }
        ptr::copy_nonoverlapping(w.as_mut_ptr(), dst, w.len());
        GlobalUnlock(hmem);
        !SetClipboardData(13, hmem).is_null()
    }
}

fn restore_clipboard_snapshot(expected_gen: u64, seq_after_write: u32) {
    if CLIP_GEN.load(Ordering::SeqCst) != expected_gen {
        return;
    }
    if unsafe { GetClipboardSequenceNumber() } != seq_after_write {
        if let Ok(mut slot) = CLIP_SNAPSHOT.lock() {
            *slot = None;
        }
        return;
    }
    let saved = CLIP_SNAPSHOT.lock().ok().and_then(|mut s| s.take());
    let Some(items) = saved else {
        return;
    };
    if !open_clipboard_retry() {
        return;
    }
    unsafe {
        if EmptyClipboard() != 0 {
            for (fmt, bytes) in items {
                let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
                if hmem.is_null() {
                    continue;
                }
                let dst = GlobalLock(hmem) as *mut u8;
                if !dst.is_null() {
                    ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
                    GlobalUnlock(hmem);
                    let _ = SetClipboardData(fmt, hmem);
                }
            }
        }
        CloseClipboard();
    }
}

pub fn write_clipboard_for_paste_with_restore(text: &str) -> bool {
    if !open_clipboard_retry() {
        return false;
    }
    if let Ok(mut slot) = CLIP_SNAPSHOT.lock()
        && slot.is_none()
    {
        *slot = Some(snapshot_current_clipboard_locked());
    }
    let ok = write_unicode_clipboard_locked(text);
    let seq = unsafe { GetClipboardSequenceNumber() };
    unsafe {
        CloseClipboard();
    }
    if ok {
        let next_gen = CLIP_GEN.fetch_add(1, Ordering::SeqCst) + 1;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(85));
            restore_clipboard_snapshot(next_gen, seq);
        });
    }
    ok
}

pub fn is_focused_password_field() -> bool {
    let mut gti: GUITHREADINFO = unsafe { std::mem::zeroed() };
    gti.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if unsafe { GetGUIThreadInfo(0, &mut gti) } == 0 || gti.hwndFocus.is_null() {
        return false;
    }
    let mut buf = [0u16; 128];
    let len = unsafe { GetClassNameW(gti.hwndFocus, buf.as_mut_ptr(), buf.len() as i32) };
    if len <= 0 {
        return false;
    }
    let cls = String::from_utf16_lossy(&buf[..len as usize]).to_ascii_lowercase();
    if !cls.contains("edit") {
        return false;
    }
    let style = unsafe { GetWindowLongW(gti.hwndFocus, GWL_STYLE) } as u32;
    if (style & 0x0020) != 0 {
        return true;
    }
    let mut pw_char: usize = 0;
    let ok = unsafe {
        SendMessageTimeoutW(
            gti.hwndFocus,
            0x00D2,
            0,
            0,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            15,
            &mut pw_char,
        )
    };
    ok != 0 && pw_char != 0
}

pub fn is_foreground_layout_us() -> bool {
    let fg = unsafe { GetForegroundWindow() };
    let tid = if fg.is_null() {
        0
    } else {
        unsafe { GetWindowThreadProcessId(fg, ptr::null_mut()) }
    };
    let hkl = unsafe { GetKeyboardLayout(tid) } as usize;
    let primary = (hkl as u16) & 0x03FF;
    primary == 0 || primary == 0x09
}

#[derive(Clone, Copy)]
struct TapHotkeyState {
    shift_held: bool,
    down_alone: bool,
    press_at: Option<Instant>,
    last_tap_at: Option<Instant>,
    ctrl_space_held: bool,
}

impl TapHotkeyState {
    const fn new() -> Self {
        Self {
            shift_held: false,
            down_alone: false,
            press_at: None,
            last_tap_at: None,
            ctrl_space_held: false,
        }
    }
}

thread_local! {
    static TAP_STATE: RefCell<TapHotkeyState> = const { RefCell::new(TapHotkeyState::new()) };
}

pub fn update_shift_tap_state(
    vk: u16,
    is_down: bool,
    is_up: bool,
    ctrl: bool,
    alt: bool,
    win: bool,
) -> bool {
    let is_shift = matches!(vk, 0x10 | 0xA0 | 0xA1);
    TAP_STATE.with(|cell| {
        let mut t = cell.borrow_mut();
        if vk == 0x20 && is_up || matches!(vk, 0x11 | 0xA2 | 0xA3) && is_up {
            t.ctrl_space_held = false;
        }
        if is_down {
            if is_shift && !ctrl && !alt && !win {
                if !t.shift_held {
                    t.shift_held = true;
                    t.down_alone = true;
                    t.press_at = Some(Instant::now());
                }
            } else {
                t.down_alone = false;
                t.last_tap_at = None;
            }
            return false;
        }
        if is_up && is_shift {
            let held_ok = t.press_at.is_some_and(|p| p.elapsed().as_millis() <= 350);
            let valid = t.down_alone && held_ok && !ctrl && !alt && !win;
            t.shift_held = false;
            t.down_alone = false;
            t.press_at = None;
            if valid {
                if t.last_tap_at.is_some_and(|prev| prev.elapsed().as_millis() <= 400) {
                    t.last_tap_at = None;
                    return true;
                }
                t.last_tap_at = Some(Instant::now());
            } else {
                t.last_tap_at = None;
            }
        }
        false
    })
}

pub fn take_fresh_ctrl_space_press() -> bool {
    TAP_STATE.with(|c| {
        let mut t = c.borrow_mut();
        let fresh = !t.ctrl_space_held;
        t.ctrl_space_held = true;
        fresh
    })
}
