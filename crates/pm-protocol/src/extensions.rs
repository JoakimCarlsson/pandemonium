//! What a client is told about the extensions a server can install, and the grammar it highlights a file with.

use serde::{Deserialize, Serialize};

/// One extension: what it is, whether it is installed on the server's machine and what installing it is doing.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtensionInfo {
    /// Its folder's name, which requests name it by.
    pub id: String,
    /// What the reader calls it.
    pub name: String,
    /// What kind of extension it is, such as `language`.
    #[serde(default)]
    pub kind: String,
    /// Its version, as its manifest gives it.
    #[serde(default)]
    pub version: String,
    /// What it is for, in a line.
    #[serde(default)]
    pub description: String,
    /// The icon the files it reads are shown with, as an SVG document, once it is installed.
    #[serde(default)]
    pub icon: Option<String>,
    /// The colour that icon is tinted, as `#rrggbb`.
    #[serde(default)]
    pub color: Option<String>,
    /// Whether it is installed only as part of another, and so left out of the list the reader picks from.
    #[serde(default)]
    pub hidden: bool,
    /// The endings of the file names written in it.
    #[serde(default)]
    pub suffixes: Vec<String>,
    /// Whole file names written in it, such as `Makefile`.
    #[serde(default)]
    pub names: Vec<String>,
    /// File names written in it, where `*` stands for any run of characters, such as `.env.*`.
    #[serde(default)]
    pub patterns: Vec<String>,
    /// Whether it is installed.
    pub installed: bool,
    /// What installing it is doing now, while it is being installed.
    #[serde(default)]
    pub busy: Option<String>,
    /// Why installing it or its language server last failed.
    #[serde(default)]
    pub problem: Option<String>,
    /// What the machine has of its language server, in words for the reader, when it has one.
    #[serde(default)]
    pub server: Option<String>,
    /// Whether it is a folder of the reader's own rather than one the list offers.
    #[serde(default)]
    pub local: bool,
    /// The extension that installed it, for a language that came with one.
    #[serde(default)]
    pub owner: Option<String>,
    /// The version the list offers when what is installed is behind it, so that it can be updated; nothing when it is current, not installed or the reader's own.
    #[serde(default)]
    pub update: Option<String>,
    /// What an extension package carries, one line for each thing, such as `theme: Hello Dusk`; empty for a language.
    #[serde(default)]
    pub provides: Vec<String>,
    /// What an extension package asks to be allowed, by the names of capabilities.
    #[serde(default)]
    pub requests: Vec<String>,
    /// What the reader allowed it, by the names of capabilities.
    #[serde(default)]
    pub granted: Vec<String>,
}

/// The ids of the capabilities an extension may ask for, with what each lets it do.
pub const CAPABILITIES: [(&str, &str); 3] = [
    ("process", "Run programs on this machine"),
    ("download", "Download files from the network"),
    ("read-project", "Read the files of a project"),
];

/// A command an extension brings, for the command palette and the slash of the prompt box.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtensionCommand {
    /// The extension that brings it, by folder name.
    pub extension: String,
    /// What it is called behind the slash and run by.
    pub id: String,
    /// What the palette shows.
    pub title: String,
    /// What it does, in a line.
    #[serde(default)]
    pub description: String,
}

/// A theme an extension brings: a colour for each role it names, in a dark and a light variant, over the default theme.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtensionTheme {
    /// The extension that brings it, by folder name.
    pub extension: String,
    /// What the reader calls it.
    pub name: String,
    /// The colour of each role of the dark variant, as `#rrggbb` or `#rrggbbaa`, by the role's name.
    #[serde(default)]
    pub dark: Vec<(String, String)>,
    /// The colour of each role of the light variant.
    #[serde(default)]
    pub light: Vec<(String, String)>,
}

/// One icon of an icon theme.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtensionIcon {
    /// The ending of the file names it is for, without the dot.
    pub suffix: String,
    /// The icon, as an SVG document.
    pub svg: String,
    /// The colour it is tinted, as `#rrggbb`.
    #[serde(default)]
    pub color: Option<String>,
}

/// An icon theme an extension brings.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtensionIconTheme {
    /// The extension that brings it, by folder name.
    pub extension: String,
    /// What the reader calls it.
    pub name: String,
    /// Its icons.
    #[serde(default)]
    pub icons: Vec<ExtensionIcon>,
}

/// What the installed extensions bring that the window itself uses.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExtensionAssets {
    /// The themes.
    #[serde(default)]
    pub themes: Vec<ExtensionTheme>,
    /// The icon themes.
    #[serde(default)]
    pub icon_themes: Vec<ExtensionIconTheme>,
    /// The commands.
    #[serde(default)]
    pub commands: Vec<ExtensionCommand>,
}

/// An installed language's grammar and queries, which a client highlights its files with.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GrammarBundle {
    /// The language's folder name.
    pub id: String,
    /// The name the grammar was built under.
    pub symbol: String,
    /// Other names a code fence may call the language by.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// The grammar, compiled to WebAssembly.
    #[serde(with = "crate::bytes")]
    pub wasm: Vec<u8>,
    /// The highlights query.
    pub highlights: String,
    /// The indents query.
    pub indents: String,
    /// The outline query.
    pub outline: String,
    /// The injections query.
    pub injections: String,
    /// The brackets query.
    pub brackets: String,
    /// The runnables query.
    #[serde(default)]
    pub runnables: String,
    /// How many columns one level of indentation is.
    pub indent: usize,
    /// Whether a level of indentation is a tab.
    #[serde(default)]
    pub tabs: bool,
    /// What starts a comment that runs to the end of the line.
    #[serde(default)]
    pub line_comment: Option<String>,
    /// The pairs the editor closes as the opening character is typed, each the opening and the closing character; brackets and double quotes when the server says nothing.
    #[serde(default)]
    pub pairs: Option<Vec<String>>,
    /// The parts of a file edited differently from the rest, by name.
    #[serde(default)]
    pub scopes: std::collections::BTreeMap<String, pm_lsp::ScopeSpec>,
    /// How the language's tags are closed as they are opened, when it has tags.
    #[serde(default)]
    pub tags: Option<pm_lsp::TagSpec>,
}

impl ExtensionInfo {
    /// Whether the language reads the file at `path`, by its whole name, a pattern its name matches or the ending of its name.
    pub fn reads(&self, path: &str) -> bool {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        self.names.iter().any(|whole| whole == name)
            || self
                .patterns
                .iter()
                .any(|pattern| pm_lsp::glob(pattern, name))
            || name.rsplit_once('.').is_some_and(|(stem, suffix)| {
                !stem.is_empty()
                    && self
                        .suffixes
                        .iter()
                        .any(|held| held.eq_ignore_ascii_case(suffix))
            })
    }
}
