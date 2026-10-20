//! The newest release of the app on a channel, as the project publishes them.

use pm_protocol::Response;
use serde_json::Value;

/// Where the project's releases are listed.
const RELEASES: &str = "https://api.github.com/repos/JoakimCarlsson/pandemonium/releases";

/// The tag the build made every night is published under.
const NIGHTLY: &str = "nightly";

/// The newest release on `channel` for someone running `running`, the server's own version when nothing: the newest one they follow of those made for everyone and their own pre-releases, or the build made every night, by the date it was made and the commit it was built from.
pub(crate) fn latest(channel: &str, running: Option<&str>) -> Response {
    let running = running.unwrap_or(env!("CARGO_PKG_VERSION"));
    let found = if channel == NIGHTLY {
        fetch(&format!("{RELEASES}/tags/{NIGHTLY}")).map(|release| {
            let text = |key: &str| text(&release, key).map(str::to_owned);
            Response::LatestRelease {
                version: text("published_at").map(|date| date.chars().take(10).collect()),
                url: text("html_url").unwrap_or_default(),
                commit: text("target_commitish"),
            }
        })
    } else {
        fetch(&format!("{RELEASES}?per_page=100")).map(|listed| {
            let newest = listed
                .as_array()
                .into_iter()
                .flatten()
                .filter(|release| !flag(release, "draft"))
                .filter_map(|release| Some((text(release, "tag_name")?, release)))
                .filter(|(tag, release)| {
                    *tag != NIGHTLY
                        && pm_protocol::follows(tag, flag(release, "prerelease"), running)
                })
                .fold(
                    None,
                    |newest: Option<(&str, &Value)>, (tag, release)| match newest {
                        Some((held, _)) if !pm_protocol::is_newer(tag, held) => newest,
                        _ => Some((tag, release)),
                    },
                );
            Response::LatestRelease {
                version: newest.map(|(tag, _)| tag.trim_start_matches('v').to_owned()),
                url: newest
                    .and_then(|(_, release)| text(release, "html_url"))
                    .unwrap_or_default()
                    .to_owned(),
                commit: None,
            }
        })
    };
    found.unwrap_or_else(|reason| {
        crate::handler::failed(format!("could not look for releases: {reason}"))
    })
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
