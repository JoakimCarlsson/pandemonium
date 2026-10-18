//! What an extension's code is allowed to have the host do for it.

use std::io::Read;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use pm_plugin_api::{Host, HostCall, ProcessOutput};
use pm_policy::{Capability, Grants};
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

/// The most a download or a file read gives back, in bytes.
const MOST_BYTES: u64 = 16 << 20;

/// Whether `address` is somewhere on the public internet: not this machine, a private, link-local, shared, documentation, benchmarking or reserved range, and, for an IPv6 address that carries an IPv4 one, not that one either.
fn is_global(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => {
            let [first, second, third, _] = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast()
                || first == 0
                || first >= 240
                || (first == 100 && (64..128).contains(&second))
                || (first == 192 && second == 0 && third <= 2)
                || (first == 198 && (18..20).contains(&second))
                || (first == 198 && second == 51 && third == 100)
                || (first == 203 && second == 0 && third == 113))
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            let first = segments[0];
            let embedded = |high: u16, low: u16| {
                let [a, b] = high.to_be_bytes();
                let [c, d] = low.to_be_bytes();
                std::net::Ipv4Addr::new(a, b, c, d)
            };
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_global(IpAddr::V4(mapped));
            }
            if segments[..6] == [0; 6] {
                return !v6.is_unspecified()
                    && !v6.is_loopback()
                    && is_global(IpAddr::V4(embedded(segments[6], segments[7])));
            }
            if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
                return is_global(IpAddr::V4(embedded(segments[6], segments[7])));
            }
            if first == 0x2002 {
                return is_global(IpAddr::V4(embedded(segments[1], segments[2])));
            }
            first & 0xe000 == 0x2000
                && !(first == 0x2001 && (segments[1] == 0 || segments[1] == 0xdb8))
        }
    }
}

/// A resolver that hands the connection only public addresses, so that what is checked is what is connected to: a host that resolves to anything that is not public is refused whole.
#[derive(Debug, Default)]
struct PublicResolver {
    /// The resolver that does the lookup.
    inner: DefaultResolver,
}

impl Resolver for PublicResolver {
    /// Resolves `uri` and refuses it unless every address it resolves to is public.
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let found = self.inner.resolve(uri, config, timeout)?;
        if found.iter().any(|address| !is_global(address.ip())) {
            return Err(ureq::Error::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "the host is not a public address",
            )));
        }
        Ok(found)
    }
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
        let config = ureq::Agent::config_builder()
            .max_redirects(0)
            .proxy(None)
            .timeout_global(Some(PATIENCE))
            .build();
        let agent = ureq::Agent::with_parts(
            config,
            DefaultConnector::default(),
            PublicResolver::default(),
        );
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
