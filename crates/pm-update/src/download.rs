//! Fetching a release's files over HTTPS, hashing the archive as it arrives.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::Failure;

/// How long a stretch of the download may stall before it counts as failed.
const PATIENCE: Duration = Duration::from_secs(30);

/// How many bytes are read at a time.
const CHUNK: usize = 64 * 1024;

/// The most a checksum file may be, in bytes.
const SUMS_LIMIT: u64 = 1024 * 1024;

/// The agent every download goes through.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_recv_body(Some(PATIENCE))
        .timeout_connect(Some(PATIENCE))
        .build()
        .into()
}

/// The response to asking for `source`, which must be an HTTPS address.
fn open(source: &str) -> Result<ureq::http::Response<ureq::Body>, String> {
    if !source.starts_with("https://") {
        return Err(format!("{source} is not an https address"));
    }
    agent()
        .get(source)
        .header(
            "User-Agent",
            concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|error| format!("could not download {source}: {error}"))
}

/// The text at `source`.
pub(crate) fn text(source: &str) -> Result<String, String> {
    open(source)?
        .body_mut()
        .with_config()
        .limit(SUMS_LIMIT)
        .read_to_string()
        .map_err(|error| format!("could not download {source}: {error}"))
}

/// Writes what is at `source` to `path`, telling `progress` how many of the `size` bytes have arrived, and returns its SHA-256 in lowercase hex once all `size` bytes did; a `size` of zero is not checked. Stops with [`Failure::Cancelled`] as soon as `cancel` is set.
pub(crate) fn file(
    source: &str,
    path: &Path,
    size: u64,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<String, Failure> {
    let mut response = open(source)?;
    let total = if size > 0 {
        size
    } else {
        response
            .headers()
            .get("Content-Length")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    };
    let mut reader = response.body_mut().with_config().limit(u64::MAX).reader();
    let mut out = File::create(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; CHUNK];
    let mut received = 0u64;
    progress(0, total);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Failure::Cancelled);
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("the download failed: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        out.write_all(&buffer[..read])
            .map_err(|error| format!("could not write the download: {error}"))?;
        received += read as u64;
        progress(received, total);
    }
    if total > 0 && received != total {
        return Err(Failure::Failed(format!(
            "the download is cut short: {received} of {total} bytes arrived"
        )));
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
