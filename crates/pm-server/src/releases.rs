//! The newest release of the app on a channel, as the project publishes them.

use pm_protocol::Response;
use serde_json::Value;

/// Where the project's releases are listed.
const RELEASES: &str = "https://api.github.com/repos/JoakimCarlsson/pandemonium/releases";

/// The newest release on `channel`: the latest one made for everyone for `stable`, the newest pre-release for `preview`, which a client from before the channel was renamed asks for as `nightly`.
pub(crate) fn latest(channel: &str) -> Response {
    let found = if matches!(channel, "preview" | "nightly") {
        fetch(&format!("{RELEASES}?per_page=100")).map(|listed| {
            listed
                .as_array()
                .into_iter()
                .flatten()
                .find(|release| flag(release, "prerelease") && !flag(release, "draft"))
                .cloned()
                .unwrap_or_default()
        })
    } else {
        fetch(&format!("{RELEASES}/latest"))
    };
    match found {
        Ok(release) => Response::LatestRelease {
            version: text(&release, "tag_name").map(|tag| tag.trim_start_matches('v').to_owned()),
            url: text(&release, "html_url").unwrap_or_default().to_owned(),
        },
        Err(reason) => crate::handler::failed(format!("could not look for releases: {reason}")),
    }
}

/// What the releases API answers at `source`, read as JSON, or why it could not be read.
fn fetch(source: &str) -> Result<Value, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build()
        .into();
    let mut response = agent
        .get(source)
        .header(
            "User-Agent",
            concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| error.to_string())?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

/// The text `release` holds under `key`, when it holds text there.
fn text<'a>(release: &'a Value, key: &str) -> Option<&'a str> {
    release.get(key).and_then(Value::as_str)
}

/// Whether `release` is marked `key`.
fn flag(release: &Value, key: &str) -> bool {
    release.get(key).and_then(Value::as_bool).unwrap_or(false)
}
