use std::env;

fn asm_gen(platform: &str, defs: &mut Vec<String>) {
    let src_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let target = env::var("TARGET").unwrap();
    let mut dir = format!("{}/src/arch/riscv", src_dir);
    if target.contains("aarch64") {
        // TODO: enable fpu fault handler if build aarch64, maybe need provide by build command
        dir = format!("{}/src/arch/aarch64", src_dir);
        defs.push("-DCONFIG_HAVE_FPU".to_string());
    }
    let inc_dir = format!("{}/include", src_dir);
    rel4_config::generator::config_gen(&platform, &defs);
    let out_inc_dir = env::var("OUT_DIR").unwrap();

    rel4_config::generator::asm_gen(&dir, "head.S", vec![&inc_dir, &out_inc_dir], &vec![], None);
    rel4_config::generator::asm_gen(&dir, "traps.S", vec![&inc_dir, &out_inc_dir], &vec![], None);
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let defs = std::env::var("MARCOS").unwrap();
    let platform = std::env::var("PLATFORM").unwrap();
    let mut common_defs: Vec<String> = defs.split_whitespace().map(|s| s.to_string()).collect();
    asm_gen(&platform, &mut common_defs);
    let linker_path = rel4_config::generator::linker_gen(&platform);
    println!("cargo:rustc-link-arg=-T{}", linker_path.to_str().unwrap());
}
