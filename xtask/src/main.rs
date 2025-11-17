#[macro_use]
extern crate duct;

mod cmake;
mod install;
mod kernel;
mod run;

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
enum Command {
    /// Execute cmake in the $PROJECT_DIR/target/build and build project
    Build(kernel::BuildOptions),
    /// Install Kernel to $PROJECT_DIR/target/kernel-install
    Install(kernel::BuildOptions),
    /// Run sel4-tests
    Run(kernel::BuildOptions),
    /// Clean Project
    Clean,
}

fn main() -> Result<(), anyhow::Error> {
    let opts = Command::parse();

    use Command::*;
    match opts {
        Build(opts) => kernel::build(&opts)?,
        Install(build_opts) => install::install(&build_opts)?,
        Run(run_opts) => run::run(&run_opts)?,
        Clean => {
            let xtask_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            cmd!("rm", "-rf", xtask_path.join("../target").to_str().unwrap()).run()?;
        }
    }

    Ok(())
}
