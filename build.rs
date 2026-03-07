// build.rs
use std::process::Command;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(rust_analyzer)");
    println!("cargo::rerun-if-changed=src/shader/shader.vert");
    println!("cargo::rerun-if-changed=src/shader/shader.frag");
    println!("cargo::rerun-if-changed=src/shader/atmos.vert");
    println!("cargo::rerun-if-changed=src/shader/atmos.frag");

    let out_dir = std::env::var("OUT_DIR").unwrap();

    for (shaderfn, outfn) in [
        ("src/shader/shader.vert", "vert.spv"),
        ("src/shader/shader.frag", "frag.spv"),
        ("src/shader/atmos.vert", "atmos_v.spv"),
        ("src/shader/atmos.frag", "atmos_f.spv"),
    ] {
        let status = Command::new("glslc")
            .args(&[shaderfn, "-o", &format!("{}/{}", out_dir, outfn)])
            .status()
            .expect("Failed to execute shader compile");

        if !status.success() {
            panic!("Shader compilation failed");
        }
    }
}
