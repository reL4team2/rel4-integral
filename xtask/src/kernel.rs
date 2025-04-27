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
/// Options for building the kernel.
///
/// # Fields
///
/// * `platform` - Specifies the target platform for the build. Defaults to "spike".
/// * `mcs` - Enables or disables MCS (Mixed Criticality Systems) support. Defaults to `false`.
/// * `smc` - Enables or disables SMC (Secure Monitor Call) support. Defaults to `false`.
/// * `nofastpath` - Disables fastpath optimizations in the kernel.
/// * `arm_pcnt` - Enables ARM performance counter support.
/// * `arm_ptmr` - Enables ARM physical timer support.
/// * `rust_only` - Builds the kernel using only Rust code, excluding any external dependencies.
/// * `bin` - Generates a binary output for the kernel. Can be specified with `-B` or `--bin`.
pub struct BuildOptions {
    #[clap(
        default_value = "spike",
        short,
        long,
        help = "support spike and qemu-arm-virt"
    )]
    pub platform: String,
    #[clap(short, long, value_parser = parse_bool, default_value = "false", help = "Enable MCS support if set to true or on")]
    pub mcs: Option<bool>,
    #[clap(short, long, value_parser = parse_bool, default_value = "false", help = "Enable SMC support if set to true or on")]
    pub smc: Option<bool>,
    #[clap(long)]
    pub nofastpath: bool,
    #[clap(long)]
    pub arm_pcnt: bool,
    #[clap(long)]
    pub arm_ptmr: bool,
    #[clap(long, help = "Only build the reL4 rust kernel")]
    pub rust_only: bool,
    #[clap(
        long,
        short = 'B',
        help = "Build kernel in binary mode",
        default_value_if("rust_only", "true", Some("true"))
    )]
    pub bin: bool,
    #[clap(
        long,
        short = 'N',
        help = "Number of nodes in the system, if > 1, enable smp",
        default_value = "1"
    )]
    pub num_nodes: usize,
}

fn cargo(command: &str, dir: &str, opts: &BuildOptions) -> Result<(), anyhow::Error> {
    let dir = PathBuf::from(dir);
    let target: String = match opts.platform.as_str() {
        "spike" => "--target=riscv64gc-unknown-none-elf".to_string(),
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
    let mut marcos = vec![format!(
        "KERNEL_STACK_BITS={}",
        rel4_config::get_int_from_cfg(&opts.platform, "memory.stack_bits").unwrap()
    )];

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

    if opts.num_nodes > 1 {
        append_features(&mut args, "ENABLE_SMP".to_string());
        marcos.push(format!("MAX_NUM_NODES={}", opts.num_nodes));
    }

    //TODO: add fpu config according the opts
    //we think it's default open this option
    append_features(&mut args, "HAVE_FPU".to_string());
    marcos.push("HAVE_FPU=true".to_string());
    match opts.platform.as_str() {
        "spike" => {
            append_features(&mut args, "RISCV_EXT_D".to_string());
            marcos.push("RISCV_EXT_D=true".to_string())
        }
        "qemu-arm-virt" => {}
        _ => return Err(anyhow::anyhow!("Unsupported platform")),
    };

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
        if opts.num_nodes > 1 {
            define.push(format!("-DSMP=TRUE"));
            define.push(format!("-DNUM_NODES={}", opts.num_nodes));
        }
        match opts.platform.as_str() {
            "spike" => {
                define.push("-DKernelRiscvExtD=ON".to_string());
            }
            "qemu-arm-virt" => {}
            _ => return Err(anyhow::anyhow!("Unsupported platform")),
        };
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
