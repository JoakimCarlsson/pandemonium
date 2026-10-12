//! The string and object forms of a mount.

use serde_json::Value;

/// What a mount attaches to the container.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// A folder or file of the desktop.
    Bind,
    /// A volume the engine manages.
    Volume,
    /// Memory-backed scratch space.
    Tmpfs,
}

/// One mount, before its variables are expanded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mount {
    /// What is attached.
    pub kind: Kind,
    /// The desktop path or volume name; none for scratch space and anonymous volumes.
    pub source: Option<String>,
    /// Where it appears in the container.
    pub target: String,
    /// Whether the container may not write to it.
    pub read_only: bool,
}

impl Mount {
    /// Reads `type=bind,source=…,target=…` or the object form.
    pub fn parse(value: &Value) -> Result<Self, String> {
        match value {
            Value::String(text) => Self::from_string(text),
            Value::Object(fields) => {
                let mut pairs = Vec::new();
                for (key, field) in fields {
                    let text = field
                        .as_str()
                        .ok_or_else(|| format!("{key} must be a string"))?;
                    pairs.push((key.clone(), Some(text.to_owned())));
                }
                Self::from_pairs(pairs)
            }
            _ => Err("expected a mount string or object".to_owned()),
        }
    }

    /// Splits the comma separated form into its keys.
    fn from_string(text: &str) -> Result<Self, String> {
        let pairs = text
            .split(',')
            .filter(|part| !part.trim().is_empty())
            .map(|part| match part.split_once('=') {
                Some((key, value)) => (key.trim().to_owned(), Some(value.trim().to_owned())),
                None => (part.trim().to_owned(), None),
            })
            .collect();
        Self::from_pairs(pairs)
    }

    /// Builds a mount from its keys, refusing the ones that would change meaning when ignored.
    fn from_pairs(pairs: Vec<(String, Option<String>)>) -> Result<Self, String> {
        let mut kind = None;
        let mut source = None;
        let mut target = None;
        let mut read_only = false;
        for (key, value) in pairs {
            match (key.as_str(), value) {
                ("type", Some(value)) => {
                    kind = Some(match value.as_str() {
                        "bind" => Kind::Bind,
                        "volume" => Kind::Volume,
                        "tmpfs" => Kind::Tmpfs,
                        other => return Err(format!("mount type {other} is not supported")),
                    });
                }
                ("source" | "src", Some(value)) => source = Some(value),
                ("target" | "destination" | "dst", Some(value)) => target = Some(value),
                ("readonly" | "ro", None) => read_only = true,
                ("readonly" | "ro", Some(value)) => read_only = value != "false" && value != "0",
                ("consistency", _) => {}
                (other, _) => return Err(format!("mount option {other} is not supported")),
            }
        }
        let kind = kind.ok_or("a mount needs a type")?;
        let target = target.ok_or("a mount needs a target")?;
        if kind == Kind::Bind && source.is_none() {
            return Err("a bind mount needs a source".to_owned());
        }
        Ok(Self {
            kind,
            source,
            target,
            read_only,
        })
    }

    /// The strings that may hold variables.
    pub fn strings(&self) -> Vec<&str> {
        self.source
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(self.target.as_str()))
            .collect()
    }

    /// Applies an infallible `edit` to every string.
    pub fn mapped(&self, edit: &mut impl FnMut(&str) -> String) -> Self {
        match self.map(&mut |text| Ok::<_, std::convert::Infallible>(edit(text))) {
            Ok(mount) => mount,
            Err(never) => match never {},
        }
    }

    /// Applies `edit` to every string.
    pub fn map<E>(&self, edit: &mut impl FnMut(&str) -> Result<String, E>) -> Result<Self, E> {
        Ok(Self {
            kind: self.kind,
            source: self.source.as_deref().map(&mut *edit).transpose()?,
            target: edit(&self.target)?,
            read_only: self.read_only,
        })
    }
}
