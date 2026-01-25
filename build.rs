// build.rs
use std::process::Command;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(rust_analyzer)");
    println!("cargo::rerun-if-changed=src/shader/shader.vert");
    println!("cargo::rerun-if-changed=src/shader/shader.frag");
    
    let out_dir = std::env::var("OUT_DIR").unwrap();
    
    // Compile vertex shader
    let status = Command::new("glslc")
        .args(&[
            "src/shader/shader.vert",
            "-o",
            &format!("{}/vert.spv", out_dir),
        ])
        .status()
        .expect("Failed to compile vertex shader");
    
    if !status.success() {
        panic!("Vertex shader compilation failed");
    }
    
    // Compile fragment shader
    let status = Command::new("glslc")
        .args(&[
            "src/shader/shader.frag",
            "-o",
            &format!("{}/frag.spv", out_dir),
        ])
        .status()
        .expect("Failed to compile fragment shader");
    
    if !status.success() {
        panic!("Fragment shader compilation failed");
    }
}