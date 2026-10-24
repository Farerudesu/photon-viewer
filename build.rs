use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=resources.rc");

    let out_dir = env::var("OUT_DIR").unwrap_or_default();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Prepare installer payload buffer to decouple setup build from release order
    let payload_path = Path::new(&out_dir).join("photon_viewer_payload.bin");
    let release_bin = Path::new("target/release/photon_viewer.exe");
    let debug_bin = Path::new("target/debug/photon_viewer.exe");

    if release_bin.exists() {
        let _ = std::fs::copy(release_bin, &payload_path);
    } else if debug_bin.exists() {
        let _ = std::fs::copy(debug_bin, &payload_path);
    } else {
        // Fallback stub so fresh checkout / cargo check never fails
        let _ = std::fs::write(&payload_path, b"");
    }

    if target_os == "windows" {
        let out_path = Path::new(&out_dir).join("resources.o");

        let status = Command::new("windres")
            .args(&["resources.rc", "-o", out_path.to_string_lossy().as_ref()])
            .status();

        if let Ok(s) = status {
            if s.success() {
                println!("cargo:rustc-link-arg={}", out_path.display());
            }
        }
    }
}
