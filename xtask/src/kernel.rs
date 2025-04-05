use clap::Parser;
use rel4_config::utils::vec_rustflags;
use std::{path::PathBuf, process::Command};

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", s)),
    }
}

#[derive(Debug, Parser, Clone)]
pub struct BuildOptions {
    #[clap(default_value = "spike", short, long)]
    pub platform: String,
    #[clap(short, long, value_parser = parse_bool, default_value = "false")]
    pub mcs: Option<bool>,
    #[clap(short, long, value_parser = parse_bool, default_value = "false")]
    pub smc: Option<bool>,
    #[clap(long)]
    pub nofastpath: bool,
    #[clap(long)]
    pub arm_pcnt: bool,
    #[clap(long)]
    pub arm_ptmr: bool,
    #[clap(long)]
    pub rust_only: bool,
    #[clap(long, short = 'B')]
    pub bin: bool,
}

fn cargo(command: &str, dir: &str, opts: &BuildOptions) -> Result<(), anyhow::Error> {
    let dir = PathBuf::from(dir);
    let target: String = match opts.platform.as_str() {
        "spike" => "--target=riscv64imac-unknown-none-elf".to_string(),
        "qemu-arm-virt" => "--target=aarch64-unknown-none-softfloat".to_string(),
        _ => return Err(anyhow::anyhow!("Unsupported platform")),
    };

    let mut args = vec![command.to_string(), target.clone(), "--release".into()];

    if opts.bin {
        args.push("--bin".into());
        args.push("rel4_kernel".into());
        args.push("--features".into());
        args.push("BUILD_BINARY".into());
    } else {
        args.push("--lib".into());
    }

    let rustflags = vec_rustflags()?;
    let mut cmd = Command::new("cargo");

    // build gcc marcos, we must add macros add xtask
    let mut marcos = vec![
        format!(
            "KERNEL_STACK_BITS={}",
            rel4_config::get_int_from_cfg(&opts.platform, "memory.stack_bits").unwrap()
        ),
    ];

    if !opts.nofastpath {
        marcos.push("FASTPATH=true".to_string());
    }

    if opts.mcs.unwrap_or(false) {
        append_features(&mut args, "KERNEL_MCS".to_string());
        marcos.push("KERNEL_MCS=true".to_string());
    }

    if opts.smc.unwrap_or(false) && target.contains("aarch64") {
        append_features(&mut args, "ENABLE_SMC".to_string());
        marcos.push("ALLOW_SMC_CALLS=true".to_string());
    }

    if opts.arm_pcnt && target.contains("aarch64") {
        append_features(&mut args, "ENABLE_ARM_PCNT".to_string());
        marcos.push("EXPORT_PCNT_USER=true".to_string());
    }

    if opts.arm_ptmr && target.contains("aarch64") {
        append_features(&mut args, "ENABLE_ARM_PTMR".to_string());
        marcos.push("EXPORT_PTMR_USER=true".to_string());
    }

    let status = cmd
        .current_dir(dir)
        .env("PLATFORM", opts.platform.as_str())
        .env("MARCOS", marcos.join(" "))
        .env("RUSTFLAGS", rustflags.join(" "))
        .args(&args)
        .status()
        .expect("failed to build userspace");

    assert!(status.success());
    Ok(())
}

/// Build the project
pub fn build(opts: &BuildOptions) -> Result<(), anyhow::Error> {
    let current_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let kernel = PathBuf::from(&current_dir).join("../kernel");
    cargo("build", kernel.to_str().unwrap(), opts)?;

    if !opts.rust_only {
        // TODO: add more defines and support lib mode
        let mut define: Vec<String> = vec![];
        if opts.bin {
            define.push("-DREL4_KERNEL=TRUE".to_string());
        }
        if opts.mcs.unwrap_or(false) {
            define.push("-DMCS=TRUE".to_string());
        }
        if opts.smc.unwrap_or(false) {
            define.push("-DKernelAllowSMCCalls=ON".to_string());
        }
        if opts.arm_pcnt {
            define.push("-DKernelArmExportPCNTUser=ON".to_string());
        }
        if opts.arm_ptmr {
            define.push("-DKernelArmExportPTMRUser=ON".to_string());
        }
        crate::cmake::sel4test_build(&opts.platform, &define.join(" "))?;
    }
    println!("Building complete, enjoy rel4!");
    Ok(())
}

#[inline]
fn append_features(args: &mut Vec<String>, feature: String) {
    args.push("--features".into());
    args.push(feature);
}
