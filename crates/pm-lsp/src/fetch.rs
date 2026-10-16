//! Fetching files from an address or a folder, and checking them against a checksum.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};

/// How long one fetch may take.
const PATIENCE: Duration = Duration::from_secs(300);

/// The agent every fetch goes through.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(PATIENCE))
        .http_status_as_error(false)
        .build()
        .into()
}

/// What a fetch that may be answered from a copy kept before came back with.
pub(crate) enum Fetched {
    /// The bytes, and the tag the server gave them, if any.
    Fresh(Vec<u8>, Option<String>),
    /// The copy kept before, tagged `tag`, is still the latest.
    Unchanged,
}

/// The bytes at `source`, an address or a path on this machine; a copy kept before under the tag `etag` is not fetched again when the address says it has not changed.
pub(crate) fn fetch_tagged(source: &str, etag: Option<&str>) -> Result<Fetched, String> {
    if !source.starts_with("https://") && !source.starts_with("http://") {
        let path = source.strip_prefix("file://").unwrap_or(source);
        return std::fs::read(path)
            .map(|bytes| Fetched::Fresh(bytes, None))
            .map_err(|error| format!("could not read {path}: {error}"));
    }
    let mut request = agent().get(source).header(
        "User-Agent",
        concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
    );
    if let Some(etag) = etag {
        request = request.header("If-None-Match", etag);
    }
    let mut response = request
        .call()
        .map_err(|error| format!("could not fetch {source}: {error}"))?;
    let status = response.status().as_u16();
    if status == 304 {
        return Ok(Fetched::Unchanged);
    }
    if !(200..300).contains(&status) {
        return Err(format!(
            "could not fetch {source}: the server answered {status}"
        ));
    }
    let tag = response
        .headers()
        .get("ETag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut bytes = Vec::new();
    response
        .body_mut()
        .with_config()
        .limit(512 * 1024 * 1024)
        .reader()
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not fetch {source}: {error}"))?;
    Ok(Fetched::Fresh(bytes, tag))
}

/// The bytes at `source`, an address or a path on this machine.
pub(crate) fn fetch(source: &str) -> Result<Vec<u8>, String> {
    match fetch_tagged(source, None)? {
        Fetched::Fresh(bytes, _) => Ok(bytes),
        Fetched::Unchanged => Err(format!("{source} answered with nothing")),
    }
}

/// The SHA-256 of `bytes`, in lowercase hex.
pub(crate) fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The bytes at `source`, once they are found to have the checksum `expected`.
pub(crate) fn fetch_checked(source: &str, expected: &str) -> Result<Vec<u8>, String> {
    let bytes = fetch(source)?;
    let found = sha256(&bytes);
    if !found.eq_ignore_ascii_case(expected.trim()) {
        return Err(format!(
            "{source} does not match its checksum: expected {expected}, got {found}"
        ));
    }
    Ok(bytes)
}

/// Writes `bytes` to `path` and lets it be run.
pub(crate) fn write_program(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, bytes)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// The program `bytes` hold once unpacked as `archive` says, `member` naming it inside a zip.
pub(crate) fn unpack(
    bytes: Vec<u8>,
    archive: crate::manifest::Archive,
    member: Option<&str>,
) -> Result<Vec<u8>, String> {
    match archive {
        crate::manifest::Archive::None => Ok(bytes),
        crate::manifest::Archive::Gz => {
            let mut unpacked = Vec::new();
            flate2::read::GzDecoder::new(bytes.as_slice())
                .read_to_end(&mut unpacked)
                .map_err(|error| format!("could not unpack the download: {error}"))?;
            Ok(unpacked)
        }
        crate::manifest::Archive::Zip => {
            let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
                .map_err(|error| format!("could not open the download: {error}"))?;
            let name = match member {
                Some(name) => name.to_owned(),
                None => zip
                    .file_names()
                    .filter_map(Result::ok)
                    .next()
                    .map(|name| name.into_owned())
                    .ok_or("the download is empty")?,
            };
            let mut file = zip
                .by_name(&name)
                .map_err(|error| format!("the download has no {name}: {error}"))?;
            let mut unpacked = Vec::new();
            file.read_to_end(&mut unpacked)
                .map_err(|error| format!("could not unpack the download: {error}"))?;
            Ok(unpacked)
        }
    }
}
