use std::{path::PathBuf, process::Command};

use anyhow::Ok;

pub(crate) fn sel4test_build(platform: &str, defines: &Vec<String>, dir: &str) -> Result<(), anyhow::Error> {
    let build_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target").join(dir);
    let build_dir_str = build_dir.to_str().unwrap();
    cmd!("rm", "-rf", build_dir_str).run()?;
    cmd!("mkdir", "-p", build_dir_str).run()?;
    Command::new("../../../init-build.sh")
        .current_dir(build_dir_str)
        .arg(format!("-DPLATFORM={}", platform))
        .arg(String::from("-DSIMULATION=true"))
        .args(defines)
        .status()?;
    cmd!("ninja").dir(build_dir_str).run()?;

    Ok(())
}

pub fn get_build_dir(is_benchmark: bool) -> &'static str {
    if is_benchmark {
        "sel4-bench"
    } else {
        "sel4-test"
    }
}
