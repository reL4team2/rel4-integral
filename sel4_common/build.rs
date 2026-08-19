use std::env;
use std::fs;
use std::io;
use std::path;

use rust_sel4_pbf_parser::parser::pbf_parser;
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    println!("cargo:rerun-if-env-changed=SEL4_KERNEL_GEN_CONFIG");
    // Source definitions YAML.
    println!(
        "cargo:rerun-if-changed={}",
        rel4_config::get_definitions_yaml_path(&platform).display()
    );
    // Source platform YAML (plain and hypervisor variant).
    println!(
        "cargo:rerun-if-changed={}",
        rel4_config::get_platform_yaml_path(&platform, false).display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        rel4_config::get_platform_yaml_path(&platform, true).display()
    );
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

    // Generate the definitions YAML from feature flags (at build time) so
    // `config_gen` reads the generated copy instead of the source file.
    let mcs = std::env::var("CARGO_FEATURE_KERNEL_MCS").is_ok();
    let smc = std::env::var("CARGO_FEATURE_ENABLE_SMC").is_ok();
    let arm_pcnt = std::env::var("CARGO_FEATURE_ENABLE_ARM_PCNT").is_ok();
    let arm_ptmr = std::env::var("CARGO_FEATURE_ENABLE_ARM_PTMR").is_ok();
    let smp = std::env::var("CARGO_FEATURE_ENABLE_SMP").is_ok();
    let fastpath = common_defs.iter().any(|m| m == "FASTPATH=true");
    let num_nodes = common_defs
        .iter()
        .find_map(|m| m.strip_prefix("MAX_NUM_NODES=").and_then(|v| v.parse().ok()))
        .unwrap_or(1);

    // Feature-flag overrides first (they gate `#[cfg(feature = "...")]` code, so
    // they must win over any external config), followed by the optional external
    // C kernel `gen_config.yaml` for the remaining static values (e.g.
    // ROOT_CNODE_SIZE_BITS). The first matching override wins.
    let mut overrides = rel4_config::build_definitions_overrides(
        hypervisor, pa_40bit, mcs, smc, arm_pcnt, arm_ptmr, fastpath, smp, num_nodes,
    );
    if let Ok(path) = env::var("SEL4_KERNEL_GEN_CONFIG") {
        let c_config = rel4_config::load_c_gen_config(path::Path::new(&path))?;
        rel4_config::warn_on_config_conflicts(&overrides, &c_config);
        overrides.extend(c_config);
    }
    rel4_config::generate_definitions_yaml(
        &platform,
        &overrides,
        path::Path::new(env::var("OUT_DIR").unwrap().as_str()),
    )?;

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

    rel4_config::generator::platform_gen(&platform, hypervisor);

    // Generate the invocation `MessageLabel` enum from the libsel4 XML files
    // (方案 A), plus a compile-time consistency check (方案 B). This keeps the
    // kernel's labels aligned with libsel4 for every config combination.
    let (kernel_arch, sel4_arch) = match arch {
        "aarch64" => ("arm", "aarch64"),
        "riscv64" => ("riscv", "riscv64"),
        other => panic!("Unsupported target: {}", other),
    };

    // Locate the libsel4 interfaces directory. Override with LIBSEL4_DIR if the
    // default relative layout (kernel/ and rel4_kernel/ as siblings) differs.
    let libsel4_dir = match std::env::var("LIBSEL4_DIR") {
        Ok(dir) => path::PathBuf::from(dir),
        Err(_) => path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../kernel/libsel4"),
    };

    println!("cargo:rerun-if-env-changed=LIBSEL4_DIR");
    println!(
        "cargo:rerun-if-changed={}",
        libsel4_dir.join("include/interfaces/object-api.xml").display()
    );

    let mut enabled_features: Vec<&str> = Vec::new();
    for feat in ["kernel_mcs", "enable_smp", "hypervisor", "enable_smc"] {
        let var = format!("CARGO_FEATURE_{}", feat.to_uppercase());
        if std::env::var(var).is_ok() {
            enabled_features.push(feat);
        }
    }

    let message_label_out = path::Path::new(env::var("OUT_DIR").unwrap().as_str())
        .join("message_label.rs");
    rel4_config::message_label_gen::generate_message_label(
        &libsel4_dir,
        kernel_arch,
        sel4_arch,
        &enabled_features,
        &message_label_out,
    )?;

    Ok(())
}
