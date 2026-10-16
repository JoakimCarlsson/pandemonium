//! What a web address holds, fetched for a client that shows it, such as a picture in markdown.

use std::io::Read;
use std::time::Duration;

/// The most bytes fetched from one address.
const FETCH_LIMIT: u64 = 32_000_000;

/// How long a fetch may take, from asking to the last byte.
const FETCH_TIME: Duration = Duration::from_secs(20);

/// The bytes `url` holds and whether they were cut short at the limit, or why they could not be fetched; only `http` and `https` addresses are fetched.
pub(crate) fn bytes(url: &str) -> Result<(Vec<u8>, bool), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("{url} is not a web address"));
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(FETCH_TIME))
        .build()
        .into();
    let mut response = agent
        .get(url)
        .header(
            "User-Agent",
            concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|error| format!("could not fetch {url}: {error}"))?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(FETCH_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not fetch {url}: {error}"))?;
    let truncated = bytes.len() as u64 > FETCH_LIMIT;
    bytes.truncate(FETCH_LIMIT as usize);
    Ok((bytes, truncated))
}
