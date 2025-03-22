use std::env;

fn asm_gen(defs: &Vec<String>) {
    let src_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let target = env::var("TARGET").unwrap();
    let mut dir = format!("{}/src/arch/riscv", src_dir);
    if target.contains("aarch64") {
        dir = format!("{}/src/arch/aarch64", src_dir);
    }
    let inc_dir = format!("{}/include", src_dir);

    rel4_config::generator::asm_gen(&dir, "head.S", &inc_dir, defs);
    rel4_config::generator::asm_gen(&dir, "traps.S", &inc_dir, defs);
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let defs = std::env::var("MARCOS").unwrap();
    let platform = std::env::var("PLATFORM").unwrap();
    let common_defs: Vec<String> = defs.split_whitespace().map(|s| s.to_string()).collect();
    asm_gen(&common_defs);
    let linker_path = rel4_config::generator::linker_gen(&platform);
    println!("cargo:rustc-link-arg=-T{}", linker_path.to_str().unwrap());
}
