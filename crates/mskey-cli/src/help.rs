const GLOBAL_HELP: &str = "\
mskey - Vietnamese input method toolkit CLI

USAGE:
    mskey <COMMAND> [OPTIONS]

COMMANDS:
    convert        Convert Vietnamese text encoding, case, or tone placement
    detect         Detect Vietnamese encoding of text
    check          Validate Vietnamese words against dictionary and syllable rules
    import-evkey   Import an EVKey .tree configuration into MS Key JSON
    macros         Export or import text macros
    help           Print help for a command
    version        Print version information

OPTIONS:
    -h, --help     Print help
    -V, --version  Print version
";

const CONVERT_HELP: &str = "\
USAGE:
    mskey convert [OPTIONS]

OPTIONS:
    --from <ENCODING>   Source encoding: auto|unicode|tcvn3|vni|nfd|viqr (default: auto)
    --to <ENCODING>     Target encoding: unicode|tcvn3|vni|nfd|viqr (default: unicode)
    --case <MODE>       Transform: none|upper|lower|sentence|title|strip|modern-tone|classic-tone (default: none)
    --in <PATH>         Input file path, or '-' for stdin (default: stdin)
    --out <PATH>        Output file path, or '-' for stdout (default: stdout)
    -h, --help          Print help
";

const DETECT_HELP: &str = "\
USAGE:
    mskey detect [OPTIONS]

OPTIONS:
    --in <PATH>   Input file path, or '-' for stdin (default: stdin)
    --json        Print JSON output {\"encoding\":\"...\"}
    -h, --help    Print help
";

const CHECK_HELP: &str = "\
USAGE:
    mskey check <WORD>... [OPTIONS]
    mskey check --in <PATH> [OPTIONS]

OPTIONS:
    --in <PATH>     Read whitespace-separated words from file or '-' for stdin
    --allow-fjwz    Allow f, j, w, z onset letters
    --json          Print JSON validation report
    -h, --help      Print help
";

const IMPORT_EVKEY_HELP: &str = "\
USAGE:
    mskey import-evkey <TREE_PATH> [--out <PATH>]

OPTIONS:
    --out <PATH>  Output JSON file path, or '-' for stdout (default: stdout)
    -h, --help    Print help
";

const MACROS_HELP: &str = "\
USAGE:
    mskey macros export --config <PATH> [--out <PATH>]
    mskey macros import --config <PATH> [--file <PATH>] [--replace]

OPTIONS:
    --config <PATH>  MS Key config JSON path
    --out <PATH>     Export destination path, or '-' for stdout (default: stdout)
    --file <PATH>    Macro text file path, or '-' for stdin (default: stdin)
    --replace        Replace existing macros instead of merging by shortcut
    -h, --help       Print help
";

pub fn global_help() -> &'static str {
    GLOBAL_HELP
}

pub fn topic_help(topic: &str) -> Option<&'static str> {
    match topic {
        "convert" => Some(CONVERT_HELP),
        "detect" => Some(DETECT_HELP),
        "check" => Some(CHECK_HELP),
        "import-evkey" => Some(IMPORT_EVKEY_HELP),
        "macros" => Some(MACROS_HELP),
        _ => None,
    }
}
