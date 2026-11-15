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

pub(crate) fn expand_macro_variables(expansion: &str, clock_offset_secs: i64) -> String {
    if !expansion.contains('{') {
        return expansion.to_string();
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        .saturating_add(clock_offset_secs);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    let (year, month, day) = civil_from_days(days);
    let date_str = format!("{day:02}/{month:02}/{year:04}");
    let time_str = format!("{hour:02}:{minute:02}");
    let datetime_str = format!("{date_str} {time_str}");
    expansion
        .replace("{datetime}", &datetime_str)
        .replace("{date}", &date_str)
        .replace("{time}", &time_str)
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
