use std::collections::HashSet;
use std::env;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr;

use mskey_core::Config;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, HWND, LPARAM};
use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegSetValueExW,
};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcess, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, SW_SHOWNORMAL,
};

const ERROR_ALREADY_EXISTS: u32 = 183;
const CF_UNICODETEXT_FMT: u32 = 13;

pub fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

pub fn from_wide_slice(slice: &[u16]) -> String {
    let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
    String::from_utf16_lossy(&slice[..end])
}

pub struct InstanceGuard(HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

pub fn acquire_single_instance() -> Option<InstanceGuard> {
    let name = to_wide("Local\\MSKey_SingleInstance_Mutex");
    let handle = unsafe { CreateMutexW(ptr::null(), 1, name.as_ptr()) };
    if handle.is_null() {
        return None;
    }
    let err = unsafe { GetLastError() };
    if err == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(handle);
        }
        return None;
    }
    Some(InstanceGuard(handle))
}

pub fn config_file_path() -> PathBuf {
    let base = env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("MSKey").join("config.json")
}

fn file_mtime(path: &Path) -> Option<std::time::SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub fn load_config() -> Config {
    let path = config_file_path();
    let local = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| Config::from_json_str(&raw).ok())
        .unwrap_or_default();

    let sync_str = local.sync_config_path.trim().to_string();
    if !sync_str.is_empty() {
        let sync_path = PathBuf::from(&sync_str);
        if sync_path != path
            && let Ok(raw) = fs::read_to_string(&sync_path)
            && let Ok(mut synced) = Config::from_json_str(&raw)
        {
            let local_t = file_mtime(&path);
            let sync_t = file_mtime(&sync_path);
            let use_sync = match (local_t, sync_t) {
                (Some(lt), Some(st)) => st > lt,
                (None, Some(_)) => true,
                _ => false,
            };
            if use_sync {
                synced.sync_config_path = sync_str;
                if let Ok(json) = synced.to_json_pretty() {
                    if let Some(parent) = path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(&path, json);
                }
                return synced;
            }
        }
    }
    local
}

pub fn save_config(cfg: &Config) {
    let path = config_file_path();
    let Ok(json) = cfg.to_json_pretty() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, &json);

    let sync_str = cfg.sync_config_path.trim();
    if !sync_str.is_empty() {
        let sync_path = PathBuf::from(sync_str);
        if sync_path != path {
            if let Some(parent) = sync_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&sync_path, &json);
        }
    }
}

pub fn is_elevated() -> bool {
    let mut token: HANDLE = ptr::null_mut();
    let ok = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
    if ok == 0 || token.is_null() {
        return false;
    }
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut returned = 0u32;
    let res = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    };
    unsafe {
        CloseHandle(token);
    }
    res != 0 && elevation.TokenIsElevated != 0
}

pub fn relaunch_as_admin() -> bool {
    let Ok(exe) = env::current_exe() else {
        return false;
    };
    let verb = to_wide("runas");
    let file = to_wide(&exe.to_string_lossy());
    let ret = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    (ret as isize) > 32
}

pub fn sync_startup_setting(auto_start: bool, run_as_admin: bool) {
    let run_key = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let value_name = to_wide("MSKey");
    let exe_str = env::current_exe()
        .map(|p| format!("\"{}\"", p.display()))
        .unwrap_or_default();

    let mut hkey: HKEY = ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            run_key.as_ptr(),
            0,
            KEY_WRITE | KEY_READ,
            &mut hkey,
        )
    };

    if auto_start && !run_as_admin {
        let _ = Command::new("schtasks")
            .args(["/Delete", "/TN", "MSKeyAutoStart", "/F"])
            .output();
        if status == 0 && !exe_str.is_empty() {
            let wide_val = to_wide(&exe_str);
            unsafe {
                RegSetValueExW(
                    hkey,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    wide_val.as_ptr() as *const u8,
                    (wide_val.len() * 2) as u32,
                );
            }
        }
    } else if status == 0 {
        unsafe {
            RegDeleteValueW(hkey, value_name.as_ptr());
        }
    }
    if status == 0 && !hkey.is_null() {
        unsafe {
            RegCloseKey(hkey);
        }
    }

    if auto_start && run_as_admin && is_elevated() && !exe_str.is_empty() {
        let _ = Command::new("schtasks")
            .args([
                "/Create",
                "/TN",
                "MSKeyAutoStart",
                "/SC",
                "ONLOGON",
                "/RL",
                "HIGHEST",
                "/F",
                "/TR",
                &exe_str,
            ])
            .output();
    } else if !auto_start || !run_as_admin {
        let _ = Command::new("schtasks")
            .args(["/Delete", "/TN", "MSKeyAutoStart", "/F"])
            .output();
    }
}

fn process_name_from_pid(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    let proc_handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if proc_handle.is_null() {
        return String::new();
    }
    let mut buf = [0u16; 512];
    let mut size = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(proc_handle, 0, buf.as_mut_ptr(), &mut size) };
    unsafe {
        CloseHandle(proc_handle);
    }
    if ok != 0 && size > 0 {
        let full = from_wide_slice(&buf[..size as usize]);
        if let Some(name) = PathBuf::from(full).file_name() {
            return name.to_string_lossy().into_owned();
        }
    }
    String::new()
}

pub fn query_window_title(hwnd: HWND) -> String {
    if hwnd.is_null() {
        return String::new();
    }
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if len > 0 {
        from_wide_slice(&buf[..len as usize])
    } else {
        String::new()
    }
}

pub fn query_foreground_info() -> (HWND, String, String, String, bool) {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return (ptr::null_mut(), String::new(), String::new(), String::new(), true);
    }
    let mut pid = 0u32;
    let tid = unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    let hkl = unsafe { GetKeyboardLayout(tid) } as usize;
    let lang_id = (hkl & 0xFFFF) as u16;
    let primary = lang_id & 0x03FF;
    let is_latin_us = primary == 0 || primary == 0x09;

    let mut class_buf = [0u16; 128];
    let clen = unsafe { GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32) };
    let class_name = if clen > 0 {
        from_wide_slice(&class_buf[..clen as usize])
    } else {
        String::new()
    };

    let proc_name = process_name_from_pid(pid);
    let title = query_window_title(hwnd);
    (hwnd, proc_name, class_name, title, is_latin_us)
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
    if unsafe { IsWindowVisible(hwnd) } == 0 {
        return 1;
    }
    let title = query_window_title(hwnd);
    if title.trim().is_empty() {
        return 1;
    }
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    let proc_name = process_name_from_pid(pid);
    if !proc_name.is_empty() && lparam != 0 {
        let out = unsafe { &mut *(lparam as *mut Vec<(String, String)>) };
        out.push((proc_name, title));
    }
    1
}

pub fn enumerate_running_windows() -> Vec<(String, String)> {
    let mut raw = Vec::<(String, String)>::new();
    unsafe {
        EnumWindows(
            Some(enum_windows_proc),
            (&mut raw as *mut Vec<(String, String)>) as LPARAM,
        );
    }
    let mut seen = HashSet::new();
    let mut list = Vec::with_capacity(raw.len());
    for (proc_name, title) in raw {
        let key = (proc_name.to_ascii_lowercase(), title.clone());
        if seen.insert(key) {
            list.push((proc_name, title));
        }
    }
    list.sort_by(|a, b| {
        a.0.to_ascii_lowercase()
            .cmp(&b.0.to_ascii_lowercase())
            .then_with(|| a.1.to_ascii_lowercase().cmp(&b.1.to_ascii_lowercase()))
    });
    list
}

pub fn read_clipboard_text() -> Option<String> {
    if unsafe { OpenClipboard(ptr::null_mut()) } == 0 {
        return None;
    }
    let hdata = unsafe { GetClipboardData(CF_UNICODETEXT_FMT) };
    let mut result = None;
    if !hdata.is_null() {
        let locked = unsafe { GlobalLock(hdata) } as *const u16;
        if !locked.is_null() {
            let mut len = 0usize;
            unsafe {
                while *locked.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(locked, len);
                result = Some(String::from_utf16_lossy(slice));
                GlobalUnlock(hdata);
            }
        }
    }
    unsafe {
        CloseClipboard();
    }
    result
}

pub fn write_clipboard_text(text: &str) -> bool {
    if unsafe { OpenClipboard(ptr::null_mut()) } == 0 {
        return false;
    }
    unsafe {
        EmptyClipboard();
    }
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let bytes = wide.len() * 2;
    let hmem = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes) };
    let mut ok = false;
    if !hmem.is_null() {
        let locked = unsafe { GlobalLock(hmem) } as *mut u16;
        if !locked.is_null() {
            unsafe {
                ptr::copy_nonoverlapping(wide.as_ptr(), locked, wide.len());
                GlobalUnlock(hmem);
                if !SetClipboardData(CF_UNICODETEXT_FMT, hmem).is_null() {
                    ok = true;
                }
            }
        }
    }
    unsafe {
        CloseClipboard();
    }
    ok
}

pub fn play_switch_beep() {
    unsafe {
        MessageBeep(0);
    }
}
