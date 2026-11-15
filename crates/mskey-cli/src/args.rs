use mskey_core::{CaseTransform, ConvertEncoding};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FromEncoding {
    Auto,
    Fixed(ConvertEncoding),
}

pub fn match_value_opt<'a>(
    args: &'a [String],
    idx: &mut usize,
    flag: &str,
) -> Result<Option<&'a str>, String> {
    let arg = &args[*idx];
    if arg == flag {
        let next = args
            .get(*idx + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        *idx += 2;
        return Ok(Some(next.as_str()));
    }
    if let Some(val) = arg
        .strip_prefix(flag)
        .and_then(|rest| rest.strip_prefix('='))
    {
        *idx += 1;
        return Ok(Some(val));
    }
    Ok(None)
}

pub fn parse_from_encoding(raw: &str) -> Result<FromEncoding, String> {
    if raw.eq_ignore_ascii_case("auto") {
        return Ok(FromEncoding::Auto);
    }
    parse_convert_encoding(raw)
        .map(FromEncoding::Fixed)
        .ok_or_else(|| {
            format!(
                "invalid --from encoding '{raw}' (expected: auto, unicode, tcvn3, vni, nfd, viqr)"
            )
        })
}

pub fn parse_to_encoding(raw: &str) -> Result<ConvertEncoding, String> {
    parse_convert_encoding(raw).ok_or_else(|| {
        format!("invalid --to encoding '{raw}' (expected: unicode, tcvn3, vni, nfd, viqr)")
    })
}

fn parse_convert_encoding(raw: &str) -> Option<ConvertEncoding> {
    match raw.to_ascii_lowercase().as_str() {
        "unicode" | "nfc" => Some(ConvertEncoding::Unicode),
        "tcvn3" => Some(ConvertEncoding::Tcvn3),
        "vni" | "vni-windows" => Some(ConvertEncoding::VniWindows),
        "nfd" | "unicode-composite" | "composite" => Some(ConvertEncoding::UnicodeComposite),
        "viqr" => Some(ConvertEncoding::Viqr),
        _ => None,
    }
}

pub fn parse_case_transform(raw: &str) -> Result<CaseTransform, String> {
    match raw.to_ascii_lowercase().as_str() {
        "none" => Ok(CaseTransform::None),
        "upper" => Ok(CaseTransform::Upper),
        "lower" => Ok(CaseTransform::Lower),
        "sentence" => Ok(CaseTransform::Sentence),
        "title" => Ok(CaseTransform::Title),
        "strip" | "strip-diacritics" => Ok(CaseTransform::StripDiacritics),
        "modern-tone" => Ok(CaseTransform::NormalizeModernTone),
        "classic-tone" => Ok(CaseTransform::NormalizeClassicTone),
        _ => Err(format!(
            "invalid --case mode '{raw}' (expected: none, upper, lower, sentence, title, strip, modern-tone, classic-tone)"
        )),
    }
}

pub fn encoding_label(encoding: ConvertEncoding) -> &'static str {
    match encoding {
        ConvertEncoding::Unicode => "unicode",
        ConvertEncoding::Tcvn3 => "tcvn3",
        ConvertEncoding::VniWindows => "vni",
        ConvertEncoding::UnicodeComposite => "nfd",
        ConvertEncoding::Viqr => "viqr",
    }
}
