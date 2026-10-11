//! The container engine's command line: the only code that builds engine commands.

use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc;

/// How many trailing lines of a failed command are kept for its error message.
const TAIL: usize = 12;

/// A local container engine, `docker` unless configured otherwise.
#[derive(Clone, Debug)]
pub struct Engine {
    /// The program that speaks the Docker command line.
    program: String,
}

/// What the engine reports about itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Info {
    /// The server's version.
    pub version: String,
    /// The operating system containers run, such as `linux`.
    pub os: String,
    /// The architecture containers run, such as `x86_64`.
    pub architecture: String,
}

/// What the engine reports about one container.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inspected {
    /// The engine's own identifier.
    pub id: String,
    /// The container name.
    pub name: String,
    /// Whether it is running.
    pub running: bool,
    /// Its labels.
    pub labels: BTreeMap<String, String>,
    /// The environment it was created with, including the image's own.
    pub env: BTreeMap<String, String>,
    /// The user it runs as when none is asked for; empty means root.
    pub user: String,
}

impl Engine {
    /// An engine reached through `program`.
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
        }
    }

    /// The program name, shown in messages.
    pub fn program(&self) -> &str {
        &self.program
    }

    /// Whether the program speaks the Docker command line with BuildKit's plain progress output.
    fn plain_progress(&self) -> bool {
        std::path::Path::new(&self.program)
            .file_name()
            .is_some_and(|name| name == "docker")
    }

    /// A command for the engine with no arguments yet.
    pub fn command(&self) -> Command {
        Command::new(&self.program)
    }

    /// A `docker exec` into `container` as `user`, in `workdir`, with `env`, ready for its program.
    pub fn exec(
        &self,
        container: &str,
        user: Option<&str>,
        workdir: &str,
        env: &[(String, String)],
        interactive: bool,
    ) -> Command {
        let mut command = self.command();
        command.arg("exec");
        if interactive {
            command.arg("-i");
        }
        if let Some(user) = user {
            command.args(["-u", user]);
        }
        command.args(["-w", workdir]);
        for (name, value) in env {
            command.arg("-e").arg(format!("{name}={value}"));
        }
        command.arg(container);
        command
    }

    /// Asks the engine about itself, which proves it is installed and running.
    pub fn info(&self) -> io::Result<Info> {
        let text = self.output(&[
            "info".to_owned(),
            "--format".to_owned(),
            "{{.ServerVersion}}|{{.OSType}}|{{.Architecture}}".to_owned(),
        ])?;
        let mut parts = text.trim().split('|');
        Ok(Info {
            version: parts.next().unwrap_or_default().to_owned(),
            os: parts.next().unwrap_or_default().to_owned(),
            architecture: parts.next().unwrap_or_default().to_owned(),
        })
    }

    /// Runs `args` to completion and returns its standard output.
    pub fn output(&self, args: &[String]) -> io::Result<String> {
        let output = self
            .command()
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| launch_error(self.command().get_program(), error))?;
        if !output.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Whether an image is already present locally.
    pub fn has_image(&self, image: &str) -> bool {
        self.output(&[
            "image".to_owned(),
            "inspect".to_owned(),
            "--format".to_owned(),
            "{{.Id}}".to_owned(),
            image.to_owned(),
        ])
        .is_ok()
    }

    /// The value of one label of a local image, when it has one.
    pub fn image_label(&self, image: &str, label: &str) -> Option<String> {
        let text = self
            .output(&[
                "image".to_owned(),
                "inspect".to_owned(),
                "--format".to_owned(),
                format!("{{{{index .Config.Labels \"{label}\"}}}}"),
                image.to_owned(),
            ])
            .ok()?;
        let text = text.trim();
        (!text.is_empty() && text != "<no value>").then(|| text.to_owned())
    }

    /// Reads one container by name or identifier, or none when it does not exist.
    pub fn inspect(&self, name: &str) -> io::Result<Option<Inspected>> {
        let text = match self.output(&[
            "container".to_owned(),
            "inspect".to_owned(),
            "--format".to_owned(),
            "{{json .}}".to_owned(),
            name.to_owned(),
        ]) {
            Ok(text) => text,
            Err(error) if error.to_string().to_lowercase().contains("no such") => return Ok(None),
            Err(error) => return Err(error),
        };
        let value: Value = serde_json::from_str(text.trim()).map_err(io::Error::other)?;
        let config = &value["Config"];
        let environment = config["Env"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter_map(|entry| entry.split_once('='))
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        let labels = config["Labels"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(name, value)| Some((name.clone(), value.as_str()?.to_owned())))
            .collect();
        Ok(Some(Inspected {
            id: value["Id"].as_str().unwrap_or_default().to_owned(),
            name: value["Name"]
                .as_str()
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_owned(),
            running: value["State"]["Running"].as_bool().unwrap_or(false),
            labels,
            env: environment,
            user: config["User"].as_str().unwrap_or_default().to_owned(),
        }))
    }

    /// The names of the containers carrying `label`, running or not.
    pub fn containers_with(&self, label: &str) -> io::Result<Vec<String>> {
        let text = self.output(&[
            "ps".to_owned(),
            "-a".to_owned(),
            "--filter".to_owned(),
            format!("label={label}"),
            "--format".to_owned(),
            "{{.Names}}".to_owned(),
        ])?;
        Ok(text
            .lines()
            .map(str::to_owned)
            .filter(|name| !name.is_empty())
            .collect())
    }

    /// The names of the volumes carrying `label`.
    pub fn volumes_with(&self, label: &str) -> io::Result<Vec<String>> {
        let text = self.output(&[
            "volume".to_owned(),
            "ls".to_owned(),
            "--filter".to_owned(),
            format!("label={label}"),
            "--format".to_owned(),
            "{{.Name}}".to_owned(),
        ])?;
        Ok(text
            .lines()
            .map(str::to_owned)
            .filter(|name| !name.is_empty())
            .collect())
    }

    /// The arguments of a Dockerfile build.
    pub fn build_arguments(
        &self,
        build: &crate::declaration::Build,
        tag: &str,
        labels: &[String],
    ) -> Vec<String> {
        let mut args = vec!["build".to_owned()];
        if self.plain_progress() {
            args.push("--progress=plain".to_owned());
        }
        args.extend([
            "-f".to_owned(),
            build.dockerfile.display().to_string(),
            "-t".to_owned(),
            tag.to_owned(),
        ]);
        for label in labels {
            args.extend(["--label".to_owned(), label.clone()]);
        }
        for (name, value) in &build.args {
            args.extend(["--build-arg".to_owned(), format!("{name}={value}")]);
        }
        if let Some(target) = &build.target {
            args.extend(["--target".to_owned(), target.clone()]);
        }
        for image in &build.cache_from {
            args.extend(["--cache-from".to_owned(), image.clone()]);
        }
        args.extend(build.options.iter().cloned());
        args.push(build.context.display().to_string());
        args
    }
}

/// Forwards each line of `stream` to `sender` from its own thread.
fn pump(
    stream: impl Read + Send + 'static,
    sender: mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut bytes = Vec::new();
        while reader
            .read_until(b'\n', &mut bytes)
            .is_ok_and(|read| read > 0)
        {
            let text = String::from_utf8_lossy(&bytes);
            let text = text
                .trim_end_matches(['\n', '\r'])
                .rsplit('\r')
                .next()
                .unwrap_or_default()
                .to_owned();
            if sender.send(text).is_err() {
                break;
            }
            bytes.clear();
        }
    })
}

/// Explains why `program` could not be started at all.
fn launch_error(program: &std::ffi::OsStr, error: io::Error) -> io::Error {
    if error.kind() == io::ErrorKind::NotFound {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{} is not installed or not on the PATH",
                program.to_string_lossy()
            ),
        )
    } else {
        error
    }
}

/// Runs `command`, handing every line of both output streams to `line` as it arrives.
///
/// A failing command is an error carrying the last lines it printed.
pub fn stream(
    mut command: Command,
    source: &str,
    line: &mut dyn FnMut(&str, &str),
) -> io::Result<()> {
    let program = command.get_program().to_owned();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| launch_error(&program, error))?;
    let (sender, lines) = mpsc::channel();
    let readers = [
        child
            .stdout
            .take()
            .map(|stream| pump(stream, sender.clone())),
        child
            .stderr
            .take()
            .map(|stream| pump(stream, sender.clone())),
    ];
    drop(sender);
    let mut tail = std::collections::VecDeque::new();
    for text in lines {
        line(source, &text);
        if tail.len() == TAIL {
            tail.pop_front();
        }
        tail.push_back(text);
    }
    for reader in readers.into_iter().flatten() {
        let _ = reader.join();
    }
    let status = child.wait()?;
    if status.success() {
        return Ok(());
    }
    let code = status.code().map_or_else(
        || "a signal".to_owned(),
        |code| format!("exit status {code}"),
    );
    Err(io::Error::other(format!(
        "{source} failed with {code}\n{}",
        tail.into_iter().collect::<Vec<_>>().join("\n")
    )))
}
