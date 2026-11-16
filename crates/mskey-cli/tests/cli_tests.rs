use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use mskey_core::{AppBehavior, CaseTransform, Config, ConvertEncoding, SwitchPreset, convert_with_encoding};
use serde_json::Value;

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), id));
        fs::create_dir_all(&path).expect("failed to create temp test dir");
        Self { path }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn path_str(&self, name: &str) -> String {
        self.file(name).to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn mskey_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mskey"))
}

fn run_args(args: &[&str]) -> Output {
    mskey_cmd()
        .args(args)
        .output()
        .expect("failed to run mskey binary")
}

fn run_with_stdin(args: &[&str], stdin_bytes: &[u8]) -> Output {
    let mut child = mskey_cmd()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn mskey binary");

    {
        let mut stdin = child.stdin.take().expect("stdin should be piped");
        stdin
            .write_all(stdin_bytes)
            .expect("failed to write stdin");
    }

    child.wait_with_output().expect("failed to wait on mskey")
}

fn stdout_str(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("stdout should be valid utf-8")
}

fn stderr_str(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).expect("stderr should be valid utf-8")
}

fn assert_file_exists(path: &Path) {
    assert!(path.exists(), "expected file to exist: {}", path.display());
}

#[test]
fn help_and_version_commands() {
    for flag in ["--help", "-h", "help"] {
        let out = run_args(&[flag]);
        assert!(out.status.success(), "{flag} failed: {}", stderr_str(&out));
        let text = stdout_str(&out);
        assert!(text.contains("convert"));
        assert!(text.contains("detect"));
        assert!(text.contains("check"));
        assert!(text.contains("import-evkey"));
        assert!(text.contains("macros"));
    }

    let topic_help = run_args(&["help", "convert"]);
    assert!(topic_help.status.success());
    assert!(stdout_str(&topic_help).contains("modern-tone"));

    for flag in ["version", "--version"] {
        let out = run_args(&[flag]);
        assert!(out.status.success(), "{flag} failed: {}", stderr_str(&out));
        let text = stdout_str(&out);
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
    }

    let unknown = run_args(&["unknown-subcommand"]);
    assert!(!unknown.status.success());
}

#[test]
fn convert_encodings_and_case_modes() {
    let sample = "Tiếng Việt Đắk";

    let to_viqr = run_with_stdin(
        &["convert", "--from", "unicode", "--to", "viqr"],
        sample.as_bytes(),
    );
    assert!(to_viqr.status.success(), "{}", stderr_str(&to_viqr));
    let viqr = stdout_str(&to_viqr);
    assert_eq!(viqr, "Tie^'ng Vie^.t DDa('k");

    let from_viqr_auto = run_with_stdin(&["convert", "--from", "auto", "--to", "unicode"], viqr.as_bytes());
    assert!(from_viqr_auto.status.success());
    assert_eq!(stdout_str(&from_viqr_auto), sample);

    let to_tcvn3 = run_with_stdin(
        &["convert", "--from", "unicode", "--to", "tcvn3"],
        sample.as_bytes(),
    );
    assert!(to_tcvn3.status.success());
    let tcvn3 = stdout_str(&to_tcvn3);
    assert_ne!(tcvn3, sample);

    let back_tcvn3 = run_with_stdin(&["convert", "--from", "tcvn3", "--to", "unicode"], tcvn3.as_bytes());
    assert!(back_tcvn3.status.success());
    assert_eq!(stdout_str(&back_tcvn3), sample);

    let dir = TempDir::new("mskey_cli_convert");
    let raw_tcvn_path = dir.file("raw_tcvn3.txt");
    let unicode_out_path = dir.file("unicode_out.txt");
    fs::write(&raw_tcvn_path, b"Ti\xD5ng Vi\xD6t").expect("write raw tcvn3");

    let file_conv = run_args(&[
        "convert",
        "--from",
        "auto",
        "--to",
        "unicode",
        "--in",
        &dir.path_str("raw_tcvn3.txt"),
        "--out",
        &dir.path_str("unicode_out.txt"),
    ]);
    assert!(file_conv.status.success(), "{}", stderr_str(&file_conv));
    assert_file_exists(&unicode_out_path);
    assert_eq!(
        fs::read_to_string(&unicode_out_path).expect("read out"),
        "Tiếng Việt"
    );

    let to_vni = run_with_stdin(&["convert", "--from=unicode", "--to=vni"], sample.as_bytes());
    assert!(to_vni.status.success());
    let back_vni = run_with_stdin(&["convert", "--from=vni", "--to=unicode"], &to_vni.stdout);
    assert!(back_vni.status.success());
    assert_eq!(stdout_str(&back_vni), sample);

    let to_nfd = run_with_stdin(&["convert", "--from", "unicode", "--to", "nfd"], sample.as_bytes());
    assert!(to_nfd.status.success());
    let back_nfd = run_with_stdin(&["convert", "--from", "nfd", "--to", "unicode"], &to_nfd.stdout);
    assert!(back_nfd.status.success());
    assert_eq!(stdout_str(&back_nfd), sample);

    let modern = run_with_stdin(
        &["convert", "--from", "unicode", "--to", "unicode", "--case", "modern-tone"],
        "hòa khóa thúy English".as_bytes(),
    );
    assert!(modern.status.success());
    assert_eq!(stdout_str(&modern), "hoà khoá thuý English");

    let classic = run_with_stdin(
        &["convert", "--from", "unicode", "--to", "unicode", "--case", "classic-tone"],
        "hoà khoá thuý English".as_bytes(),
    );
    assert!(classic.status.success());
    assert_eq!(stdout_str(&classic), "hòa khóa thúy English");

    let upper = run_with_stdin(&["convert", "--case", "upper"], "tiếng việt".as_bytes());
    assert!(upper.status.success());
    assert_eq!(stdout_str(&upper), "TIẾNG VIỆT");

    let lower = run_with_stdin(&["convert", "--case", "lower"], "TIẾNG VIỆT".as_bytes());
    assert!(lower.status.success());
    assert_eq!(stdout_str(&lower), "tiếng việt");

    let sentence = run_with_stdin(&["convert", "--case", "sentence"], "xin chào. tôi đi.".as_bytes());
    assert!(sentence.status.success());
    assert_eq!(stdout_str(&sentence), "Xin chào. Tôi đi.");

    let title = run_with_stdin(&["convert", "--case", "title"], "don't stop tiếng việt".as_bytes());
    assert!(title.status.success());
    assert_eq!(stdout_str(&title), "Don't Stop Tiếng Việt");

    let stripped = run_with_stdin(&["convert", "--case", "strip"], sample.as_bytes());
    assert!(stripped.status.success());
    assert_eq!(stdout_str(&stripped), "Tieng Viet Dak");
}

#[test]
fn detect_encoding_plain_and_json() {
    let sample = "Tiếng Việt";
    let tcvn3 = convert_with_encoding(sample, ConvertEncoding::Unicode, ConvertEncoding::Tcvn3, CaseTransform::None);
    let vni = convert_with_encoding(sample, ConvertEncoding::Unicode, ConvertEncoding::VniWindows, CaseTransform::None);
    let nfd = convert_with_encoding(sample, ConvertEncoding::Unicode, ConvertEncoding::UnicodeComposite, CaseTransform::None);
    let viqr = convert_with_encoding(sample, ConvertEncoding::Unicode, ConvertEncoding::Viqr, CaseTransform::None);

    let cases = [
        ("Con mèo", "unicode"),
        (tcvn3.as_str(), "tcvn3"),
        (vni.as_str(), "vni"),
        (nfd.as_str(), "nfd"),
        (viqr.as_str(), "viqr"),
    ];

    for (text, expected) in cases {
        let plain = run_with_stdin(&["detect"], text.as_bytes());
        assert!(plain.status.success(), "{}", stderr_str(&plain));
        assert_eq!(stdout_str(&plain).trim(), expected);

        let json_out = run_with_stdin(&["detect", "--json"], text.as_bytes());
        assert!(json_out.status.success(), "{}", stderr_str(&json_out));
        let parsed: Value = serde_json::from_slice(&json_out.stdout).expect("valid json");
        assert_eq!(parsed["encoding"], Value::String(expected.to_string()));
    }

    let dir = TempDir::new("mskey_cli_detect");
    let raw_tcvn_path = dir.file("legacy.tcvn3");
    fs::write(&raw_tcvn_path, b"Ti\xD5ng Vi\xD6t").expect("write legacy tcvn3");

    let file_detect = run_args(&["detect", "--in", &dir.path_str("legacy.tcvn3"), "--json"]);
    assert!(file_detect.status.success(), "{}", stderr_str(&file_detect));
    let parsed: Value = serde_json::from_slice(&file_detect.stdout).expect("valid json");
    assert_eq!(parsed["encoding"], "tcvn3");
}

#[test]
fn check_words_dict_syllable_fjwz_and_json() {
    let ok_single = run_args(&["check", "tiếng"]);
    assert!(ok_single.status.success(), "{}", stderr_str(&ok_single));
    assert!(stdout_str(&ok_single).contains("valid"));

    let ok_multi = run_args(&["check", "tiếng", "việt", "Đắk", "mưới"]);
    assert!(ok_multi.status.success(), "{}", stderr_str(&ok_multi));

    let bad = run_args(&["check", "tiếng", "hello"]);
    assert!(!bad.status.success());
    let bad_text = stdout_str(&bad);
    assert!(bad_text.contains("tiếng: valid"));
    assert!(bad_text.contains("hello: invalid"));

    let fjwz_disallowed = run_args(&["check", "fá"]);
    assert!(!fjwz_disallowed.status.success());

    let fjwz_allowed = run_args(&["check", "fá", "--allow-fjwz", "--json"]);
    assert!(fjwz_allowed.status.success(), "{}", stderr_str(&fjwz_allowed));
    let parsed_fjwz: Value = serde_json::from_slice(&fjwz_allowed.stdout).expect("valid json");
    assert_eq!(parsed_fjwz["valid"], Value::Bool(true));
    assert_eq!(parsed_fjwz["allow_fjwz"], Value::Bool(true));
    assert_eq!(parsed_fjwz["results"][0]["word"], "fá");
    assert_eq!(parsed_fjwz["results"][0]["valid"], Value::Bool(true));
    assert_eq!(parsed_fjwz["results"][0]["valid_syllable"], Value::Bool(true));

    let dir = TempDir::new("mskey_cli_check");
    let words_path = dir.file("words.txt");
    fs::write(&words_path, "con mèo\nnotviet\n").expect("write words");

    let file_check = run_args(&["check", "--in", &dir.path_str("words.txt"), "--json"]);
    assert!(!file_check.status.success());
    let parsed_file: Value = serde_json::from_slice(&file_check.stdout).expect("valid json");
    assert_eq!(parsed_file["valid"], Value::Bool(false));
    let results = parsed_file["results"].as_array().expect("results array");
    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["valid"], Value::Bool(true));
    assert_eq!(results[1]["valid"], Value::Bool(true));
    assert_eq!(results[2]["valid"], Value::Bool(false));
}

#[test]
fn import_evkey_writes_valid_config_json() {
    let dir = TempDir::new("mskey_cli_evkey");
    let tree_path = dir.file("evkey.tree");
    let out_path = dir.file("config.json");

    let tree = "setting{\nQTelex=1\nSStartConsonant=1\nSEndConsonant=1\nSKey=4\nShowOSD=1\n}\n[macro]\nddn:Đà Nẵng\\nVN\non_vn{\n*chrome*\n}\n";
    fs::write(&tree_path, tree).expect("write tree file");

    let out = run_args(&[
        "import-evkey",
        &dir.path_str("evkey.tree"),
        "--out",
        &dir.path_str("config.json"),
    ]);
    assert!(out.status.success(), "{}", stderr_str(&out));
    assert_file_exists(&out_path);

    let cfg_raw = fs::read_to_string(&out_path).expect("read config json");
    let cfg = Config::from_json_str(&cfg_raw).expect("parse exported config");
    assert!(cfg.quick_telex);
    assert!(cfg.quick_start_consonant);
    assert!(cfg.quick_end_consonant);
    assert_eq!(cfg.switch_preset, SwitchPreset::ShiftShift);
    assert!(cfg.show_switch_osd);
    assert_eq!(
        cfg.behavior_for_app("Google-chrome.exe"),
        AppBehavior::ForceVietnamese
    );
    assert_eq!(cfg.macros.len(), 1);
    assert_eq!(cfg.macros[0].shortcut, "ddn");
    assert_eq!(cfg.macros[0].expansion, "Đà Nẵng\nVN");

    let stdout_import = run_args(&["import-evkey", &dir.path_str("evkey.tree")]);
    assert!(stdout_import.status.success());
    let cfg_stdout = Config::from_json_str(&stdout_str(&stdout_import));
    assert!(cfg_stdout.is_ok());
}

#[test]
fn macros_import_and_export_roundtrip() {
    let dir = TempDir::new("mskey_cli_macros");
    let cfg_path = dir.file("config.json");
    let macro_in_path = dir.file("macros_in.txt");
    let macro_out_path = dir.file("macros_out.txt");

    let initial_cfg = Config {
        macros: Config::parse_macro_text("hn:Hà Nội\nddn:Old\n"),
        ..Config::default()
    };
    fs::write(
        &cfg_path,
        initial_cfg.to_json_pretty().expect("serialize initial config"),
    )
    .expect("write initial config");

    fs::write(&macro_in_path, "# comment\nddn:Đà Nẵng\\nVN\nsg:Sài Gòn\n")
        .expect("write macro file");

    let import_out = run_args(&[
        "macros",
        "import",
        "--config",
        &dir.path_str("config.json"),
        "--file",
        &dir.path_str("macros_in.txt"),
    ]);
    assert!(import_out.status.success(), "{}", stderr_str(&import_out));

    let updated_cfg = Config::from_json_str(&fs::read_to_string(&cfg_path).expect("read cfg"))
        .expect("parse updated config");
    assert_eq!(updated_cfg.macros.len(), 3);
    assert_eq!(updated_cfg.macros[0].shortcut, "hn");
    assert_eq!(updated_cfg.macros[1].expansion, "Đà Nẵng\nVN");
    assert_eq!(updated_cfg.macros[2].shortcut, "sg");

    let export_out = run_args(&[
        "macros",
        "export",
        "--config",
        &dir.path_str("config.json"),
        "--out",
        &dir.path_str("macros_out.txt"),
    ]);
    assert!(export_out.status.success(), "{}", stderr_str(&export_out));
    assert_file_exists(&macro_out_path);
    let exported_txt = fs::read_to_string(&macro_out_path).expect("read exported macros");
    assert!(exported_txt.contains("hn:Hà Nội"));
    assert!(exported_txt.contains("ddn:Đà Nẵng\\nVN"));
    assert!(exported_txt.contains("sg:Sài Gòn"));

    let export_stdout = run_args(&["macros", "export", "--config", &dir.path_str("config.json")]);
    assert!(export_stdout.status.success());
    assert_eq!(stdout_str(&export_stdout), exported_txt);

    let stdin_import = run_with_stdin(
        &[
            "macros",
            "import",
            "--config",
            &dir.path_str("config.json"),
            "--replace",
        ],
        "only:One\n".as_bytes(),
    );
    assert!(stdin_import.status.success(), "{}", stderr_str(&stdin_import));
    let replaced_cfg = Config::from_json_str(&fs::read_to_string(&cfg_path).expect("read cfg"))
        .expect("parse replaced config");
    assert_eq!(replaced_cfg.macros.len(), 1);
    assert_eq!(replaced_cfg.macros[0].shortcut, "only");
    assert_eq!(replaced_cfg.macros[0].expansion, "One");

    let missing_cfg = run_args(&["macros", "export", "--config", &dir.path_str("missing.json")]);
    assert!(!missing_cfg.status.success());
}
