//! Checked downloads shared by server and extension installation.

use std::io::{Cursor, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

/// Reads an HTTPS resource with a bounded response size.
pub fn download(url: &str) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err("Downloads require an HTTPS URL.".into());
    }
    let mut response = ureq::get(url).call().map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("Download exceeds 256 MiB.".into());
    }
    Ok(bytes)
}

/// Verifies a downloaded resource against its declared SHA-256 digest.
pub fn download_checked(url: &str, expected: &str) -> Result<Vec<u8>, String> {
    let bytes = download(url)?;
    verify_checksum(&bytes, expected)?;
    Ok(bytes)
}

/// Checks asset bytes before activation, including bundled offline packages.
pub fn verify_checksum(bytes: &[u8], expected: &str) -> Result<(), String> {
    let digest = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if !digest.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "Checksum mismatch: expected {expected}, got {digest}."
        ));
    }
    Ok(())
}

/// Extracts a ZIP without allowing links, traversal or oversized entries.
pub fn unpack_zip(bytes: &[u8], directory: &Path) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if entry.enclosed_name().is_none() || entry.is_symlink() {
            return Err("Archive contains a link or a path outside its directory.".into());
        }
        total = total.saturating_add(entry.size());
        if total > 512 * 1024 * 1024 {
            return Err("Unpacked archive exceeds 512 MiB.".into());
        }
    }
    archive
        .extract(directory)
        .map_err(|error| error.to_string())
}
