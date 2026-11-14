use std::env;
use std::process::ExitCode;

use mskey_linux::config_store::render_ibus_component_xml;

#[cfg(target_os = "linux")]
fn run_platform(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--ibus") {
        return mskey_linux::ibus_dbus::run_ibus_service();
    }
    if args.iter().any(|a| a == "--convert") {
        return mskey_linux::gtk_ui::run_converter_only();
    }
    if let Some(pos) = args.iter().position(|a| a == "--verify-smoke") {
        let out = args.get(pos + 1).map(|s| s.as_str()).unwrap_or("smoke_linux.txt");
        return mskey_linux::gtk_ui::run_setup_ui(Some(out));
    }
    mskey_linux::gtk_ui::run_setup_ui(None)
}

#[cfg(not(target_os = "linux"))]
fn run_platform(_args: &[String]) -> i32 {
    eprintln!("mskey-linux runtime is supported only on Linux targets");
    1
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--print-xml") {
        let exe = env::current_exe()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "/usr/local/bin/mskey-linux".to_string());
        print!("{}", render_ibus_component_xml(&exe, &exe));
        return ExitCode::SUCCESS;
    }
    let code = run_platform(&args);
    ExitCode::from(code.clamp(0, 255) as u8)
}
