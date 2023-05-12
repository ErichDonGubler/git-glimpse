use clap::Parser;
use git_aliases::{init_logger, list_branches_cmd};

#[derive(Debug, Parser)]
struct Args {}

fn main() -> anyhow::Result<()> {
    init_logger();
    let Args {} = Args::parse();
    Ok(list_branches_cmd().run()?)
}
