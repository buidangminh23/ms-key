use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use mskey_core::update::{
    GITHUB_LATEST_RELEASE_API, ReleaseAsset, ReleaseInfo, checksum_asset, expected_sha256_for,
    is_newer_version, parse_github_release_json, parse_sha256sums, preferred_linux_asset,
    sha256_file, verify_sha256_match,
};
use serde_json::Value;

use crate::config_store::{detect_active_x11_window, list_x11_windows};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActiveWindowInfo {
    pub app: String,
    pub title: String,
}

pub fn is_browser_app(app: &str) -> bool {
    let lower = app.to_ascii_lowercase();
    [
        "chrome",
        "chromium",
        "firefox",
        "brave",
        "msedge",
        "edge",
        "vivaldi",
        "opera",
        "zen",
        "librewolf",
    ]
    .iter()
    .any(|k| lower.contains(k))
}

pub fn is_terminal_app(app: &str) -> bool {
    let lower = app.to_ascii_lowercase();
    [
        "alacritty",
        "kitty",
        "wezterm",
        "gnome-terminal",
        "konsole",
        "foot",
        "xfce4-terminal",
        "tilix",
        "terminator",
        "urxvt",
        "xterm",
    ]
    .iter()
    .any(|k| lower.contains(k))
}

pub fn parse_hypr_active_window(raw: &str) -> Option<ActiveWindowInfo> {
    let val: Value = serde_json::from_str(raw).ok()?;
    let mut app = String::new();
    for key in ["class", "initialClass"] {
        if let Some(s) = val.get(key).and_then(|v| v.as_str()) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                app = trimmed.to_string();
                break;
            }
        }
    }
    if app.is_empty() {
        return None;
    }
    let title = val
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    Some(ActiveWindowInfo { app, title })
}

pub fn parse_hypr_active_class(raw: &str) -> Option<String> {
    parse_hypr_active_window(raw).map(|w| w.app)
}

fn sway_node_info(node: &Value) -> Option<ActiveWindowInfo> {
    let mut app = String::new();
    if let Some(id) = node.get("app_id").and_then(|v| v.as_str()) {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            app = trimmed.to_string();
        }
    }
    if app.is_empty()
        && let Some(cls) = node
            .get("window_properties")
            .and_then(|w| w.get("class"))
            .and_then(|v| v.as_str())
    {
        let trimmed = cls.trim();
        if !trimmed.is_empty() {
            app = trimmed.to_string();
        }
    }
    if app.is_empty() {
        return None;
    }
    let title = node
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| {
            node.get("window_properties")
                .and_then(|w| w.get("title"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    Some(ActiveWindowInfo { app, title })
}

fn find_sway_focused_info(node: &Value) -> Option<ActiveWindowInfo> {
    if node.get("focused").and_then(|v| v.as_bool()) == Some(true)
        && let Some(info) = sway_node_info(node)
    {
        return Some(info);
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(arr) = node.get(key).and_then(|v| v.as_array()) {
            for child in arr {
                if let Some(found) = find_sway_focused_info(child) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn collect_sway_windows(node: &Value, out: &mut Vec<ActiveWindowInfo>) {
    if let Some(info) = sway_node_info(node) {
        out.push(info);
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(arr) = node.get(key).and_then(|v| v.as_array()) {
            for child in arr {
                collect_sway_windows(child, out);
            }
        }
    }
}

pub fn parse_sway_focused_window(raw: &str) -> Option<ActiveWindowInfo> {
    let val: Value = serde_json::from_str(raw).ok()?;
    find_sway_focused_info(&val)
}

pub fn parse_sway_focused_app(raw: &str) -> Option<String> {
    parse_sway_focused_window(raw).map(|w| w.app)
}

pub fn parse_gnome_eval_class(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let rest = trimmed.strip_prefix("(true,")?.trim_end_matches(')').trim();
    let unquoted = rest.trim_matches('\'').trim_matches('"').trim();
    if unquoted.is_empty() || unquoted == "null" || unquoted == "undefined" {
        None
    } else {
        Some(unquoted.to_string())
    }
}

pub fn parse_hypr_clients_json(raw: &str) -> Vec<ActiveWindowInfo> {
    let Ok(val) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    let Some(arr) = val.as_array() else {
        return Vec::new();
    };
    let mut list = Vec::new();
    for item in arr {
        let mut app = String::new();
        for key in ["class", "initialClass"] {
            if let Some(s) = item.get(key).and_then(|v| v.as_str()) {
                let t = s.trim();
                if !t.is_empty() {
                    app = t.to_string();
                    break;
                }
            }
        }
        if app.is_empty() {
            continue;
        }
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        list.push(ActiveWindowInfo { app, title });
    }
    list
}

pub fn parse_setxkbmap_layout(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("layout:") {
            let first = rest.trim().split(',').next()?.trim();
            let base = first.split(['(', '+']).next()?.trim();
            if !base.is_empty() {
                return Some(base.to_string());
            }
        }
    }
    None
}

pub fn parse_gnome_mru_xkb(raw: &str) -> Option<String> {
    let start = raw.find("('")?;
    let rest = &raw[start + 2..];
    let q1 = rest.find('\'')?;
    let kind = &rest[..q1];
    if kind.eq_ignore_ascii_case("ibus") {
        return Some("us".to_string());
    }
    let after = &rest[q1 + 1..];
    let s2 = after.find('\'')?;
    let rem = &after[s2 + 1..];
    let e2 = rem.find('\'')?;
    let layout = rem[..e2].trim();
    let base = layout.split(['+', '(']).next()?.trim();
    if base.is_empty() {
        None
    } else {
        Some(base.to_string())
    }
}

pub fn is_us_xkb_layout(layout: &str) -> bool {
    let base = layout.trim().split(['(', '+', ',']).next().unwrap_or("").trim();
    matches!(base.to_ascii_lowercase().as_str(), "us" | "en")
}

fn query_gnome_eval(js: &str) -> Option<String> {
    let out = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.Shell",
            "--object-path",
            "/org/gnome/Shell",
            "--method",
            "org.gnome.Shell.Eval",
            js,
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    parse_gnome_eval_class(&s)
}

pub fn detect_active_window() -> Option<ActiveWindowInfo> {
    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
        && let Ok(out) = Command::new("hyprctl").args(["activewindow", "-j"]).output()
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(info) = parse_hypr_active_window(&s) {
            return Some(info);
        }
    }
    if env::var_os("SWAYSOCK").is_some()
        && let Ok(out) = Command::new("swaymsg").args(["-t", "get_tree"]).output()
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(info) = parse_sway_focused_window(&s) {
            return Some(info);
        }
    }
    if env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_ascii_lowercase().contains("gnome"))
        .unwrap_or(false)
        && let Some(cls) = query_gnome_eval("global.display.focus_window?.get_wm_class()")
    {
        let title = query_gnome_eval("global.display.focus_window?.get_title()").unwrap_or_default();
        return Some(ActiveWindowInfo { app: cls, title });
    }
    detect_active_x11_window().map(|(app, title)| ActiveWindowInfo { app, title })
}

pub fn detect_active_app() -> Option<String> {
    detect_active_window().map(|w| w.app)
}

pub fn list_running_windows() -> Vec<ActiveWindowInfo> {
    let mut results = Vec::new();
    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
        && let Ok(out) = Command::new("hyprctl").args(["clients", "-j"]).output()
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        results = parse_hypr_clients_json(&s);
    } else if env::var_os("SWAYSOCK").is_some()
        && let Ok(out) = Command::new("swaymsg").args(["-t", "get_tree"]).output()
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Ok(val) = serde_json::from_str::<Value>(&s) {
            collect_sway_windows(&val, &mut results);
        }
    }
    if results.is_empty() {
        for (app, title) in list_x11_windows() {
            results.push(ActiveWindowInfo { app, title });
        }
    }
    let mut deduped: Vec<ActiveWindowInfo> = Vec::new();
    for item in results {
        let same = deduped
            .iter()
            .any(|existing| existing.app.eq_ignore_ascii_case(&item.app) && existing.title == item.title);
        if !same {
            deduped.push(item);
        }
    }
    deduped
}

fn read_clipboard_cli_cmd(bin: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(bin).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let txt = String::from_utf8_lossy(&out.stdout).to_string();
    if txt.is_empty() { None } else { Some(txt) }
}

fn write_clipboard_cli_cmd(bin: &str, args: &[&str], text: &str) -> bool {
    let Ok(mut child) = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take()
        && stdin.write_all(text.as_bytes()).is_err()
    {
        let _ = child.kill();
        return false;
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

pub fn read_clipboard_text() -> Option<String> {
    if env::var_os("WAYLAND_DISPLAY").is_some()
        && let Some(txt) = read_clipboard_cli_cmd("wl-paste", &["--no-newline"])
            .or_else(|| read_clipboard_cli_cmd("wl-paste", &[]))
    {
        return Some(txt);
    }
    if let Some(txt) = read_clipboard_cli_cmd("xclip", &["-o", "-selection", "clipboard"])
        .or_else(|| read_clipboard_cli_cmd("xsel", &["--clipboard", "--output"]))
    {
        return Some(txt);
    }
    #[cfg(target_os = "linux")]
    if let Some(txt) = linux_gui::read_clipboard_gtk_safe() {
        return Some(txt);
    }
    None
}

pub fn write_clipboard_text(text: &str) -> bool {
    if env::var_os("WAYLAND_DISPLAY").is_some()
        && write_clipboard_cli_cmd("wl-copy", &[], text)
    {
        return true;
    }
    if write_clipboard_cli_cmd("xclip", &["-i", "-selection", "clipboard"], text)
        || write_clipboard_cli_cmd("xsel", &["--clipboard", "--input"], text)
    {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        linux_gui::write_clipboard_gtk_safe(text)
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxStagedUpdate {
    pub version: String,
    pub asset_name: String,
    pub file_path: PathBuf,
    pub sha256: String,
    pub is_deb: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxInstallOutcome {
    pub restart_setup: Option<PathBuf>,
    pub message: String,
}

fn linux_curl_bytes(url: &str, max_secs: &str, accept: Option<&str>) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new("curl");
    cmd.args(["-fsSL", "--proto", "=https", "--max-time", max_secs]);
    if let Some(hdr) = accept {
        cmd.args(["-H", hdr]);
    }
    cmd.args(["-H", "User-Agent: MSKey-Linux", url]);
    let out = cmd.output().map_err(|e| format!("Không thể chạy curl: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("Lỗi kết nối HTTPS (mã {})", out.status)
        } else {
            format!("Lỗi kết nối HTTPS: {err}")
        });
    }
    Ok(out.stdout)
}

fn linux_curl_to_file(url: &str, dest: &Path) -> Result<(), String> {
    let dest_str = dest.to_string_lossy().to_string();
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--proto",
            "=https",
            "--max-time",
            "180",
            "-H",
            "User-Agent: MSKey-Linux",
            "-o",
            &dest_str,
            url,
        ])
        .output()
        .map_err(|e| format!("Không thể chạy curl: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("Lỗi tải gói cập nhật (mã {})", out.status)
        } else {
            format!("Lỗi tải gói cập nhật: {err}")
        });
    }
    Ok(())
}

pub fn check_latest_release() -> Result<ReleaseInfo, String> {
    let bytes = linux_curl_bytes(
        GITHUB_LATEST_RELEASE_API,
        "10",
        Some("Accept: application/vnd.github+json"),
    )?;
    let body = String::from_utf8_lossy(&bytes);
    parse_github_release_json(&body)
        .ok_or_else(|| "Không phân tích được thông tin phiên bản mới".to_string())
}

pub fn format_update_check_message(current_version: &str, res: Result<ReleaseInfo, String>) -> (bool, String) {
    match res {
        Ok(info) => {
            if is_newer_version(current_version, &info.version) {
                (
                    true,
                    format!(
                        "Có bản mới {} (hiện tại {}): {}",
                        info.version, current_version, info.html_url
                    ),
                )
            } else {
                (false, format!("Đang dùng bản mới nhất ({current_version})"))
            }
        }
        Err(msg) => (false, msg),
    }
}

pub fn is_deb_installed() -> bool {
    Path::new("/var/lib/dpkg/info/mskey.list").exists()
        || env::current_exe()
            .ok()
            .is_some_and(|p| p.starts_with("/usr/bin/"))
}

pub fn select_linux_update_asset(info: &ReleaseInfo) -> Option<(ReleaseAsset, ReleaseAsset)> {
    let asset = preferred_linux_asset(&info.assets, env::consts::ARCH, is_deb_installed())?.clone();
    let sums = checksum_asset(&info.assets)?.clone();
    Some((asset, sums))
}

pub fn linux_downloads_dir() -> PathBuf {
    if let Some(xdg) = env::var_os("XDG_DOWNLOAD_DIR") {
        let p = PathBuf::from(xdg);
        if !p.as_os_str().is_empty() {
            return p;
        }
    }
    if let Some(home) = env::var_os("HOME") {
        let dl = PathBuf::from(home).join("Downloads");
        return dl;
    }
    env::temp_dir()
}

pub fn download_and_verify_linux_update(
    version: &str,
    asset: &ReleaseAsset,
    sums_asset: &ReleaseAsset,
    dest_dir: Option<&Path>,
) -> Result<LinuxStagedUpdate, String> {
    let sums_bytes = linux_curl_bytes(&sums_asset.download_url, "15", None)?;
    let sums_text = String::from_utf8(sums_bytes)
        .map_err(|_| "Tệp SHA256SUMS không phải UTF-8 hợp lệ".to_string())?;
    let sums = parse_sha256sums(&sums_text);
    if expected_sha256_for(&sums, &asset.name).is_none() {
        return Err(format!(
            "Không tìm thấy SHA-256 cho '{}' trong {}",
            asset.name, sums_asset.name
        ));
    }

    let dir = dest_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(linux_downloads_dir);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Không tạo được thư mục tải về: {e}"))?;
    let safe_name = asset
        .name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("mskey-linux-update");
    let file_path = dir.join(safe_name);
    linux_curl_to_file(&asset.download_url, &file_path)?;

    let actual_sha = match sha256_file(&file_path) {
        Ok(h) => h,
        Err(e) => {
            let _ = fs::remove_file(&file_path);
            return Err(format!("Không tính được SHA-256: {e}"));
        }
    };
    if !verify_sha256_match(&sums, &asset.name, &actual_sha) {
        let _ = fs::remove_file(&file_path);
        return Err(format!(
            "Sai mã SHA-256 cho {}. Tệp đã bị xóa.",
            asset.name
        ));
    }

    Ok(LinuxStagedUpdate {
        version: version.to_string(),
        asset_name: asset.name.clone(),
        file_path,
        sha256: actual_sha,
        is_deb: asset.name.to_ascii_lowercase().ends_with(".deb"),
    })
}

fn find_file_recursive(dir: &Path, filename: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(hit) = find_file_recursive(&p, filename) {
                return Some(hit);
            }
        } else if p.file_name().and_then(|n| n.to_str()) == Some(filename) {
            return Some(p);
        }
    }
    None
}

pub const LINUX_UPDATE_HELPER: &str = "/usr/lib/mskey/mskey-update";
const INSTALL_TIME_LIMIT: Duration = Duration::from_secs(15 * 60);

pub fn has_linux_update_helper() -> bool {
    Path::new(LINUX_UPDATE_HELPER).is_file()
}

fn wait_with_limit(child: &mut Child, limit: Duration) -> std::io::Result<Option<ExitStatus>> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if started.elapsed() >= limit {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

fn restart_ibus_in_background() {
    if let Ok(mut ibus) = Command::new("ibus").arg("restart").spawn() {
        std::thread::spawn(move || {
            let _ = ibus.wait();
        });
    }
}

fn installer_failure(status: Option<ExitStatus>, detail: &str, limit: Duration) -> String {
    let reason = match status.and_then(|s| s.code()) {
        None if status.is_none() => format!("Cập nhật quá {} phút nên đã dừng", limit.as_secs() / 60),
        Some(126) => "Đã hủy cấp quyền cập nhật".to_string(),
        Some(127) => "Máy chưa cho phép cập nhật không cần mật khẩu".to_string(),
        Some(code) => format!("Trình cập nhật dừng với mã {code}"),
        None => "Trình cập nhật bị dừng đột ngột".to_string(),
    };
    let detail = detail.trim();
    if detail.is_empty() { format!("{reason}.") } else { format!("{reason}: {detail}") }
}

fn last_error_line(text: &str) -> String {
    text.lines()
        .rev()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .find(|l| !l.starts_with("W:") && !l.contains("Download is performed unsandboxed"))
        .unwrap_or_default()
        .to_string()
}

fn run_bounded(mut command: Command, limit: Duration) -> Result<(Option<ExitStatus>, String), String> {
    static RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let log_path = env::temp_dir().join(format!("mskey-update-{}-{run}.log", std::process::id()));
    let log = fs::File::create(&log_path).map_err(|e| format!("Không tạo được nhật ký cập nhật: {e}"))?;
    let log_err = log.try_clone().map_err(|e| format!("Không tạo được nhật ký cập nhật: {e}"))?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .spawn()
        .map_err(|e| format!("Không chạy được trình cập nhật: {e}"))?;
    let status = wait_with_limit(&mut child, limit).map_err(|e| format!("Không theo dõi được trình cập nhật: {e}"))?;
    let output = fs::read_to_string(&log_path).unwrap_or_default();
    let _ = fs::remove_file(&log_path);
    Ok((status, output))
}

pub fn install_linux_update_with_helper(version: &str) -> Result<LinuxInstallOutcome, String> {
    install_with_helper_command(version, Command::new("pkexec"), INSTALL_TIME_LIMIT)
}

fn install_with_helper_command(version: &str, mut command: Command, limit: Duration) -> Result<LinuxInstallOutcome, String> {
    command.args(["--disable-internal-agent", LINUX_UPDATE_HELPER, version]);
    let (status, output) = run_bounded(command, limit)?;
    if status.is_some_and(|s| s.success()) {
        restart_ibus_in_background();
        return Ok(LinuxInstallOutcome {
            restart_setup: env::current_exe().ok(),
            message: format!("Đã cập nhật MS Key lên v{version}."),
        });
    }
    Err(installer_failure(status, &last_error_line(&output), limit))
}

fn finish_deb_install(
    child_command: Command,
    staged: &LinuxStagedUpdate,
    setup_path: Option<PathBuf>,
    limit: Duration,
) -> Result<LinuxInstallOutcome, String> {
    let (status, output) = run_bounded(child_command, limit)?;
    if status.is_some_and(|s| s.success()) {
        restart_ibus_in_background();
        return Ok(LinuxInstallOutcome {
            restart_setup: setup_path,
            message: format!("Đã cập nhật MS Key lên v{}.", staged.version),
        });
    }
    Err(format!(
        "{} Gói đã xác thực nằm tại {}.",
        installer_failure(status, &last_error_line(&output), limit),
        staged.file_path.display()
    ))
}

pub fn apply_linux_staged_update(staged: &LinuxStagedUpdate) -> Result<LinuxInstallOutcome, String> {
    let short_hash: String = staged.sha256.chars().take(12).collect();
    let setup_path = env::current_exe().ok();
    if staged.is_deb {
        if Command::new("pkexec").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() {
            let mut command = Command::new("pkexec");
            command.args(["--disable-internal-agent", "dpkg", "-i"]).arg(&staged.file_path);
            return finish_deb_install(command, staged, setup_path, INSTALL_TIME_LIMIT);
        }
        return Ok(LinuxInstallOutcome {
            restart_setup: None,
            message: format!(
                "Đã tải và xác thực gói tại {}. Chạy: sudo dpkg -i \"{}\"",
                staged.file_path.display(),
                staged.file_path.display()
            ),
        });
    }

    let extract_dir = env::temp_dir().join(format!("mskey-extract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&extract_dir);
    fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Không tạo được thư mục giải nén tạm: {e}"))?;
    let tar_res = Command::new("tar")
        .args([
            "-xzf",
            &staged.file_path.to_string_lossy(),
            "-C",
            &extract_dir.to_string_lossy(),
        ])
        .output()
        .map_err(|e| format!("Không thể chạy tar: {e}"))?;
    if !tar_res.status.success() {
        let _ = fs::remove_dir_all(&extract_dir);
        return Err("Giải nén gói cập nhật .tar.gz thất bại".to_string());
    }

    let new_bin = find_file_recursive(&extract_dir, "mskey-linux");
    if let (Some(src_bin), Some(cur_exe)) = (new_bin, setup_path)
        && let Some(cur_dir) = cur_exe.parent()
    {
        let temp_dest = cur_dir.join(format!(".mskey-linux.new.{}", std::process::id()));
        if fs::copy(&src_bin, &temp_dest).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&temp_dest, fs::Permissions::from_mode(0o755));
            }
            if fs::rename(&temp_dest, &cur_exe).is_ok() {
                let _ = fs::remove_dir_all(&extract_dir);
                return Ok(LinuxInstallOutcome {
                    restart_setup: Some(cur_exe),
                    message: format!("Đã cập nhật MS Key v{} (SHA-256 {}...).", staged.version, short_hash),
                });
            }
            let _ = fs::remove_file(&temp_dest);
        }
    }

    let _ = fs::remove_dir_all(&extract_dir);
    if let Some(parent) = staged.file_path.parent() {
        let _ = Command::new("xdg-open").arg(parent).spawn();
    }
    Ok(LinuxInstallOutcome {
        restart_setup: None,
        message: format!(
            "Đã tải và xác thực SHA-256 ({}...) tại:\n{}\n\nThư mục cài đặt cần quyền root; hãy giải nén và chạy install.sh với sudo.",
            short_hash,
            staged.file_path.display()
        ),
    })
}

#[cfg(all(test, unix))]
mod installer_tests {
    use super::*;

    fn fake(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", script, "fake-pkexec"]);
        command
    }

    #[test]
    fn a_successful_helper_reports_the_new_version() {
        let out = install_with_helper_command("9.9.9", fake("exit 0"), Duration::from_secs(5)).unwrap();
        assert!(out.message.contains("v9.9.9"));
    }

    #[test]
    fn a_hung_helper_is_stopped_at_the_limit() {
        let started = Instant::now();
        let err = install_with_helper_command("9.9.9", fake("sleep 30"), Duration::from_millis(400)).unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(err.contains("quá"));
    }

    #[test]
    fn a_failed_helper_shows_its_last_message() {
        let err = install_with_helper_command("9.9.9", fake("echo 'W: noise' >&2; echo 'Sai mã SHA-256' >&2; exit 6"), Duration::from_secs(5)).unwrap_err();
        assert!(err.contains("mã 6"));
        assert!(err.contains("Sai mã SHA-256"));
    }

    #[test]
    fn a_refused_authorization_is_explained() {
        let err = install_with_helper_command("9.9.9", fake("exit 126"), Duration::from_secs(5)).unwrap_err();
        assert!(err.contains("hủy"));
    }
}

pub fn is_us_keyboard_layout() -> bool {
    if env::var_os("DISPLAY").is_some()
        && let Ok(out) = Command::new("setxkbmap").arg("-query").output()
            && out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Some(layout) = parse_setxkbmap_layout(&s) {
                    return is_us_xkb_layout(&layout);
                }
            }
    if let Ok(out) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.input-sources", "mru-sources"])
        .output()
        && out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(layout) = parse_gnome_mru_xkb(&s) {
                return is_us_xkb_layout(&layout);
            }
        }
    true
}

#[cfg(target_os = "linux")]
mod linux_gui {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::fs::{File, OpenOptions};
    use std::os::fd::AsRawFd;
    use std::os::raw::c_int;
    use std::process::Command;
    use std::ptr;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::config_store::config_dir;
    use crate::gio_ffi::{cstr, g_source_remove, g_timeout_add};
    use crate::gtk_ffi::*;
    use crate::gtk_helpers::{read_clipboard_gtk, write_clipboard_gtk};

    unsafe extern "C" {
        fn flock(fd: c_int, operation: c_int) -> c_int;
    }

    struct OsdWidgets {
        win: *mut GtkWidget,
        label: *mut GtkWidget,
        css_loaded: bool,
        timer_id: u32,
    }

    thread_local! {
        static OSD: RefCell<Option<OsdWidgets>> = const { RefCell::new(None) };
        static CARET_HUD: RefCell<Option<OsdWidgets>> = const { RefCell::new(None) };
    }

    static CLIPBOARD_GEN: AtomicU64 = AtomicU64::new(0);
    static OSD_CSS_ONCE: OnceLock<()> = OnceLock::new();

    pub fn ensure_gtk_init() -> bool {
        static INIT: OnceLock<bool> = OnceLock::new();
        *INIT.get_or_init(|| unsafe { gtk_init_check(ptr::null_mut(), ptr::null_mut()) != 0 })
    }

    pub fn flush_gtk_events() {
        if !ensure_gtk_init() {
            return;
        }
        unsafe {
            for _ in 0..64 {
                if gtk_events_pending() == 0 {
                    break;
                }
                gtk_main_iteration_do(0);
            }
        }
    }

    pub fn acquire_setup_lock() -> Option<File> {
        let dir = config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("setup.lock");
        let file = OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path).ok()?;
        let rc = unsafe { flock(file.as_raw_fd(), 2 | 4) };
        if rc == 0 { Some(file) } else { None }
    }

    pub fn trigger_paste_keystroke(shift_insert: bool) {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        if wayland {
            let mut cmd = Command::new("wtype");
            if shift_insert {
                cmd.args(["-M", "shift", "-k", "Insert", "-m", "shift"]);
            } else {
                cmd.args(["-M", "ctrl", "v", "-m", "ctrl"]);
            }
            if let Ok(mut child) = cmd.spawn() {
                let _ = child.wait();
                return;
            }
        }
        let combo = if shift_insert { "Shift+Insert" } else { "ctrl+v" };
        if let Ok(mut child) = Command::new("xdotool")
            .args(["key", "--clearmodifiers", combo])
            .spawn()
        {
            let _ = child.wait();
        }
    }

    pub fn trigger_copy_keystroke(terminal_mode: bool) {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        if wayland {
            let mut cmd = Command::new("wtype");
            if terminal_mode {
                cmd.args(["-M", "ctrl", "-M", "shift", "c", "-m", "shift", "-m", "ctrl"]);
            } else {
                cmd.args(["-M", "ctrl", "c", "-m", "ctrl"]);
            }
            if let Ok(mut child) = cmd.spawn() {
                let _ = child.wait();
                return;
            }
        }
        let combo = if terminal_mode { "ctrl+shift+c" } else { "ctrl+c" };
        if let Ok(mut child) = Command::new("xdotool")
            .args(["key", "--clearmodifiers", combo])
            .spawn()
        {
            let _ = child.wait();
        }
    }

    pub fn read_clipboard_gtk_safe() -> Option<String> {
        if !ensure_gtk_init() {
            return None;
        }
        let txt = read_clipboard_gtk();
        if txt.is_empty() { None } else { Some(txt) }
    }

    pub fn write_clipboard_gtk_safe(text: &str) -> bool {
        if !ensure_gtk_init() {
            return false;
        }
        write_clipboard_gtk(text);
        unsafe {
            let display = gdk_display_get_default();
            let atom = gdk_atom_intern_static_string(cstr("CLIPBOARD").as_ptr());
            if !display.is_null() && !atom.is_null() {
                let cb = gtk_clipboard_get_for_display(display, atom);
                if !cb.is_null() {
                    gtk_clipboard_store(cb);
                }
            }
        }
        flush_gtk_events();
        true
    }

    unsafe extern "C" fn restore_clipboard_cb(data: *mut c_void) -> c_int {
        if data.is_null() {
            return 0;
        }
        let boxed = unsafe { Box::from_raw(data as *mut (u64, String)) };
        if CLIPBOARD_GEN.load(Ordering::Relaxed) == boxed.0 {
            write_clipboard_gtk(&boxed.1);
            unsafe {
                let display = gdk_display_get_default();
                let atom = gdk_atom_intern_static_string(cstr("CLIPBOARD").as_ptr());
                if !display.is_null() && !atom.is_null() {
                    let cb = gtk_clipboard_get_for_display(display, atom);
                    if !cb.is_null() {
                        gtk_clipboard_store(cb);
                    }
                }
            }
            flush_gtk_events();
        }
        0
    }

    pub fn apply_clipboard_paste_linux(text: &str, shift_insert: bool, delay_ms: u32) {
        if !ensure_gtk_init() {
            return;
        }
        let paste_gen = CLIPBOARD_GEN.fetch_add(1, Ordering::Relaxed) + 1;
        let previous = read_clipboard_gtk();
        write_clipboard_gtk(text);
        unsafe {
            let display = gdk_display_get_default();
            let atom = gdk_atom_intern_static_string(cstr("CLIPBOARD").as_ptr());
            if !display.is_null() && !atom.is_null() {
                let cb = gtk_clipboard_get_for_display(display, atom);
                if !cb.is_null() {
                    gtk_clipboard_store(cb);
                }
            }
        }
        flush_gtk_events();
        trigger_paste_keystroke(shift_insert);
        flush_gtk_events();
        if !previous.is_empty() {
            let payload = Box::into_raw(Box::new((paste_gen, previous)));
            let wait = 95 + delay_ms.min(500);
            unsafe {
                g_timeout_add(wait, Some(restore_clipboard_cb), payload as *mut c_void);
            }
        }
    }

    fn hide_popup(cell_ref: &'static std::thread::LocalKey<RefCell<Option<OsdWidgets>>>) {
        cell_ref.with(|cell| {
            if let Some(ref mut w) = *cell.borrow_mut() {
                if w.timer_id != 0 {
                    unsafe {
                        g_source_remove(w.timer_id);
                    }
                    w.timer_id = 0;
                }
                unsafe {
                    gtk_widget_hide(w.win);
                }
            }
        });
    }

    unsafe extern "C" fn hide_osd_cb(_: *mut c_void) -> c_int {
        OSD.with(|cell| {
            if let Some(ref mut osd) = *cell.borrow_mut() {
                osd.timer_id = 0;
                unsafe {
                    gtk_widget_hide(osd.win);
                }
            }
        });
        0
    }

    unsafe extern "C" fn hide_caret_hud_cb(_: *mut c_void) -> c_int {
        CARET_HUD.with(|cell| {
            if let Some(ref mut hud) = *cell.borrow_mut() {
                hud.timer_id = 0;
                unsafe {
                    gtk_widget_hide(hud.win);
                }
            }
        });
        0
    }

    fn ensure_osd_css_loaded() {
        OSD_CSS_ONCE.get_or_init(|| unsafe {
            let css = gtk_css_provider_new();
            let rules = "window.mskey-osd { border-radius: 10px; border: 1px solid #0e7490; background: #0f172a; }
                         window.mskey-osd.osd-en { border-color: #d97706; background: #1e293b; }
                         .mskey-osd-text { font-weight: 700; font-size: 13px; color: #f8fafc; padding: 2px 8px; }
                         window.mskey-caret-hud { border-radius: 6px; border: 1px solid #0e7490; background: #0f172a; }
                         window.mskey-caret-hud.osd-en { border-color: #d97706; background: #1e293b; }
                         .mskey-caret-hud-text { font-weight: 700; font-size: 11px; color: #f8fafc; padding: 1px 5px; }";
            let rc = cstr(rules);
            gtk_css_provider_load_from_data(css, rc.as_ptr(), -1, ptr::null_mut());
            let screen = gdk_screen_get_default();
            if !screen.is_null() {
                gtk_style_context_add_provider_for_screen(
                    screen,
                    css,
                    GTK_STYLE_PROVIDER_PRIORITY_APPLICATION + 10,
                );
            }
        });
    }

    fn init_popup_window(
        width: c_int,
        height: c_int,
        border: u32,
        win_class: &str,
        text_class: &str,
        initial_text: &str,
    ) -> OsdWidgets {
        unsafe {
            let win = gtk_window_new(GTK_WINDOW_POPUP);
            gtk_window_set_decorated(win, 0);
            gtk_window_set_keep_above(win, 1);
            gtk_window_set_accept_focus(win, 0);
            gtk_window_set_skip_taskbar_hint(win, 1);
            gtk_window_set_skip_pager_hint(win, 1);
            gtk_window_set_default_size(win, width, height);
            gtk_container_set_border_width(win, border);
            let ctx = gtk_widget_get_style_context(win);
            gtk_style_context_add_class(ctx, cstr(win_class).as_ptr());

            let lbl = gtk_label_new(cstr(initial_text).as_ptr());
            gtk_label_set_xalign(lbl, 0.5);
            let lctx = gtk_widget_get_style_context(lbl);
            gtk_style_context_add_class(lctx, cstr(text_class).as_ptr());
            gtk_container_add(win, lbl);

            ensure_osd_css_loaded();
            OsdWidgets {
                win,
                label: lbl,
                css_loaded: true,
                timer_id: 0,
            }
        }
    }

    struct PopupSpec {
        width: i32,
        height: i32,
        y_offset: i32,
        fallback_right_margin: i32,
        fallback_y: i32,
        duration_ms: u32,
    }

    fn show_popup_pill(
        cell_ref: &'static std::thread::LocalKey<RefCell<Option<OsdWidgets>>>,
        init_fn: fn() -> OsdWidgets,
        text: &str,
        vietnamese: bool,
        anchor: Option<(i32, i32)>,
        spec: &PopupSpec,
        hide_cb: unsafe extern "C" fn(*mut c_void) -> c_int,
    ) {
        if !ensure_gtk_init() {
            return;
        }
        cell_ref.with(|cell| {
            let mut opt = cell.borrow_mut();
            if opt.is_none() {
                *opt = Some(init_fn());
            }
            if let Some(ref mut w) = *opt {
                if w.timer_id != 0 {
                    unsafe {
                        g_source_remove(w.timer_id);
                    }
                    w.timer_id = 0;
                }
                unsafe {
                    let _ = w.css_loaded;
                    gtk_label_set_text(w.label, cstr(text).as_ptr());
                    let ctx = gtk_widget_get_style_context(w.win);
                    let cls = cstr("osd-en");
                    if vietnamese {
                        gtk_style_context_remove_class(ctx, cls.as_ptr());
                    } else {
                        gtk_style_context_add_class(ctx, cls.as_ptr());
                    }
                    let screen = gdk_screen_get_default();
                    let sw = if !screen.is_null() {
                        gdk_screen_get_width(screen)
                    } else {
                        1280
                    };
                    let sh = if !screen.is_null() {
                        gdk_screen_get_height(screen)
                    } else {
                        800
                    };
                    let max_x = (sw - spec.width - 12).max(16);
                    let max_y = (sh - spec.height - 16).max(16);
                    let (x, y) = match anchor {
                        Some((ax, ay)) if ax > 0 || ay > 0 => {
                            let target_y = if spec.y_offset < 0 && ay + spec.y_offset < 12 {
                                ay + 24
                            } else {
                                ay + spec.y_offset
                            };
                            ((ax + 8).clamp(16, max_x), target_y.clamp(12, max_y))
                        }
                        _ => ((sw - spec.fallback_right_margin).max(24), spec.fallback_y),
                    };
                    gtk_window_move(w.win, x, y);
                    gtk_widget_show_all(w.win);
                    w.timer_id = g_timeout_add(spec.duration_ms, Some(hide_cb), ptr::null_mut());
                }
            }
        });
    }

    fn init_osd_window() -> OsdWidgets {
        init_popup_window(168, 38, 8, "mskey-osd", "mskey-osd-text", "V  |  Tiếng Việt")
    }

    fn init_caret_hud_window() -> OsdWidgets {
        init_popup_window(30, 22, 2, "mskey-caret-hud", "mskey-caret-hud-text", "V")
    }

    pub fn show_linux_switch_osd(vietnamese: bool, anchor: Option<(i32, i32)>) {
        let text = if vietnamese {
            "V  |  Tiếng Việt"
        } else {
            "E  |  English"
        };
        let spec = PopupSpec {
            width: 188,
            height: 44,
            y_offset: 28,
            fallback_right_margin: 196,
            fallback_y: 48,
            duration_ms: 950,
        };
        show_popup_pill(&OSD, init_osd_window, text, vietnamese, anchor, &spec, hide_osd_cb);
    }

    pub fn show_linux_caret_hud(vietnamese: bool, anchor: Option<(i32, i32)>) {
        let text = if vietnamese { "V" } else { "E" };
        let spec = PopupSpec {
            width: 34,
            height: 26,
            y_offset: -28,
            fallback_right_margin: 64,
            fallback_y: 24,
            duration_ms: 1500,
        };
        show_popup_pill(
            &CARET_HUD,
            init_caret_hud_window,
            text,
            vietnamese,
            anchor,
            &spec,
            hide_caret_hud_cb,
        );
    }

    pub fn hide_linux_caret_hud() {
        hide_popup(&CARET_HUD);
    }
}

#[cfg(target_os = "linux")]
pub use linux_gui::{
    acquire_setup_lock, apply_clipboard_paste_linux, ensure_gtk_init, flush_gtk_events,
    hide_linux_caret_hud, show_linux_caret_hud, show_linux_switch_osd, trigger_copy_keystroke,
};
