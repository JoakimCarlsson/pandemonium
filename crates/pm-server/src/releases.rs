//! The newest release of the app on a channel, as the project publishes them.

use pm_protocol::Response;

/// Where the project's releases are listed.
const RELEASES: &str = "https://api.github.com/repos/JoakimCarlsson/pandemonium/releases";

/// The newest release on `channel`: the latest one made for everyone, or the build made every night, by the date it was made.
pub(crate) fn latest(channel: &str) -> Response {
    let nightly = channel == "nightly";
    let source = if nightly {
        format!("{RELEASES}/tags/nightly")
    } else {
        format!("{RELEASES}/latest")
    };
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build()
        .into();
    let found = agent
        .get(&source)
        .header(
            "User-Agent",
            concat!("pandemonium/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| error.to_string())
        .and_then(|mut response| {
            let text = response
                .body_mut()
                .read_to_string()
                .map_err(|error| error.to_string())?;
            serde_json::from_str::<serde_json::Value>(&text).map_err(|error| error.to_string())
        });
    match found {
        Ok(release) => {
            let text = |key: &str| release.get(key).and_then(|value| value.as_str());
            let version = if nightly {
                text("published_at").map(|date| date.chars().take(10).collect())
            } else {
                text("tag_name").map(|tag| tag.trim_start_matches('v').to_owned())
            };
            Response::LatestRelease {
                version,
                url: text("html_url").unwrap_or_default().to_owned(),
            }
        }
        Err(reason) => crate::handler::failed(format!("could not look for releases: {reason}")),
    }
}
