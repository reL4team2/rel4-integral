use std::{path::PathBuf, process::Command};

use crate::kernel::{parse_cmake_defines, BuildOptions};

/// Build the project
pub fn install(opts: &BuildOptions) -> Result<(), anyhow::Error> {
    let current_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let proj_dir = current_dir.join("../");
    let target_dir = proj_dir.join("target");
    let kernel_dir = proj_dir.join("../kernel");
    let kbuild_dir = target_dir.join("kernel-build");
    let install_dir = target_dir.join("kernel-install");
    super::kernel::cargo("build", proj_dir.to_str().unwrap(), opts)?;

    let defines = parse_cmake_defines(opts)?;

    let preload_cmake = match opts.platform.as_str() {
        "spike" => "kernel-settings-riscv64.cmake",
        "qemu-arm-virt" => "kernel-settings-aarch64.cmake",
        _ => unreachable!(),
    };

    println!("preload cmake file {}", preload_cmake);

    // TODO: run rm only when platform changing
    cmd!("rm", "-rf", kbuild_dir.to_str().unwrap()).run()?;

    // Run CMake command to constructor build directory
    let mut cmake_cmd = Command::new("cmake");
    cmake_cmd
        .args(["-C", kernel_dir.join(preload_cmake).to_str().unwrap()])
        .arg(format!(
            "-DCMAKE_INSTALL_PREFIX={}",
            install_dir.to_str().unwrap()
        ))
        .args(defines)
        .args(["-S", kernel_dir.to_str().unwrap()])
        .args(["-B", kbuild_dir.to_str().unwrap()])
        .args(["-G", "Ninja"]);

    cmake_cmd.status()?;

    // Run ninja command to build and install sel4 kernel
    cmd!("ninja", "-C", kbuild_dir.to_str().unwrap(), "all").run()?;
    cmd!("ninja", "-C", kbuild_dir.to_str().unwrap(), "install").run()?;
    println!("Building complete, enjoy rel4!");
    Ok(())
}
