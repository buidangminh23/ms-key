use std::env;
use std::ffi::c_void;
use std::fs::{self, File};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use mskey_core::update::{
    GITHUB_LATEST_RELEASE_API, GITHUB_RELEASES_URL, ReleaseAsset, ReleaseInfo, checksum_asset,
    expected_sha256_for, format_bytes_human, is_newer_version, parse_github_release_json,
    parse_sha256sums, preferred_windows_asset, release_notes_preview, sha256_file,
    verify_sha256_match,
};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
use windows_sys::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE,
    WINHTTP_OPTION_REDIRECT_POLICY, WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_LOCATION, WINHTTP_QUERY_STATUS_CODE,
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryDataAvailable, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest, WinHttpSetOption, WinHttpSetTimeouts,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyWindow, GetDlgItem, IDNO, IDYES, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK,
    MB_YESNO, MB_YESNOCANCEL, MessageBoxW, PostMessageW, SW_SHOWNORMAL,
};

use crate::dialog_helpers::write_window_text;
use crate::platform::to_wide;
use crate::state::WM_APP_UPDATE_RESULT;
use crate::ui_controls::{IDC_BTN_CHECK_UPDATE, IDC_LBL_FOOTER_VER};

const MAX_API_BYTES: u64 = 4 * 1024 * 1024;
const MAX_PKG_BYTES: u64 = 128 * 1024 * 1024;
const MAX_REDIRECTS: usize = 8;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

enum UpdateOutcome {
    Available(ReleaseInfo),
    UpToDate(String),
    Failed(String),
}

struct StagedUpdate {
    version: String,
    package_path: PathBuf,
    staging_dir: PathBuf,
    is_installer: bool,
    sha256: String,
}

enum UpdateReport {
    Check {
        silent: bool,
        outcome: UpdateOutcome,
    },
    Downloaded {
        html_url: String,
        result: Result<StagedUpdate, String>,
    },
}

static OP_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static PENDING_REPORT: Mutex<Option<UpdateReport>> = Mutex::new(None);

struct HttpHandle(*mut c_void);

impl Drop for HttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                WinHttpCloseHandle(self.0);
            }
        }
    }
}

fn set_update_ui_state(owner: HWND, busy: bool, button_label: &str, footer_suffix: Option<&str>) {
    if owner.is_null() {
        return;
    }
    let btn = unsafe { GetDlgItem(owner, IDC_BTN_CHECK_UPDATE as i32) };
    if !btn.is_null() {
        write_window_text(btn, button_label);
        unsafe {
            EnableWindow(btn, if busy { 0 } else { 1 });
            InvalidateRect(btn, ptr::null(), 0);
        }
    }
    let lbl = unsafe { GetDlgItem(owner, IDC_LBL_FOOTER_VER as i32) };
    if !lbl.is_null() {
        let suffix = footer_suffix.unwrap_or("Tiếng Việt Unicode");
        let text = format!("MS Key v{} • {}", env!("CARGO_PKG_VERSION"), suffix);
        write_window_text(lbl, &text);
    }
    unsafe {
        InvalidateRect(owner, ptr::null(), 0);
    }
}

pub fn check_for_updates_async(main_hwnd: HWND, silent: bool) {
    if main_hwnd.is_null() {
        return;
    }
    if OP_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        if !silent {
            show_info_box(
                main_hwnd,
                "Cập nhật MS Key",
                "Đang kiểm tra hoặc tải bản cập nhật. Vui lòng chờ giây lát.",
                MB_OK | MB_ICONINFORMATION,
            );
        }
        return;
    }
    set_update_ui_state(main_hwnd, true, "Đang kiểm tra...", Some("Đang kiểm tra cập nhật..."));
    let hwnd_raw = main_hwnd as usize;
    let spawn_res = thread::Builder::new()
        .name("mskey-update-check".to_string())
        .spawn(move || {
            let outcome = run_update_check();
            if let Ok(mut slot) = PENDING_REPORT.lock() {
                *slot = Some(UpdateReport::Check { silent, outcome });
            }
            OP_IN_FLIGHT.store(false, Ordering::Release);
            unsafe {
                PostMessageW(hwnd_raw as HWND, WM_APP_UPDATE_RESULT, 0, 0);
            }
        });
    if spawn_res.is_err() {
        OP_IN_FLIGHT.store(false, Ordering::Release);
        set_update_ui_state(main_hwnd, false, "Kiểm tra cập nhật", None);
    }
}

fn start_download_async(
    main_hwnd: HWND,
    info: ReleaseInfo,
    asset: ReleaseAsset,
    sums_asset: ReleaseAsset,
) {
    if main_hwnd.is_null() || OP_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    set_update_ui_state(
        main_hwnd,
        true,
        "Đang tải về...",
        Some(&format!("Đang tải v{}...", info.version)),
    );
    let hwnd_raw = main_hwnd as usize;
    let html_url = release_page_url(&info);
    let spawn_res = thread::Builder::new()
        .name("mskey-update-download".to_string())
        .spawn(move || {
            let result = download_and_verify_update(&info.version, &asset, &sums_asset);
            if let Ok(mut slot) = PENDING_REPORT.lock() {
                *slot = Some(UpdateReport::Downloaded { html_url, result });
            }
            OP_IN_FLIGHT.store(false, Ordering::Release);
            unsafe {
                PostMessageW(hwnd_raw as HWND, WM_APP_UPDATE_RESULT, 0, 0);
            }
        });
    if spawn_res.is_err() {
        OP_IN_FLIGHT.store(false, Ordering::Release);
        set_update_ui_state(main_hwnd, false, "Kiểm tra cập nhật", None);
    }
}

pub fn handle_update_result_message(owner: HWND) {
    let report = PENDING_REPORT.lock().ok().and_then(|mut s| s.take());
    let Some(report) = report else {
        return;
    };
    set_update_ui_state(owner, false, "Kiểm tra cập nhật", None);
    let current = env!("CARGO_PKG_VERSION");

    match report {
        UpdateReport::Check { silent, outcome } => match outcome {
            UpdateOutcome::Available(info) => {
                set_update_ui_state(
                    owner,
                    false,
                    "Kiểm tra cập nhật",
                    Some(&format!("Có bản mới v{}", info.version)),
                );
                present_available_update(owner, current, info);
            }
            UpdateOutcome::UpToDate(latest) => {
                if silent {
                    return;
                }
                let msg = format!(
                    "Bạn đang sử dụng phiên bản mới nhất (v{}).\nPhiên bản phát hành gần nhất: v{}.",
                    current, latest
                );
                show_info_box(owner, "Cập nhật MS Key", &msg, MB_OK | MB_ICONINFORMATION);
            }
            UpdateOutcome::Failed(err) => {
                if silent {
                    return;
                }
                let msg = format!(
                    "Không thể kiểm tra cập nhật:\n{}\n\nBạn có muốn mở trang GitHub Releases thủ công?",
                    err
                );
                if show_info_box(owner, "Cập nhật MS Key", &msg, MB_YESNO | MB_ICONWARNING) == IDYES {
                    open_url(GITHUB_RELEASES_URL);
                }
            }
        },
        UpdateReport::Downloaded { html_url, result } => match result {
            Ok(staged) => {
                let short_hash: String = staged.sha256.chars().take(12).collect();
                let kind = if staged.is_installer {
                    "trình cài đặt tự động"
                } else {
                    "bản portable"
                };
                let prompt = format!(
                    "Đã tải và xác thực SHA-256 ({}...) cho MS Key v{} ({}).\n\nNhấn Yes để đóng MS Key, cập nhật và tự mở lại ngay.",
                    short_hash, staged.version, kind
                );
                if show_info_box(owner, "Sẵn sàng cập nhật MS Key", &prompt, MB_YESNO | MB_ICONINFORMATION)
                    == IDYES
                {
                    match launch_detached_updater(&staged) {
                        Ok(()) => unsafe {
                            DestroyWindow(owner);
                        },
                        Err(err) => {
                            let _ = fs::remove_dir_all(&staged.staging_dir);
                            let msg = format!(
                                "Không thể khởi chạy trình cập nhật tự động:\n{}\n\nBạn có muốn mở trang phát hành để tải thủ công?",
                                err
                            );
                            if show_info_box(owner, "Cập nhật MS Key", &msg, MB_YESNO | MB_ICONWARNING)
                                == IDYES
                            {
                                open_url(&html_url);
                            }
                        }
                    }
                } else {
                    let _ = fs::remove_dir_all(&staged.staging_dir);
                }
            }
            Err(err) => {
                let msg = format!(
                    "Không thể tải hoặc xác thực bản cập nhật:\n{}\n\nBạn có muốn mở trang GitHub Releases để tải thủ công?",
                    err
                );
                if show_info_box(owner, "Cập nhật MS Key", &msg, MB_YESNO | MB_ICONWARNING) == IDYES {
                    open_url(&html_url);
                }
            }
        },
    }
}

fn present_available_update(owner: HWND, current: &str, info: ReleaseInfo) {
    let notes = release_notes_preview(&info.body, 320);
    let installed = is_installed_exe();
    let asset = preferred_windows_asset(&info.assets, env::consts::ARCH, installed).cloned();
    let sums = checksum_asset(&info.assets).cloned();
    let page_url = release_page_url(&info);

    if let (Some(pkg), Some(checksum)) = (asset, sums) {
        let size_str = format_bytes_human(pkg.size);
        let size_part = if size_str.is_empty() {
            String::new()
        } else {
            format!(" ({size_str})")
        };
        let mut body = format!(
            "Đã có phiên bản mới: v{} (hiện tại: v{}).\nGói cập nhật: {}{}\n\n• Yes: Tự động tải về, kiểm tra SHA-256 và cập nhật.\n• No: Mở trang phát hành trên GitHub.\n• Cancel: Để sau.",
            info.version, current, pkg.name, size_part
        );
        if !notes.is_empty() {
            body.push_str("\n\nGhi chú phát hành:\n");
            body.push_str(&notes);
        }
        let res = show_info_box(
            owner,
            "Cập nhật MS Key",
            &body,
            MB_YESNOCANCEL | MB_ICONINFORMATION,
        );
        match res {
            IDYES => start_download_async(owner, info, pkg, checksum),
            IDNO => open_url(&page_url),
            _ => {}
        }
    } else {
        let mut body = format!(
            "Đã có phiên bản mới: v{} (hiện tại: v{}).\n\nBạn có muốn mở trang tải về trên GitHub?",
            info.version, current
        );
        if !notes.is_empty() {
            body.push_str("\n\nGhi chú phát hành:\n");
            body.push_str(&notes);
        }
        if show_info_box(owner, "Cập nhật MS Key", &body, MB_YESNO | MB_ICONINFORMATION) == IDYES {
            open_url(&page_url);
        }
    }
}

fn release_page_url(info: &ReleaseInfo) -> String {
    let trimmed = info.html_url.trim();
    if trimmed.is_empty() {
        GITHUB_RELEASES_URL.to_string()
    } else {
        trimmed.to_string()
    }
}

fn show_info_box(owner: HWND, title: &str, body: &str, flags: u32) -> i32 {
    let title_w = to_wide(title);
    let body_w = to_wide(body);
    unsafe { MessageBoxW(owner, body_w.as_ptr(), title_w.as_ptr(), flags) }
}

fn open_url(url: &str) {
    let verb = to_wide("open");
    let target = to_wide(url);
    unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

fn is_installed_exe() -> bool {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("Uninstall.exe").exists()))
        .unwrap_or(false)
}

fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("mskey-update.bin");
    let clean: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "mskey-update.bin".to_string()
    } else {
        clean
    }
}

fn download_and_verify_update(
    version: &str,
    asset: &ReleaseAsset,
    sums_asset: &ReleaseAsset,
) -> Result<StagedUpdate, String> {
    let sums_bytes = fetch_https_bytes(
        &sums_asset.download_url,
        "text/plain, application/octet-stream",
        MAX_API_BYTES,
    )?;
    let sums_text = String::from_utf8(sums_bytes)
        .map_err(|_| "Tệp SHA256SUMS không phải UTF-8 hợp lệ".to_string())?;
    let sums = parse_sha256sums(&sums_text);
    if expected_sha256_for(&sums, &asset.name).is_none() {
        return Err(format!(
            "Không tìm thấy checksum SHA-256 cho '{}' trong {}",
            asset.name, sums_asset.name
        ));
    }

    let staging_dir = env::temp_dir().join(format!(
        "mskey-update-{}-{}",
        version.replace(['/', '\\'], "_"),
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Không tạo được thư mục tạm cập nhật: {e}"))?;

    let safe_name = sanitize_filename(&asset.name);
    let package_path = staging_dir.join(&safe_name);
    let download_res = (|| -> Result<StagedUpdate, String> {
        let mut out = File::create(&package_path)
            .map_err(|e| format!("Không thể tạo file tải về: {e}"))?;
        download_https_into(
            &asset.download_url,
            "application/octet-stream",
            MAX_PKG_BYTES,
            &mut out,
        )?;
        out.flush()
            .map_err(|e| format!("Không thể ghi hoàn tất file tải về: {e}"))?;
        drop(out);

        let actual_sha = sha256_file(&package_path)
            .map_err(|e| format!("Không thể tính SHA-256 của file tải về: {e}"))?;
        if !verify_sha256_match(&sums, &asset.name, &actual_sha) {
            return Err(format!(
                "Sai mã kiểm tra SHA-256 cho {}. Tệp đã bị từ chối vì lý do an toàn.",
                asset.name
            ));
        }

        let is_installer = safe_name.to_ascii_lowercase().ends_with(".exe");
        Ok(StagedUpdate {
            version: version.to_string(),
            package_path: package_path.clone(),
            staging_dir: staging_dir.clone(),
            is_installer,
            sha256: actual_sha,
        })
    })();

    if download_res.is_err() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    download_res
}

fn build_windows_updater_script(
    pid: u32,
    package_path: &Path,
    target_exe: &Path,
    staging_dir: &Path,
    is_installer: bool,
) -> Result<String, String> {
    let target_dir = target_exe
        .parent()
        .ok_or_else(|| "Không xác định được thư mục cài đặt hiện tại".to_string())?;
    let pkg = escape_ps_single(&package_path.to_string_lossy());
    let exe = escape_ps_single(&target_exe.to_string_lossy());
    let dir = escape_ps_single(&target_dir.to_string_lossy());
    let stg = escape_ps_single(&staging_dir.to_string_lossy());

    let apply_block = if is_installer {
        format!(
            "$p = New-Object System.Diagnostics.Process\n\
             $p.StartInfo.FileName = '{pkg}'\n\
             $p.StartInfo.Arguments = '/S /D=' + '{dir}'\n\
             $p.StartInfo.UseShellExecute = $false\n\
             $null = $p.Start()\n\
             $p.WaitForExit()\n\
             if ($p.ExitCode -ne 0) {{ throw ('Installer exited ' + $p.ExitCode) }}\n"
        )
    } else {
        format!(
            "$ext = Join-Path '{stg}' 'unpacked'\n\
             New-Item -ItemType Directory -Force -Path $ext | Out-Null\n\
             Expand-Archive -LiteralPath '{pkg}' -DestinationPath $ext -Force\n\
             $srcExe = Get-ChildItem -Path $ext -Filter 'MSKey.exe' -Recurse | Select-Object -First 1\n\
             if (-not $srcExe) {{ throw 'MSKey.exe missing in archive' }}\n\
             Copy-Item -LiteralPath $srcExe.FullName -Destination '{exe}' -Force\n\
             foreach ($txt in @('LICENSE.txt', 'THIRD-PARTY-NOTICES.txt')) {{\n\
               $found = Get-ChildItem -Path $ext -Filter $txt -Recurse | Select-Object -First 1\n\
               if ($found) {{ Copy-Item -LiteralPath $found.FullName -Destination (Join-Path '{dir}' $txt) -Force }}\n\
             }}\n"
        )
    };

    Ok(format!(
        "$ErrorActionPreference = 'Stop'\n\
         for ($i = 0; $i -lt 120; $i++) {{\n\
           if (-not (Get-Process -Id {pid} -ErrorAction SilentlyContinue)) {{ break }}\n\
           Start-Sleep -Milliseconds 150\n\
         }}\n\
         {apply_block}\
         Remove-Item -LiteralPath '{stg}' -Recurse -Force -ErrorAction SilentlyContinue\n\
         if (Test-Path -LiteralPath '{exe}') {{\n\
           Start-Process -FilePath '{exe}' -ArgumentList '--show'\n\
         }}\n"
    ))
}

fn escape_ps_single(value: &str) -> String {
    value.replace('\'', "''")
}

fn encode_ps_utf16le_base64(script: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = *chunk.get(1).unwrap_or(&0) as usize;
        let b2 = *chunk.get(2).unwrap_or(&0) as usize;
        out.push(TABLE[b0 >> 2] as char);
        out.push(TABLE[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((b1 & 0x0f) << 2) | (b2 >> 6)] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[b2 & 0x3f] as char
        } else {
            '='
        });
    }
    out
}

fn launch_detached_updater(staged: &StagedUpdate) -> Result<(), String> {
    let current_exe = env::current_exe()
        .map_err(|e| format!("Không xác định được đường dẫn ứng dụng: {e}"))?;
    let script = build_windows_updater_script(
        std::process::id(),
        &staged.package_path,
        &current_exe,
        &staged.staging_dir,
        staged.is_installer,
    )?;
    let encoded = encode_ps_utf16le_base64(&script);
    Command::new("powershell.exe")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-EncodedCommand", &encoded])
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Không thể chạy PowerShell updater: {e}"))?;
    Ok(())
}

fn parse_https_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("https://")?;
    let (authority, path_raw) = match rest.split_once('/') {
        Some((a, p)) => (a, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    if authority.is_empty() {
        return None;
    }
    let (host, port) = if let Some((h, p)) = authority.rsplit_once(':')
        && !h.contains(']')
        && let Ok(parsed) = p.parse::<u16>()
    {
        (h.to_string(), parsed)
    } else {
        (authority.to_string(), INTERNET_DEFAULT_HTTPS_PORT)
    };
    Some((host, port, path_raw))
}

fn run_update_check() -> UpdateOutcome {
    let raw = match fetch_https_bytes(
        GITHUB_LATEST_RELEASE_API,
        "Accept: application/vnd.github+json\r\n",
        MAX_API_BYTES,
    ) {
        Ok(b) => b,
        Err(e) => return UpdateOutcome::Failed(e),
    };
    let Ok(json) = String::from_utf8(raw) else {
        return UpdateOutcome::Failed("Phản hồi không phải UTF-8 hợp lệ".to_string());
    };
    let Some(info) = parse_github_release_json(&json) else {
        return UpdateOutcome::Failed("Không phân tích được phản hồi phát hành từ GitHub".to_string());
    };
    let current = env!("CARGO_PKG_VERSION");
    if is_newer_version(current, &info.version) {
        UpdateOutcome::Available(info)
    } else {
        UpdateOutcome::UpToDate(info.version)
    }
}

fn fetch_https_bytes(url: &str, extra_headers: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    download_https_into(url, extra_headers, max_bytes, &mut out)?;
    Ok(out)
}

fn query_redirect_location(req: *mut c_void) -> Option<String> {
    let mut byte_len: u32 = 0;
    unsafe {
        WinHttpQueryHeaders(
            req,
            WINHTTP_QUERY_LOCATION,
            ptr::null(),
            ptr::null_mut(),
            &mut byte_len,
            ptr::null_mut(),
        );
    }
    if byte_len == 0 || byte_len > 16 * 1024 {
        return None;
    }
    let mut buf = vec![0u16; (byte_len as usize).div_ceil(2) + 1];
    let ok = unsafe {
        WinHttpQueryHeaders(
            req,
            WINHTTP_QUERY_LOCATION,
            ptr::null(),
            buf.as_mut_ptr().cast(),
            &mut byte_len,
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        return None;
    }
    let chars = (byte_len as usize) / 2;
    let slice = &buf[..chars.min(buf.len())];
    let s = String::from_utf16_lossy(slice).trim_end_matches('\0').trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn download_https_into(
    url: &str,
    extra_headers: &str,
    max_bytes: u64,
    dest: &mut dyn Write,
) -> Result<(), String> {
    let agent_w = to_wide("MSKey-Windows");
    let mut session = HttpHandle(unsafe {
        WinHttpOpen(
            agent_w.as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            ptr::null(),
            ptr::null(),
            0,
        )
    });
    if session.0.is_null() {
        session = HttpHandle(unsafe {
            WinHttpOpen(
                agent_w.as_ptr(),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                ptr::null(),
                ptr::null(),
                0,
            )
        });
    }
    if session.0.is_null() {
        return Err("Không khởi tạo được WinHTTP session".to_string());
    }
    unsafe {
        WinHttpSetTimeouts(session.0, 10_000, 15_000, 15_000, 60_000);
    }

    let mut current_url = url.to_string();
    let verb_w = to_wide("GET");
    let formatted_headers = if extra_headers.is_empty() {
        String::new()
    } else if extra_headers.contains(':') {
        if extra_headers.ends_with("\r\n") {
            extra_headers.to_string()
        } else {
            format!("{extra_headers}\r\n")
        }
    } else {
        format!("Accept: {extra_headers}\r\n")
    };
    let headers_w = to_wide(&formatted_headers);

    for _hop in 0..=MAX_REDIRECTS {
        let Some((host, port, object_path)) = parse_https_url(&current_url) else {
            return Err("URL cập nhật không hợp lệ hoặc không dùng HTTPS".to_string());
        };
        let host_w = to_wide(&host);
        let conn = HttpHandle(unsafe {
            WinHttpConnect(session.0, host_w.as_ptr(), port, 0)
        });
        if conn.0.is_null() {
            return Err(format!("Không kết nối được tới {host}"));
        }

        let path_w = to_wide(&object_path);
        let req = HttpHandle(unsafe {
            WinHttpOpenRequest(
                conn.0,
                verb_w.as_ptr(),
                path_w.as_ptr(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                WINHTTP_FLAG_SECURE,
            )
        });
        if req.0.is_null() {
            return Err("Không tạo được yêu cầu HTTPS".to_string());
        }

        let redirect_policy = WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;
        unsafe {
            WinHttpSetOption(
                req.0,
                WINHTTP_OPTION_REDIRECT_POLICY,
                (&redirect_policy as *const u32).cast(),
                std::mem::size_of::<u32>() as u32,
            );
        }

        let hdr_ptr = if formatted_headers.is_empty() {
            ptr::null()
        } else {
            headers_w.as_ptr()
        };
        let hdr_len = if formatted_headers.is_empty() {
            0
        } else {
            (headers_w.len().saturating_sub(1)) as u32
        };
        if unsafe { WinHttpSendRequest(req.0, hdr_ptr, hdr_len, ptr::null(), 0, 0, 0) } == 0 {
            return Err("Không gửi được yêu cầu HTTPS".to_string());
        }
        if unsafe { WinHttpReceiveResponse(req.0, ptr::null_mut()) } == 0 {
            return Err("Không nhận được phản hồi từ máy chủ".to_string());
        }

        let mut status_code: u32 = 0;
        let mut size: u32 = std::mem::size_of::<u32>() as u32;
        if unsafe {
            WinHttpQueryHeaders(
                req.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                ptr::null(),
                (&mut status_code as *mut u32).cast(),
                &mut size,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err("Không đọc được mã trạng thái HTTP".to_string());
        }

        if matches!(status_code, 301 | 302 | 303 | 307 | 308) {
            let Some(next_loc) = query_redirect_location(req.0) else {
                return Err(format!("Chuyển hướng HTTP {status_code} thiếu Location"));
            };
            if !next_loc.starts_with("https://") {
                return Err("Từ chối chuyển hướng sang kết nối không phải HTTPS".to_string());
            }
            current_url = next_loc;
            continue;
        }

        if status_code != 200 {
            return Err(format!("Máy chủ trả về mã HTTP {}", status_code));
        }

        let mut total: u64 = 0;
        let mut chunk = vec![0u8; 64 * 1024];
        loop {
            let mut avail: u32 = 0;
            if unsafe { WinHttpQueryDataAvailable(req.0, &mut avail) } == 0 {
                return Err("Lỗi đọc dữ liệu phản hồi".to_string());
            }
            if avail == 0 {
                break;
            }
            let to_read = (avail as usize).min(chunk.len()) as u32;
            let mut read: u32 = 0;
            if unsafe { WinHttpReadData(req.0, chunk.as_mut_ptr().cast(), to_read, &mut read) } == 0
            {
                return Err("Không đọc được nội dung phản hồi".to_string());
            }
            if read == 0 {
                break;
            }
            total = total.saturating_add(read as u64);
            if total > max_bytes {
                return Err("Kích thước tệp tải về vượt quá giới hạn cho phép".to_string());
            }
            dest.write_all(&chunk[..read as usize])
                .map_err(|e| format!("Không thể ghi dữ liệu tải về: {e}"))?;
        }
        return Ok(());
    }

    Err("Quá nhiều lần chuyển hướng HTTP".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_url_parsing_and_ps_encoding() {
        assert_eq!(
            parse_https_url("https://api.github.com/repos/a/b?x=1"),
            Some(("api.github.com".to_string(), 443, "/repos/a/b?x=1".to_string()))
        );
        assert_eq!(
            parse_https_url("https://example.com:8443/file.zip"),
            Some(("example.com".to_string(), 8443, "/file.zip".to_string()))
        );
        assert!(parse_https_url("http://insecure.example.com/a").is_none());
        assert_eq!(encode_ps_utf16le_base64("hi"), "aABpAA==");
        assert_eq!(sanitize_filename("../MSKey-Setup.exe"), "MSKey-Setup.exe");
    }

    #[test]
    fn windows_updater_script_builds_for_installer_and_portable() {
        let pkg = Path::new(r"C:\Temp\MSKey-Setup-0.1.5.exe");
        let exe = Path::new(r"C:\Users\Minh Spark\AppData\Local\Programs\MS Key\MSKey.exe");
        let stg = Path::new(r"C:\Temp\mskey-update");
        let inst = build_windows_updater_script(1234, pkg, exe, stg, true).unwrap();
        assert!(inst.contains("/S /D="));
        assert!(inst.contains("MSKey.exe"));

        let zip = Path::new(r"C:\Temp\MSKey-0.1.5-portable.zip");
        let port = build_windows_updater_script(1234, zip, exe, stg, false).unwrap();
        assert!(port.contains("Expand-Archive"));
        assert!(port.contains("Copy-Item"));
    }
}
