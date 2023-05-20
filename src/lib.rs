use std::{
    io::{self, Cursor},
    process::{exit, Command, ExitStatus, Output},
};

use anyhow::{anyhow, Context};
use ezcmd::{EasyCommand, ExecuteError, RunErrorKind};

pub fn run<F>(f: F)
where
    F: FnOnce() -> Result<()>,
{
    init_logger();
    match f() {
        Ok(()) => (),
        Err(e) => match e {
            Error::SubprocessFailedWithExplanation { code } => exit(code.unwrap_or(255)),
            Error::Other { source } => {
                log::error!("{source:?}");
                exit(254);
            }
        },
    }
}

fn init_logger() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init()
}

#[derive(Debug)]
pub enum Error {
    SubprocessFailedWithExplanation { code: Option<i32> },
    Other { source: anyhow::Error },
}

impl Error {
    fn other(source: anyhow::Error) -> Self {
        Self::Other { source }
    }

    fn from_status(status: ExitStatus) -> Result<()> {
        match status.code() {
            Some(0) => Ok(()),
            Some(_) | None => Err(Self::SubprocessFailedWithExplanation {
                code: status.code(),
            }),
        }
    }

    fn from_run(source: ExecuteError<RunErrorKind>) -> Self {
        let ExecuteError { source, .. } = source;
        // TODO: Not super happy about basically cloning this.
        match source {
            RunErrorKind::SpawnAndWait(e) => Self::other(e.into()),
            RunErrorKind::UnsuccessfulExitCode { code } => {
                Self::SubprocessFailedWithExplanation { code }
            }
        }
    }
}

impl From<anyhow::Error> for Error {
    fn from(value: anyhow::Error) -> Self {
        Self::other(value)
    }
}

impl From<ExecuteError<RunErrorKind>> for Error {
    fn from(value: ExecuteError<RunErrorKind>) -> Self {
        Self::from_run(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn show_graph<'a, I>(object_names: I) -> Result<()>
where
    I: IntoIterator<Item = &'a str> + Clone,
{
    let merge_base = {
        let mut output = output(EasyCommand::new_with("git", |cmd| {
            cmd.args(["merge-base", "--octopus"])
                .args(object_names.clone().into_iter())
        }))?;
        if output.len() != 1 {
            return Err(Error::other(anyhow!(
                "expected a single line of output, but got {}; \
                output: {output:#?}",
                output.len()
            )));
        }
        output.pop().unwrap()
    };
    prettylog(|cmd| {
        cmd.arg("--ancestry-path")
            .arg(format!("^{merge_base}^@"))
            .args(object_names.clone().into_iter())
    })
}

pub fn list_branches_cmd(config: impl FnOnce(&mut Command) -> &mut Command) -> EasyCommand {
    EasyCommand::new_with("git", |cmd| {
        config(cmd.args(["branch", "--list", "--format=%(refname:short)"]))
    })
}

pub fn output(mut cmd: EasyCommand) -> Result<Vec<String>> {
    let output = cmd.output().map_err(Into::into).map_err(Error::other)?;
    let Output {
        stdout,
        stderr,
        status,
    } = output;

    let status_res = Error::from_status(status);
    if let Err(_) = &status_res {
        io::copy(&mut Cursor::new(stderr), &mut io::stderr()).unwrap();
    }
    status_res?;

    let stdout = String::from_utf8(stdout)
        .context("`stdout` was not UTF-8 (!?)")
        .map_err(Error::other)?;
    Ok(stdout.lines().map(|line| line.trim().to_owned()).collect())
}

pub fn prettylog(config: impl FnOnce(&mut Command) -> &mut Command) -> Result<()> {
    EasyCommand::new_with("git", |cmd| {
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
    .spawn_and_wait()
    .map_err(Into::into)
    .map_err(Error::other)
    .and_then(Error::from_status)
}
