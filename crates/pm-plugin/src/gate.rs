//! What an extension's code is allowed to have the host do for it.

use std::io::Read;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use pm_plugin_api::{Host, HostCall, ProcessOutput};
use pm_policy::{Capability, Grants};

/// The most a download or a file read gives back, in bytes.
const MOST_BYTES: u64 = 16 << 20;

/// Whether `address` is somewhere on the public internet: not this machine, a private or link-local network, or another address that is not routed globally.
fn is_global(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            let [first, second, ..] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || (first == 100 && (64..128).contains(&second))
                || first == 0)
        }
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(mapped) => is_global(IpAddr::V4(mapped)),
            None => {
                let first = v6.segments()[0];
                !(v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_multicast()
                    || first & 0xfe00 == 0xfc00
                    || first & 0xffc0 == 0xfe80)
            }
        },
    }
}

/// Nothing when every address the host of `url` resolves to is global, and why not otherwise.
fn check_public(url: &str) -> Result<(), String> {
    let uri: ureq::http::Uri = url
        .parse()
        .map_err(|_| format!("{url} is not an address"))?;
    let host = uri
        .host()
        .ok_or_else(|| format!("{url} names no host"))?
        .trim_matches(['[', ']']);
    let port = uri
        .port_u16()
        .unwrap_or(if uri.scheme_str() == Some("https") {
            443
        } else {
            80
        });
    let found: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("could not resolve {host}: {error}"))?
        .collect();
    if found.is_empty() || found.iter().any(|address| !is_global(address.ip())) {
        return Err(format!("{host} is not a public address"));
    }
    Ok(())
}

/// How long a download may take.
const PATIENCE: Duration = Duration::from_secs(60);

/// The host an extension's code runs against: it does what the extension asks only after the grants say it may.
pub(crate) struct Gate {
    /// What the reader allowed this extension.
    grants: Grants,
    /// The folder of the project the command runs in, when it runs in one.
    project: Option<PathBuf>,
}

impl Gate {
    /// A gate for an extension holding `grants`, running a command in `project`.
    pub(crate) fn new(grants: Grants, project: Option<PathBuf>) -> Self {
        Self { grants, project }
    }

    /// The project folder, or why there is none.
    fn project(&self) -> Result<&Path, String> {
        self.project
            .as_deref()
            .ok_or_else(|| "the command is not running in a project".to_owned())
    }

    /// The path `relative` leads to inside the project, resolved so that a link cannot lead out of it.
    fn inside_project(&self, relative: &str) -> Result<PathBuf, String> {
        let project = self
            .project()?
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let path = project
            .join(relative)
            .canonicalize()
            .map_err(|error| format!("{relative}: {error}"))?;
        if path.starts_with(&project) {
            Ok(path)
        } else {
            Err(format!("{relative} is outside the project"))
        }
    }

    /// Runs `program` with `args` in `cwd` or the project, and waits for it.
    fn process(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let folder = match cwd {
            Some(relative) => self.inside_project(relative)?,
            None => self.project()?.to_path_buf(),
        };
        let output = Command::new(program)
            .args(args)
            .current_dir(folder)
            .output()
            .map_err(|error| format!("could not run {program}: {error}"))?;
        serde_json::to_value(ProcessOutput {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
        .map_err(|error| error.to_string())
    }

    /// The text at the address `url`.
    fn download(&self, url: &str) -> Result<serde_json::Value, String> {
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(format!("{url} is not an http address"));
        }
        check_public(url)?;
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .timeout_global(Some(PATIENCE))
            .build()
            .into();
        let mut body = agent
            .get(url)
            .call()
            .map_err(|error| format!("could not fetch {url}: {error}"))?
            .into_body();
        let mut text = String::new();
        body.as_reader()
            .take(MOST_BYTES)
            .read_to_string(&mut text)
            .map_err(|error| format!("could not read {url}: {error}"))?;
        Ok(serde_json::Value::String(text))
    }

    /// The text of the file `path` of the project.
    fn read(&self, path: &str) -> Result<serde_json::Value, String> {
        let file = self.inside_project(path)?;
        let mut text = String::new();
        std::fs::File::open(&file)
            .and_then(|opened| opened.take(MOST_BYTES).read_to_string(&mut text))
            .map_err(|error| format!("{path}: {error}"))?;
        Ok(serde_json::Value::String(text))
    }
}

impl Host for Gate {
    /// Does what `call` asks when its capability was granted, and refuses it with the reason otherwise.
    fn call(&self, call: HostCall) -> Result<serde_json::Value, String> {
        match call {
            HostCall::Process { program, args, cwd } => {
                self.grants
                    .check(Capability::Process)
                    .map_err(|denied| denied.to_string())?;
                self.process(&program, &args, cwd.as_deref())
            }
            HostCall::Download { url } => {
                self.grants
                    .check(Capability::Download)
                    .map_err(|denied| denied.to_string())?;
                self.download(&url)
            }
            HostCall::ReadFile { path } => {
                self.grants
                    .check(Capability::ReadProject)
                    .map_err(|denied| denied.to_string())?;
                self.read(&path)
            }
        }
    }
}
