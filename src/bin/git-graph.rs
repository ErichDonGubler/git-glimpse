use std::process::exit;

use clap::Parser;
use git_aliases::{init_logger, show_graph};

#[derive(Debug, Parser)]
struct Args {
    /// The set of branches to generate a graph for. If none are specified, then all local branches
    /// are used.
    branches: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    init_logger();
    let Args { branches } = Args::parse();
    let branches = if branches.is_empty() {
        None
    } else {
        Some(&*branches)
    };
    let status = show_graph(branches)?;
    exit(status.code().unwrap_or(255))
}
