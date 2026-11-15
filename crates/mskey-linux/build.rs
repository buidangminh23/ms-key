use std::env;
use std::process::Command;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    if let Ok(out) = Command::new("pkg-config").args(["--libs", "gtk+-3.0"]).output()
        && out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for tok in text.split_whitespace() {
                if let Some(lib) = tok.strip_prefix("-l") {
                    println!("cargo:rustc-link-lib={lib}");
                } else if let Some(dir) = tok.strip_prefix("-L") {
                    println!("cargo:rustc-link-search=native={dir}");
                }
            }
            return;
        }
    for lib in ["gtk-3", "gdk-3", "gio-2.0", "gobject-2.0", "glib-2.0"] {
        println!("cargo:rustc-link-lib={lib}");
    }
}
