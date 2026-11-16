use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use mskey_core::update::{
    GITHUB_LATEST_RELEASE_API, ReleaseAsset, ReleaseInfo, checksum_asset, expected_sha256_for,
    format_bytes_human, is_newer_version, parse_github_release_json, parse_sha256sums,
    preferred_cli_asset, preferred_linux_asset, preferred_macos_asset, preferred_windows_asset,
    sha256_file, verify_sha256_match,
};

use crate::args::match_value_opt;
use crate::help::topic_help;
use crate::io_util::{read_utf8_input, strip_bom, write_text_output};

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpdateTarget {
    App,
    Cli,
}

impl UpdateTarget {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "app" => Ok(Self::App),
            "cli" => Ok(Self::Cli),
            other => Err(format!("invalid --target '{other}' (expected: app, cli)")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Cli => "cli",
        }
    }
}

pub fn run_update(args: &[String]) -> Result<ExitCode, String> {
    let mut check_only = false;
    let mut download = false;
    let mut apply = false;
    let mut json_mode = false;
    let mut target: Option<UpdateTarget> = None;
    let mut to_dir: Option<PathBuf> = None;
    let mut manifest_path: Option<String> = None;
    let mut checksums_path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        if let Some(val) = match_value_opt(args, &mut i, "--target")? {
            target = Some(UpdateTarget::parse(val)?);
            continue;
        }
        if let Some(val) = match_value_opt(args, &mut i, "--to-dir")? {
            to_dir = Some(PathBuf::from(val));
            continue;
        }
        if let Some(val) = match_value_opt(args, &mut i, "--manifest")? {
            manifest_path = Some(val.to_string());
            continue;
        }
        if let Some(val) = match_value_opt(args, &mut i, "--checksums")? {
            checksums_path = Some(val.to_string());
            continue;
        }
        match args[i].as_str() {
            "--help" | "-h" => {
                let text = topic_help("update").unwrap_or_default();
                write_text_output(None, text)?;
                return Ok(ExitCode::SUCCESS);
            }
            "--check" => {
                check_only = true;
                i += 1;
            }
            "--download" => {
                download = true;
                i += 1;
            }
            "--apply" | "--yes" => {
                apply = true;
                i += 1;
            }
            "--json" => {
                json_mode = true;
                i += 1;
            }
            other => return Err(format!("unknown argument '{other}' for update")),
        }
    }

    if check_only && (download || apply) {
        return Err("--check cannot be combined with --download or --apply".to_string());
    }

    let resolved_target = target.unwrap_or(if apply {
        UpdateTarget::Cli
    } else {
        UpdateTarget::App
    });
    if apply && resolved_target != UpdateTarget::Cli {
        return Err("--apply is only supported with --target cli in mskey CLI (use --download for GUI installers)".to_string());
    }

    let current = env!("CARGO_PKG_VERSION");
    let info = fetch_release_info(manifest_path.as_deref())?;
    let newer = is_newer_version(current, &info.version);
    let selected = select_target_asset(&info.assets, resolved_target).cloned();
    let app_asset = select_target_asset(&info.assets, UpdateTarget::App).cloned();
    let cli_asset = select_target_asset(&info.assets, UpdateTarget::Cli).cloned();

    if !download && !apply {
        let out = format_check_output(
            current,
            &info,
            newer,
            resolved_target,
            app_asset.as_ref(),
            cli_asset.as_ref(),
            json_mode,
        )?;
        write_text_output(None, &out)?;
        return Ok(ExitCode::SUCCESS);
    }

    let asset = selected.ok_or_else(|| {
        format!(
            "no matching {} release asset found for {}/{}",
            resolved_target.as_str(),
            env::consts::OS,
            env::consts::ARCH
        )
    })?;

    let allow_file = manifest_path.is_some() || checksums_path.is_some();
    let sums_text = load_checksums_text(&info.assets, checksums_path.as_deref(), allow_file)?;
    let sums = parse_sha256sums(&sums_text);
    if expected_sha256_for(&sums, &asset.name).is_none() {
        return Err(format!("missing SHA-256 entry for '{}' in checksums", asset.name));
    }

    let dest_dir = to_dir.unwrap_or_else(|| {
        if apply {
            env::temp_dir().join(format!("mskey-cli-update-{}", std::process::id()))
        } else {
            env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        }
    });
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("failed to create directory '{}': {e}", dest_dir.display()))?;

    let file_name = asset
        .name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("mskey-update.bin");
    let pkg_path = dest_dir.join(file_name);
    download_asset_url(&asset.download_url, &pkg_path, allow_file)?;

    let actual_sha = sha256_file(&pkg_path)
        .map_err(|e| format!("failed to compute SHA-256: {e}"))?;
    if !verify_sha256_match(&sums, &asset.name, &actual_sha) {
        let _ = fs::remove_file(&pkg_path);
        return Err(format!("SHA-256 verification failed for {}", asset.name));
    }

    if apply {
        let exe_path = install_cli_archive(&pkg_path, &dest_dir)?;
        let _ = fs::remove_dir_all(&dest_dir);
        if json_mode {
            let payload = serde_json::json!({
                "status": "applied",
                "current_version": current,
                "latest_version": info.version,
                "asset": asset.name,
                "sha256": actual_sha,
                "executable": exe_path.display().to_string(),
            });
            let pretty = serde_json::to_string_pretty(&payload)
                .map_err(|e| format!("failed to serialize JSON: {e}"))?;
            write_text_output(None, &format!("{pretty}\n"))?;
        } else {
            write_text_output(
                None,
                &format!(
                    "Updated CLI to v{} (SHA-256: {actual_sha})\nExecutable: {}\n",
                    info.version,
                    exe_path.display()
                ),
            )?;
        }
        return Ok(ExitCode::SUCCESS);
    }

    if json_mode {
        let payload = serde_json::json!({
            "status": "downloaded",
            "current_version": current,
            "latest_version": info.version,
            "target": resolved_target.as_str(),
            "asset": asset.name,
            "sha256": actual_sha,
            "path": pkg_path.display().to_string(),
        });
        let pretty = serde_json::to_string_pretty(&payload)
            .map_err(|e| format!("failed to serialize JSON: {e}"))?;
        write_text_output(None, &format!("{pretty}\n"))?;
    } else {
        write_text_output(
            None,
            &format!(
                "Downloaded {} ({})\nSHA-256 verified: {actual_sha}\n",
                pkg_path.display(),
                format_bytes_human(asset.size)
            ),
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

fn select_target_asset(assets: &[ReleaseAsset], target: UpdateTarget) -> Option<&ReleaseAsset> {
    match target {
        UpdateTarget::Cli => preferred_cli_asset(assets, env::consts::OS, env::consts::ARCH),
        UpdateTarget::App => match env::consts::OS {
            "windows" => preferred_windows_asset(assets, env::consts::ARCH, false),
            "macos" => preferred_macos_asset(assets, false),
            _ => preferred_linux_asset(assets, env::consts::ARCH, false),
        },
    }
}

fn curl_bin() -> &'static str {
    if cfg!(windows) { "curl.exe" } else { "curl" }
}

fn local_file_url_path(url: &str, allow_file: bool) -> Option<PathBuf> {
    if !allow_file {
        return None;
    }
    let file_path = url.strip_prefix("file://")?;
    let clean = if cfg!(windows) {
        file_path.trim_start_matches('/')
    } else {
        file_path
    };
    Some(PathBuf::from(clean))
}

fn run_curl_https(
    url: &str,
    accept: Option<&str>,
    dest: Option<&Path>,
    max_time: &str,
) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err(format!("refusing non-HTTPS URL '{url}'"));
    }
    let mut cmd = Command::new(curl_bin());
    cmd.args(["-fsSL", "--proto", "=https", "--max-time", max_time]);
    if let Some(h) = accept {
        cmd.args(["-H", h]);
    }
    cmd.args(["-H", concat!("User-Agent: mskey-cli/", env!("CARGO_PKG_VERSION"))]);
    if let Some(path) = dest {
        let s = path.to_string_lossy().to_string();
        cmd.args(["-o", &s]);
    }
    cmd.arg(url);
    let out = cmd
        .output()
        .map_err(|e| format!("failed to execute {}: {e}", curl_bin()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("HTTPS request failed with status {}", out.status)
        } else {
            format!("HTTPS request failed: {err}")
        });
    }
    Ok(out.stdout)
}

fn fetch_url_bytes(url: &str, accept: Option<&str>, allow_file: bool) -> Result<Vec<u8>, String> {
    if let Some(path) = local_file_url_path(url, allow_file) {
        return fs::read(&path)
            .map_err(|e| format!("failed to read local URL '{}': {e}", path.display()));
    }
    run_curl_https(url, accept, None, "20")
}

fn download_asset_url(url: &str, dest: &Path, allow_file: bool) -> Result<(), String> {
    if let Some(path) = local_file_url_path(url, allow_file) {
        fs::copy(&path, dest)
            .map_err(|e| format!("failed to copy '{}': {e}", path.display()))?;
        return Ok(());
    }
    let _ = run_curl_https(url, None, Some(dest), "180")?;
    Ok(())
}

fn fetch_release_info(manifest_path: Option<&str>) -> Result<ReleaseInfo, String> {
    let raw = if let Some(path) = manifest_path {
        read_utf8_input(Some(path))?
    } else {
        let bytes = fetch_url_bytes(
            GITHUB_LATEST_RELEASE_API,
            Some("Accept: application/vnd.github+json"),
            false,
        )?;
        String::from_utf8(bytes).map_err(|_| "release manifest is not valid UTF-8".to_string())?
    };
    parse_github_release_json(strip_bom(&raw))
        .ok_or_else(|| "failed to parse GitHub release JSON".to_string())
}

fn load_checksums_text(
    assets: &[ReleaseAsset],
    checksums_path: Option<&str>,
    allow_file: bool,
) -> Result<String, String> {
    if let Some(path) = checksums_path {
        let raw = read_utf8_input(Some(path))?;
        return Ok(strip_bom(&raw).to_string());
    }
    let asset = checksum_asset(assets)
        .ok_or_else(|| "release does not include a SHA256SUMS asset".to_string())?;
    let bytes = fetch_url_bytes(&asset.download_url, None, allow_file)?;
    String::from_utf8(bytes).map_err(|_| "checksum file is not valid UTF-8".to_string())
}

fn install_cli_archive(pkg_path: &Path, work_dir: &Path) -> Result<PathBuf, String> {
    let unpack_dir = work_dir.join("unpacked");
    fs::create_dir_all(&unpack_dir)
        .map_err(|e| format!("failed to create unpack directory: {e}"))?;

    let lower = pkg_path.to_string_lossy().to_ascii_lowercase();
    if lower.ends_with(".zip") && cfg!(windows) {
        let ps = format!(
            "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
            pkg_path.to_string_lossy().replace('\'', "''"),
            unpack_dir.to_string_lossy().replace('\'', "''"),
        );
        let out = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &ps])
            .output()
            .map_err(|e| format!("failed to run PowerShell Expand-Archive: {e}"))?;
        if !out.status.success() {
            return Err("failed to extract Windows zip archive".to_string());
        }
    } else {
        let out = Command::new("tar")
            .args([
                "-xzf",
                &pkg_path.to_string_lossy(),
                "-C",
                &unpack_dir.to_string_lossy(),
            ])
            .output()
            .map_err(|e| format!("failed to run tar: {e}"))?;
        if !out.status.success() {
            return Err("failed to extract archive with tar".to_string());
        }
    }

    let bin_name = if cfg!(windows) { "mskey.exe" } else { "mskey" };
    let new_bin = find_named_file(&unpack_dir, bin_name)
        .ok_or_else(|| format!("'{bin_name}' not found inside extracted archive"))?;
    let cur_exe = env::current_exe()
        .map_err(|e| format!("failed to locate current executable: {e}"))?;

    if cfg!(windows) {
        let old_exe = cur_exe.with_extension("exe.old");
        let _ = fs::remove_file(&old_exe);
        fs::rename(&cur_exe, &old_exe)
            .map_err(|e| format!("failed to stage existing executable: {e}"))?;
        if let Err(e) = fs::copy(&new_bin, &cur_exe) {
            let _ = fs::rename(&old_exe, &cur_exe);
            return Err(format!("failed to install updated executable: {e}"));
        }
        let _ = fs::remove_file(&old_exe);
    } else {
        let tmp_new = cur_exe.with_extension("new");
        fs::copy(&new_bin, &tmp_new)
            .map_err(|e| format!("failed to copy new executable: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp_new, fs::Permissions::from_mode(0o755));
        }
        if let Err(e) = fs::rename(&tmp_new, &cur_exe) {
            let _ = fs::remove_file(&tmp_new);
            return Err(format!("failed to replace executable: {e}"));
        }
    }
    Ok(cur_exe)
}

fn find_named_file(dir: &Path, filename: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(hit) = find_named_file(&path, filename) {
                return Some(hit);
            }
        } else if path.file_name().and_then(|s| s.to_str()) == Some(filename) {
            return Some(path);
        }
    }
    None
}

fn format_check_output(
    current: &str,
    info: &ReleaseInfo,
    newer: bool,
    target: UpdateTarget,
    app_asset: Option<&ReleaseAsset>,
    cli_asset: Option<&ReleaseAsset>,
    json_mode: bool,
) -> Result<String, String> {
    if json_mode {
        let report = serde_json::json!({
            "current_version": current,
            "latest_version": info.version,
            "tag_name": info.tag_name,
            "update_available": newer,
            "target": target.as_str(),
            "release_url": info.html_url,
            "app_asset": app_asset.map(|a| serde_json::json!({
                "name": a.name,
                "url": a.download_url,
                "size": a.size,
            })),
            "cli_asset": cli_asset.map(|a| serde_json::json!({
                "name": a.name,
                "url": a.download_url,
                "size": a.size,
            })),
        });
        let pretty = serde_json::to_string_pretty(&report)
            .map_err(|e| format!("failed to serialize JSON: {e}"))?;
        return Ok(format!("{pretty}\n"));
    }

    let mut out = format!(
        "Current version: {current}\nLatest version:  {}\nStatus:          {}\nRelease URL:     {}\n",
        info.version,
        if newer { "update available" } else { "up to date" },
        info.html_url,
    );
    if let Some(a) = app_asset {
        out.push_str(&format!("App package:     {} ({})\n", a.name, format_bytes_human(a.size)));
    }
    if let Some(c) = cli_asset {
        out.push_str(&format!("CLI package:     {} ({})\n", c.name, format_bytes_human(c.size)));
    }
    Ok(out)
}
