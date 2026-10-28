//! What an extension's code is allowed to have the host do for it.

use std::io::Read;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use pm_plugin_api::{Host, HostCall, ProcessOutput, Release, ReleaseAsset, StorageEntry};
use pm_policy::{Capability, Grants};
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

/// The most a download or a file read gives back, in bytes.
const MOST_BYTES: u64 = 16 << 20;

/// The most a file fetched into the storage may be, in bytes.
const MOST_FILE: u64 = 512 << 20;

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

/// How many redirects a download of a release asset follows, each to a public address.
const REDIRECTS: u32 = 5;

/// How long the user's shell may take to print its environment.
const SHELL_PATIENCE: Duration = Duration::from_secs(5);

/// What is done with a line an extension says about what it is doing.
pub type Status = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

/// The host an extension's code runs against: it does what the extension asks only after the grants say it may.
pub(crate) struct Gate {
    /// What the reader allowed this extension.
    grants: Grants,
    /// The folder of the project the command runs in, when it runs in one.
    project: Option<PathBuf>,
    /// The folder the extension keeps what it fetches in.
    storage: PathBuf,
    /// Who hears what the extension says it is doing.
    status: Option<Status>,
}

impl Gate {
    /// A gate for an extension holding `grants`, running a command in `project`, keeping what it fetches in `storage` and telling `status` what it is doing.
    pub(crate) fn new(
        grants: Grants,
        project: Option<PathBuf>,
        storage: PathBuf,
        status: Option<Status>,
    ) -> Self {
        Self {
            grants,
            project,
            storage,
            status,
        }
    }

    /// The path `relative` leads to inside the extension's storage, or why it does not stay inside.
    fn in_storage(&self, relative: &str) -> Result<PathBuf, String> {
        let path = Path::new(relative);
        if relative.is_empty()
            || !path
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
        {
            return Err(format!("{relative} does not stay inside the storage"));
        }
        Ok(self.storage.join(path))
    }

    /// The path of the program `program` on this machine, or nothing.
    fn which(&self, program: &str) -> serde_json::Value {
        pm_tools::installed(program)
            .map(|path| serde_json::Value::String(path.to_string_lossy().into_owned()))
            .unwrap_or(serde_json::Value::Null)
    }

    /// The environment of the user's login shell, or this process's own when the shell does not answer in time.
    fn shell_env(&self) -> serde_json::Value {
        let own = || -> serde_json::Map<String, serde_json::Value> {
            std::env::vars()
                .map(|(name, value)| (name, serde_json::Value::String(value)))
                .collect()
        };
        let Some(shell) = std::env::var_os("SHELL").filter(|_| !cfg!(windows)) else {
            return serde_json::Value::Object(own());
        };
        let Ok(mut child) = Command::new(shell)
            .args(["-l", "-c", "env"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .spawn()
        else {
            return serde_json::Value::Object(own());
        };
        let started = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if started.elapsed() < SHELL_PATIENCE => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return serde_json::Value::Object(own());
                }
            }
        }
        let mut text = String::new();
        if let Some(mut out) = child.stdout.take() {
            let _ = out.read_to_string(&mut text);
        }
        let held: serde_json::Map<String, serde_json::Value> = text
            .lines()
            .filter_map(|line| line.split_once('='))
            .filter(|(name, _)| !name.is_empty() && !name.contains(' '))
            .map(|(name, value)| (name.to_owned(), serde_json::Value::String(value.to_owned())))
            .collect();
        serde_json::Value::Object(if held.is_empty() { own() } else { held })
    }

    /// The latest release of the repository `repository`, written `owner/name`, as GitHub lists it.
    fn latest_release(&self, repository: &str) -> Result<serde_json::Value, String> {
        let plain = |part: &str| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        };
        let valid = repository
            .split_once('/')
            .is_some_and(|(owner, name)| plain(owner) && plain(name));
        if !valid {
            return Err(format!("{repository} is not written owner/name"));
        }
        let url = format!("https://api.github.com/repos/{repository}/releases/latest");
        let text = self.download(&url)?;
        let release: serde_json::Value = serde_json::from_str(text.as_str().unwrap_or_default())
            .map_err(|error| format!("{repository}: GitHub answered badly: {error}"))?;
        let version = release
            .get("tag_name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("{repository} has no release"))?
            .to_owned();
        let assets = release
            .get("assets")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|asset| {
                Some(ReleaseAsset {
                    name: asset.get("name")?.as_str()?.to_owned(),
                    url: asset.get("browser_download_url")?.as_str()?.to_owned(),
                    sha256: asset
                        .get("digest")
                        .and_then(serde_json::Value::as_str)
                        .and_then(|digest| digest.strip_prefix("sha256:"))
                        .map(str::to_owned),
                })
            })
            .collect();
        serde_json::to_value(Release { version, assets }).map_err(|error| error.to_string())
    }

    /// Fetches `url` into the storage at `path`, checked against `sha256` when given, and made a program when `executable`.
    fn download_file(
        &self,
        url: &str,
        path: &str,
        sha256: Option<&str>,
        executable: bool,
    ) -> Result<serde_json::Value, String> {
        use sha2::{Digest, Sha256};
        if !url.starts_with("https://") {
            return Err(format!("{url} is not an https address"));
        }
        let target = self.in_storage(path)?;
        let config = ureq::Agent::config_builder()
            .max_redirects(REDIRECTS)
            .proxy(None)
            .timeout_global(Some(PATIENCE * 5))
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
        let mut bytes = Vec::new();
        body.as_reader()
            .take(MOST_FILE)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("could not read {url}: {error}"))?;
        if let Some(wanted) = sha256 {
            let digest = Sha256::digest(&bytes);
            let got: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
            if !got.eq_ignore_ascii_case(wanted) {
                return Err(format!("{url} does not match its checksum"));
            }
        }
        if let Some(folder) = target.parent() {
            std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        }
        let staging = target.with_extension("partial");
        std::fs::write(&staging, &bytes).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        if executable {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
        #[cfg(not(unix))]
        let _ = executable;
        std::fs::rename(&staging, &target).map_err(|error| error.to_string())?;
        Ok(serde_json::Value::String(
            target.to_string_lossy().into_owned(),
        ))
    }

    /// What the folder `dir` of the storage holds, or the top of the storage.
    fn storage_list(&self, dir: Option<&str>) -> Result<serde_json::Value, String> {
        let folder = match dir {
            Some(dir) => self.in_storage(dir)?,
            None => self.storage.clone(),
        };
        let mut entries: Vec<StorageEntry> = std::fs::read_dir(&folder)
            .map(|read| {
                read.filter_map(Result::ok)
                    .map(|entry| StorageEntry {
                        name: entry.file_name().to_string_lossy().into_owned(),
                        path: entry.path().to_string_lossy().into_owned(),
                        dir: entry.path().is_dir(),
                    })
                    .filter(|entry| !entry.name.ends_with(".partial"))
                    .collect()
            })
            .unwrap_or_default();
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        serde_json::to_value(entries).map_err(|error| error.to_string())
    }

    /// Takes `path` out of the storage.
    fn storage_remove(&self, path: &str) -> Result<serde_json::Value, String> {
        let target = self.in_storage(path)?;
        let outcome = if target.is_dir() {
            std::fs::remove_dir_all(&target)
        } else {
            std::fs::remove_file(&target)
        };
        match outcome {
            Ok(()) => Ok(serde_json::Value::Null),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(serde_json::Value::Null)
            }
            Err(error) => Err(format!("{path}: {error}")),
        }
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
            HostCall::Which { program } => {
                self.grants
                    .check(Capability::Process)
                    .map_err(|denied| denied.to_string())?;
                Ok(self.which(&program))
            }
            HostCall::ShellEnv => {
                self.grants
                    .check(Capability::Process)
                    .map_err(|denied| denied.to_string())?;
                Ok(self.shell_env())
            }
            HostCall::LatestRelease { repository } => {
                self.grants
                    .check(Capability::Download)
                    .map_err(|denied| denied.to_string())?;
                self.latest_release(&repository)
            }
            HostCall::DownloadFile {
                url,
                path,
                sha256,
                executable,
            } => {
                self.grants
                    .check(Capability::Download)
                    .map_err(|denied| denied.to_string())?;
                self.download_file(&url, &path, sha256.as_deref(), executable)
            }
            HostCall::StorageList { dir } => {
                self.grants
                    .check(Capability::Download)
                    .map_err(|denied| denied.to_string())?;
                self.storage_list(dir.as_deref())
            }
            HostCall::StorageRemove { path } => {
                self.grants
                    .check(Capability::Download)
                    .map_err(|denied| denied.to_string())?;
                self.storage_remove(&path)
            }
            HostCall::Status { text } => {
                if let Some(status) = &self.status {
                    status(&text);
                }
                Ok(serde_json::Value::Null)
            }
        }
    }
}
