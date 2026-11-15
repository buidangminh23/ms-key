use std::ffi::c_void;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use mskey_core::update::{
    GITHUB_LATEST_RELEASE_API, GITHUB_RELEASES_URL, ReleaseInfo, is_newer_version,
    parse_github_release_json,
};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER,
    WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryDataAvailable, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
    WinHttpSendRequest,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_YESNO, MessageBoxW, PostMessageW,
    SW_SHOWNORMAL,
};

use crate::platform::to_wide;
use crate::state::WM_APP_UPDATE_RESULT;

enum UpdateOutcome {
    Available(ReleaseInfo),
    UpToDate(String),
    Failed(String),
}

struct UpdateReport {
    silent: bool,
    outcome: UpdateOutcome,
}

static CHECK_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
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

pub fn check_for_updates_async(main_hwnd: HWND, silent: bool) {
    if main_hwnd.is_null() {
        return;
    }
    if CHECK_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let hwnd_raw = main_hwnd as usize;
    let spawn_res = thread::Builder::new()
        .name("mskey-update".to_string())
        .spawn(move || {
            let outcome = run_update_check();
            if let Ok(mut slot) = PENDING_REPORT.lock() {
                *slot = Some(UpdateReport { silent, outcome });
            }
            CHECK_IN_FLIGHT.store(false, Ordering::Release);
            unsafe {
                PostMessageW(hwnd_raw as HWND, WM_APP_UPDATE_RESULT, 0, 0);
            }
        });
    if spawn_res.is_err() {
        CHECK_IN_FLIGHT.store(false, Ordering::Release);
    }
}

pub fn handle_update_result_message(owner: HWND) {
    let report = PENDING_REPORT.lock().ok().and_then(|mut s| s.take());
    let Some(report) = report else {
        return;
    };
    let current = env!("CARGO_PKG_VERSION");
    match report.outcome {
        UpdateOutcome::Available(info) => {
            let notes: String = info.body.trim().chars().take(360).collect();
            let mut body = format!(
                "Đã có phiên bản mới: v{} (hiện tại: v{}).\n\nBạn có muốn mở trang tải về trên GitHub?",
                info.version, current
            );
            if !notes.is_empty() {
                body.push_str("\n\nGhi chú phát hành:\n");
                body.push_str(&notes);
            }
            let title_w = to_wide("Cập nhật MSKey");
            let body_w = to_wide(&body);
            let res = unsafe {
                MessageBoxW(
                    owner,
                    body_w.as_ptr(),
                    title_w.as_ptr(),
                    MB_YESNO | MB_ICONINFORMATION,
                )
            };
            if res == IDYES {
                let target_url = if info.html_url.trim().is_empty() {
                    GITHUB_RELEASES_URL
                } else {
                    info.html_url.trim()
                };
                open_url(target_url);
            }
        }
        UpdateOutcome::UpToDate(latest) => {
            if report.silent {
                return;
            }
            let msg = format!(
                "Bạn đang sử dụng phiên bản mới nhất (v{}).\nPhiên bản phát hành gần nhất: v{}.",
                current, latest
            );
            let title_w = to_wide("Cập nhật MSKey");
            let msg_w = to_wide(&msg);
            unsafe {
                MessageBoxW(
                    owner,
                    msg_w.as_ptr(),
                    title_w.as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
        }
        UpdateOutcome::Failed(err) => {
            if report.silent {
                return;
            }
            let msg = format!(
                "Không thể kiểm tra cập nhật:\n{}\n\nBạn có muốn mở trang GitHub Releases thủ công?",
                err
            );
            let title_w = to_wide("Cập nhật MSKey");
            let msg_w = to_wide(&msg);
            let res = unsafe {
                MessageBoxW(
                    owner,
                    msg_w.as_ptr(),
                    title_w.as_ptr(),
                    MB_YESNO | MB_ICONWARNING,
                )
            };
            if res == IDYES {
                open_url(GITHUB_RELEASES_URL);
            }
        }
    }
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

fn parse_https_url(url: &str) -> Option<(&str, String)> {
    let rest = url.strip_prefix("https://")?;
    let (host, path) = rest.split_once('/')?;
    Some((host, format!("/{path}")))
}

fn run_update_check() -> UpdateOutcome {
    let json = match fetch_https_get(GITHUB_LATEST_RELEASE_API) {
        Ok(s) => s,
        Err(e) => return UpdateOutcome::Failed(e),
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

fn fetch_https_get(url: &str) -> Result<String, String> {
    let Some((host, object_path)) = parse_https_url(url) else {
        return Err("URL cập nhật không hợp lệ".to_string());
    };
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

    let host_w = to_wide(host);
    let conn = HttpHandle(unsafe {
        WinHttpConnect(session.0, host_w.as_ptr(), INTERNET_DEFAULT_HTTPS_PORT, 0)
    });
    if conn.0.is_null() {
        return Err("Không kết nối được tới GitHub API".to_string());
    }

    let verb_w = to_wide("GET");
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

    let headers = to_wide("Accept: application/vnd.github+json\r\n");
    let headers_len = (headers.len().saturating_sub(1)) as u32;
    if unsafe {
        WinHttpSendRequest(
            req.0,
            headers.as_ptr(),
            headers_len,
            ptr::null(),
            0,
            0,
            0,
        )
    } == 0
    {
        return Err("Không gửi được yêu cầu kiểm tra cập nhật".to_string());
    }

    if unsafe { WinHttpReceiveResponse(req.0, ptr::null_mut()) } == 0 {
        return Err("Không nhận được phản hồi từ GitHub".to_string());
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
    if status_code != 200 {
        return Err(format!("GitHub API trả về mã HTTP {}", status_code));
    }

    let mut body = Vec::<u8>::new();
    loop {
        let mut avail: u32 = 0;
        if unsafe { WinHttpQueryDataAvailable(req.0, &mut avail) } == 0 {
            return Err("Lỗi đọc dữ liệu phản hồi".to_string());
        }
        if avail == 0 {
            break;
        }
        let to_read = avail.min(64 * 1024);
        let mut chunk = vec![0u8; to_read as usize];
        let mut read: u32 = 0;
        if unsafe { WinHttpReadData(req.0, chunk.as_mut_ptr().cast(), to_read, &mut read) } == 0 {
            return Err("Không đọc được nội dung phản hồi".to_string());
        }
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read as usize]);
        if body.len() > 2 * 1024 * 1024 {
            break;
        }
    }

    String::from_utf8(body).map_err(|_| "Phản hồi không phải UTF-8 hợp lệ".to_string())
}
