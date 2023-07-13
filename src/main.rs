use std::{ffi::OsString, process::Command};

use clap::Parser;
use ezcmd::EasyCommand;
use git_glimpse::{git_config, list_branches_cmd, run, show_graph, stdout_lines};

/// Show a minimal graph of Git commits for various use cases.
///
/// When no arguments are specified, this commands runs as if the `stack` command was invoked
/// with no arguments.
///
/// This binary has two optional points of Git configuration:
///
/// * `glimpse.base`: Sets the mainline branch. It is recommended that you use this only if
///   this command does not correctly detect your mainline branch out-of-the-box.
///
/// * `glimpse.pretty`: The fallback value for the `--format` argument of this command.
#[derive(Debug, Parser)]
struct Args {
    /// Set the `--pretty` argument for underlying Git CLI calls.
    #[clap(long, short)]
    format: Option<String>,
    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}

#[derive(Debug, Parser)]
enum Subcommand {
    /// Select the current "stack" of commits.
    ///
    /// A "stack" selection in this context includes the currently checked out branch and mainline
    /// branch. This is useful for day-to-day work, where you want only the commits relevant to
    /// what you're currently working on.
    Stack {
        #[clap(long, short)]
        base: Option<String>,
        #[clap(flatten)]
        config: PresetConfig,
        #[clap(flatten)]
        files: FileSelection,
    },
    /// Select all local Git branches.
    ///
    /// During typical work using Git, you may have several different "stacks" of work (see also
    /// the `stack` command). These tend to correspond to locally checked out branches. This
    /// command is useful for viewing all of them at the same time.
    ///
    /// If the set of branches you'd like to work with is significantly smaller than this set of
    /// branches, this command might be too noisy for you. You may want to consider using a Git
    /// alias for a `select` command invocation instead.
    Locals {
        #[clap(flatten)]
        config: PresetConfig,
        #[clap(flatten)]
        files: FileSelection,
    },
    /// Select a custom set of commit-ish refs.
    Select {
        /// Additional branches to include.
        branches: Vec<String>,
        #[clap(flatten)]
        files: FileSelection,
    },
}

#[derive(Debug, Parser)]
struct PresetConfig {
    /// Also include all `@{upstream}` counterparts to selected branches.
    #[clap(long = "upstreams", short = 'u')]
    select_upstreams: bool,
    /// Also select all `@{push}` counterparts to selected branches.
    #[clap(long = "pushes", short = 'p')]
    select_pushes: bool,
    /// Also select the last tag that contains `HEAD`.
    #[clap(long = "last-tag")]
    select_last_tag: bool,
}

#[derive(Debug, Parser)]
struct FileSelection {
    /// Files by which to filter history.
    #[clap(raw(true))]
    files: Vec<OsString>,
}

fn main() {
    run(|| {
        let Args { format, subcommand } = Args::parse();
        let subcommand = subcommand.unwrap_or_else(|| Subcommand::Stack {
            base: None,
            config: PresetConfig {
                select_upstreams: false,
                select_pushes: false,
                select_last_tag: false,
            },
            files: FileSelection { files: vec![] },
        });
        let current_branch = || {
            stdout_lines(EasyCommand::new_with("git", |cmd| {
                cmd.args(["branch", "--show-current"])
            }))
            .map(|mut lines| {
                let current_branch = lines.pop();
                log::trace!("current branch: {current_branch:?}");
                log::trace!("`HEAD` is detached: {:?}", current_branch.is_some());
                debug_assert!(lines.is_empty());
                current_branch
            })
        };
        let branches = |sel_config: &_,
                        cmd_config: &dyn Fn(&mut Command) -> &mut Command|
         -> git_glimpse::Result<_> {
            let PresetConfig {
                select_upstreams,
                select_pushes,
                select_last_tag,
            } = sel_config;
            let head_is_detached = current_branch()?.is_none();

            let mut format = "--format=".to_owned();
            if head_is_detached {
                format.push_str("%(if)%(HEAD)%(then)HEAD%(else)");
            }
            format.push_str("%(refname:short)");
            let mut include_in_format = |prop_name: &str| {
                format += &format!("%(if)%({prop_name})%(then)\n%({prop_name}:short)%(end)");
            };
            if *select_upstreams {
                include_in_format("upstream");
            }
            if *select_pushes {
                include_in_format("push");
            }
            if head_is_detached {
                format.push_str("%(end)");
            }

            let mut branches = stdout_lines(list_branches_cmd(|cmd| cmd_config(cmd.arg(format))))?;

            if *select_last_tag {
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
        let (branches, files) = match subcommand {
            Subcommand::Stack {
                base,
                config,
                files: FileSelection { files },
            } => {
                let specified_base = base
                    .map(Ok)
                    .or_else(|| git_config("glimpse.base").transpose())
                    .transpose()?;
                let base = specified_base.as_deref().unwrap_or_else(|| {
                    let default = "main";
                    log::debug!("no base branch specified in command line or configuration, falling back to {default:?}");
                    default
                });

                let branches = if let Some(current_branch) = current_branch()? {
                    let mut config = config;
                    if current_branch == base {
                        config.select_upstreams = true;
                    }
                    branches(&config, &|cmd| {
                        if base != current_branch {
                            cmd.arg(base);
                        }
                        cmd.arg(&current_branch)
                    })?
                } else {
                    let mut branches = branches(&config, &|cmd| cmd.arg(base))?;
                    branches.push("HEAD".to_owned());
                    branches
                };
                (branches, files)
            }
            Subcommand::Locals {
                config,
                files: FileSelection { files },
            } => (branches(&config, &|cmd| cmd)?, files),
            Subcommand::Select {
                branches,
                files: FileSelection { files },
            } => (branches, files),
        };
        log::debug!("showing graph for branches {branches:?}");
        show_graph(
            format,
            branches.iter().map(|s| s.as_str()),
            files.iter().map(|f| f.as_os_str()),
        )
    })
}
