//! Where the browser helper and its engine are kept, and downloading them when the reader turns the browser on.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Where this version's browser downloads are published; the environment variable names a folder or address to use instead.
const RELEASES: &str = "https://github.com/JoakimCarlsson/pandemonium/releases/download";

/// The most bytes of the download that are accepted.
const LIMIT: u64 = 1 << 30;

/// The steps of installing, in order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// Fetching the archive, with how many bytes have arrived.
    Downloading(u64),
    /// Checking it against its checksum.
    Verifying,
    /// Unpacking it into place.
    Unpacking,
}

/// Whether the browser is built for this system.
pub fn supported() -> bool {
    cfg!(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(
            target_os = "macos",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "windows", target_arch = "x86_64"),
    ))
}

/// The platform this build runs on, as downloads name it: `linux-x86_64`, `macos-aarch64`, `windows-x86_64`.
fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// `name` as a program file of this system.
fn program(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// The folder this version's download is unpacked into.
fn folder() -> Option<PathBuf> {
    pm_store::data_directory().map(|data| data.join("browser").join(env!("CARGO_PKG_VERSION")))
}

/// Where the engine keeps cookies and caches.
pub(crate) fn profile() -> Option<PathBuf> {
    pm_store::data_directory().map(|data| data.join("browser-profile"))
}

/// The helper program: the one named by `PANDEMONIUM_BROWSER_HELPER`, the one downloaded, or one built beside the window's own program, whichever exists first.
pub fn helper() -> Option<PathBuf> {
    let name = program("pandemonium-browser");
    let named = std::env::var_os("PANDEMONIUM_BROWSER_HELPER").map(PathBuf::from);
    let downloaded = folder().map(|folder| folder.join(&name));
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join(&name)));
    named
        .into_iter()
        .chain(downloaded)
        .chain(beside)
        .find(|path| path.is_file())
}

/// Whether the browser can be started.
pub fn installed() -> bool {
    helper().is_some()
}

/// The text at `source`, a folder or address the files are published in, with `name` after it.
fn open(base: &str, name: &str) -> Result<Box<dyn Read + Send>, String> {
    if base.starts_with("https://") || base.starts_with("http://") {
        let address = format!("{}/{name}", base.trim_end_matches('/'));
        let response = ureq::get(&address)
            .header(
                "User-Agent",
                concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
            )
            .call()
            .map_err(|error| format!("could not fetch {address}: {error}"))?;
        return Ok(Box::new(
            response
                .into_body()
                .into_with_config()
                .limit(LIMIT)
                .reader(),
        ));
    }
    let path = Path::new(base.strip_prefix("file://").unwrap_or(base)).join(name);
    File::open(&path)
        .map(|file| Box::new(file) as Box<dyn Read + Send>)
        .map_err(|error| format!("could not read {}: {error}", path.display()))
}

/// Downloads the browser for this version into its folder, checked against the checksum published beside it, and calls `progress` as each step starts.
pub fn install(progress: &dyn Fn(Step)) -> Result<(), String> {
    if !supported() {
        return Err("the embedded browser is not available on this system yet".to_owned());
    }
    let target = folder().ok_or("this machine has no data folder")?;
    let base = std::env::var("PANDEMONIUM_BROWSER_URL")
        .unwrap_or_else(|_| format!("{RELEASES}/v{}", env!("CARGO_PKG_VERSION")));
    let name = format!("pandemonium-browser-{}.tar.gz", platform());
    let staging = target.with_extension("partial");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let outcome = fetch_into(&base, &name, &staging, &target, progress);
    let _ = std::fs::remove_dir_all(&staging);
    outcome
}

/// Fetches `name` from `base` into `staging`, checks it and moves its contents to `target`.
fn fetch_into(
    base: &str,
    name: &str,
    staging: &Path,
    target: &Path,
    progress: &dyn Fn(Step),
) -> Result<(), String> {
    progress(Step::Downloading(0));
    let mut received = 0u64;
    let archive = staging.join(name);
    let mut hasher = Sha256::new();
    {
        let mut source = open(base, name)?;
        let mut file = File::create(&archive).map_err(|error| error.to_string())?;
        let mut chunk = vec![0; 1 << 20];
        loop {
            let read = source
                .read(&mut chunk)
                .map_err(|error| format!("the download broke off: {error}"))?;
            if read == 0 {
                break;
            }
            received += read as u64;
            progress(Step::Downloading(received));
            hasher.update(&chunk[..read]);
            file.write_all(&chunk[..read])
                .map_err(|error| error.to_string())?;
        }
    }
    progress(Step::Verifying);
    let mut published = String::new();
    open(base, &format!("{name}.sha256"))?
        .take(4096)
        .read_to_string(&mut published)
        .map_err(|error| error.to_string())?;
    let expected = published.split_whitespace().next().unwrap_or_default();
    let actual = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if !actual.eq_ignore_ascii_case(expected) {
        return Err("the download does not match its checksum".to_owned());
    }
    progress(Step::Unpacking);
    let unpacked = staging.join("unpacked");
    let file = File::open(&archive).map_err(|error| error.to_string())?;
    tar::Archive::new(flate2::read::GzDecoder::new(file))
        .unpack(&unpacked)
        .map_err(|error| format!("could not unpack the download: {error}"))?;
    if !unpacked.join(program("pandemonium-browser")).is_file() {
        return Err("the download holds no browser".to_owned());
    }
    let _ = std::fs::remove_dir_all(target);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::rename(&unpacked, target).map_err(|error| error.to_string())
}
