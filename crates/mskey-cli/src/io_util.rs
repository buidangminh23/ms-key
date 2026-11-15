use std::fs;
use std::io::{self, Read, Write};

pub fn read_utf8_input(path: Option<&str>) -> Result<String, String> {
    match path {
        None | Some("-") => {
            io::read_to_string(io::stdin().lock()).map_err(|err| format!("failed to read stdin: {err}"))
        }
        Some(p) => fs::read_to_string(p).map_err(|err| format!("failed to read '{p}': {err}")),
    }
}

pub fn read_encoding_input(
    path: Option<&str>,
    allow_latin1_fallback: bool,
) -> Result<String, String> {
    let bytes = match path {
        None | Some("-") => {
            let mut buf = Vec::new();
            io::stdin()
                .lock()
                .read_to_end(&mut buf)
                .map_err(|err| format!("failed to read stdin: {err}"))?;
            buf
        }
        Some(p) => fs::read(p).map_err(|err| format!("failed to read '{p}': {err}"))?,
    };
    match String::from_utf8(bytes) {
        Ok(text) => Ok(text),
        Err(err) if allow_latin1_fallback => {
            Ok(err.into_bytes().into_iter().map(char::from).collect())
        }
        Err(err) => Err(format!("input is not valid UTF-8: {err}")),
    }
}

pub fn write_text_output(path: Option<&str>, content: &str) -> Result<(), String> {
    match path {
        None | Some("-") => {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(content.as_bytes())
                .and_then(|()| stdout.flush())
                .map_err(|err| format!("failed to write stdout: {err}"))
        }
        Some(p) => {
            fs::write(p, content.as_bytes()).map_err(|err| format!("failed to write '{p}': {err}"))
        }
    }
}

pub fn strip_bom(input: &str) -> &str {
    input.strip_prefix('\u{FEFF}').unwrap_or(input)
}
