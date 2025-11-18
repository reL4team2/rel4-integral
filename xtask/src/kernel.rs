use clap::Parser;
use rel4_config::utils::vec_rustflags;
use std::os::unix::fs::symlink;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

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
/// * `benchmark` -Enable Benchmark.
#[derive(Debug, Parser, Clone)]
pub struct BuildOptions {
    #[clap(
        default_value = "spike",
        short,
        long,
        help = "support spike and qemu-arm-virt"
    )]
    pub platform: String,
    #[clap(
        short,
        long,
        default_value_t = false,
        help = "Enable MCS support if set to true or on"
    )]
    pub mcs: bool,
    #[clap(
        short,
        long,
        default_value_t = false,
        help = "Enable SMC support if set to true or on"
    )]
    pub smc: bool,
    #[clap(long, help = "Disable fastpath feature")]
    pub nofastpath: bool,
    #[clap(long, default_value_t = false, help = "Enable pcnt regs read/write in userspace")]
    pub arm_pcnt: bool,
    #[clap(long, default_value_t = false, help = "Enable ptmr regs read/write in userspace")]
    pub arm_ptmr: bool,
    #[clap(long, default_value_t = false, help = "Enable hypervisor feature(TODO)")]
    pub arm_hypervisor: bool,
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
        default_value_t = 1
    )]
    pub num_nodes: usize,
    #[clap(default_value = "INFO", help = "set log level", long)]
    pub log: String,
    #[clap(long)]
    pub benchmark: bool,
}

/// Parse CMAKE DEFINES from build options
pub fn parse_cmake_defines(opts: &BuildOptions) -> Result<Vec<String>, anyhow::Error> {
    let mut define: Vec<String> = vec![];
    // define.push("-DCMAKE_INSTALL_PREFIX=install".to_string());
    if opts.bin {
        define.push("-DREL4_KERNEL=TRUE".to_string());
    }
    if opts.mcs {
        define.push("-DMCS=TRUE".to_string());
    }
    if opts.smc {
        define.push("-DKernelAllowSMCCalls=ON".to_string());
    }
    if opts.arm_pcnt {
        define.push("-DKernelArmExportPCNTUser=ON".to_string());
    }
    if opts.arm_ptmr {
        define.push("-DKernelArmExportPTMRUser=ON".to_string());
    }
    if opts.arm_hypervisor {
        define.push("-DKernelArmHypervisorSupport=ON".to_string());
    }
    if opts.num_nodes > 1 {
        define.push(format!("-DSMP=TRUE"));
        define.push(format!("-DNUM_NODES={}", opts.num_nodes));
    }
    match opts.platform.as_str() {
        "spike" => define.push("-DKernelRiscvExtD=ON".to_string()),
        "qemu-arm-virt" => {}
        _ => return Err(anyhow::anyhow!("Unsupported platform")),
    };
    Ok(define)
}

pub fn cargo(command: &str, dir: &str, opts: &BuildOptions) -> Result<(), anyhow::Error> {
    let dir: PathBuf = PathBuf::from(dir);
    let target = match opts.platform.as_str() {
        "spike" => "--target=riscv64gc-unknown-none-elf",
        "qemu-arm-virt" => "--target=aarch64-unknown-none-softfloat",
        _ => return Err(anyhow::anyhow!("Unsupported platform")),
    };
    let current_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let easy_setting_cmake_file = PathBuf::from(&current_dir).join("../../easy-settings.cmake");

    let mut args = vec![command.to_string(), target.to_string(), "--release".into()];

    if opts.bin {
        args.push("--bin".into());
        args.push("rel4_kernel".into());
        args.push("--features".into());
        args.push("build_binary".into());
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

    if opts.mcs {
        append_features(&mut args, "kernel_mcs".to_string());
        marcos.push("KERNEL_MCS=true".to_string());
    }

    if opts.smc && target.contains("aarch64") {
        append_features(&mut args, "enable_smc".to_string());
        marcos.push("ALLOW_SMC_CALLS=true".to_string());
    }

    if opts.arm_pcnt && target.contains("aarch64") {
        append_features(&mut args, "enable_arm_pcnt".to_string());
        marcos.push("EXPORT_PCNT_USER=true".to_string());
    }

    if opts.arm_ptmr && target.contains("aarch64") {
        append_features(&mut args, "enable_arm_ptmr".to_string());
        marcos.push("EXPORT_PTMR_USER=true".to_string());
    }

    if opts.arm_hypervisor && target.contains("aarch64") {
        append_features(&mut args, "hypervisor".to_string());
        marcos.push("ARCH_ARM_HYP=true".to_string());
        marcos.push("AARCH64_VSPACE_S2_START_L1=true".to_string());
    }

    if Path::new(&easy_setting_cmake_file).exists() {
        fs::remove_file(easy_setting_cmake_file.clone())?;
        println!("Removed existing easy-settings.cmake");
    }

    if Path::new(&easy_setting_cmake_file).exists() {
        return Err(anyhow::anyhow!("unknown file exist"));
    }

    if opts.benchmark {
        append_features(&mut args, "enable_benchmark".to_string());
        // TODO: I'm not sure whether should I add the feature of C code.
        // marcos.push("EXPORT_PTMR_USER=true".to_string());

        let target =
            PathBuf::from(&current_dir).join("../../projects/sel4bench/easy-settings.cmake");
        if Path::new(&target).exists() {
            symlink(target, easy_setting_cmake_file)?;
            println!("Created symlink to sel4bench easy-settings.cmake");
        } else {
            return Err(anyhow::anyhow!("Target file does not exist"));
        }

        let nanopb_dst_dir_path = PathBuf::from(&current_dir).join("../../nanopb");
        let nanopb_src_dir_path = PathBuf::from(&current_dir).join("../../tools/nanopb");
        if !Path::new(&nanopb_dst_dir_path).exists() {
            symlink(nanopb_src_dir_path, nanopb_dst_dir_path)?;
            println!("Created symlink to nanopb");
        }
    } else {
        let target =
            PathBuf::from(&current_dir).join("../../projects/sel4test/easy-settings.cmake");
        if Path::new(&target).exists() {
            symlink(target, easy_setting_cmake_file)?;
            println!("Created symlink to sel4test easy-settings.cmake");
        } else {
            return Err(anyhow::anyhow!("Target file does not exist"));
        }
    }

    if opts.num_nodes > 1 {
        append_features(&mut args, "enable_smp".to_string());
        marcos.push(format!("MAX_NUM_NODES={}", opts.num_nodes));
        marcos.push("ENABLE_SMP_SUPPORT=true".to_string());
    }

    //TODO: add fpu config according the opts
    //we think it's default open this option
    append_features(&mut args, "have_fpu".to_string());
    marcos.push("HAVE_FPU=true".to_string());
    match opts.platform.as_str() {
        "spike" => {
            append_features(&mut args, "riscv_ext_d".to_string());
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
        .env("LOG", opts.log.as_str())
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
        let defines = parse_cmake_defines(opts)?;
        crate::cmake::sel4test_build(&opts.platform, &defines)?;
    }
    println!("Building complete, enjoy rel4!");
    Ok(())
}

#[inline]
fn append_features(args: &mut Vec<String>, feature: String) {
    args.push("--features".into());
    args.push(feature);
}
