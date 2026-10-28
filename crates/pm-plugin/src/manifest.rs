//! An extension's `extension.toml`: what it is, what it carries and what it asks to be allowed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A language folder the extension carries.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct LanguageEntry {
    /// The folder, inside the extension, holding the language's `config.toml` and queries.
    pub folder: String,
}

/// A file the extension carries: a theme or an icon theme.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    /// The file, inside the extension.
    pub file: String,
}

/// One icon of an icon theme's list, as the manifest of the theme writes it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct IconEntry {
    /// The ending of the file names the icon is for, without the dot.
    pub suffix: String,
    /// The SVG file, inside the extension.
    pub file: String,
    /// The colour the icon is tinted, as `#rrggbb`.
    #[serde(default)]
    pub color: Option<String>,
}

/// A debug adapter the extension brings.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdapterEntry {
    /// The name it is known by in the protocol's handshake.
    pub id: String,
    /// What the reader calls it.
    pub name: String,
    /// The programs it may be installed as, the one to prefer first.
    pub programs: Vec<String>,
    /// What each of them is started with; `{port}` stands for the port it listens on.
    #[serde(default)]
    pub arguments: Vec<String>,
    /// `stdio`, or `tcp` for an adapter that listens on a port the editor picks.
    #[serde(default = "stdio")]
    pub connect: String,
    /// The names a scenario may call it by.
    pub types: Vec<String>,
    /// Whether the extension's own code says which program it is, and fetches it when the machine does not have it, in place of `programs`.
    #[serde(default)]
    pub provided: bool,
}

/// An adapter talks over its standard input and output unless its entry says otherwise.
fn stdio() -> String {
    "stdio".to_owned()
}

/// A tool server the extension brings, to be given to the agents.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerEntry {
    /// What a row calls it.
    pub name: String,
    /// The command that starts it, or the address it listens at when that begins with `http://` or `https://`.
    pub target: String,
    /// The environment it is started with, or the headers it is sent when it is an address.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// What it is for, in a line.
    #[serde(default)]
    pub description: String,
}

/// A command the extension brings, shown in the command palette and behind the slash of the prompt box.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandEntry {
    /// What it is called behind the slash and run by.
    pub id: String,
    /// What the palette shows.
    pub title: String,
    /// What it does, in a line.
    #[serde(default)]
    pub description: String,
    /// The text it puts in the prompt box, in which `{argument}` stands for what was typed after its name; a command without it is run by the extension's module.
    #[serde(default)]
    pub prompt: Option<String>,
}

/// What an extension is and carries.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// What the reader calls it.
    pub name: String,
    /// Its version.
    #[serde(default)]
    pub version: String,
    /// What it is for, in a line.
    #[serde(default)]
    pub description: String,
    /// The version of the plugin API it was written for; a build that speaks a newer one loads it.
    #[serde(default = "first_api")]
    pub api: u32,
    /// The WebAssembly module, inside the extension, that runs its commands.
    #[serde(default)]
    pub wasm: Option<String>,
    /// What it asks to be allowed, by the names [`pm_policy::Capability`] writes.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// The languages it adds.
    #[serde(default)]
    pub languages: Vec<LanguageEntry>,
    /// The themes it adds.
    #[serde(default)]
    pub themes: Vec<FileEntry>,
    /// The icon themes it adds.
    #[serde(default)]
    pub icon_themes: Vec<FileEntry>,
    /// The debug adapters it adds.
    #[serde(default)]
    pub debug_adapters: Vec<AdapterEntry>,
    /// The tool servers it adds.
    #[serde(default)]
    pub mcp_servers: Vec<ServerEntry>,
    /// The commands it adds.
    #[serde(default)]
    pub commands: Vec<CommandEntry>,
}

/// An extension was written for the first version of the API unless its manifest says another.
fn first_api() -> u32 {
    1
}

impl Manifest {
    /// The manifest written in `text`, or why it cannot be read.
    pub fn parse(text: &str) -> Result<Self, String> {
        let manifest: Self =
            toml::from_str(text).map_err(|error| format!("{}: {error}", crate::MANIFEST))?;
        if manifest.name.trim().is_empty() {
            return Err(format!("{} gives the extension no name", crate::MANIFEST));
        }
        if manifest.api > pm_plugin_api::API_VERSION {
            return Err(format!(
                "{} is written for plugin API {}, which this build does not speak yet",
                manifest.name, manifest.api
            ));
        }
        if let Some(unknown) = manifest
            .capabilities
            .iter()
            .find(|name| pm_policy::Capability::parse(name).is_none())
        {
            return Err(format!(
                "{} asks for {unknown}, which is no capability",
                manifest.name
            ));
        }
        Ok(manifest)
    }
}
