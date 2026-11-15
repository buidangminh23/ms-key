use std::fmt::Write as _;
use std::process::ExitCode;

use mskey_core::{
    CaseTransform, Config, ConvertEncoding, MacroEntry, convert_with_encoding,
    detect_convert_encoding, dict, syllable,
};

use crate::args::{
    FromEncoding, encoding_label, match_value_opt, parse_case_transform, parse_from_encoding,
    parse_to_encoding,
};
use crate::help::topic_help;
use crate::io_util::{read_encoding_input, read_utf8_input, strip_bom, write_text_output};

struct WordCheckResult {
    word: String,
    valid: bool,
    in_dict: bool,
    valid_syllable: bool,
}

pub fn run_convert(args: &[String]) -> Result<ExitCode, String> {
    let mut from = FromEncoding::Auto;
    let mut to = ConvertEncoding::Unicode;
    let mut case_mode = CaseTransform::None;
    let mut in_path: Option<String> = None;
    let mut out_path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        if let Some(val) = match_value_opt(args, &mut i, "--from")? {
            from = parse_from_encoding(val)?;
            continue;
        }
        if let Some(val) = match_value_opt(args, &mut i, "--to")? {
            to = parse_to_encoding(val)?;
            continue;
        }
        if let Some(val) = match_value_opt(args, &mut i, "--case")? {
            case_mode = parse_case_transform(val)?;
            continue;
        }
        if let Some(val) = match_value_opt(args, &mut i, "--in")? {
            in_path = Some(val.to_string());
            continue;
        }
        if let Some(val) = match_value_opt(args, &mut i, "--out")? {
            out_path = Some(val.to_string());
            continue;
        }
        let arg = &args[i];
        if arg == "--help" || arg == "-h" {
            return print_subcommand_help("convert");
        }
        return Err(format!("unexpected argument '{arg}' for convert"));
    }

    let allow_latin1 = matches!(
        from,
        FromEncoding::Auto
            | FromEncoding::Fixed(ConvertEncoding::Tcvn3 | ConvertEncoding::VniWindows)
    );
    let input = read_encoding_input(in_path.as_deref(), allow_latin1)?;
    let source = match from {
        FromEncoding::Auto => detect_convert_encoding(&input),
        FromEncoding::Fixed(enc) => enc,
    };
    let output = convert_with_encoding(&input, source, to, case_mode);
    write_text_output(out_path.as_deref(), &output)?;
    Ok(ExitCode::SUCCESS)
}

pub fn run_detect(args: &[String]) -> Result<ExitCode, String> {
    let mut in_path: Option<String> = None;
    let mut json_mode = false;

    let mut i = 0;
    while i < args.len() {
        if let Some(val) = match_value_opt(args, &mut i, "--in")? {
            in_path = Some(val.to_string());
            continue;
        }
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => return print_subcommand_help("detect"),
            "--json" => {
                json_mode = true;
                i += 1;
            }
            _ => return Err(format!("unexpected argument '{arg}' for detect")),
        }
    }

    let input = read_encoding_input(in_path.as_deref(), true)?;
    let label = encoding_label(detect_convert_encoding(&input));
    let output = if json_mode {
        serde_json::to_string(&serde_json::json!({ "encoding": label }))
            .map_err(|err| format!("failed to format JSON: {err}"))?
    } else {
        label.to_string()
    };
    write_text_output(None, &format!("{output}\n"))?;
    Ok(ExitCode::SUCCESS)
}

pub fn run_check(args: &[String]) -> Result<ExitCode, String> {
    let mut in_path: Option<String> = None;
    let mut allow_fjwz = false;
    let mut json_mode = false;
    let mut positional_only = false;
    let mut word_args = Vec::new();

    let mut i = 0;
    while i < args.len() {
        if !positional_only {
            if let Some(val) = match_value_opt(args, &mut i, "--in")? {
                in_path = Some(val.to_string());
                continue;
            }
            let arg = &args[i];
            match arg.as_str() {
                "--help" | "-h" => return print_subcommand_help("check"),
                "--allow-fjwz" => {
                    allow_fjwz = true;
                    i += 1;
                    continue;
                }
                "--json" => {
                    json_mode = true;
                    i += 1;
                    continue;
                }
                "--" => {
                    positional_only = true;
                    i += 1;
                    continue;
                }
                _ if arg.starts_with('-') && arg != "-" => {
                    return Err(format!("unknown flag '{arg}' for check"));
                }
                _ => {}
            }
        }
        word_args.push(args[i].clone());
        i += 1;
    }

    if in_path.is_some() && !word_args.is_empty() {
        return Err("cannot specify both positional words and --in".to_string());
    }
    if in_path.is_none() && word_args.is_empty() {
        return Err("missing <WORD> or --in PATH for check".to_string());
    }

    let tokens = collect_check_tokens(in_path.as_deref(), &word_args)?;
    let results: Vec<WordCheckResult> = tokens
        .iter()
        .map(|word| check_word(word, allow_fjwz))
        .collect();
    let all_valid = results.iter().all(|r| r.valid);
    let report = format_check_report(&results, all_valid, allow_fjwz, json_mode)?;
    write_text_output(None, &report)?;
    Ok(if all_valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

pub fn run_import_evkey(args: &[String]) -> Result<ExitCode, String> {
    let mut tree_path: Option<String> = None;
    let mut out_path: Option<String> = None;
    let mut positional_only = false;

    let mut i = 0;
    while i < args.len() {
        if !positional_only {
            if let Some(val) = match_value_opt(args, &mut i, "--out")? {
                out_path = Some(val.to_string());
                continue;
            }
            let arg = &args[i];
            match arg.as_str() {
                "--help" | "-h" => return print_subcommand_help("import-evkey"),
                "--" => {
                    positional_only = true;
                    i += 1;
                    continue;
                }
                _ if arg.starts_with('-') && arg != "-" => {
                    return Err(format!("unknown flag '{arg}' for import-evkey"));
                }
                _ => {}
            }
        }
        if tree_path.is_some() {
            return Err(format!(
                "unexpected extra argument '{}' for import-evkey",
                args[i]
            ));
        }
        tree_path = Some(args[i].clone());
        i += 1;
    }

    let Some(path) = tree_path else {
        return Err("missing <TREE_PATH> for import-evkey".to_string());
    };
    let raw = read_utf8_input(Some(&path))?;
    let config = Config::import_evkey_tree(strip_bom(&raw));
    let json = config
        .to_json_pretty()
        .map_err(|err| format!("failed to serialize config: {err}"))?;
    write_text_output(out_path.as_deref(), &format!("{json}\n"))?;
    Ok(ExitCode::SUCCESS)
}

pub fn run_macros(args: &[String]) -> Result<ExitCode, String> {
    let Some(sub) = args.first() else {
        return Err("missing macros subcommand (expected: export, import)".to_string());
    };
    match sub.as_str() {
        "--help" | "-h" | "help" => print_subcommand_help("macros"),
        "export" => run_macros_export(&args[1..]),
        "import" => run_macros_import(&args[1..]),
        other => Err(format!(
            "unknown macros subcommand '{other}' (expected: export, import)"
        )),
    }
}

fn run_macros_export(args: &[String]) -> Result<ExitCode, String> {
    let mut config_path: Option<String> = None;
    let mut out_path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        if let Some(val) = match_value_opt(args, &mut i, "--config")? {
            config_path = Some(val.to_string());
            continue;
        }
        if let Some(val) = match_value_opt(args, &mut i, "--out")? {
            out_path = Some(val.to_string());
            continue;
        }
        let arg = &args[i];
        if arg == "--help" || arg == "-h" {
            return print_subcommand_help("macros");
        }
        return Err(format!("unexpected argument '{arg}' for macros export"));
    }

    let Some(cfg_path) = config_path else {
        return Err("missing --config PATH for macros export".to_string());
    };
    let config = load_config(&cfg_path)?;
    let text = config.format_macro_text();
    write_text_output(out_path.as_deref(), &text)?;
    Ok(ExitCode::SUCCESS)
}

fn run_macros_import(args: &[String]) -> Result<ExitCode, String> {
    let mut config_path: Option<String> = None;
    let mut file_path: Option<String> = None;
    let mut replace = false;

    let mut i = 0;
    while i < args.len() {
        if let Some(val) = match_value_opt(args, &mut i, "--config")? {
            config_path = Some(val.to_string());
            continue;
        }
        if let Some(val) = match_value_opt(args, &mut i, "--file")? {
            file_path = Some(val.to_string());
            continue;
        }
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => return print_subcommand_help("macros"),
            "--replace" => {
                replace = true;
                i += 1;
            }
            _ => return Err(format!("unexpected argument '{arg}' for macros import")),
        }
    }

    let Some(cfg_path) = config_path else {
        return Err("missing --config PATH for macros import".to_string());
    };
    if cfg_path == "-" {
        return Err("--config cannot be '-' for macros import".to_string());
    }
    let mut config = load_config(&cfg_path)?;
    let raw_macros = read_utf8_input(file_path.as_deref())?;
    let imported = Config::parse_macro_text(strip_bom(&raw_macros));
    let count = imported.len();
    if replace {
        config.macros = imported;
    } else {
        merge_macros(&mut config.macros, imported);
    }
    let json = config
        .to_json_pretty()
        .map_err(|err| format!("failed to serialize config: {err}"))?;
    write_text_output(Some(&cfg_path), &format!("{json}\n"))?;
    write_text_output(None, &format!("Imported {count} macro(s) into {cfg_path}\n"))?;
    Ok(ExitCode::SUCCESS)
}

fn collect_check_tokens(in_path: Option<&str>, word_args: &[String]) -> Result<Vec<String>, String> {
    if let Some(path) = in_path {
        let raw = read_utf8_input(Some(path))?;
        return Ok(strip_bom(&raw)
            .split_whitespace()
            .map(str::to_string)
            .collect());
    }
    let tokens: Vec<String> = word_args
        .iter()
        .flat_map(|item| item.split_whitespace().map(str::to_string))
        .collect();
    if tokens.is_empty() {
        return Err("no words provided for check".to_string());
    }
    Ok(tokens)
}

fn check_word(word: &str, allow_fjwz: bool) -> WordCheckResult {
    let in_dict = dict::is_vietnamese_word(word, allow_fjwz);
    let chars: Vec<char> = word.chars().collect();
    let valid_syllable = syllable::parse_syllable(&chars, allow_fjwz)
        .is_some_and(|parsed| syllable::is_valid_complete_syllable(&parsed, allow_fjwz));
    WordCheckResult {
        word: word.to_string(),
        valid: in_dict || valid_syllable,
        in_dict,
        valid_syllable,
    }
}

fn format_check_report(
    results: &[WordCheckResult],
    all_valid: bool,
    allow_fjwz: bool,
    json_mode: bool,
) -> Result<String, String> {
    if json_mode {
        let entries: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "word": r.word,
                    "valid": r.valid,
                    "in_dict": r.in_dict,
                    "valid_syllable": r.valid_syllable,
                })
            })
            .collect();
        let report = serde_json::json!({
            "valid": all_valid,
            "allow_fjwz": allow_fjwz,
            "results": entries,
        });
        let pretty = serde_json::to_string_pretty(&report)
            .map_err(|err| format!("failed to format JSON: {err}"))?;
        return Ok(format!("{pretty}\n"));
    }

    let mut out = String::new();
    for r in results {
        let status = if r.valid { "valid" } else { "invalid" };
        let _ = writeln!(
            out,
            "{}: {status} (dict={}, syllable={})",
            r.word, r.in_dict, r.valid_syllable
        );
    }
    Ok(out)
}

fn load_config(path: &str) -> Result<Config, String> {
    let raw = read_utf8_input(Some(path))?;
    Config::from_json_str(strip_bom(&raw))
        .map_err(|err| format!("failed to parse config '{path}': {err}"))
}

fn merge_macros(existing: &mut Vec<MacroEntry>, imported: Vec<MacroEntry>) {
    for entry in imported {
        if let Some(found) = existing
            .iter_mut()
            .find(|m| m.shortcut.eq_ignore_ascii_case(&entry.shortcut))
        {
            found.expansion = entry.expansion;
            found.enabled = entry.enabled;
        } else {
            existing.push(entry);
        }
    }
}

fn print_subcommand_help(topic: &str) -> Result<ExitCode, String> {
    let text = topic_help(topic).unwrap_or_default();
    write_text_output(None, text)?;
    Ok(ExitCode::SUCCESS)
}
