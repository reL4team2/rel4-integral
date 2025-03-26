use std::path::PathBuf;
use std::process::Command;

use clap::Parser;

#[derive(Parser, Debug)]
pub struct InstallOptions {
    #[clap(flatten)]
    build_options: super::kernel::BuildOptions,
    #[clap(long, default_value = "/opt/rel4")]
    sel4_prefix: String,
}

pub fn install(opts: &mut InstallOptions) -> Result<(), anyhow::Error> {
    opts.build_options.bin = true;
    opts.build_options.rust_only = true;
    let bin_dir = PathBuf::from(&opts.sel4_prefix).join("bin");
    super::kernel::install(&opts.build_options, bin_dir.to_str().unwrap())?;

    clone_and_build_project()?;
    rust_sel4_install(opts)?;
    Ok(())
}

fn clone_and_build_project() -> Result<(), anyhow::Error> {
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
            "-DCMAKE_INSTALL_PREFIX=/tmp/rust-seL4",
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

fn rust_sel4_install(opts: &mut InstallOptions) -> Result<(), anyhow::Error> {
    let mut cmd = Command::new("rustup");
    let mut args: Vec<String> = vec![
        "run".into(),
        "nightly-2024-08-01".into(),
        "cargo".into(),
        "install".into(),
        "--git".into(),
        "https://github.com/reL4team2/rust-sel4.git".into(),
        "--rev".into(),
        "642b58d807c5e5fc22f0c15d1467d6bec328faa9".into(),
        "--root".into(),
        "/tmp/rust-sel4".into(),
        "sel4-kernel-loader-add-payload".into(),
    ];

    cmd.env_remove("RUSTUP_TOOLCHAIN").env_remove("CARGO").args(&args).status().expect("failed install rust-sel4");
    
    Ok(())
}