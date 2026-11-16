use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const GITHUB_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/buidangminh23/ms-key/releases/latest";
pub const GITHUB_RELEASES_URL: &str = "https://github.com/buidangminh23/ms-key/releases";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub version: String,
    pub html_url: String,
    pub body: String,
    pub assets: Vec<ReleaseAsset>,
}

pub fn normalize_version_str(version: &str) -> String {
    let trimmed = version.trim();
    let stripped = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    stripped.to_string()
}

fn parse_semver_parts(version: &str) -> Option<(u64, u64, u64, Option<String>)> {
    let clean = normalize_version_str(version);
    if clean.is_empty() {
        return None;
    }
    let without_build = clean.split('+').next().unwrap_or(&clean);
    let (core, pre) = match without_build.split_once('-') {
        Some((c, p)) => (c, Some(p.to_string())),
        None => (without_build, None),
    };
    let mut nums = core.split('.');
    let major = nums.next()?.parse::<u64>().ok()?;
    let minor = nums.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = nums.next().unwrap_or("0").parse::<u64>().ok()?;
    if nums.next().is_some() {
        return None;
    }
    Some((major, minor, patch, pre))
}

fn compare_prerelease(a: &str, b: &str) -> Ordering {
    let mut a_parts = a.split('.');
    let mut b_parts = b.split('.');
    loop {
        match (a_parts.next(), b_parts.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(pa), Some(pb)) => {
                let ord = match (pa.parse::<u64>(), pb.parse::<u64>()) {
                    (Ok(na), Ok(nb)) => na.cmp(&nb),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => pa.cmp(pb),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    let Some((cmaj, cmin, cpat, cpre)) = parse_semver_parts(current) else {
        return false;
    };
    let Some((nmaj, nmin, npat, npre)) = parse_semver_parts(candidate) else {
        return false;
    };
    match (nmaj, nmin, npat).cmp(&(cmaj, cmin, cpat)) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => match (cpre.as_deref(), npre.as_deref()) {
            (Some(_), None) => true,
            (None, _) => false,
            (Some(ca), Some(na)) => compare_prerelease(na, ca) == Ordering::Greater,
        },
    }
}

pub fn parse_github_release_json(json: &str) -> Option<ReleaseInfo> {
    let value: Value = serde_json::from_str(json).ok()?;
    if value.get("draft").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let tag_name = value.get("tag_name").and_then(Value::as_str)?.trim();
    if tag_name.is_empty() {
        return None;
    }
    let version = normalize_version_str(tag_name);
    let html_url = value
        .get("html_url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let body = value
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut assets = Vec::new();
    if let Some(arr) = value.get("assets").and_then(Value::as_array) {
        for item in arr {
            let Some(name) = item.get("name").and_then(Value::as_str) else {
                continue;
            };
            let Some(download_url) = item.get("browser_download_url").and_then(Value::as_str) else {
                continue;
            };
            let size = item.get("size").and_then(Value::as_u64).unwrap_or(0);
            assets.push(ReleaseAsset {
                name: name.to_string(),
                download_url: download_url.to_string(),
                size,
            });
        }
    }
    Some(ReleaseInfo {
        tag_name: tag_name.to_string(),
        version,
        html_url,
        body,
        assets,
    })
}

pub fn parse_sha256sums(content: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(rest) = parts.next() else {
            continue;
        };
        let hash = hash.trim();
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let file = rest
            .trim()
            .trim_start_matches('*')
            .trim_start_matches("./")
            .to_string();
        if !file.is_empty() {
            out.insert(file, hash.to_ascii_lowercase());
        }
    }
    out
}

pub fn verify_sha256_match(
    sums: &BTreeMap<String, String>,
    filename: &str,
    actual_hex: &str,
) -> bool {
    let normalized_actual = actual_hex.trim().to_ascii_lowercase();
    if normalized_actual.len() != 64 {
        return false;
    }
    let clean_name = filename.trim().trim_start_matches("./");
    if let Some(expected) = sums.get(clean_name) {
        return expected == &normalized_actual;
    }
    let base = clean_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(clean_name);
    sums.iter().any(|(k, v)| {
        let kb = k.rsplit(['/', '\\']).next().unwrap_or(k.as_str());
        kb == base && v == &normalized_actual
    })
}

pub fn expected_sha256_for<'a>(sums: &'a BTreeMap<String, String>, filename: &str) -> Option<&'a str> {
    let clean = filename.trim().trim_start_matches("./");
    if let Some(v) = sums.get(clean) {
        return Some(v.as_str());
    }
    let base = clean.rsplit(['/', '\\']).next().unwrap_or(clean);
    sums.iter().find_map(|(k, v)| {
        let kb = k.rsplit(['/', '\\']).next().unwrap_or(k.as_str());
        if kb == base { Some(v.as_str()) } else { None }
    })
}

pub fn checksum_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS"))
        .or_else(|| {
            assets
                .iter()
                .find(|a| a.name.to_ascii_lowercase().ends_with(".sha256"))
        })
}

fn norm_arch(arch: &str) -> (&'static str, &'static str) {
    match arch.to_ascii_lowercase().as_str() {
        "aarch64" | "arm64" => ("arm64", "arm64"),
        _ => ("x64", "amd64"),
    }
}

fn is_cli_asset(name_lower: &str) -> bool {
    name_lower.starts_with("mskey-cli-") || name_lower.contains("-cli-")
}

pub fn preferred_windows_asset<'a>(
    assets: &'a [ReleaseAsset],
    arch: &str,
    prefer_installer: bool,
) -> Option<&'a ReleaseAsset> {
    let (win_arch, _) = norm_arch(arch);
    let matches_win = |name: &str| {
        let low = name.to_ascii_lowercase();
        !is_cli_asset(&low) && (low.contains("windows") || low.contains("-win-"))
    };
    let arch_matches = |name: &str| {
        let low = name.to_ascii_lowercase();
        low.contains(win_arch)
    };
    if prefer_installer {
        if let Some(hit) = assets.iter().find(|a| {
            let low = a.name.to_ascii_lowercase();
            matches_win(&a.name) && low.ends_with(".exe") && arch_matches(&a.name)
        }) {
            return Some(hit);
        }
        if win_arch == "arm64"
            && let Some(hit) = assets.iter().find(|a| {
                let low = a.name.to_ascii_lowercase();
                matches_win(&a.name) && low.ends_with(".zip") && arch_matches(&a.name)
            })
        {
            return Some(hit);
        }
    }
    assets
        .iter()
        .find(|a| {
            let low = a.name.to_ascii_lowercase();
            matches_win(&a.name) && low.ends_with(".zip") && arch_matches(&a.name)
        })
        .or_else(|| {
            assets.iter().find(|a| {
                let low = a.name.to_ascii_lowercase();
                matches_win(&a.name) && low.ends_with(".exe") && arch_matches(&a.name)
            })
        })
        .or_else(|| {
            assets.iter().find(|a| {
                let low = a.name.to_ascii_lowercase();
                matches_win(&a.name) && (low.ends_with(".zip") || low.ends_with(".exe"))
            })
        })
}

pub fn preferred_linux_asset<'a>(
    assets: &'a [ReleaseAsset],
    arch: &str,
    prefer_deb: bool,
) -> Option<&'a ReleaseAsset> {
    let (_, lin_arch) = norm_arch(arch);
    let is_deb = |name: &str| {
        let low = name.to_ascii_lowercase();
        !is_cli_asset(&low) && low.ends_with(".deb")
    };
    let is_tar = |name: &str| {
        let low = name.to_ascii_lowercase();
        !is_cli_asset(&low) && low.contains("linux") && low.ends_with(".tar.gz")
    };
    let arch_ok = |name: &str| name.to_ascii_lowercase().contains(lin_arch);
    if prefer_deb
        && let Some(hit) = assets.iter().find(|a| is_deb(&a.name) && arch_ok(&a.name))
    {
        return Some(hit);
    }
    assets
        .iter()
        .find(|a| is_tar(&a.name) && arch_ok(&a.name))
        .or_else(|| assets.iter().find(|a| is_deb(&a.name) && arch_ok(&a.name)))
        .or_else(|| assets.iter().find(|a| is_tar(&a.name)))
}

pub fn preferred_macos_asset(assets: &[ReleaseAsset], prefer_zip: bool) -> Option<&ReleaseAsset> {
    let is_mac = |name: &str| {
        let low = name.to_ascii_lowercase();
        !is_cli_asset(&low) && low.contains("macos")
    };
    let exts: &[&str] = if prefer_zip {
        &[".zip", ".dmg", ".pkg"]
    } else {
        &[".dmg", ".pkg", ".zip"]
    };
    for ext in exts {
        if let Some(hit) = assets
            .iter()
            .find(|a| is_mac(&a.name) && a.name.to_ascii_lowercase().ends_with(ext))
        {
            return Some(hit);
        }
    }
    None
}

pub fn preferred_cli_asset<'a>(
    assets: &'a [ReleaseAsset],
    os: &str,
    arch: &str,
) -> Option<&'a ReleaseAsset> {
    let (win_arch, lin_arch) = norm_arch(arch);
    let os_low = os.to_ascii_lowercase();
    let (os_key, arch_key, ext) = match os_low.as_str() {
        "windows" => ("windows", win_arch, ".zip"),
        "macos" | "darwin" => ("macos", "universal", ".tar.gz"),
        _ => ("linux", lin_arch, ".tar.gz"),
    };
    assets.iter().find(|a| {
        let low = a.name.to_ascii_lowercase();
        is_cli_asset(&low) && low.contains(os_key) && low.contains(arch_key) && low.ends_with(ext)
    })
}

pub fn format_bytes_human(bytes: u64) -> String {
    if bytes == 0 {
        return String::new();
    }
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", (bytes + 512) / 1024)
    } else {
        format!("{bytes} B")
    }
}

pub fn release_notes_preview(body: &str, max_chars: usize) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() || max_chars == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (count, ch) in trimmed.chars().enumerate() {
        if count >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

const fn next_prime(mut n: u64) -> u64 {
    loop {
        if n <= 2 {
            return 2;
        }
        let mut d = 2u64;
        let mut ok = true;
        while d * d <= n {
            if n.is_multiple_of(d) {
                ok = false;
                break;
            }
            d += 1;
        }
        if ok {
            return n;
        }
        n += 1;
    }
}

const fn isqrt_u128(n: u128) -> u128 {
    let mut lo = 0u128;
    let mut hi = 1u128 << 36;
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if mid * mid <= n {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

const fn icbrt_u128(n: u128) -> u128 {
    let mut lo = 0u128;
    let mut hi = 1u128 << 36;
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if mid * mid * mid <= n {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

const fn build_sha256_init() -> [u32; 8] {
    let mut out = [0u32; 8];
    let mut p = 2u64;
    let mut idx = 0usize;
    while idx < 8 {
        p = next_prime(p);
        out[idx] = isqrt_u128((p as u128) << 64) as u32;
        p += 1;
        idx += 1;
    }
    out
}

const fn build_sha256_round_k() -> [u32; 64] {
    let mut out = [0u32; 64];
    let mut p = 2u64;
    let mut idx = 0usize;
    while idx < 64 {
        p = next_prime(p);
        out[idx] = icbrt_u128((p as u128) << 96) as u32;
        p += 1;
        idx += 1;
    }
    out
}

const SHA256_IV: [u32; 8] = build_sha256_init();
const SHA256_RC: [u32; 64] = build_sha256_round_k();

pub struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    block_len: usize,
    total_bits: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: SHA256_IV,
            block: [0u8; 64],
            block_len: 0,
            total_bits: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.total_bits = self.total_bits.wrapping_add((data.len() as u64).wrapping_mul(8));
        let mut offset = 0usize;
        if self.block_len > 0 {
            let take = (64 - self.block_len).min(data.len());
            self.block[self.block_len..self.block_len + take].copy_from_slice(&data[..take]);
            self.block_len += take;
            offset += take;
            if self.block_len == 64 {
                let blk = self.block;
                self.compress(&blk);
                self.block_len = 0;
            }
        }
        while offset + 64 <= data.len() {
            let mut blk = [0u8; 64];
            blk.copy_from_slice(&data[offset..offset + 64]);
            self.compress(&blk);
            offset += 64;
        }
        if offset < data.len() {
            let rem = data.len() - offset;
            self.block[..rem].copy_from_slice(&data[offset..]);
            self.block_len = rem;
        }
    }

    pub fn finalize_hex(mut self) -> String {
        let len = self.block_len;
        self.block[len] = 0x80;
        let mut pos = len + 1;
        if pos > 56 {
            self.block[pos..64].fill(0);
            let blk = self.block;
            self.compress(&blk);
            pos = 0;
        }
        self.block[pos..56].fill(0);
        self.block[56..64].copy_from_slice(&self.total_bits.to_be_bytes());
        let blk = self.block;
        self.compress(&blk);

        let mut hex = String::with_capacity(64);
        for word in self.state {
            for b in word.to_be_bytes() {
                const DIGITS: &[u8; 16] = b"0123456789abcdef";
                hex.push(DIGITS[(b >> 4) as usize] as char);
                hex.push(DIGITS[(b & 0x0f) as usize] as char);
            }
        }
        hex
    }

    fn compress(&mut self, blk: &[u8; 64]) {
        let mut sched = [0u32; 64];
        for (item, &chunk) in sched[..16].iter_mut().zip(blk.as_chunks::<4>().0) {
            *item = u32::from_be_bytes(chunk);
        }
        for i in 16..64 {
            let s0 = sched[i - 15].rotate_right(7) ^ sched[i - 15].rotate_right(18) ^ (sched[i - 15] >> 3);
            let s1 = sched[i - 2].rotate_right(17) ^ sched[i - 2].rotate_right(19) ^ (sched[i - 2] >> 10);
            sched[i] = sched[i - 16]
                .wrapping_add(s0)
                .wrapping_add(sched[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_RC[i])
                .wrapping_add(sched[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        let delta = [a, b, c, d, e, f, g, h];
        for i in 0..8 {
            self.state[i] = self.state[i].wrapping_add(delta[i]);
        }
    }
}

pub fn sha256_bytes(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize_hex()
}

pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_comparison_works() {
        assert!(is_newer_version("0.1.2", "v0.1.3"));
        assert!(is_newer_version("0.1.2", "0.2.0"));
        assert!(is_newer_version("0.2.0-rc.1", "0.2.0"));
        assert!(!is_newer_version("0.2.0", "0.2.0-rc.2"));
        assert!(!is_newer_version("0.1.2", "0.1.2"));
        assert!(!is_newer_version("0.2.0", "0.1.9"));
        assert!(is_newer_version("0.2.0-rc.1", "0.2.0-rc.2"));
    }

    #[test]
    fn github_release_json_and_sha256sums_parse() {
        let json = r#"{
            "tag_name": "v0.2.0",
            "draft": false,
            "html_url": "https://github.com/buidangminh23/mskey/releases/tag/v0.2.0",
            "body": "Release notes",
            "assets": [
                {
                    "name": "mskey-win-x64.zip",
                    "browser_download_url": "https://example.com/mskey-win-x64.zip",
                    "size": 123456
                }
            ]
        }"#;
        let info = parse_github_release_json(json).unwrap();
        assert_eq!(info.version, "0.2.0");
        assert_eq!(info.assets.len(), 1);
        assert_eq!(info.assets[0].name, "mskey-win-x64.zip");

        let sums_txt = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  ./mskey-win-x64.zip\n";
        let sums = parse_sha256sums(sums_txt);
        assert!(verify_sha256_match(
            &sums,
            "mskey-win-x64.zip",
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF"
        ));
        assert_eq!(
            expected_sha256_for(&sums, "mskey-win-x64.zip"),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        );
        assert!(!verify_sha256_match(&sums, "mskey-win-x64.zip", "deadbeef"));
    }

    #[test]
    fn sha256_matches_standard_vectors_and_files() {
        assert_eq!(
            sha256_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_bytes(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let dir = std::env::temp_dir();
        let path = dir.join(format!("mskey_sha_test_{}.bin", std::process::id()));
        std::fs::write(&path, b"mskey-update-payload-0123456789").unwrap();
        let file_hex = sha256_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(file_hex, sha256_bytes(b"mskey-update-payload-0123456789"));
    }

    #[test]
    fn release_asset_selectors_pick_platform_packages() {
        let mk = |name: &str| ReleaseAsset {
            name: name.to_string(),
            download_url: format!("https://example.com/{name}"),
            size: 1024,
        };
        let assets = vec![
            mk("SHA256SUMS"),
            mk("MSKey-Setup-0.1.5-windows-x64.exe"),
            mk("MSKey-0.1.5-windows-x64-portable.zip"),
            mk("MSKey-0.1.5-windows-arm64-portable.zip"),
            mk("mskey-cli-0.1.5-windows-x64.zip"),
            mk("mskey_0.1.5_amd64.deb"),
            mk("MSKey-0.1.5-linux-amd64.tar.gz"),
            mk("mskey-cli-0.1.5-linux-amd64.tar.gz"),
            mk("MSKey-0.1.5-macos-universal.dmg"),
            mk("MSKey-0.1.5-macos-universal.zip"),
            mk("mskey-cli-0.1.5-macos-universal.tar.gz"),
        ];

        assert_eq!(checksum_asset(&assets).unwrap().name, "SHA256SUMS");
        assert_eq!(
            preferred_windows_asset(&assets, "x86_64", true).unwrap().name,
            "MSKey-Setup-0.1.5-windows-x64.exe"
        );
        assert_eq!(
            preferred_windows_asset(&assets, "x86_64", false).unwrap().name,
            "MSKey-0.1.5-windows-x64-portable.zip"
        );
        assert_eq!(
            preferred_windows_asset(&assets, "aarch64", true).unwrap().name,
            "MSKey-0.1.5-windows-arm64-portable.zip"
        );
        assert_eq!(
            preferred_linux_asset(&assets, "x86_64", true).unwrap().name,
            "mskey_0.1.5_amd64.deb"
        );
        assert_eq!(
            preferred_linux_asset(&assets, "x86_64", false).unwrap().name,
            "MSKey-0.1.5-linux-amd64.tar.gz"
        );
        assert_eq!(
            preferred_macos_asset(&assets, true).unwrap().name,
            "MSKey-0.1.5-macos-universal.zip"
        );
        assert_eq!(
            preferred_macos_asset(&assets, false).unwrap().name,
            "MSKey-0.1.5-macos-universal.dmg"
        );
        assert_eq!(
            preferred_cli_asset(&assets, "windows", "x86_64").unwrap().name,
            "mskey-cli-0.1.5-windows-x64.zip"
        );
        assert_eq!(
            preferred_cli_asset(&assets, "linux", "x86_64").unwrap().name,
            "mskey-cli-0.1.5-linux-amd64.tar.gz"
        );
        assert_eq!(
            preferred_cli_asset(&assets, "macos", "aarch64").unwrap().name,
            "mskey-cli-0.1.5-macos-universal.tar.gz"
        );
        assert_eq!(format_bytes_human(2_621_440), "2.5 MB");
        assert_eq!(format_bytes_human(2048), "2 KB");
        assert_eq!(release_notes_preview("abcdef", 4), "abcd...");
    }
}
