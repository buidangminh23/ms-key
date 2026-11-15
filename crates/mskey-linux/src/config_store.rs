use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mskey_core::Config;

pub fn config_dir() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME")
        && !xdg.trim().is_empty() {
            return PathBuf::from(xdg).join("mskey");
        }
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("mskey")
}

pub fn config_file_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn resolve_sync_config_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        return Some(PathBuf::from(home).join(rest));
    }
    if trimmed == "~" {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        return Some(PathBuf::from(home));
    }
    Some(PathBuf::from(trimmed))
}

fn read_valid_config(path: &Path) -> Option<Config> {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| Config::from_json_str(&raw).ok())
}

pub fn file_mtime_nanos(path: &Path) -> u128 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

pub fn load_config_at(primary: &Path) -> Config {
    let primary_cfg = read_valid_config(primary);
    let mut cfg = primary_cfg.clone().unwrap_or_default();
    if let Some(sync_path) = resolve_sync_config_path(&cfg.sync_config_path)
        && sync_path != primary
        && let Some(mut synced) = read_valid_config(&sync_path)
    {
        let sync_mtime = file_mtime_nanos(&sync_path);
        let primary_mtime = file_mtime_nanos(primary);
        if primary_cfg.is_none() || sync_mtime > primary_mtime {
            if synced.sync_config_path.trim().is_empty() {
                synced.sync_config_path = cfg.sync_config_path.clone();
            }
            if let Ok(json) = synced.to_json_pretty() {
                let _ = write_atomic(primary, &json);
            }
            cfg = synced;
        }
    }
    cfg
}

pub fn load_config() -> Config {
    load_config_at(&config_file_path())
}

fn write_atomic(path: &Path, body: &str) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = PathBuf::from(format!("{}.tmp", path.to_string_lossy()));
    fs::write(&tmp, body).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

pub fn save_config_at(primary: &Path, cfg: &Config) {
    if let Ok(json) = cfg.to_json_pretty() {
        let _ = write_atomic(primary, &json);
        if let Some(sync_path) = resolve_sync_config_path(&cfg.sync_config_path)
            && sync_path != primary
        {
            let _ = write_atomic(&sync_path, &json);
        }
    }
}

pub fn save_config(cfg: &Config) {
    save_config_at(&config_file_path(), cfg);
}

pub fn last_esc_word_file_path() -> PathBuf {
    config_dir().join("last_esc_word.txt")
}

pub fn save_last_esc_word(word: Option<&str>) {
    if let Some(w) = word {
        let trimmed = w.trim();
        if !trimmed.is_empty() {
            let _ = write_atomic(&last_esc_word_file_path(), trimmed);
        }
    }
}

pub fn load_last_esc_word() -> Option<String> {
    fs::read_to_string(last_esc_word_file_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn app_state_file_path() -> PathBuf {
    config_dir().join("app_state.json")
}

pub fn load_app_state() -> HashMap<String, bool> {
    fs::read_to_string(app_state_file_path())
        .ok()
        .and_then(|raw| serde_json::from_str::<HashMap<String, bool>>(&raw).ok())
        .unwrap_or_default()
}

pub fn save_app_state(map: &HashMap<String, bool>) {
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = write_atomic(&app_state_file_path(), &json);
    }
}

pub fn export_config_json(path: &Path, cfg: &Config) -> Result<(), String> {
    let json = cfg.to_json_pretty().map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn import_config_json(path: &Path) -> Result<Config, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Config::from_json_str(&raw).map_err(|e| e.to_string())
}

pub fn parse_tz_offset(raw: &str) -> Option<i32> {
    let t = raw.trim();
    let (sign, rest) = if let Some(r) = t.strip_prefix('+') {
        (1i32, r)
    } else {
        let r = t.strip_prefix('-')?;
        (-1i32, r)
    };
    let clean: String = rest.chars().filter(|c| *c != ':').collect();
    if clean.len() != 4 || !clean.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let hours: i32 = clean[0..2].parse().ok()?;
    let mins: i32 = clean[2..4].parse().ok()?;
    if hours > 23 || mins > 59 {
        return None;
    }
    Some(sign * (hours * 3600 + mins * 60))
}

pub fn local_utc_offset_secs() -> i32 {
    if let Ok(out) = Command::new("date").arg("+%z").output()
        && out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(secs) = parse_tz_offset(&s) {
                return secs;
            }
        }
    0
}

pub fn config_mtime_nanos() -> u128 {
    file_mtime_nanos(&config_file_path())
}

pub fn effective_config_mtime_nanos(cfg: &Config) -> u128 {
    let primary = config_file_path();
    let mut best = file_mtime_nanos(&primary);
    if let Some(sync_path) = resolve_sync_config_path(&cfg.sync_config_path)
        && sync_path != primary
    {
        best = best.max(file_mtime_nanos(&sync_path));
    }
    best
}

pub fn sync_linux_autostart(enable: bool) {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let autostart_dir = env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&home).join(".config"))
        .join("autostart");
    let file = autostart_dir.join("mskey.desktop");
    if !enable {
        let _ = fs::remove_file(file);
        return;
    }
    let exe = env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "mskey-linux".to_string());
    let _ = fs::create_dir_all(&autostart_dir);
    let body = format!(
        "[Desktop Entry]\nType=Application\nName=MS Key\nExec=\"{exe}\" --autostart\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n"
    );
    let _ = fs::write(file, body);
}

pub fn resolve_ibus_address() -> Option<String> {
    if let Ok(addr) = env::var("IBUS_ADDRESS") {
        let t = addr.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    if let Ok(out) = Command::new("ibus").arg("address").output()
        && out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() && !s.eq_ignore_ascii_case("(null)") {
                return Some(s);
            }
        }
    let home = env::var("HOME").ok()?;
    let bus_dir = Path::new(&home).join(".config/ibus/bus");
    let entries = fs::read_dir(bus_dir).ok()?;
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(txt) = fs::read_to_string(&path)
            && let Some(addr) = parse_ibus_bus_file(&txt) {
                let mt = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                if best.as_ref().map(|(t, _)| mt >= *t).unwrap_or(true) {
                    best = Some((mt, addr));
                }
            }
    }
    best.map(|(_, a)| a)
}

pub fn parse_ibus_bus_file(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("IBUS_ADDRESS=") {
            let addr = rest.trim().trim_matches('\'').trim_matches('"');
            if !addr.is_empty() {
                return Some(addr.to_string());
            }
        }
    }
    None
}

pub fn detect_system_dark_mode() -> bool {
    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        && out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            if s.contains("prefer-dark") {
                return true;
            }
            if s.contains("prefer-light") || s.contains("default") {
                return false;
            }
        }
    env::var("GTK_THEME")
        .map(|t| t.to_ascii_lowercase().contains("dark"))
        .unwrap_or(false)
}

fn extract_xprop_quoted(line: &str) -> Option<String> {
    let idx = line.find('=')?;
    let rhs = line[idx + 1..].trim();
    if rhs.starts_with('"') && rhs.ends_with('"') && rhs.len() >= 2 {
        let inner = &rhs[1..rhs.len() - 1];
        let unescaped = inner.replace("\\\"", "\"").replace("\\\\", "\\");
        let trimmed = unescaped.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    let last_quoted = line.rsplit('"').nth(1)?.trim();
    if last_quoted.is_empty() {
        None
    } else {
        Some(last_quoted.to_string())
    }
}

pub fn parse_x11_wm_class(raw: &str) -> Option<String> {
    for line in raw.lines() {
        if line.contains("WM_CLASS") {
            let last_quoted = line.rsplit('"').nth(1)?.trim();
            if !last_quoted.is_empty() {
                return Some(last_quoted.to_string());
            }
        }
    }
    None
}

pub fn parse_x11_wm_name(raw: &str) -> Option<String> {
    let mut fallback = None;
    for line in raw.lines() {
        if line.contains("_NET_WM_NAME")
            && let Some(val) = extract_xprop_quoted(line)
        {
            return Some(val);
        }
        if line.contains("WM_NAME")
            && fallback.is_none()
        {
            fallback = extract_xprop_quoted(line);
        }
    }
    fallback
}

pub fn query_x11_window_props(win_id: &str) -> (Option<String>, String) {
    let out = match Command::new("xprop")
        .args(["-id", win_id, "WM_CLASS", "_NET_WM_NAME", "WM_NAME"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return (None, String::new()),
    };
    let raw = String::from_utf8_lossy(&out.stdout);
    let app = parse_x11_wm_class(&raw);
    let title = parse_x11_wm_name(&raw).unwrap_or_default();
    (app, title)
}

pub fn detect_active_x11_window() -> Option<(String, String)> {
    let win_out = Command::new("xprop").args(["-root", "_NET_ACTIVE_WINDOW"]).output().ok()?;
    if !win_out.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&win_out.stdout);
    let win_id = raw
        .split_whitespace()
        .find(|w| w.starts_with("0x"))?
        .trim_end_matches(',');
    if win_id == "0x0" {
        return None;
    }
    let (app, title) = query_x11_window_props(win_id);
    app.map(|a| (a, title))
}

pub fn detect_active_x11_app() -> Option<String> {
    detect_active_x11_window().map(|(app, _)| app)
}

pub fn list_x11_windows() -> Vec<(String, String)> {
    let mut out_list = Vec::new();
    let list_out = match Command::new("xprop").args(["-root", "_NET_CLIENT_LIST"]).output() {
        Ok(o) if o.status.success() => o,
        _ => return out_list,
    };
    let raw = String::from_utf8_lossy(&list_out.stdout);
    for token in raw.split_whitespace() {
        let win_id = token.trim_end_matches(',');
        if !win_id.starts_with("0x") || win_id == "0x0" {
            continue;
        }
        let (app_opt, title) = query_x11_window_props(win_id);
        if let Some(app) = app_opt {
            out_list.push((app, title));
        }
    }
    out_list
}

pub fn render_ibus_component_xml(exec_path: &str, setup_path: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <component>\n\
         \t<name>org.freedesktop.IBus.MSKey</name>\n\
         \t<description>MS Key Vietnamese Unicode Input Method</description>\n\
         \t<exec>{exec_path} --ibus</exec>\n\
         \t<version>{version}</version>\n\
         \t<author>MS Key</author>\n\
         \t<license>MIT</license>\n\
         \t<textdomain>mskey</textdomain>\n\
         \t<engines>\n\
         \t\t<engine>\n\
         \t\t\t<name>mskey</name>\n\
         \t\t\t<language>vi</language>\n\
         \t\t\t<license>MIT</license>\n\
         \t\t\t<author>MS Key</author>\n\
         \t\t\t<icon>mskey</icon>\n\
         \t\t\t<layout>us</layout>\n\
         \t\t\t<longname>MS Key (Tiếng Việt)</longname>\n\
         \t\t\t<description>Vietnamese Unicode IME inspired by EVKey, OpenKey, Gonhanh, IBus Bamboo</description>\n\
         \t\t\t<rank>99</rank>\n\
         \t\t\t<setup>{setup_path} --setup</setup>\n\
         \t\t</engine>\n\
         \t</engines>\n\
         </component>\n",
        version = env!("CARGO_PKG_VERSION")
    )
}
