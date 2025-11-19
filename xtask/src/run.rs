use std::{path::PathBuf, process::Command};

use crate::kernel::{cargo, parse_cmake_defines, BuildOptions};

pub fn run(opts: &BuildOptions) -> Result<(), anyhow::Error> {
    let current_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let proj_dir = PathBuf::from(&current_dir).join("..");
    let kernel_dir = PathBuf::from(&current_dir).join("../kernel");
    cargo("build", kernel_dir.to_str().unwrap(), opts)?;

    let defines = parse_cmake_defines(opts)?;
    crate::cmake::sel4test_build(
        &opts.platform,
        &defines,
        super::cmake::get_build_dir(opts.benchmark),
    )?;
    let mut cmd = Command::new("./simulate");
    if opts.num_nodes > 1 {
        cmd.args(["--cpu-num", "4"]);
    }
    cmd.current_dir(
        proj_dir
            .join("target")
            .join(super::cmake::get_build_dir(opts.benchmark))
            .to_str()
            .unwrap(),
    )
    .status()?;
    println!("Building complete, enjoy rel4!");
    Ok(())
}
