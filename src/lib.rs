use std::process::{Command, ExitStatus, Output};

use anyhow::{ensure, Context};
use ezcmd::EasyCommand;

pub fn init_logger() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init()
}

pub fn show_graph(branches: Option<&[String]>) -> anyhow::Result<ExitStatus> {
    let auto_listed;
    let tips = if let Some(branches) = branches {
        branches
    } else {
        auto_listed = list_branches()?;
        &*auto_listed
    };
    let merge_base = {
        let mut output = output(EasyCommand::new_with("git", |cmd| {
            cmd.args(["merge-base", "--octopus"])
                .args(tips.iter().map(|s| &*s))
        }))?;
        ensure!(output.len() == 1, "");
        let output = output.pop().unwrap();
        ensure!(!output.is_empty(), "");
        output
    };
    prettylog(|cmd| {
        cmd.arg("--ancestry-path")
            .arg(format!("^{merge_base}~"))
            .args(tips.iter())
    })
}

pub fn list_branches_cmd() -> EasyCommand {
    EasyCommand::new_with("git", |cmd| {
        cmd.args(["branch", "--format=%(refname:short)"])
    })
}

pub fn list_branches() -> anyhow::Result<Vec<String>> {
    output(list_branches_cmd())
}

fn output(mut cmd: EasyCommand) -> anyhow::Result<Vec<String>> {
    let output = cmd.output()?;
    let Output { stdout, .. } = output;
    // TODO: warn on `stderr` and non-success exit code
    let stdout = String::from_utf8(stdout).context("`stdout` was not UTF-8 (!?)")?;
    Ok(stdout.lines().map(|line| line.trim().to_owned()).collect())
}

pub fn prettylog(config: impl FnOnce(&mut Command) -> &mut Command) -> anyhow::Result<ExitStatus> {
    Ok(EasyCommand::new_with("git", |cmd| {
        config(cmd.args([
            "log",
            "--graph",
            "--decorate",
            "--format=format:\
            %C(dim white) ---%C(reset) %C(bold blue)%h%C(reset) %C(dim white)-\
            %C(reset)%C(auto)%d%C(reset)\n\
            %C(white)%s%C(reset) %C(dim white)%an%C(reset) %C(dim green)(%ar)%C(reset)",
        ]))
        .arg("--") // Make it unambiguous that we're specifying branches first
    })
    .spawn_and_wait()?)
}
