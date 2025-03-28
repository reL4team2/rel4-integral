use std::path::PathBuf;
use std::process::Command;

use clap::Parser;

#[derive(Parser, Debug)]
pub struct InstallOptions {
    #[clap(flatten)]
    build_options: super::kernel::BuildOptions,
    #[clap(long, default_value = "/workspace/.seL4")]
    sel4_prefix: String,
}

pub fn install(opts: &mut InstallOptions) -> Result<(), anyhow::Error> {
    opts.build_options.bin = true;
    opts.build_options.rust_only = true;
    let bin_dir = PathBuf::from(&opts.sel4_prefix).join("bin");
    super::kernel::install(&opts.build_options, bin_dir.to_str().unwrap())?;

    install_libsel4(opts)?;
    install_sel4_kernel_loader_add_payload(opts)?;
    install_sel4_kernel_loade(opts)?;
    
    Ok(())
}

fn install_libsel4(opts: &mut InstallOptions) -> Result<(), anyhow::Error> {
    let build_dir = "/tmp/seL4_c_impl";
    if std::fs::remove_dir_all(build_dir).is_err() {
        // Do nothing if the directory does not exist
    }
    let status = Command::new("git")
        .args(&["clone", "https://github.com/reL4team2/seL4_c_impl.git", build_dir, "--config", "advice.detachedHead=false"])
        .status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("Failed to clone repository"));
    }

    let build_path = PathBuf::from(build_dir).join("build");
    let status = Command::new("cmake")
        .args(&[
            "-DCROSS_COMPILER_PREFIX=aarch64-linux-gnu-",
            format!("-DCMAKE_INSTALL_PREFIX={}", opts.sel4_prefix).as_str(),
            "-DKernelAllowSMCCalls=ON",
            "-DREL4_KERNEL=TRUE",
            "-DKernelArmExportPCNTUser=ON",
            "-DKernelArmExportPTMRUser=ON",
            "-C", "./kernel-settings-aarch64.cmake",
            "-G", "Ninja",
            "-S", ".",
            "-B", build_path.to_str().unwrap(),
        ])
        .current_dir(build_dir)
        .status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("Failed to configure project with CMake"));
    }

    let status = Command::new("ninja")
        .args(&["-C", "build", "all"])
        .current_dir(build_dir)
        .status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("Failed to build project with Ninja"));
    }

    let status = Command::new("ninja")
        .args(&["-C", "build", "install"])
        .current_dir(build_dir)
        .status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("Failed to install project with Ninja"));
    }

    // Remove the source directory after installation
    std::fs::remove_dir_all(build_dir)?;

    Ok(())
}

fn install_sel4_kernel_loader_add_payload(opts: &mut InstallOptions) -> Result<(), anyhow::Error> {
    let mut cmd = Command::new("rustup");
    let url: String = "https://github.com/reL4team2/rust-sel4.git".into();
    let rev: String = "642b58d807c5e5fc22f0c15d1467d6bec328faa9".into();

    let args: Vec<String> = vec![
        "run".into(),
        "nightly-2024-08-01".into(),
        "cargo".into(),
        "install".into(),
        "--git".into(), url.clone(),
        "--rev".into(), rev.clone(),
        "--root".into(), opts.sel4_prefix.clone(),
        "sel4-kernel-loader-add-payload".into(),
    ];

    cmd.env_remove("RUSTUP_TOOLCHAIN").env_remove("CARGO").args(&args).status().expect("failed install sel4-kernel-loader-add-payload");
    
    Ok(())
}

fn install_sel4_kernel_loade(opts: &mut InstallOptions) -> Result<(), anyhow::Error> {
    let mut cmd = Command::new("rustup");
    let args: Vec<String>  = vec![
        "run".into(),
        "nightly-2024-08-01".into(),
        "cargo".into(),
        "install".into(),
        "-Z".into(), "build-std=core,compiler_builtins".into(),
        "-Z".into(), "build-std-features=compiler-builtins-mem".into(),
        "--target".into(), "aarch64-unknown-none".into(),
        "--git".into(), "https://github.com/reL4team2/rust-sel4.git".into(),
        "--rev".into(), "642b58d807c5e5fc22f0c15d1467d6bec328faa9".into(),
        "--root".into(), opts.sel4_prefix.clone(),
        "sel4-kernel-loader".into(),
    ];

    cmd.env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("CARGO")
        .env("SEL4_PREFIX", opts.sel4_prefix.clone())
        .env("CC_aarch64_unknown_none", "aarch64-linux-gnu-gcc")
        .args(&args)
        .status().expect("failed install sel4-kernel-loader");
    
    Ok(())
}