use std::cmp::Ordering;
use std::collections::BTreeMap;

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
        assert!(!verify_sha256_match(&sums, "mskey-win-x64.zip", "deadbeef"));
    }
}
