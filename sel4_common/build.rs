use std::env;
use std::fs;
use std::io;
use std::path;

use rust_sel4_pbf_parser::parser::pbf_parser;
fn main() {
    println!("cargo:rerun-if-changed=src/structure_gen.rs");
    let out_dir = env::var("OUT_DIR").unwrap();
    let dir_path = path::Path::new(&out_dir).join("pbf");
    if dir_path.exists() && dir_path.is_dir() {
        if let Err(e) = fs::remove_dir_all(&dir_path) {
            eprintln!("cannot del dir {}: {}", dir_path.display(), e);
            std::process::exit(1);
        } else {
            println!("dir {} has been all del", dir_path.display());
        }
    } else {
        if !dir_path.exists() {
            println!("dir {} not exist, and no need to del", dir_path.display());
        } else {
            eprintln!("path {} is not a dir", dir_path.display());
        }
    }

    match fs::create_dir(&dir_path) {
        Ok(_) => println!("Directory created successfully: {}", dir_path.display()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            println!("Directory already exists: {}", dir_path.display());
        }
        Err(e) => {
            eprintln!("Failed to create directory: {}", e);
        }
    }
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_string());
    if std::env::var("CARGO_FEATURE_KERNEL_MCS").is_ok() {
        if target == String::from("aarch64-unknown-none-softfloat") {
            if std::env::var("CARGO_FEATURE_ENABLE_SMC").is_ok() {
                pbf_parser(
                    String::from("./pbf/aarch64/smc_mcs"),
                    dir_path.to_str().unwrap().to_string(),
                );
            } else {
                pbf_parser(
                    String::from("./pbf/aarch64/mcs"),
                    dir_path.to_str().unwrap().to_string(),
                );
            }
        } else if target == String::from("riscv64imac-unknown-none-elf") {
            pbf_parser(
                String::from("./pbf/riscv64/mcs"),
                dir_path.to_str().unwrap().to_string(),
            );
        }
    } else {
        if target == String::from("aarch64-unknown-none-softfloat") {
            if std::env::var("CARGO_FEATURE_ENABLE_SMC").is_ok() {
                pbf_parser(
                    String::from("./pbf/aarch64/smc_nomcs"),
                    dir_path.to_str().unwrap().to_string(),
                );
            } else {
                pbf_parser(
                    String::from("./pbf/aarch64/nomcs"),
                    dir_path.to_str().unwrap().to_string(),
                );
            }
        } else if target == String::from("riscv64imac-unknown-none-elf") {
            pbf_parser(
                String::from("./pbf/riscv64/nomcs"),
                dir_path.to_str().unwrap().to_string(),
            );
        }
    }

    let current_dir = env::current_dir().unwrap();
    let src_file_1 = path::Path::new(&dir_path).join("structures.bf.rs");
    let dest_file_1 = path::Path::new(&current_dir).join("src/structures_gen.rs");

    let src_file_2 = path::Path::new(&dir_path).join("shared_types.bf.rs");
    let dest_file_2 = path::Path::new(&current_dir).join("src/shared_types_bf_gen.rs");

    let src_file_3 = path::Path::new(&dir_path).join("shared_types.rs");
    let dest_file_3 = path::Path::new(&current_dir).join("src/shared_types_gen.rs");

    let src_file_4 = path::Path::new(&dir_path).join("types.rs");
    let dest_file_4 = path::Path::new(&current_dir).join("src/types_gen.rs");

    if let Err(e) = fs::copy(src_file_1, dest_file_1) {
        eprintln!("Failed to copy file: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = fs::copy(src_file_2, dest_file_2) {
        eprintln!("Failed to copy file: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = fs::copy(src_file_3, dest_file_3) {
        eprintln!("Failed to copy file: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = fs::copy(src_file_4, dest_file_4) {
        eprintln!("Failed to copy file: {}", e);
        std::process::exit(1);
    }
}
