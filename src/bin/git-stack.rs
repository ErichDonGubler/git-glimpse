use std::process::exit;

use clap::Parser;
use git_aliases::{init_logger, show_graph};

#[derive(Debug, Parser)]
struct Args {
    args: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    init_logger();
    let Args { args } = Args::parse();
    // TODO: use something nicer than just `main`, maybe?
    let status = show_graph(Some(
        &*["main", "HEAD"]
            .into_iter()
            .map(ToOwned::to_owned)
            .chain(args)
            .collect::<Vec<_>>(),
    ))?;
    exit(status.code().unwrap_or(255))
}
