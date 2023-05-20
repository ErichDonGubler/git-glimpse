use std::ffi::OsString;

use clap::Parser;
use git_aliases::{list_branches_cmd, run};

/// A wrapper around `git branch --list` that outputs a branch name per line.
#[derive(Debug, Parser)]
struct Args {
    #[clap(allow_hyphen_values = true, trailing_var_arg = true)]
    args: Vec<OsString>,
}

fn main() {
    run(|| {
        let Args { args } = match Args::try_parse() {
            Ok(args) => args,
            Err(e) => e.exit(),
        };
        Ok(list_branches_cmd(|cmd| cmd.args(args)).run()?)
    })
}
