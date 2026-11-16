use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use windows_sys::Win32::System::SystemInformation::GetLocalTime;

const MAX_LOG_BYTES: u64 = 256 * 1024;
const MIN_UPTIME_FOR_RELAUNCH: Duration = Duration::from_secs(30);
const DETACHED_PROCESS: u32 = 0x0000_0008;

static STARTED: OnceLock<Instant> = OnceLock::new();

pub fn crash_log_path() -> PathBuf {
    crate::platform::config_file_path().with_file_name("crash.log")
}

pub fn install(relaunch_enabled: bool) {
    install_with(crash_log_path(), relaunch_enabled);
}

fn install_with(log_path: PathBuf, relaunch_enabled: bool) {
    STARTED.get_or_init(Instant::now);
    std::panic::set_hook(Box::new(move |info| {
        let uptime = STARTED.get().map(|s| s.elapsed()).unwrap_or_default();
        let relaunch = relaunch_enabled && should_relaunch(uptime);
        let entry = format_entry(&local_timestamp(), uptime, &panic_text(info), relaunch);
        append_log(&log_path, &entry);
        if relaunch {
            relaunch_self();
        }
    }));
}

pub fn should_relaunch(uptime: Duration) -> bool {
    uptime >= MIN_UPTIME_FOR_RELAUNCH
}

fn panic_text(info: &PanicHookInfo<'_>) -> String {
    let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    };
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "unknown location".to_string());
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("unnamed");
    format!("thread '{name}' panicked at {location}: {message}")
}

pub fn format_entry(timestamp: &str, uptime: Duration, panic: &str, relaunch: bool) -> String {
    format!(
        "[{timestamp}] MS Key {} crashed after {}s; {}; relaunch={}\n",
        env!("CARGO_PKG_VERSION"),
        uptime.as_secs(),
        panic,
        if relaunch { "yes" } else { "no" }
    )
}

pub fn append_log(path: &Path, entry: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let too_big = fs::metadata(path).map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false);
    let file = if too_big {
        OpenOptions::new().create(true).write(true).truncate(true).open(path)
    } else {
        OpenOptions::new().create(true).append(true).open(path)
    };
    if let Ok(mut file) = file {
        let _ = file.write_all(entry.as_bytes());
        let _ = file.flush();
    }
}

fn local_timestamp() -> String {
    let st = unsafe {
        let mut st = std::mem::zeroed();
        GetLocalTime(&mut st);
        st
    };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    )
}

fn relaunch_self() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe)
            .arg("--relaunch")
            .creation_flags(DETACHED_PROCESS)
            .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_entries_append_and_relaunch_only_after_warm_up() {
        assert!(!should_relaunch(Duration::from_secs(3)));
        assert!(should_relaunch(Duration::from_secs(144)));

        let entry = format_entry("2026-11-16 22:37:12", Duration::from_secs(144), "thread 'main' panicked at src\\hook.rs:1:2: boom", true);
        assert!(entry.contains("crashed after 144s"));
        assert!(entry.contains("hook.rs:1:2: boom"));
        assert!(entry.ends_with("relaunch=yes\n"));

        let dir = std::env::temp_dir().join(format!("mskey-crash-test-{}", std::process::id()));
        let path = dir.join("crash.log");
        append_log(&path, "one\n");
        append_log(&path, "two\n");
        assert_eq!(fs::read_to_string(&path).unwrap(), "one\ntwo\n");
        let _ = fs::remove_file(&path);

        install_with(path.clone(), false);
        let caught = std::panic::catch_unwind(|| panic!("boom-from-test"));
        let _ = std::panic::take_hook();
        assert!(caught.is_err());
        let logged = fs::read_to_string(&path).unwrap();
        assert!(logged.contains("boom-from-test"), "{logged}");
        assert!(logged.contains("crash.rs:"), "{logged}");
        assert!(logged.contains("relaunch=no"), "{logged}");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }
}
