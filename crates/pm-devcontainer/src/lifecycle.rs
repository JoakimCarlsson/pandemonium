//! Running the declaration's lifecycle commands, in their three forms.

use crate::declaration::{Command, Single};
use crate::engine::{self, Engine};
use std::path::Path;
use std::process::Command as Process;

/// Receives each output line of a command, from whichever thread produced it.
pub type Log<'a> = &'a (dyn Fn(&str, &str) + Sync);

/// Where inside a container commands run.
#[derive(Clone, Debug)]
pub struct Target {
    /// The engine reaching the container.
    pub engine: Engine,
    /// The container name.
    pub container: String,
    /// The user commands run as; none leaves the container's own.
    pub user: Option<String>,
    /// The working directory.
    pub workdir: String,
    /// The variables commands run with.
    pub env: Vec<(String, String)>,
}

impl Target {
    /// The process running `single` inside the container.
    fn process(&self, single: &Single) -> Process {
        let mut process = self.engine.exec(
            &self.container,
            self.user.as_deref(),
            &self.workdir,
            &self.env,
            false,
        );
        match single {
            Single::Shell(text) => process.args(["sh", "-c", text]),
            Single::Exec(arguments) => process.args(arguments),
        };
        process
    }
}

/// Runs `command` inside the container, waiting for every part of it.
pub fn in_container(
    target: &Target,
    name: &str,
    command: &Command,
    log: Log<'_>,
) -> Result<(), String> {
    run(name, command, log, &|single| target.process(single))
}

/// Runs `command` on the desktop, in `directory`.
pub fn on_desktop(
    directory: &Path,
    name: &str,
    command: &Command,
    log: Log<'_>,
) -> Result<(), String> {
    run(name, command, log, &|single| {
        let mut process = match single {
            Single::Shell(text) => {
                let mut process = Process::new("sh");
                process.args(["-c", text]);
                process
            }
            Single::Exec(arguments) => {
                let mut process = Process::new(&arguments[0]);
                process.args(&arguments[1..]);
                process
            }
        };
        process.current_dir(directory);
        process
    })
}

/// Runs the parts of `command` through `process`, sequentially or all at once.
fn run(
    name: &str,
    command: &Command,
    log: Log<'_>,
    process: &(dyn Fn(&Single) -> Process + Sync),
) -> Result<(), String> {
    match command {
        Command::Single(single) => one(name, single, log, process),
        Command::Parallel(parts) => {
            let failures = std::thread::scope(|scope| {
                parts
                    .iter()
                    .map(|(part, single)| {
                        let source = format!("{name}:{part}");
                        scope.spawn(move || one(&source, single, log, process))
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .filter_map(|handle| {
                        handle
                            .join()
                            .unwrap_or_else(|_| Err("panicked".to_owned()))
                            .err()
                    })
                    .collect::<Vec<_>>()
            });
            if failures.is_empty() {
                Ok(())
            } else {
                Err(failures.join("\n"))
            }
        }
    }
}

/// Runs one program, streaming its output under `source`.
fn one(
    source: &str,
    single: &Single,
    log: Log<'_>,
    process: &(dyn Fn(&Single) -> Process + Sync),
) -> Result<(), String> {
    engine::stream(process(single), source, &mut |source, line| {
        log(source, line)
    })
    .map_err(|error| error.to_string())
}
