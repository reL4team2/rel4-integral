use std::{path::PathBuf, process::Command};

use anyhow::Ok;

pub(crate) fn sel4test_build(platform: &str, defines: &Vec<String>) -> Result<(), anyhow::Error> {
    let build_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/sel4-test");
    // let build_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../build");
    let build_dir_str = build_dir.to_str().unwrap();
    cmd!("rm", "-rf", build_dir_str).run()?;
    cmd!("mkdir", "-p", build_dir_str).run()?;
    println!("platform={}. defines={:?}", platform, defines);
    Command::new("../../../init-build.sh")
        .current_dir(build_dir_str)
        .arg(format!("-DPLATFORM={}", platform))
        .arg(format!("-DSIMULATION=true"))
        .args(defines)
        .status()?;
    cmd!("ninja").dir(build_dir_str).run()?;
    // let status = Command::new("sh")
    //     .arg("-c")
    //     .arg(format!(
    //         "mkdir -p {} && cd {} && ../../init-build.sh -DPLATFORM={} -DSIMULATION=TRUE {} && ninja clean",
    //         build_dir_str, build_dir_str, platform, defines.join(" ")
    //     ))
    //     .status()
    //     .expect("failed to execute cmake command");

    // assert!(status.success());

    // let status = Command::new("sh")
    //     .arg("-c")
    //     .arg(format!("cd {} && ninja", build_dir_str))
    //     .status()
    //     .expect("failed to execute cmake command");

    // assert!(status.success());

    Ok(())
}
