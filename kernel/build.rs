use std::env;
use std::process::Command;
use std::fs::File;
use std::io::Read;
use serde_yaml::Value;

fn get_value_from_yaml(file_path: &str, key: &str) -> Option<String> {
    let mut file = File::open(file_path).expect("Unable to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("Unable to read file");

    let yaml: Value = serde_yaml::from_str(&contents).expect("Unable to parse YAML");
    let keys = key.split('.');

    let mut current_value = &yaml;
    for k in keys {
        current_value = current_value.get(k)?;
    }

    current_value.as_str().map(|s| s.to_string())
}

fn get_int_from_yaml(file_path: &str, key: &str) -> Option<usize> {
    let mut file = File::open(file_path).expect(file_path);
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("Unable to read file");

    let yaml: Value = serde_yaml::from_str(&contents).expect("Unable to parse YAML");
    let keys = key.split('.');

    let mut current_value = &yaml;
    for k in keys {
        current_value = current_value.get(k)?;
    }

    current_value.as_u64().map(|n| n as usize)
}

fn parse_custom_definitions() -> Vec<String> {
    let args: Vec<String> = env::args().collect();
    let mut definitions = Vec::new();

    for arg in args.iter().skip(1) {
        if arg.starts_with("-D") {
            definitions.push(arg.clone());
        }
    }

    definitions
}

fn gcc_gen(dir: &str, name: &str, inc_dir: &str, cfg_dir: &str) {
    let src = format!("{}/{}", dir, name);
    let out_dir = env::var("OUT_DIR").unwrap();
    let out = format!("{}/{}", out_dir, name);
    let inc_param = format!("-I{}", inc_dir);
    let stack_arg = format!("-DCONFIG_KERNEL_STACK_BITS={}", get_int_from_yaml(cfg_dir, "memory.stack_bits").unwrap());

    let mut build_args = vec![
        "-E",
        &inc_param,
        &src,
        &stack_arg,
    ];

    let defs = parse_custom_definitions();
    for d in defs.iter() {
        build_args.push(d);
    }

    build_args.push("-o");
    build_args.push(&out);

    let status = std::process::Command::new("gcc")
        .args(&build_args)
        .status();

    match status {
        Ok(s) if s.success() => println!("Successfully preprocessed assembly: {}", name),
        Ok(s) => eprintln!("gcc returned a non-zero status: {}", s),
        Err(e) => eprintln!("Failed to preprocess assembly: {}", e),
    }
}

fn asm_gen() {
    let src_dir = format!("{}/../sel4_common", env::var("CARGO_MANIFEST_DIR").unwrap());
    let target = env::var("TARGET").unwrap();
    let mut dir = format!("{}/src/arch/riscv64", src_dir);
    if target.contains("aarch64") {
        dir = format!("{}/src/arch/aarch64", src_dir);
    }
    let inc_dir = format!("{}/include", src_dir);
    let cfg_file = format!("{}/src/platform/spike.yml", src_dir);

    gcc_gen(&dir, "head.S", &inc_dir, &cfg_file);
    gcc_gen(&dir, "traps.S", &inc_dir, &cfg_file);
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
    asm_gen();
}
