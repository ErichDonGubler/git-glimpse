use std::{ffi::OsString, process::exit};

use clap::Parser;
use git_aliases::{init_logger, prettylog};

#[derive(Debug, Parser)]
struct Args {
    args: Vec<OsString>,
}

fn main() -> anyhow::Result<()> {
    init_logger();
    let Args { args } = Args::parse();
    exit(prettylog(|cmd| cmd.args(args))?.code().unwrap_or(255))
}
