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
fn python_gen() {
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_string());
    let mut platform = "";
    if target == "aarch64-unknown-none-softfloat" {
        platform = "-pqemu-arm-virt"
    } else if target == "riscv64imac-unknown-none-elf" {
        platform = "-pspike"
    }
    if std::env::var("CARGO_FEATURE_KERNEL_MCS").is_ok() {
        Command::new("python3")
            .args(&[
                "generator.py",
                platform,
                "-d CONFIG_HAVE_FPU",
                "-d CONFIG_FASTPATH",
                "-d CONFIG_KERNEL_MCS",
            ])
            .status()
            .expect("Failed to generate");
    } else {
        Command::new("python3")
            .args(&[
                "generator.py",
                platform,
                "-d CONFIG_HAVE_FPU",
                "-d CONFIG_FASTPATH",
            ])
            .status()
            .expect("Failed to generate");
    }
}

fn main() {
    python_gen();

    file_gen("src/arch/riscv", "head.S");
}
