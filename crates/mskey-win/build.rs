use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=mskey.manifest");
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let manifest = dir.join("mskey.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
