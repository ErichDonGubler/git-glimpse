use std::process::Command;

use clap::Parser;
use ezcmd::EasyCommand;
use git_aliases::{list_branches_cmd, run, show_graph, stdout_lines};

/// Show a minimal graph of Git commits.
///
/// When no arguments are specified, this commands runs as if the `local --upstreams --pushes` was
/// invoked.
#[derive(Debug, Parser)]
struct Args {
    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}

#[derive(Debug, Parser)]
enum Subcommand {
    Stack {
        #[clap(long, short)]
        base: Option<String>,
        #[clap(flatten)]
        config: PresetConfig,
    },
    /// Display local branches and, optionally, their upstreams.
    Local {
        #[clap(flatten)]
        config: PresetConfig,
    },
    Select {
        /// Additional branches to include.
        branches: Vec<String>,
    },
}

#[derive(Debug, Parser)]
struct PresetConfig {
    /// Include all `@{upstream}` counterparts.
    #[clap(long = "upstreams", short = 'u')]
    select_upstreams: bool,
    /// Include all `@{push}` counterparts.
    #[clap(long = "pushes", short = 'p')]
    select_pushes: bool,
    /// Include the last tag that contains `HEAD`.
    #[clap(long = "last-tag")]
    select_last_tag: bool,
}

fn main() {
    run(|| {
        let Args { subcommand } = Args::parse();
        let subcommand = subcommand.unwrap_or_else(|| Subcommand::Local {
            config: PresetConfig {
                select_upstreams: true,
                select_pushes: true,
                select_last_tag: false,
            },
        });
        let branches = |sel_config,
                        cmd_config: &dyn Fn(&mut Command) -> &mut Command|
         -> git_aliases::Result<_> {
            let PresetConfig {
                select_upstreams,
                select_pushes,
                select_last_tag,
            } = sel_config;
            let head_is_detached = stdout_lines(EasyCommand::new_with("git", |cmd| {
                cmd.args(["branch", "--show-current"])
            }))?
            .is_empty();
            log::trace!("`HEAD` is detached: {head_is_detached:?}");

            let mut format = "--format=".to_owned();
            if head_is_detached {
                format.push_str("%(if)%(HEAD)%(then)HEAD%(else)");
            }
            format.push_str("%(refname:short)");
            let mut include_in_format = |prop_name: &str| {
                format += &format!("%(if)%({prop_name})%(then)\n%({prop_name}:short)%(end)");
            };
            if select_upstreams {
                include_in_format("upstream");
            }
            if select_pushes {
                include_in_format("push");
            }
            if head_is_detached {
                format.push_str("%(end)");
            }

            let mut branches = stdout_lines(list_branches_cmd(|cmd| cmd_config(cmd.arg(format))))?;

            if select_last_tag {
                match stdout_lines(EasyCommand::new_with("git", |cmd| {
                    cmd.args(["rev-list", "--tags", "--max-count=1"])
                }))?
                .pop()
                {
                    Some(last_tag) => branches.push(last_tag),
                    None => log::warn!("last tag requested, but no last tag was found"),
                }
            }

            Ok(branches)
        };
        let branches = match subcommand {
            Subcommand::Stack { base, config } => branches(config, &|cmd| {
                cmd.args([base.as_deref().unwrap_or("main"), "HEAD"])
            })?,
            Subcommand::Local { config } => branches(config, &|cmd| cmd)?,
            Subcommand::Select { branches } => branches,
        };
        log::debug!("showing graph for branches {branches:?}");
        show_graph(branches.iter().map(|s| s.as_str()))
    })
}
