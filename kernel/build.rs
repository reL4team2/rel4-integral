use std::env;

fn asm_gen(
    platform: &str,
    defs: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let src_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let target = env::var("TARGET").unwrap();
    let mut dir = format!("{}/src/arch/riscv", src_dir);
    if target.contains("aarch64") {
        // TODO: enable fpu fault handler if build aarch64, maybe need provide by build command
        dir = format!("{}/src/arch/aarch64", src_dir);
        // defs.push("-DCONFIG_HAVE_FPU".to_string());
    }
    let inc_dir = format!("{}/include", src_dir);

    // Generate the definitions YAML from feature flags (at build time) so
    // `config_gen` reads the generated copy instead of the source file.
    let hypervisor = std::env::var("CARGO_FEATURE_HYPERVISOR").is_ok();
    let pa_40bit = std::env::var("CARGO_FEATURE_PA_40BIT").is_ok();
    let mcs = std::env::var("CARGO_FEATURE_KERNEL_MCS").is_ok();
    let smc = std::env::var("CARGO_FEATURE_ENABLE_SMC").is_ok();
    let arm_pcnt = std::env::var("CARGO_FEATURE_ENABLE_ARM_PCNT").is_ok();
    let arm_ptmr = std::env::var("CARGO_FEATURE_ENABLE_ARM_PTMR").is_ok();
    let smp = std::env::var("CARGO_FEATURE_ENABLE_SMP").is_ok();
    let fastpath = defs.iter().any(|m| m == "FASTPATH=true");
    let num_nodes = defs
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
        let c_config = rel4_config::load_c_gen_config(std::path::Path::new(&path))?;
        rel4_config::warn_on_config_conflicts(&overrides, &c_config);
        overrides.extend(c_config);
    }
    rel4_config::generate_definitions_yaml(
        platform,
        &overrides,
        std::path::Path::new(env::var("OUT_DIR").unwrap().as_str()),
    )?;

    rel4_config::generator::config_gen(platform, defs);
    let out_inc_dir = env::var("OUT_DIR").unwrap();

    rel4_config::generator::asm_gen(&dir, "head.S", vec![&inc_dir, &out_inc_dir], defs, None);
    rel4_config::generator::asm_gen(&dir, "traps.S", vec![&inc_dir, &out_inc_dir], defs, None);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("cargo:rerun-if-changed=build.rs");
    // The generated head.S/traps.S are produced by preprocessing the assembly
    // sources (and the headers they #include) below, so track them explicitly:
    // otherwise editing traps.S/head.S would not re-run this build script and
    // the kernel would be linked against a stale copy.
    println!("cargo:rerun-if-changed=src/arch/aarch64/head.S");
    println!("cargo:rerun-if-changed=src/arch/aarch64/traps.S");
    println!("cargo:rerun-if-changed=src/arch/riscv/head.S");
    println!("cargo:rerun-if-changed=src/arch/riscv/traps.S");
    println!("cargo:rerun-if-changed=include");
    println!("cargo:rerun-if-changed=../rel4_config/cfg");
    println!("cargo:rerun-if-env-changed=MARCOS");
    println!("cargo:rerun-if-env-changed=PLATFORM");
    println!("cargo:rerun-if-env-changed=SEL4_KERNEL_GEN_CONFIG");

    let defs = std::env::var("MARCOS").unwrap();
    let platform = std::env::var("PLATFORM").unwrap();
    let hypervisor = std::env::var("CARGO_FEATURE_HYPERVISOR").is_ok();
    let mut common_defs: Vec<String> = defs.split_whitespace().map(|s| s.to_string()).collect();
    asm_gen(&platform, &mut common_defs)?;
    let linker_path = rel4_config::generator::linker_gen(&platform, hypervisor);
    println!("cargo:rustc-link-arg=-T{}", linker_path.to_str().unwrap());
    Ok(())
}
