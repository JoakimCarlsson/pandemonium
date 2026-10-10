//! Process builders and portable pipes on a machine.

use crate::Host;
use crate::remote::Connection;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// An owned writable process pipe.
pub type Input = Box<dyn Write + Send>;
/// An owned readable process pipe.
pub type Reader = Box<dyn Read + Send>;
/// The completion of a process with captured output.
pub use std::process::Output;

/// How a child's standard stream is connected.
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub enum Stdio {
    /// Inherit the local standard stream.
    #[default]
    Inherit,
    /// Connect an owned pipe.
    Piped,
    /// Connect the null device.
    Null,
}
impl Stdio {
    /// Connects an owned pipe.
    pub fn piped() -> Self {
        Self::Piped
    }
    /// Connects the null device.
    pub fn null() -> Self {
        Self::Null
    }
    /// Converts a stream mode for local process creation.
    fn local(self) -> std::process::Stdio {
        match self {
            Self::Inherit => std::process::Stdio::inherit(),
            Self::Piped => std::process::Stdio::piped(),
            Self::Null => std::process::Stdio::null(),
        }
    }
}

/// Serializable process parameters, independent of shell quoting.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Spec {
    /// The executable path or name.
    pub program: String,
    /// Each argument, sent separately.
    pub args: Vec<String>,
    /// The child's directory.
    pub cwd: Option<PathBuf>,
    /// Extra environment variables.
    pub env: Vec<(String, String)>,
    /// Standard input mode.
    pub stdin: Stdio,
    /// Standard output mode.
    pub stdout: Stdio,
    /// Standard error mode.
    pub stderr: Stdio,
    /// An optional Unix process group.
    pub group: Option<i32>,
}

/// A command builder bound to its execution machine.
pub struct Command {
    /// The execution machine.
    host: Host,
    /// The transport representation.
    spec: Spec,
    /// The local builder, retaining native argument encodings.
    local: std::process::Command,
}
impl Command {
    /// Creates a builder on `host`.
    pub(crate) fn on(host: Host, program: impl AsRef<OsStr>) -> Self {
        Self {
            local: std::process::Command::new(&program),
            host,
            spec: Spec {
                program: program.as_ref().to_string_lossy().into_owned(),
                args: Vec::new(),
                cwd: None,
                env: Vec::new(),
                stdin: Stdio::Inherit,
                stdout: Stdio::Inherit,
                stderr: Stdio::Inherit,
                group: None,
            },
        }
    }
    /// The executable named by the builder.
    pub fn get_program(&self) -> &OsStr {
        self.local.get_program()
    }
    /// The arguments named by the builder.
    pub fn get_args(&self) -> std::process::CommandArgs<'_> {
        self.local.get_args()
    }
    /// The environment overrides named by the builder.
    pub fn get_envs(&self) -> std::process::CommandEnvs<'_> {
        self.local.get_envs()
    }
    /// Appends one argument.
    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.local.arg(&arg);
        self.spec
            .args
            .push(arg.as_ref().to_string_lossy().into_owned());
        self
    }
    /// Appends separately encoded arguments.
    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
        self
    }
    /// Sets the directory on the execution machine.
    pub fn current_dir(&mut self, path: impl AsRef<Path>) -> &mut Self {
        self.local.current_dir(&path);
        self.spec.cwd = Some(path.as_ref().to_path_buf());
        self
    }
    /// Sets an environment variable.
    pub fn env(&mut self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.local.env(&name, &value);
        self.spec.env.push((
            name.as_ref().to_string_lossy().into_owned(),
            value.as_ref().to_string_lossy().into_owned(),
        ));
        self
    }
    /// Sets several environment variables.
    pub fn envs<I, K, V>(&mut self, env: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (name, value) in env {
            self.env(name, value);
        }
        self
    }
    /// Sets standard input mode.
    pub fn stdin(&mut self, mode: Stdio) -> &mut Self {
        self.local.stdin(mode.local());
        self.spec.stdin = mode;
        self
    }
    /// Sets standard output mode.
    pub fn stdout(&mut self, mode: Stdio) -> &mut Self {
        self.local.stdout(mode.local());
        self.spec.stdout = mode;
        self
    }
    /// Sets standard error mode.
    pub fn stderr(&mut self, mode: Stdio) -> &mut Self {
        self.local.stderr(mode.local());
        self.spec.stderr = mode;
        self
    }
    /// Places the process in a Unix process group.
    #[cfg(unix)]
    pub fn process_group(&mut self, group: i32) -> &mut Self {
        use std::os::unix::process::CommandExt;
        self.local.process_group(group);
        self.spec.group = Some(group);
        self
    }
    /// Starts the child and takes ownership of its pipes.
    pub fn spawn(&mut self) -> io::Result<Child> {
        match &self.host.0 {
            None => Child::local(self.local.spawn()?),
            Some(remote) => remote.spawn(&self.spec),
        }
    }
    /// Captures standard output and error until the child exits.
    pub fn output(&mut self) -> io::Result<Output> {
        self.stdout(Stdio::Piped).stderr(Stdio::Piped);
        if matches!(self.spec.stdin, Stdio::Inherit) {
            self.stdin(Stdio::Null);
        }
        self.spawn()?.wait_with_output()
    }
    /// Waits for a command without capturing its streams.
    pub fn status(&mut self) -> io::Result<std::process::ExitStatus> {
        self.spawn()?.wait()
    }
    /// Builds a native process on the endpoint.
    pub(crate) fn from_spec(spec: &Spec) -> Self {
        let mut command = Host::local().command(&spec.program);
        command.args(&spec.args);
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        command.envs(spec.env.iter().map(|(key, value)| (key, value)));
        command
            .stdin(endpoint_stdio(spec.stdin))
            .stdout(endpoint_stdio(spec.stdout))
            .stderr(endpoint_stdio(spec.stderr));
        #[cfg(unix)]
        if let Some(group) = spec.group {
            command.process_group(group);
        }
        command
    }
}

/// An owned child, with machine independent standard pipes.
pub struct Child {
    /// Bytes sent to the child.
    pub stdin: Option<Input>,
    /// Bytes written to standard output.
    pub stdout: Option<Reader>,
    /// Bytes written to standard error.
    pub stderr: Option<Reader>,
    /// The process control endpoint.
    pub(crate) process: Process,
}

/// Local process control or a channel on an SSH connection.
pub(crate) enum Process {
    /// A native child.
    Local(std::process::Child),
    /// A remote process tied to a single connection generation.
    Remote(ArcProcess),
}
/// A remote process channel and the connection that owns it.
pub(crate) struct ArcProcess {
    /// The owning transport.
    pub connection: std::sync::Arc<Connection>,
    /// The process channel.
    pub channel: u32,
}
impl Child {
    /// Takes native pipes out of a local child.
    fn local(mut process: std::process::Child) -> io::Result<Self> {
        Ok(Self {
            stdin: process.stdin.take().map(|pipe| Box::new(pipe) as Input),
            stdout: process.stdout.take().map(|pipe| Box::new(pipe) as Reader),
            stderr: process.stderr.take().map(|pipe| Box::new(pipe) as Reader),
            process: Process::Local(process),
        })
    }
    /// The local pid, or the channel id for a remote child.
    pub fn id(&self) -> u32 {
        match &self.process {
            Process::Local(child) => child.id(),
            Process::Remote(remote) => remote.channel,
        }
    }
    /// Waits for the child's exit.
    pub fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        match &mut self.process {
            Process::Local(child) => child.wait(),
            Process::Remote(remote) => loop {
                if let Some(status) = remote.try_wait()? {
                    return Ok(status);
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            },
        }
    }
    /// Reads an exit status without waiting for the child to exit.
    pub fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        match &mut self.process {
            Process::Local(child) => child.try_wait(),
            Process::Remote(remote) => remote.try_wait(),
        }
    }
    /// Kills the child on its execution machine.
    pub fn kill(&mut self) -> io::Result<()> {
        match &mut self.process {
            Process::Local(child) => child.kill(),
            Process::Remote(remote) => remote
                .connection
                .request("kill", serde_json::json!({"channel":remote.channel}))
                .map(drop),
        }
    }
    /// Drains both output streams concurrently before waiting for exit.
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        drop(self.stdin.take());
        let stdout = self.stdout.take();
        let stderr = self.stderr.take();
        let error = std::thread::spawn(move || drain(stderr));
        let stdout = drain(stdout)?;
        let stderr = error
            .join()
            .map_err(|_| io::Error::other("process reader panicked"))??;
        Ok(Output {
            status: self.wait()?,
            stdout,
            stderr,
        })
    }
}

impl ArcProcess {
    /// Polls exit on the remote endpoint.
    fn try_wait(&self) -> io::Result<Option<std::process::ExitStatus>> {
        let status: Option<i32> = serde_json::from_value(
            self.connection
                .request("try_wait", serde_json::json!({"channel":self.channel}))?,
        )?;
        Ok(status.map(exit_status))
    }
}
/// Converts a portable exit code into the platform process status.
pub(crate) fn exit_status(code: i32) -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }
}
/// Reads all bytes from an optional process output.
fn drain(pipe: Option<Reader>) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_end(&mut bytes)?;
    }
    Ok(bytes)
}

impl Drop for ArcProcess {
    /// Releases endpoint process bookkeeping after the child handle is gone.
    fn drop(&mut self) {
        let _ = self
            .connection
            .request("release", serde_json::json!({"channel":self.channel}));
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsRawHandle for Child {
    /// Returns the native handle of a locally spawned process.
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
        match &self.process {
            Process::Local(child) => std::os::windows::io::AsRawHandle::as_raw_handle(child),
            Process::Remote(_) => std::ptr::null_mut(),
        }
    }
}

/// Keeps child streams from inheriting the endpoint's protocol pipes.
fn endpoint_stdio(mode: Stdio) -> Stdio {
    match mode {
        Stdio::Inherit => Stdio::Null,
        mode => mode,
    }
}
