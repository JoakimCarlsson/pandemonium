//! The newest release of the app on a channel, as the project publishes them.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use pm_protocol::{ReleaseAsset, Response, ServerMessage};
use pm_update::{Asset, Failure};
use serde_json::Value;

use crate::core::Connection;

/// Whether the download of an update in progress is to stop.
static CANCEL: AtomicBool = AtomicBool::new(false);

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
            name: text(&release, "name")
                .filter(|name| !name.is_empty())
                .map(str::to_owned),
            published: text(&release, "published_at").map(str::to_owned),
            body: text(&release, "body")
                .filter(|body| !body.trim().is_empty())
                .map(str::to_owned),
            assets: release
                .get("assets")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(asset)
                .collect(),
        },
        Err(reason) => crate::handler::failed(format!("could not look for releases: {reason}")),
    }
}

/// The file `listed` describes, when it names where it is downloaded from.
fn asset(listed: &Value) -> Option<ReleaseAsset> {
    Some(ReleaseAsset {
        name: text(listed, "name")?.to_owned(),
        url: text(listed, "browser_download_url")?.to_owned(),
        size: listed.get("size").and_then(Value::as_u64).unwrap_or(0),
    })
}

/// Downloads the build of `version` on `channel` made for this machine, telling `connection` how far it has got, and unpacks it; the answer says where.
pub(crate) fn download(connection: &Arc<Connection>, channel: &str, version: &str) -> Response {
    let found = match latest(channel) {
        Response::LatestRelease { assets, .. } => assets,
        failed => return failed,
    };
    let assets: Vec<Asset> = found
        .into_iter()
        .map(|asset| Asset {
            name: asset.name,
            url: asset.url,
            size: asset.size,
        })
        .collect();
    let chosen = match pm_update::choose_here(&assets, channel, version) {
        Ok(chosen) => chosen,
        Err(reason) => return crate::handler::failed(reason),
    };
    CANCEL.store(false, Ordering::Relaxed);
    let mut progress = |received, total| {
        let _ = connection
            .outbox
            .send(ServerMessage::UpdateProgress { received, total });
    };
    match pm_update::stage(&chosen, &CANCEL, &mut progress) {
        Ok(path) => Response::UpdateStaged {
            path: path.to_string_lossy().into_owned(),
        },
        Err(Failure::Cancelled) => Response::Done,
        Err(Failure::Failed(reason)) => crate::handler::failed(reason),
    }
}

/// Stops the download of an update in progress.
pub(crate) fn cancel() {
    CANCEL.store(true, Ordering::Relaxed);
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
