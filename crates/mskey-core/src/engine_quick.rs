use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::syllable::{is_valid_typing_prefix, parse_syllable};
use crate::tables::{BaseVowel, decode_vowel, encode_vowel};
use crate::transform::reanchor_tone;

pub(crate) fn try_apply_quick_start(
    display: &mut Vec<char>,
    raw_len: usize,
    starts: &mut Vec<usize>,
    ch: char,
) -> bool {
    if !display.is_empty() {
        return false;
    }
    let (c0, c1) = match ch {
        'f' => ('p', 'h'),
        'F' => ('P', 'h'),
        'j' => ('g', 'i'),
        'J' => ('G', 'i'),
        'w' => ('q', 'u'),
        'W' => ('Q', 'u'),
        _ => return false,
    };
    let start = raw_len.saturating_sub(1);
    display.push(c0);
    display.push(c1);
    starts.push(start);
    starts.push(raw_len);
    true
}

pub(crate) fn try_apply_quick_telex(
    display: &mut Vec<char>,
    raw_len: usize,
    starts: &mut Vec<usize>,
    ch: char,
) -> bool {
    if display.len() != 1 {
        return false;
    }
    let first = display[0];
    if !first.eq_ignore_ascii_case(&ch) {
        return false;
    }
    let second = match ch.to_ascii_lowercase() {
        'c' | 'k' | 'p' | 't' => 'h',
        'g' => 'i',
        'n' => 'g',
        'q' => 'u',
        _ => return false,
    };
    display.push(if ch.is_uppercase() {
        second.to_ascii_uppercase()
    } else {
        second
    });
    starts.push(raw_len.saturating_sub(1));
    true
}

pub(crate) fn try_apply_quick_end_consonant(
    display: &mut Vec<char>,
    raw_len: usize,
    starts: &mut Vec<usize>,
    ch: char,
    config: &Config,
) -> bool {
    let Some(&last) = display.last() else {
        return false;
    };
    if decode_vowel(last).is_none() {
        return false;
    }
    let (c0, c1) = match ch {
        'g' => ('n', 'g'),
        'G' => ('N', 'G'),
        'h' => ('n', 'h'),
        'H' => ('N', 'H'),
        'k' => ('c', 'h'),
        'K' => ('C', 'H'),
        _ => return false,
    };
    let mut candidate = display.clone();
    promote_open_u_ohorn(&mut candidate, config.allow_fjwz);
    candidate.push(c0);
    candidate.push(c1);
    let Some(parsed) = parse_syllable(&candidate, config.allow_fjwz) else {
        return false;
    };
    if !is_valid_typing_prefix(&parsed, config.allow_fjwz) {
        return false;
    }
    *display = candidate;
    let start = raw_len.saturating_sub(1);
    starts.push(start);
    starts.push(raw_len);
    reanchor_tone(display, config);
    true
}

pub(crate) fn promote_open_u_ohorn(display: &mut [char], allow_fjwz: bool) {
    let Some(parsed) = parse_syllable(display, allow_fjwz) else {
        return;
    };
    if !parsed.coda.is_empty() || parsed.vowels.len() != 2 {
        return;
    }
    let (b0, t0, u0) = parsed.vowels[0];
    let (b1, _, _) = parsed.vowels[1];
    if b0 == BaseVowel::U && b1 == BaseVowel::OHorn {
        display[parsed.vowel_start] = encode_vowel(BaseVowel::UHorn, t0, u0);
    }
}

pub(crate) struct MacroExpansion {
    pub text: String,
    pub cursor_char_idx: Option<usize>,
}

struct DateParts {
    year: i64,
    month: u32,
    day: u32,
    hour: i64,
    minute: i64,
    second: i64,
    weekday_idx: usize,
}

const WEEKDAYS_VI: [&str; 7] = [
    "Thứ Hai",
    "Thứ Ba",
    "Thứ Tư",
    "Thứ Năm",
    "Thứ Sáu",
    "Thứ Bảy",
    "Chủ Nhật",
];

fn current_date_parts(clock_offset_secs: i64) -> DateParts {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        .saturating_add(clock_offset_secs);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    let second = rem % 60;
    let (year, month, day) = civil_from_days(days);
    let weekday_idx = (days + 3).rem_euclid(7) as usize;
    DateParts {
        year,
        month,
        day,
        hour,
        minute,
        second,
        weekday_idx,
    }
}

fn format_custom_date(fmt: &str, dt: &DateParts) -> String {
    let mut out = String::with_capacity(fmt.len() + 8);
    let mut i = 0;
    while i < fmt.len() {
        let rest = &fmt[i..];
        if rest.starts_with("YYYY") {
            out.push_str(&format!("{:04}", dt.year));
            i += 4;
        } else if rest.starts_with("YY") {
            out.push_str(&format!("{:02}", dt.year.rem_euclid(100)));
            i += 2;
        } else if rest.starts_with("MM") {
            out.push_str(&format!("{:02}", dt.month));
            i += 2;
        } else if rest.starts_with('M') {
            out.push_str(&dt.month.to_string());
            i += 1;
        } else if rest.starts_with("DD") {
            out.push_str(&format!("{:02}", dt.day));
            i += 2;
        } else if rest.starts_with('D') {
            out.push_str(&dt.day.to_string());
            i += 1;
        } else if rest.starts_with("HH") {
            out.push_str(&format!("{:02}", dt.hour));
            i += 2;
        } else if rest.starts_with('H') {
            out.push_str(&dt.hour.to_string());
            i += 1;
        } else if rest.starts_with("mm") {
            out.push_str(&format!("{:02}", dt.minute));
            i += 2;
        } else if rest.starts_with('m') {
            out.push_str(&dt.minute.to_string());
            i += 1;
        } else if rest.starts_with("ss") {
            out.push_str(&format!("{:02}", dt.second));
            i += 2;
        } else if rest.starts_with('s') {
            out.push_str(&dt.second.to_string());
            i += 1;
        } else if rest.starts_with("dddd") || rest.starts_with("ddd") {
            out.push_str(WEEKDAYS_VI[dt.weekday_idx]);
            i += if rest.starts_with("dddd") { 4 } else { 3 };
        } else if let Some(ch) = rest.chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

pub(crate) fn expand_macro_variables_with_context(
    expansion: &str,
    clock_offset_secs: i64,
    clipboard: Option<&str>,
) -> MacroExpansion {
    if !expansion.contains('{') && !expansion.contains("$|") {
        return MacroExpansion {
            text: expansion.to_string(),
            cursor_char_idx: None,
        };
    }
    let dt = current_date_parts(clock_offset_secs);
    let mut out = String::with_capacity(expansion.len() + 16);
    let mut cursor_char_idx = None;
    let mut i = 0;
    while i < expansion.len() {
        let rest = &expansion[i..];
        if rest.starts_with("$|") {
            if cursor_char_idx.is_none() {
                cursor_char_idx = Some(out.chars().count());
            }
            i += 2;
        } else if rest.starts_with("{cursor}") {
            if cursor_char_idx.is_none() {
                cursor_char_idx = Some(out.chars().count());
            }
            i += "{cursor}".len();
        } else if rest.starts_with("{clipboard}") {
            if let Some(cb) = clipboard {
                out.push_str(cb);
            }
            i += "{clipboard}".len();
        } else if rest.starts_with("{datetime}") {
            out.push_str(&format!(
                "{:02}/{:02}/{:04} {:02}:{:02}",
                dt.day, dt.month, dt.year, dt.hour, dt.minute
            ));
            i += "{datetime}".len();
        } else if rest.starts_with("{date}") {
            out.push_str(&format!("{:02}/{:02}/{:04}", dt.day, dt.month, dt.year));
            i += "{date}".len();
        } else if rest.starts_with("{time}") {
            out.push_str(&format!("{:02}:{:02}", dt.hour, dt.minute));
            i += "{time}".len();
        } else if rest.starts_with("{year}") {
            out.push_str(&format!("{:04}", dt.year));
            i += "{year}".len();
        } else if rest.starts_with("{month}") {
            out.push_str(&format!("{:02}", dt.month));
            i += "{month}".len();
        } else if rest.starts_with("{day}") {
            out.push_str(&format!("{:02}", dt.day));
            i += "{day}".len();
        } else if rest.starts_with("{weekday}") {
            out.push_str(WEEKDAYS_VI[dt.weekday_idx]);
            i += "{weekday}".len();
        } else if let Some(after_prefix) = rest.strip_prefix("{date:")
            && let Some(end) = after_prefix.find('}')
        {
            out.push_str(&format_custom_date(&after_prefix[..end], &dt));
            i += "{date:".len() + end + 1;
        } else if let Some(ch) = rest.chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    MacroExpansion {
        text: out,
        cursor_char_idx,
    }
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32)
}
