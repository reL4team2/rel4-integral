mod kernel;
mod cmake;
mod install;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Options {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Parser)]
struct ReleaseOptions {
    /// Do not run cargo release on eBPF directory
    #[clap(short = 'i', long)]
    ignore_ebpf: bool,
    /// Do not run cargo release on workspace packages
    #[clap(short = 'I', long)]
    ignore_ws: bool,
    /// Arguments to pass to cargo release
    args: Vec<String>,
}

#[derive(Debug, Parser)]
enum Command {
    // Build eBPF and userland code
    Build(kernel::BuildOptions),
    Install(install::InstallOptions),
}

fn main() -> Result<(), anyhow::Error> {
    let opts = Options::parse();

    use Command::*;
    match opts.command {
        Build(opts) => kernel::build(&opts)?,
        Install(mut opts) => install::install(&mut opts)?,
    }

    Ok(())
}