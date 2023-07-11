use std::{
    ffi::OsStr,
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

pub fn show_graph<'a, Os, Fs>(format: Option<String>, object_names: Os, files: Fs) -> Result<()>
where
    Os: IntoIterator<Item = &'a str> + Clone,
    Fs: IntoIterator<Item = &'a OsStr> + Clone,
{
    let merge_base = {
        let mut output = stdout_lines(EasyCommand::new_with("git", |cmd| {
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
    let format = format
        .map(Ok)
        .or_else(|| {
            git_config("glimpse.pretty")
                .map(|configged| {
                    if configged.is_some() {
                        log::trace!(
                            "no format specified, using format from `glimpse.pretty` config: \
                            {configged:?}"
                        );
                    } else {
                        log::trace!(
                            "no format specified, no format found in `glimpse.pretty` config"
                        );
                    }
                    configged
                })
                .transpose()
        })
        .transpose()?;
    EasyCommand::new_with("git", |cmd| {
        cmd.args(["log", "--graph", "--decorate"]);
        if let Some(format) = format {
            cmd.arg(format!("--format={format}"));
        }
        cmd.arg("--ancestry-path")
            .arg(format!("^{merge_base}^@"))
            .args(object_names.clone().into_iter())
            .arg("--") // Make it unambiguous that we're specifying branches first
            .args(files)
    })
    .spawn_and_wait()
    .map_err(Into::into)
    .map_err(Error::other)
    .and_then(Error::from_status)
}

pub fn list_branches_cmd(config: impl FnOnce(&mut Command) -> &mut Command) -> EasyCommand {
    EasyCommand::new_with("git", |cmd| {
        config(cmd.args(["branch", "--list", "--format=%(refname:short)"]))
    })
}

pub fn stdout_lines(mut cmd: EasyCommand) -> Result<Vec<String>> {
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

pub fn git_config(path: &str) -> Result<Option<String>> {
    let mut cmd = EasyCommand::new_with("git", |cmd| cmd.arg("config").arg(path));
    let output = cmd.output().map_err(Into::into).map_err(Error::other)?;
    let Output {
        stdout,
        stderr,
        status,
    } = output;

    match status.code() {
        Some(0) => (),
        Some(1) => return Ok(None),
        _ => {
            io::copy(&mut Cursor::new(stderr), &mut io::stderr()).unwrap();
            return Err(Error::from_status(status).unwrap_err());
        }
    };

    let stdout = String::from_utf8(stdout)
        .context("`stdout` was not UTF-8 (!?)")
        .map_err(Error::other)?;

    let mut lines = stdout.lines().map(|line| line.trim().to_owned());
    log::trace!("`stdout` of {cmd}: {lines:?}");

    let first_line = lines.next();
    assert!(
        lines.next().is_none(),
        "{cmd} returned more than a single line of output"
    );
    Ok(first_line)
}
