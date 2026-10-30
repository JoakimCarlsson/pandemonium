//! Dev container features: install scripts that are fetched from a registry and run while the image is built.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use sha2::Digest;

use crate::bridge::Progress;
use crate::provision::output_of;

/// A feature a `devcontainer.json` asks for.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Feature {
    /// The registry reference, such as `ghcr.io/devcontainers/features/node:1`.
    pub reference: String,
    /// The options it is given, as `name=value`.
    pub options: Vec<(String, String)>,
}

/// The features in the object `value`, in the order the file lists them.
pub(crate) fn parse(value: Option<&Value>) -> Vec<Feature> {
    let Some(Value::Object(entries)) = value else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|(_, options)| !matches!(options, Value::Bool(false)))
        .map(|(reference, options)| Feature {
            reference: reference.clone(),
            options: match options {
                Value::String(version) => vec![("version".to_owned(), version.clone())],
                Value::Object(options) => options
                    .iter()
                    .filter_map(|(name, value)| Some((name.clone(), scalar(value)?)))
                    .collect(),
                _ => Vec::new(),
            },
        })
        .collect()
}

/// The text of a string, number or boolean.
fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// Where a feature lives in its registry: the host, the repository and the tag.
fn locate(reference: &str) -> Result<(&str, &str, &str), String> {
    let (host, rest) = reference
        .split_once('/')
        .ok_or_else(|| format!("{reference} is not a feature in a registry"))?;
    let (repository, tag) = rest.rsplit_once(':').unwrap_or((rest, "latest"));
    let plain = |text: &str, extra: &[char]| {
        !text.is_empty()
            && text
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || extra.contains(&character))
    };
    let named = plain(host, &['.', '-'])
        && host.contains('.')
        && !host.starts_with(['.', '-'])
        && !host.ends_with(['.', '-'])
        && host.rsplit('.').next().is_some_and(|label| {
            label
                .chars()
                .all(|character| character.is_ascii_alphabetic())
        });
    if !named
        || !plain(repository, &['.', '-', '_', '/'])
        || repository
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || !plain(tag, &['.', '-', '_'])
    {
        return Err(format!("{reference} is not a feature in a registry"));
    }
    Ok((host, repository, tag))
}

/// The text `url` serves, asking with `headers`.
fn fetch(url: &str, headers: &[String], into: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut command = Command::new("curl");
    command.args([
        "-fsSL",
        "--max-time",
        "300",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
    ]);
    for header in headers {
        command.args(["-H", header]);
    }
    if let Some(file) = into {
        command.arg("-o").arg(file);
    }
    command.arg(url);
    output_of(&mut command).map_err(|reason| format!("could not fetch {url}: {reason}"))
}

/// Downloads the feature at `reference` into the folder `into`.
fn download(reference: &str, into: &Path) -> Result<(), String> {
    let (host, repository, tag) = locate(reference)?;
    let mut headers = Vec::new();
    if host == "ghcr.io" {
        let token = fetch(
            &format!("https://{host}/token?service={host}&scope=repository:{repository}:pull"),
            &[],
            None,
        )?;
        let token: Value = serde_json::from_slice(&token)
            .map_err(|error| format!("{reference} gave no token: {error}"))?;
        let token = token["token"].as_str().unwrap_or_default();
        headers.push(format!("Authorization: Bearer {token}"));
    }
    let mut accept = headers.clone();
    accept.push("Accept: application/vnd.oci.image.manifest.v1+json".to_owned());
    let manifest = fetch(
        &format!("https://{host}/v2/{repository}/manifests/{tag}"),
        &accept,
        None,
    )?;
    let manifest: Value = serde_json::from_slice(&manifest)
        .map_err(|error| format!("{reference} has no manifest: {error}"))?;
    let digest = manifest["layers"][0]["digest"]
        .as_str()
        .ok_or_else(|| format!("{reference} holds no feature"))?;
    let wanted = digest
        .strip_prefix("sha256:")
        .filter(|hash| hash.len() == 64 && hash.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')))
        .ok_or_else(|| format!("{reference} names a layer that is not a sha256 digest"))?;
    let archive = into.join("feature.tar");
    fetch(
        &format!("https://{host}/v2/{repository}/blobs/{digest}"),
        &headers,
        Some(&archive),
    )?;
    let bytes =
        std::fs::read(&archive).map_err(|error| format!("could not read {reference}: {error}"))?;
    let actual: String = sha2::Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual != wanted {
        return Err(format!("{reference} does not match its digest"));
    }
    refuse_links(&archive)?;
    output_of(
        Command::new("tar")
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(into),
    )?;
    std::fs::remove_file(&archive).map_err(|error| format!("could not clean up: {error}"))
}

/// Refuses the archive at `archive` when it holds a link, or a path that is absolute or climbs out of the folder it is extracted to, so that nothing in it can make a later write on this machine land elsewhere.
fn refuse_links(archive: &Path) -> Result<(), String> {
    let listing = output_of(Command::new("tar").arg("-tvf").arg(archive))?;
    let names = output_of(Command::new("tar").arg("-tf").arg(archive))?;
    let linked = String::from_utf8_lossy(&listing)
        .lines()
        .any(|line| line.starts_with(['l', 'h']));
    let escaping = String::from_utf8_lossy(&names)
        .lines()
        .any(|name| name.starts_with('/') || name.split('/').any(|part| part == ".."));
    if linked || escaping {
        return Err("the feature holds a link or a path outside its folder".to_owned());
    }
    Ok(())
}

/// The name of an option as the install script reads it from its environment.
fn variable(option: &str) -> String {
    let mut name = String::new();
    for character in option.chars() {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_uppercase());
        } else if !name.ends_with('_') {
            name.push('_');
        }
    }
    name
}

/// `text` as one single-quoted word for a shell.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The lines of a shell script that export what one feature's install script reads: its options, with the defaults it names for the ones the file leaves out.
fn environment(feature: &Feature, metadata: &Value) -> String {
    let mut chosen: Vec<(String, String)> = feature
        .options
        .iter()
        .map(|(name, value)| (variable(name), value.clone()))
        .collect();
    if let Some(options) = metadata["options"].as_object() {
        for (name, option) in options {
            let name = variable(name);
            if let Some(default) = option.get("default").and_then(scalar)
                && !chosen.iter().any(|(taken, _)| *taken == name)
            {
                chosen.push((name, default));
            }
        }
    }
    let mut script = String::from(
        "export _REMOTE_USER=root _REMOTE_USER_HOME=/root _CONTAINER_USER=root _CONTAINER_USER_HOME=/root\n",
    );
    for (name, value) in chosen {
        script.push_str(&format!("export {name}={}\n", quoted(&value)));
    }
    script
}

/// Builds the image `tag` from `base` with `features` installed in it, and returns `tag`.
pub(crate) fn install(
    features: &[Feature],
    base: &str,
    tag: &str,
    scratch: &Path,
    progress: Progress,
) -> Result<String, String> {
    let _ = std::fs::remove_dir_all(scratch);
    std::fs::create_dir_all(scratch)
        .map_err(|error| format!("could not make {}: {error}", scratch.display()))?;
    let outcome = build(features, base, tag, scratch, progress);
    let _ = std::fs::remove_dir_all(scratch);
    outcome
}

/// The work of [`install`], with the folder it works in left for the caller to remove.
fn build(
    features: &[Feature],
    base: &str,
    tag: &str,
    scratch: &Path,
    progress: Progress,
) -> Result<String, String> {
    let mut dockerfile = format!("FROM {base}\nUSER root\n");
    for (index, feature) in features.iter().enumerate() {
        progress(&format!("Fetching {}", feature.reference));
        let folder: PathBuf = scratch.join(format!("feature{index}"));
        std::fs::create_dir_all(&folder)
            .map_err(|error| format!("could not make {}: {error}", folder.display()))?;
        download(&feature.reference, &folder)?;
        let described = folder.join("devcontainer-feature.json");
        let metadata: Value = std::fs::symlink_metadata(&described)
            .is_ok_and(|entry| entry.is_file())
            .then(|| std::fs::read_to_string(&described).ok())
            .flatten()
            .and_then(|text| serde_json::from_str(&crate::devcontainer::plain_json(&text)).ok())
            .unwrap_or(Value::Null);
        let options = scratch.join(format!("env{index}"));
        std::fs::create_dir_all(&options)
            .map_err(|error| format!("could not make {}: {error}", options.display()))?;
        std::fs::write(
            options.join("pandemonium-env.sh"),
            environment(feature, &metadata),
        )
        .map_err(|error| format!("could not write the options: {error}"))?;
        dockerfile.push_str(&format!(
            "COPY feature{index} /tmp/pandemonium-feature{index}\nCOPY env{index}/pandemonium-env.sh /tmp/pandemonium-feature{index}/pandemonium-env.sh\nRUN cd /tmp/pandemonium-feature{index} && . ./pandemonium-env.sh && chmod +x install.sh && ./install.sh && rm -rf /tmp/pandemonium-feature{index}\n"
        ));
        if let Some(variables) = metadata["containerEnv"].as_object() {
            for (name, value) in variables {
                if let Some(value) = value.as_str()
                    && !name.contains(|character: char| {
                        !(character.is_ascii_alphanumeric() || character == '_')
                    })
                    && !value.contains(['\n', '\r'])
                {
                    dockerfile.push_str(&format!("ENV {name}={}\n", quoted_value(value)));
                }
            }
        }
    }
    std::fs::write(scratch.join("Dockerfile"), dockerfile)
        .map_err(|error| format!("could not write the Dockerfile: {error}"))?;
    progress("Installing the features");
    output_of(crate::container::engine(["build", "-t", tag]).arg(scratch))?;
    Ok(tag.to_owned())
}

/// `value` as one word of a Dockerfile `ENV` line, which keeps `${NAME}` references for the build to expand.
fn quoted_value(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
