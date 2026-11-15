//! The extensions page's rows: what each extension is, how it stands and
//! what its button does.

use pm_protocol::ExtensionInfo;

/// How long a line of what an extension brings may be, in characters.
const PROVIDES_LINE: usize = 80;

/// How long a reason a row has room for, in characters.
const PROBLEM_LENGTH: usize = 96;

/// The tone a line of a row is drawn in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    /// Secondary text.
    Muted,
    /// Faint text.
    #[default]
    Subtle,
    /// The accent, for something newer to install.
    Accent,
    /// The danger tone, for a failure.
    Danger,
}

/// What an extension's button does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExtensionAction {
    /// Installs it.
    #[default]
    Install,
    /// Installs what is newer than what is installed.
    Update,
    /// Takes it off the machine.
    Uninstall,
    /// Nothing, while it is being installed.
    Installing,
}

impl ExtensionAction {
    /// What the button says.
    pub fn label(self) -> &'static str {
        match self {
            Self::Install => "Install",
            Self::Update => "Update",
            Self::Uninstall => "Uninstall",
            Self::Installing => "Installing",
        }
    }
}

/// An extension as the extensions page lists it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extension {
    /// Its folder's name, which requests name it by.
    pub id: String,
    /// What the reader calls it.
    pub name: String,
    /// Its version, or empty.
    pub version: String,
    /// What it is for and the endings of the files it reads, in a line.
    pub about: String,
    /// What it carries, in lines.
    pub provides: Vec<String>,
    /// The extension that installed it, in words, when another did.
    pub owner: Option<String>,
    /// How it stands, in words, or empty.
    pub standing: String,
    /// The tone `standing` is drawn in.
    pub tone: Tone,
    /// Whether it is installed.
    pub installed: bool,
    /// Whether it is a package rather than a language.
    pub package: bool,
    /// Whether it is a folder of the reader's own.
    pub local: bool,
    /// What its button does.
    pub action: ExtensionAction,
    /// The endings of the names of the files it reads, without their dots.
    pub suffixes: Vec<String>,
    /// The whole names of the files it reads.
    pub names: Vec<String>,
    /// The capabilities it asks to be allowed, each with what it lets it do, and whether the reader allowed it.
    pub capabilities: Vec<Capability>,
}

/// A capability an extension package asks to be allowed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capability {
    /// Its name, which requests name it by.
    pub id: String,
    /// What it lets the extension do, in words for the reader.
    pub about: String,
    /// Whether the reader allowed it.
    pub granted: bool,
}

/// The capabilities `info` asks to be allowed, each allowed as `granted` says.
fn capabilities(info: &ExtensionInfo, granted: impl Fn(&str) -> bool) -> Vec<Capability> {
    info.requests
        .iter()
        .map(|id| Capability {
            id: id.clone(),
            about: pm_protocol::CAPABILITIES
                .iter()
                .find(|(known, _)| known == id)
                .map_or_else(|| id.clone(), |(_, about)| (*about).to_owned()),
            granted: granted(id),
        })
        .collect()
}

/// What the reader does about extensions beyond pressing a row's button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionDo {
    /// Pick a folder on this machine to install an extension from.
    InstallFromFolder,
    /// Allow the installed extension with this id the capability so named, or take it back.
    Grant(String, String, bool),
    /// Allow the extension of the folder being installed the capability so named, or not.
    PreviewGrant(String, bool),
    /// Install the extension of the folder being installed, with what was allowed.
    InstallPreview,
    /// Give up installing the extension of the folder.
    CancelPreview,
}

/// An extension in a folder the reader picked, shown before it is installed with what it carries and asks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtensionPreview {
    /// The folder, on this machine.
    pub path: String,
    /// The extension as its folder describes it, or nothing until the server answered.
    pub extension: Option<Extension>,
    /// Why the folder holds no extension that can be installed, when it does not.
    pub problem: Option<String>,
}

impl ExtensionPreview {
    /// The preview of the extension `info` describes, asking for nothing allowed yet.
    pub fn read(&mut self, info: &ExtensionInfo) {
        let mut extension = Extension::of(info);
        extension.capabilities = capabilities(info, |_| false);
        self.extension = Some(extension);
    }

    /// The names of the capabilities allowed so far.
    pub fn grants(&self) -> Vec<String> {
        self.extension
            .iter()
            .flat_map(|extension| &extension.capabilities)
            .filter(|capability| capability.granted)
            .map(|capability| capability.id.clone())
            .collect()
    }
}

impl Extension {
    /// The row for the extension `info` describes.
    pub fn of(info: &ExtensionInfo) -> Self {
        let endings = info
            .suffixes
            .iter()
            .take(4)
            .map(|suffix| format!(".{suffix}"))
            .collect::<Vec<_>>()
            .join(" ");
        let about = if info.description.is_empty() {
            endings
        } else {
            format!("{}  {endings}", info.description)
                .trim_end()
                .to_owned()
        };
        let (standing, tone) = standing(info);
        Self {
            id: info.id.clone(),
            name: info.name.clone(),
            version: info.version.clone(),
            about,
            provides: wrapped(&info.provides, PROVIDES_LINE),
            owner: info
                .owner
                .as_ref()
                .map(|owner| format!("From the {owner} extension")),
            standing,
            tone,
            installed: info.installed,
            package: info.kind == "extension",
            local: info.local,
            action: match (info.busy.is_some(), info.installed) {
                (true, _) => ExtensionAction::Installing,
                (false, true) if info.update.is_some() => ExtensionAction::Update,
                (false, true) => ExtensionAction::Uninstall,
                (false, false) => ExtensionAction::Install,
            },
            suffixes: info.suffixes.clone(),
            names: info.names.clone(),
            capabilities: capabilities(info, |id| info.granted.iter().any(|held| held == id)),
        }
    }

    /// Whether it is a language that reads the file at `path`: by the
    /// ending of its name in any case, or by its whole name.
    pub fn reads(&self, path: &str) -> bool {
        let name = path.rsplit('/').next().unwrap_or(path);
        let lower = name.to_ascii_lowercase();
        let ending = lower.rsplit_once('.').filter(|(stem, _)| !stem.is_empty());
        !self.package
            && (self.names.iter().any(|whole| whole == name)
                || ending.is_some_and(|(_, ending)| {
                    self.suffixes
                        .iter()
                        .any(|suffix| suffix.eq_ignore_ascii_case(ending))
                }))
    }

    /// Whether the row matches `needle`, already in lower case, by its name
    /// or what it is for.
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || self.name.to_lowercase().contains(needle)
            || self.about.to_lowercase().contains(needle)
    }
}

/// The rows of the extensions the reader picks from in `infos`, those
/// installed first, then each by name.
pub fn listed(infos: &[ExtensionInfo]) -> Vec<Extension> {
    let mut rows: Vec<Extension> = infos
        .iter()
        .filter(|info| !info.hidden)
        .map(Extension::of)
        .collect();
    rows.sort_by_key(|row| (!row.installed, row.name.to_lowercase()));
    rows
}

/// What `info` is doing or has, in words, and their tone; empty for one
/// that is only available.
fn standing(info: &ExtensionInfo) -> (String, Tone) {
    if let Some(busy) = &info.busy {
        return (format!("{busy}…"), Tone::Muted);
    }
    if let Some(problem) = &info.problem {
        return (shortened(problem, PROBLEM_LENGTH), Tone::Danger);
    }
    if info.kind == "extension" {
        return (String::new(), Tone::Subtle);
    }
    if let Some(update) = &info.update {
        return (update.clone(), Tone::Accent);
    }
    match (&info.server, info.installed) {
        (Some(server), true) => (server.clone(), Tone::Muted),
        (None, true) => ("No language server".to_owned(), Tone::Subtle),
        (_, false) => (String::new(), Tone::Subtle),
    }
}

/// `words` cut to `most` characters, with an ellipsis where it was cut.
fn shortened(words: &str, most: usize) -> String {
    if words.chars().count() <= most {
        return words.to_owned();
    }
    let kept: String = words.chars().take(most - 1).collect();
    format!("{}…", kept.trim_end())
}

/// `items` joined with ` · ` and broken into lines of at most `most`
/// characters, each item whole.
fn wrapped(items: &[String], most: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for item in items {
        match lines.last_mut() {
            Some(last) if last.chars().count() + 3 + item.chars().count() <= most => {
                last.push_str(" · ");
                last.push_str(item);
            }
            _ => lines.push(item.clone()),
        }
    }
    lines
}

/// The language among `extensions` not installed that reads the file at
/// `path`, which the file's tab offers to install, unless its id is among `declined`.
pub fn language_offer<'a>(
    extensions: &'a [Extension],
    path: &str,
    declined: &[String],
) -> Option<&'a Extension> {
    extensions.iter().find(|extension| {
        !extension.installed && !declined.contains(&extension.id) && extension.reads(path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A language reads files by the ending of their names in any case,
    /// a package reads none, and a bare dotfile has no ending.
    #[test]
    fn languages_offered_by_ending() {
        let mut ada = Extension::of(&ExtensionInfo {
            id: "ada".to_owned(),
            name: "Ada".to_owned(),
            suffixes: vec!["adb".to_owned()],
            ..ExtensionInfo::default()
        });
        assert!(ada.reads("src/MAIN.ADB"));
        assert!(!ada.reads(".adb"));
        assert_eq!(
            language_offer(std::slice::from_ref(&ada), "a.adb", &[]).map(|found| found.id.clone()),
            Some("ada".to_owned())
        );
        assert!(language_offer(std::slice::from_ref(&ada), "a.adb", &["ada".to_owned()]).is_none());
        ada.package = true;
        assert!(!ada.reads("a.adb"));
    }

    /// Installed extensions come first, hidden ones are left out, and the
    /// button follows how each stands.
    #[test]
    fn lists_installed_first() {
        let infos = vec![
            ExtensionInfo {
                id: "zig".to_owned(),
                name: "Zig".to_owned(),
                installed: true,
                ..ExtensionInfo::default()
            },
            ExtensionInfo {
                id: "ada".to_owned(),
                name: "Ada".to_owned(),
                ..ExtensionInfo::default()
            },
            ExtensionInfo {
                id: "hidden".to_owned(),
                hidden: true,
                ..ExtensionInfo::default()
            },
        ];
        let rows = listed(&infos);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "zig");
        assert_eq!(rows[0].action, ExtensionAction::Uninstall);
        assert_eq!(rows[1].action, ExtensionAction::Install);
    }

    /// Long lists of what an extension brings break between items.
    #[test]
    fn wraps_between_items() {
        let items = vec!["a".repeat(50), "b".repeat(20), "c".repeat(20)];
        let lines = wrapped(&items, 80);
        assert_eq!(lines.len(), 2);
    }
}
