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

pub fn load_config() -> Config {
    let path = config_file_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| Config::from_json_str(&raw).ok())
        .unwrap_or_default()
}

fn write_atomic(path: &Path, body: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, body).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

pub fn save_config(cfg: &Config) {
    if let Ok(json) = cfg.to_json_pretty() {
        let _ = write_atomic(&config_file_path(), &json);
    }
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
    fs::metadata(config_file_path())
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0)
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

pub fn detect_active_x11_app() -> Option<String> {
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
    let cls_out = Command::new("xprop").args(["-id", win_id, "WM_CLASS"]).output().ok()?;
    if !cls_out.status.success() {
        return None;
    }
    let cls_raw = String::from_utf8_lossy(&cls_out.stdout);
    let last_quoted = cls_raw.rsplit('"').nth(1)?.trim();
    if last_quoted.is_empty() {
        None
    } else {
        Some(last_quoted.to_string())
    }
}

pub fn render_ibus_component_xml(exec_path: &str, setup_path: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <component>\n\
         \t<name>org.freedesktop.IBus.MSKey</name>\n\
         \t<description>MS Key Vietnamese Unicode Input Method</description>\n\
         \t<exec>{exec_path} --ibus</exec>\n\
         \t<version>0.1.0</version>\n\
         \t<author>MS Key</author>\n\
         \t<license>MIT</license>\n\
         \t<textdomain>mskey</textdomain>\n\
         \t<engines>\n\
         \t\t<engine>\n\
         \t\t\t<name>mskey</name>\n\
         \t\t\t<language>vi</language>\n\
         \t\t\t<license>MIT</license>\n\
         \t\t\t<author>MS Key</author>\n\
         \t\t\t<icon>preferences-desktop-locale</icon>\n\
         \t\t\t<layout>us</layout>\n\
         \t\t\t<longname>MS Key (Tiếng Việt)</longname>\n\
         \t\t\t<description>Vietnamese Unicode IME inspired by EVKey, OpenKey, Gonhanh, IBus Bamboo</description>\n\
         \t\t\t<rank>99</rank>\n\
         \t\t\t<setup>{setup_path} --setup</setup>\n\
         \t\t</engine>\n\
         \t</engines>\n\
         </component>\n"
    )
}
