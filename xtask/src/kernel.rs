use std::{
    process::Command, 
    path::PathBuf
};
use clap::Parser;
use rel4_config::utils::vec_rustflags;

#[derive(Debug, Parser, Clone)]
pub struct BuildOptions {
    #[clap(default_value = "spike", short, long)]
    pub platform: String,
    #[clap(short, long)]
    pub mcs: bool,
    #[clap(short, long)]
    pub smc: bool,
    #[clap(long)]
    pub nofastpath: bool,
}

fn cargo(command: &str, dir: &str, opts: &BuildOptions) -> Result<(), anyhow::Error> {
    let dir = PathBuf::from(dir);
    let target: String = match opts.platform.as_str() {
        "spike" => {"--target=riscv64imac-unknown-none-elf".to_string()},
        "qemu-arm-virt" => {"--target=aarch64-unknown-none-softfloat".to_string()},
        _ => return Err(anyhow::anyhow!("Unsupported platform")),
    };

    let mut args = vec![
        command.to_string(),
        target,
        "--release".into(),
        "--bin".into(),
        "rel4_kernel".into(),
        "--features".into(),
        "BUILD_BINARY".into(),
    ];


    let rustflags = vec_rustflags()?;
    let mut cmd = Command::new("cargo");

    // build gcc marcos, we must add macros add xtask
    let mut marcos = vec![
        format!("-DPLATFOMR={}", &opts.platform),
        format!(
            "-DCONFIG_KERNEL_STACK_BITS={}", 
            rel4_config::get_int_from_cfg(&opts.platform, "memory.stack_bits").unwrap())
        ];

    if !opts.nofastpath {
        marcos.push("-DCONFIG_FASTPATH=ON".to_string());
    }

    if opts.mcs {
        append_features(&mut args, "KERNEL_MCS".to_string());
        marcos.push("-DCONFIG_KERNEL_MCS=ON".to_string());
    }

    if opts.smc {
        append_features(&mut args, "KERNEL_SMC".to_string());
    }

    let status = cmd.current_dir(dir).
        env("PLATFORM", opts.platform.as_str()).
        env("MARCOS", marcos.join(" ")).
        env("RUSTFLAGS", rustflags.join(" ")).
        args(&args).status().expect("failed to build userspace");
    
    assert!(status.success());
    Ok(())
}

/// Build the project
pub fn build(opts: &BuildOptions) -> Result<(), anyhow::Error> {
    cargo("build", "kernel",  opts)?;
    println!("Building the project");
    Ok(())
}

#[inline]
fn append_features(args: &mut Vec<String>, feature: String) {
    args.push("--features".into());
    args.push(feature);
}
