use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=resources.rc");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let out_dir = env::var("OUT_DIR").unwrap();
        let out_path = Path::new(&out_dir).join("resources.o");

        let status = Command::new("windres")
            .args(&["resources.rc", "-o", out_path.to_str().unwrap()])
            .status();

        if let Ok(s) = status {
            if s.success() {
                println!("cargo:rustc-link-arg={}", out_path.display());
            }
        }
    }
}
