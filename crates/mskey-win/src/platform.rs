use std::collections::HashSet;
use std::env;
use std::ffi::c_void;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr;
use std::time::Duration;

use mskey_core::Config;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, HWND, LPARAM, LocalFree};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER, TokenElevation, TokenUser,
};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegSetValueExW,
};
use windows_sys::Win32::System::StationsAndDesktops::{
    GetProcessWindowStation, GetThreadDesktop, GetUserObjectInformationW, UOI_NAME,
};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcess, GetCurrentThreadId, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, EnumWindows, FindWindowW, GetClassNameW, GetForegroundWindow,
    GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, SW_SHOWNORMAL,
};

use crate::state::WM_APP_OPEN_MAIN;

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

fn user_object_name(handle: *mut c_void) -> String {
    if handle.is_null() {
        return String::new();
    }
    let mut needed = 0u32;
    let mut buf = [0u16; 128];
    let ok = unsafe {
        GetUserObjectInformationW(
            handle,
            UOI_NAME,
            buf.as_mut_ptr() as *mut c_void,
            (buf.len() * 2) as u32,
            &mut needed,
        )
    };
    if ok == 0 || needed < 2 {
        return String::new();
    }
    let units = ((needed as usize) / 2).min(buf.len());
    from_wide_slice(&buf[..units])
}

fn current_desktop_tag() -> String {
    let winsta = unsafe { GetProcessWindowStation() };
    let desktop = unsafe { GetThreadDesktop(GetCurrentThreadId()) };
    let ws = user_object_name(winsta);
    let dk = user_object_name(desktop);
    match (ws.is_empty(), dk.is_empty()) {
        (false, false) => format!("{ws}\\{dk}"),
        (true, false) => dk,
        (false, true) => ws,
        (true, true) => "default".to_string(),
    }
}

fn single_instance_mutex_name_for(desktop_tag: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in desktop_tag.to_ascii_lowercase().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("Local\\MSKey_SingleInstance_{hash:016x}")
}

fn try_create_single_instance_mutex(name: &[u16]) -> Option<InstanceGuard> {
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

pub fn acquire_single_instance_for(wait_for_release: bool) -> Option<InstanceGuard> {
    let mutex_name = single_instance_mutex_name_for(&current_desktop_tag());
    acquire_named_instance(&mutex_name, wait_for_release)
}

fn acquire_named_instance(mutex_name: &str, wait_for_release: bool) -> Option<InstanceGuard> {
    let name = to_wide(mutex_name);
    let attempts = if wait_for_release { 80 } else { 1 };
    for idx in 0..attempts {
        if let Some(guard) = try_create_single_instance_mutex(&name) {
            return Some(guard);
        }
        if idx + 1 < attempts {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    None
}

pub fn signal_existing_instance() -> bool {
    let cls = to_wide("MSKeyMainWindow");
    for attempt in 0..4 {
        let hwnd = unsafe { FindWindowW(cls.as_ptr(), ptr::null()) };
        if !hwnd.is_null() {
            let mut pid = 0u32;
            unsafe {
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid != 0 {
                    let _ = AllowSetForegroundWindow(pid);
                }
            }
            return unsafe { PostMessageW(hwnd, WM_APP_OPEN_MAIN, 0, 0) } != 0;
        }
        if attempt < 3 {
            std::thread::sleep(Duration::from_millis(40));
        }
    }
    false
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

pub fn relaunch_as_admin(show: bool) -> bool {
    let Ok(exe) = env::current_exe() else {
        return false;
    };
    let verb = to_wide("runas");
    let file = to_wide(&exe.to_string_lossy());
    let params = to_wide(if show { "--relaunch --show" } else { "--relaunch" });
    let ret = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
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
        delete_autostart_task();
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
        register_autostart_task();
    } else if !auto_start || !run_as_admin {
        delete_autostart_task();
    }
}

const AUTOSTART_TASK: &str = "MSKeyAutoStart";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn schtasks() -> Command {
    let mut cmd = Command::new("schtasks");
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

fn delete_autostart_task() {
    let _ = schtasks()
        .args(["/Delete", "/TN", AUTOSTART_TASK, "/F"])
        .output();
}

fn register_autostart_task() -> bool {
    let Ok(exe) = env::current_exe() else {
        return false;
    };
    let Some(sid) = current_user_sid() else {
        return false;
    };
    let xml = autostart_task_xml(&exe, &sid);
    let path = env::temp_dir().join(format!("mskey-autostart-{}.xml", std::process::id()));
    let mut bytes = vec![0xFF, 0xFE];
    for unit in xml.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    if fs::write(&path, &bytes).is_err() {
        return false;
    }
    let created = schtasks()
        .args(["/Create", "/TN", AUTOSTART_TASK, "/F", "/XML"])
        .arg(&path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let _ = fs::remove_file(&path);
    created
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn autostart_task_xml(exe: &Path, user_sid: &str) -> String {
    let command = xml_escape(&exe.display().to_string());
    let work_dir = exe
        .parent()
        .map(|p| xml_escape(&p.display().to_string()))
        .unwrap_or_default();
    let sid = xml_escape(user_sid);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Start MS Key when this user signs in</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{sid}</UserId>
      <Delay>PT5S</Delay>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{sid}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>3</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      <WorkingDirectory>{work_dir}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#
    )
}

fn current_user_sid() -> Option<String> {
    let mut token: HANDLE = ptr::null_mut();
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
    if opened == 0 || token.is_null() {
        return None;
    }
    let mut needed = 0u32;
    unsafe {
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut needed);
    }
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8).max(1)];
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr() as *mut _,
            (buffer.len() * 8) as u32,
            &mut needed,
        )
    };
    unsafe {
        CloseHandle(token);
    }
    if ok == 0 {
        return None;
    }
    let user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
    let mut text: *mut u16 = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 || text.is_null() {
        return None;
    }
    let mut len = 0usize;
    while unsafe { *text.add(len) } != 0 {
        len += 1;
    }
    let sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, len) });
    unsafe {
        LocalFree(text as *mut _);
    }
    Some(sid)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_task_xml_never_expires_and_runs_at_normal_priority() {
        let xml = autostart_task_xml(
            Path::new(r"C:\Users\A&B\AppData\Local\Programs\MS Key\MSKey.exe"),
            "S-1-5-21-1-2-3-1001",
        );
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<Priority>4</Priority>"));
        assert!(xml.contains("<Delay>PT5S</Delay>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
        assert_eq!(xml.matches("<UserId>S-1-5-21-1-2-3-1001</UserId>").count(), 2);
        assert!(xml.contains(
            r"<Command>C:\Users\A&amp;B\AppData\Local\Programs\MS Key\MSKey.exe</Command>"
        ));
        assert!(xml.contains(
            r"<WorkingDirectory>C:\Users\A&amp;B\AppData\Local\Programs\MS Key</WorkingDirectory>"
        ));
    }

    #[test]
    fn current_user_sid_is_a_user_sid() {
        let sid = current_user_sid().expect("token user sid");
        assert!(sid.starts_with("S-1-5-"), "{sid}");
    }

    #[test]
    fn desktop_scoped_mutex_names_differ_across_desktops() {
        let interactive = single_instance_mutex_name_for("WinSta0\\Default");
        let same_case_insensitive = single_instance_mutex_name_for("winsta0\\default");
        let isolated = single_instance_mutex_name_for("WinSta0\\exebox-test");
        assert_eq!(interactive, same_case_insensitive);
        assert_ne!(interactive, isolated);
        assert!(interactive.starts_with("Local\\MSKey_SingleInstance_"));
        assert!(!current_desktop_tag().is_empty());

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let test_name = format!(
            "Local\\MSKey_SingleInstance_Test_{}_{nanos}",
            std::process::id()
        );
        let first = acquire_named_instance(&test_name, false).expect("first instance lock");
        assert!(acquire_named_instance(&test_name, false).is_none());
        assert!(acquire_named_instance(&test_name, true).is_none());
        drop(first);
        assert!(acquire_named_instance(&test_name, false).is_some());
    }
}