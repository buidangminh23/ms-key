mod args;
mod commands;
mod help;
mod io_util;

use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            let _ = writeln!(io::stderr().lock(), "error: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let raw_args: Vec<String> = std::env::args().collect();
    let args = raw_args.get(1..).unwrap_or(&[]);
    let Some(cmd) = args.first() else {
        let _ = io::stderr().lock().write_all(help::global_help().as_bytes());
        return Ok(ExitCode::from(2));
    };

    match cmd.as_str() {
        "--help" | "-h" => {
            io_util::write_text_output(None, help::global_help())?;
            Ok(ExitCode::SUCCESS)
        }
        "help" => {
            let Some(topic) = args.get(1) else {
                io_util::write_text_output(None, help::global_help())?;
                return Ok(ExitCode::SUCCESS);
            };
            let Some(text) = help::topic_help(topic) else {
                return Err(format!(
                    "unknown help topic '{topic}'; run 'mskey --help' for usage"
                ));
            };
            io_util::write_text_output(None, text)?;
            Ok(ExitCode::SUCCESS)
        }
        "version" | "--version" | "-V" => {
            io_util::write_text_output(None, &format!("mskey {}\n", env!("CARGO_PKG_VERSION")))?;
            Ok(ExitCode::SUCCESS)
        }
        "convert" => commands::run_convert(&args[1..]),
        "detect" => commands::run_detect(&args[1..]),
        "check" => commands::run_check(&args[1..]),
        "import-evkey" => commands::run_import_evkey(&args[1..]),
        "macros" => commands::run_macros(&args[1..]),
        other => Err(format!(
            "unknown command '{other}'; run 'mskey --help' for usage"
        )),
    }
}
