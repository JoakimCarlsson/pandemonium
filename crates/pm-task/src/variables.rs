//! The variables a task's text may name, and putting their values in.

use std::collections::BTreeMap;
use std::path::Path;

/// What the name of a capture of a runnables query is put after to make the variable its text is read through.
pub const CUSTOM: &str = "PM_CUSTOM_";

/// The variables that name a place in a file, and so only have a value for a task run from one.
const PLACED: [&str; 8] = [
    "PM_FILE",
    "PM_RELATIVE_FILE",
    "PM_FILENAME",
    "PM_STEM",
    "PM_DIRNAME",
    "PM_ROW",
    "PM_COLUMN",
    CUSTOM,
];

/// The values of the variables a task is run with, by name.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Variables(BTreeMap<String, String>);

impl Variables {
    /// The variables of a project at `root`: `PM_ROOT`.
    pub fn of_project(root: &Path) -> Self {
        let mut held = BTreeMap::new();
        held.insert("PM_ROOT".to_owned(), root.to_string_lossy().into_owned());
        Self(held)
    }

    /// The variables of the file `path`, written from the root of the project at `root`, with the captures `custom` of the place being run, and its `row` and `column` when it has them; the language the file is written in is `language`.
    pub fn of_file(
        root: &Path,
        path: &str,
        language: &str,
        custom: &BTreeMap<String, String>,
        place: Option<(usize, usize)>,
    ) -> Self {
        let mut variables = Self::of_project(root);
        let relative = Path::new(path);
        let name = |part: Option<&std::ffi::OsStr>| {
            part.map(|part| part.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        variables.set(
            "PM_FILE",
            root.join(relative).to_string_lossy().into_owned(),
        );
        variables.set("PM_RELATIVE_FILE", path.replace('\\', "/"));
        variables.set("PM_FILENAME", name(relative.file_name()));
        variables.set("PM_STEM", name(relative.file_stem()));
        variables.set(
            "PM_DIRNAME",
            relative
                .parent()
                .map(|parent| parent.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default(),
        );
        variables.set("PM_LANGUAGE", language.to_owned());
        if let Some((row, column)) = place {
            variables.set("PM_ROW", row.to_string());
            variables.set("PM_COLUMN", column.to_string());
        }
        for (name, text) in custom {
            variables.set(&format!("{CUSTOM}{name}"), text.clone());
        }
        variables
    }

    /// Gives `name` the value `value`.
    pub fn set(&mut self, name: &str, value: String) {
        self.0.insert(name.to_owned(), value);
    }

    /// The value of `name`, when it has one.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    /// Every variable and its value.
    pub fn all(&self) -> &BTreeMap<String, String> {
        &self.0
    }

    /// `text` with each `$NAME`, `${NAME}` and `${NAME:fallback}` that names a variable replaced by its value, quoted for the shell when `quoted`; a name that is not a variable is left as written, for the shell to read.
    pub fn put_in(&self, text: &str, quoted: bool) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find('$') {
            out.push_str(&rest[..at]);
            let after = &rest[at + 1..];
            let (name, fallback, length) = match after.strip_prefix('{') {
                Some(inner) => match inner.find('}') {
                    Some(end) => {
                        let (name, fallback) = match inner[..end].split_once(':') {
                            Some((name, fallback)) => (name, Some(fallback)),
                            None => (&inner[..end], None),
                        };
                        (name, fallback, end + 2)
                    }
                    None => ("", None, 0),
                },
                None => {
                    let end = after
                        .find(|character: char| {
                            !character.is_ascii_alphanumeric() && character != '_'
                        })
                        .unwrap_or(after.len());
                    (&after[..end], None, end)
                }
            };
            let value = self.0.get(name).map(String::as_str).or(fallback);
            match value {
                Some(value) if !name.is_empty() => {
                    out.push_str(&if quoted {
                        quote(value)
                    } else {
                        value.to_owned()
                    });
                    rest = &after[length..];
                }
                _ => {
                    out.push('$');
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        out
    }
}

/// Whether `text` names a variable that only has a value for a task run from a place in a file.
pub fn needs_place(text: &str) -> bool {
    PLACED.iter().any(|name| text.contains(name))
}

/// `value` as one word of a shell: as it is when it holds nothing the shell reads, and otherwise quoted.
pub fn quote(value: &str) -> String {
    let plain = !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-./:=@%+,".contains(character));
    if plain {
        value.to_owned()
    } else if cfg!(windows) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
