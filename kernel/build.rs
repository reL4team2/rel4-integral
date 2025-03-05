use std::env;
use std::process::Command;

fn file_gen(dir: &str, name: &str) {
    let src = format!("{}/{}", dir, name);
    let out_dir = env::var("OUT_DIR").unwrap();
    let out = format!("{}/{}", out_dir, name);

    Command::new("gcc")
        .args(&[
            "-E",
            "-I./include", // 使用相对路径添加头文件路径
            &src,          // 汇编文件路径
            "-o",
            &out, // 输出路径
        ])
        .status()
        .expect("Failed to preprocess assembly");
}

fn main() {
    file_gen("src/arch/riscv", "head.S");

    println!("cargo:rerun-if-changed=build.rs");
}
