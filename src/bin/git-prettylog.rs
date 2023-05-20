use std::ffi::OsString;

use clap::Parser;
use git_aliases::{prettylog, run};

/// A wrapper around `git log --graph --decorate --format…`.
#[derive(Debug, Parser)]
struct Args {
    #[clap(allow_hyphen_values = true, trailing_var_arg = true)]
    args: Vec<OsString>,
}

fn main() {
    run(|| {
        let Args { args } = Args::parse();
        prettylog(|cmd| cmd.args(args))
    })
}
