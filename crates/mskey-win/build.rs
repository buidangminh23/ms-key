use std::env;
use std::path::PathBuf;
use std::process::Command;

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
        let icon = dir.join("../../assets/branding/mskey.ico");
        println!("cargo:rerun-if-changed={}", icon.display());
        let out = PathBuf::from(env::var("OUT_DIR").unwrap());
        let rc = out.join("mskey.rc");
        let res = out.join("mskey.res");
        std::fs::write(&rc, format!("101 ICON \"{}\"\n", icon.to_string_lossy().replace('\\', "/"))).unwrap();
        let compiler = resource_compiler();
        let status = Command::new(compiler).arg("/nologo").arg("/fo").arg(&res).arg(&rc)
            .status().expect("Windows SDK resource compiler is required");
        assert!(status.success(), "Windows icon resource compilation failed");
        println!("cargo:rustc-link-arg-bins={}", res.display());
    }
}

fn resource_compiler() -> PathBuf {
    if Command::new("rc.exe").arg("/?").output().is_ok() {
        return PathBuf::from("rc.exe");
    }
    let sdk = env::var_os("WindowsSdkDir").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env::var_os("ProgramFiles(x86)").unwrap_or_else(|| "C:/Program Files (x86)".into()))
            .join("Windows Kits/10")
    });
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(sdk.join("bin")).into_iter().flatten()
        .filter_map(Result::ok).map(|entry| entry.path().join("x64/rc.exe"))
        .filter(|path| path.is_file()).collect();
    candidates.sort();
    candidates.pop().expect("Install the Windows SDK resource compiler")
}
