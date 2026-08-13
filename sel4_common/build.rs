use std::env;
use std::fs;
use std::io;
use std::path;

use rust_sel4_pbf_parser::parser::pbf_parser;
fn main() {
    let arch = match env::var("TARGET").expect("TARGET not set").as_str() {
        "aarch64-unknown-none-softfloat" => "aarch64",
        "riscv64gc-unknown-none-elf" => "riscv64",
        _ => panic!("Unsupported target"),
    };
    // let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    // let arch = arch.as_str();
    let platform = std::env::var("PLATFORM").unwrap();
    println!("cargo:rerun-if-changed=pbf/{}/structure_gen.rs", arch);
    // Re-run the generator when these env vars change (e.g. switching
    // hypervisor/pa_40bit features), otherwise stale generated code is reused.
    println!("cargo:rerun-if-env-changed=MARCOS");
    println!("cargo:rerun-if-env-changed=PLATFORM");
    let out_dir = path::Path::new(env::var("OUT_DIR").unwrap().as_str()).join("pbf");
    let src_dir = path::Path::new(env::var("CARGO_MANIFEST_DIR").unwrap().as_str()).join("pbf");
    if out_dir.exists() && out_dir.is_dir() {
        if let Err(e) = fs::remove_dir_all(&out_dir) {
            eprintln!("cannot del dir {}: {}", out_dir.display(), e);
            std::process::exit(1);
        } else {
            println!("dir {} has been all del", out_dir.display());
        }
    } else if !out_dir.exists() {
        println!("dir {} not exist, and no need to del", out_dir.display());
    } else {
        eprintln!("path {} is not a dir", out_dir.display());
    }

    match fs::create_dir(&out_dir) {
        Ok(_) => println!("Directory created successfully: {}", out_dir.display()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            println!("Directory already exists: {}", out_dir.display());
        }
        Err(e) => {
            eprintln!("Failed to create directory: {}", e);
        }
    }

    let common_include = src_dir.join("include");
    let arch_include = src_dir.join("include").join(arch);

    let defs = std::env::var("MARCOS").unwrap();
    let mut common_defs: Vec<String> = defs.split_whitespace().map(|s| s.to_string()).collect();
    if arch.contains("aarch64") {
        // TODO: enable fpu fault handler if build aarch64, maybe need provide by build command
        common_defs.push("have_fpu=true".to_string());
    }
    // pt levels: 3-level only when hypervisor + pa_40bit, otherwise 4-level.
    let hypervisor = std::env::var("CARGO_FEATURE_HYPERVISOR").is_ok();
    let pa_40bit = std::env::var("CARGO_FEATURE_PA_40BIT").is_ok();
    if hypervisor && pa_40bit {
        common_defs.push("PT_LEVELS=3".to_string());
    } else {
        common_defs.push("PT_LEVELS=4".to_string());
    }

    rel4_config::generator::config_gen(&platform, &common_defs);
    let out_inc_dir = env::var("OUT_DIR").unwrap();

    rel4_config::generator::asm_gen(
        src_dir.join(arch).to_str().unwrap(),
        "structures.bf",
        vec![
            common_include.to_str().unwrap(),
            arch_include.to_str().unwrap(),
            out_inc_dir.as_str(),
        ],
        &vec![],
        Some(out_dir.join("structures.bf.pbf").to_str().unwrap()),
    );

    rel4_config::generator::asm_gen(
        src_dir.join(arch).to_str().unwrap(),
        "shared_types.bf",
        vec![
            common_include.to_str().unwrap(),
            arch_include.to_str().unwrap(),
            out_inc_dir.as_str(),
        ],
        &vec![],
        Some(out_dir.join("shared_types.bf.pbf").to_str().unwrap()),
    );

    pbf_parser(
        out_dir.to_str().unwrap().to_string(),
        out_dir.to_str().unwrap().to_string(),
    );

    rel4_config::generator::platform_gen(&platform);
}
