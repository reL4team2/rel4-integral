use std::{path::PathBuf, process::Command};

use anyhow::Ok;

pub(crate) fn sel4test_build(platform: &str, define: &str) -> Result<(), anyhow::Error> {
    let build_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../build");
    let build_dir_str = build_dir.to_str().unwrap();
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "mkdir -p {} && cd {} && ../../init-build.sh -DPLATFORM={} -DSIMULATION=TRUE {} && ninja clean",
            build_dir_str, build_dir_str, platform, define
        ))
        .status()
        .expect("failed to execute cmake command");

    assert!(status.success());

    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("cd {} && ninja", build_dir_str))
        .status()
        .expect("failed to execute cmake command");

    assert!(status.success());

    Ok(())
}
