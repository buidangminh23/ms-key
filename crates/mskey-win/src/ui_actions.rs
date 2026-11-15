use std::fs;
use std::path::Path;
use std::process::Command;

use mskey_core::{AppBehavior, Config};
use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{EnumChildWindows, GetClientRect, GetParent, GetWindowRect};

use crate::dialog_helpers::read_window_text;
use crate::platform::{config_file_path, save_config, to_wide};
use crate::state::with_state;

pub fn import_evkey_tree_via_dialog(hwnd: HWND) {
    let mut file_buf = [0u16; 512];
    let filter = to_wide("EVKey Config (*.tree)|*.tree|All Files (*.*)|*.*|")
        .into_iter()
        .map(|u| if u == b'|' as u16 { 0 } else { u })
        .collect::<Vec<u16>>();
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = hwnd;
    ofn.lpstrFilter = filter.as_ptr();
    ofn.lpstrFile = file_buf.as_mut_ptr();
    ofn.nMaxFile = file_buf.len() as u32;
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST;
    if unsafe { GetOpenFileNameW(&mut ofn) } != 0 {
        let len = file_buf.iter().position(|&c| c == 0).unwrap_or(0);
        let path = String::from_utf16_lossy(&file_buf[..len]);
        if let Ok(raw) = fs::read_to_string(&path) {
            let imported = Config::import_evkey_tree(&raw);
            save_config(&imported);
            with_state(|st| {
                st.user_preferred_ve = imported.vietnamese_enabled;
                st.engine.update_config(imported);
            });
        }
    }
}

pub fn open_config_directory() {
    let path = config_file_path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
        let _ = Command::new("explorer").arg(dir).spawn();
    }
}

pub fn add_current_app_force_english() {
    with_state(|st| {
        if !st.fg_process.is_empty() {
            let proc_name = st.fg_process.clone();
            let mut cfg = st.engine.config().clone();
            cfg.set_app_rule(&proc_name, AppBehavior::ForceEnglish);
            save_config(&cfg);
            st.engine.update_config(cfg);
        }
    });
}

pub fn export_json_backup_dialog(hwnd: HWND) {
    let filter = crate::platform_extra::make_win_filter(&[
        ("MS Key JSON (*.json)", "*.json"),
        ("All Files (*.*)", "*.*"),
    ]);
    let Some(path) = crate::platform_extra::browse_file_dialog(hwnd, true, &filter, "json") else {
        return;
    };
    let cfg = with_state(|st| st.engine.config().clone());
    let _ = crate::platform_extra::export_config_json(&cfg, &path);
}

pub fn import_json_backup_dialog(hwnd: HWND) -> bool {
    let filter = crate::platform_extra::make_win_filter(&[
        ("MS Key JSON (*.json)", "*.json"),
        ("All Files (*.*)", "*.*"),
    ]);
    let Some(path) = crate::platform_extra::browse_file_dialog(hwnd, false, &filter, "json") else {
        return false;
    };
    let Ok(cfg) = crate::platform_extra::import_config_json(&path) else {
        return false;
    };
    save_config(&cfg);
    with_state(|st| {
        st.user_preferred_ve = cfg.vietnamese_enabled;
        st.engine.update_config(cfg);
    });
    true
}

pub fn dump_layout_report(hwnd: HWND, out_path: &Path) {
    let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetClientRect(hwnd, &mut client) };
    let mut items: Vec<(String, i32, i32, i32, i32)> = Vec::new();
    let ptr = &mut items as *mut Vec<(String, i32, i32, i32, i32)> as LPARAM;
    unsafe extern "system" fn collect_child(child: HWND, lparam: LPARAM) -> i32 {
        let list = unsafe { &mut *(lparam as *mut Vec<(String, i32, i32, i32, i32)>) };
        let parent = unsafe { GetParent(child) };
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe {
            GetWindowRect(child, &mut r);
            ScreenToClient(parent, &mut r.left as *mut i32 as *mut _);
            ScreenToClient(parent, &mut r.right as *mut i32 as *mut _);
        }
        let id = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetDlgCtrlID(child) };
        let txt = read_window_text(child);
        list.push((
            format!("id={id} {txt}"),
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
        ));
        1
    }
    unsafe {
        EnumChildWindows(hwnd, Some(collect_child), ptr);
    }
    let mut lines = vec![format!("client={}x{}", client.right, client.bottom)];
    for (name, x, y, w, h) in items {
        lines.push(format!("{name}: x={x}, y={y}, w={w}, h={h}"));
    }
    let _ = fs::write(out_path, lines.join("\n"));
}
