//! The three forms a lifecycle command is written in.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One program to run: a shell string or an argument vector.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Single {
    /// A string handed to `sh -c`.
    Shell(String),
    /// A program and its arguments, run without a shell.
    Exec(Vec<String>),
}

/// A lifecycle command: one program, or named programs run together.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Command {
    /// A shell string or an argument vector.
    Single(Single),
    /// Named programs started together; all must succeed.
    Parallel(Vec<(String, Single)>),
}

impl Command {
    /// Reads a command from its JSON form, or `None` when it asks for nothing.
    pub fn parse(value: &Value) -> Result<Option<Self>, String> {
        match value {
            Value::Null => Ok(None),
            Value::Object(named) => {
                let mut parts = Vec::new();
                for (name, part) in named {
                    if let Some(single) =
                        Single::parse(part).map_err(|error| format!("{name}: {error}"))?
                    {
                        parts.push((name.clone(), single));
                    }
                }
                Ok((!parts.is_empty()).then_some(Self::Parallel(parts)))
            }
            other => Ok(Single::parse(other)?.map(Self::Single)),
        }
    }

    /// Applies `edit` to every string of the command.
    pub fn map<E>(&self, edit: &mut impl FnMut(&str) -> Result<String, E>) -> Result<Self, E> {
        Ok(match self {
            Self::Single(single) => Self::Single(single.map(edit)?),
            Self::Parallel(parts) => Self::Parallel(
                parts
                    .iter()
                    .map(|(name, single)| Ok((name.clone(), single.map(edit)?)))
                    .collect::<Result<_, E>>()?,
            ),
        })
    }

    /// Applies an infallible `edit` to every string of the command.
    pub fn mapped(&self, edit: &mut impl FnMut(&str) -> String) -> Self {
        match self.map(&mut |text| Ok::<_, std::convert::Infallible>(edit(text))) {
            Ok(command) => command,
            Err(never) => match never {},
        }
    }

    /// Every string of the command, for checking what they reference.
    pub fn strings(&self) -> Vec<&str> {
        match self {
            Self::Single(single) => single.strings(),
            Self::Parallel(parts) => parts
                .iter()
                .flat_map(|(_, single)| single.strings())
                .collect(),
        }
    }
}

impl Single {
    /// Reads a shell string or an argument array; empty ones ask for nothing.
    fn parse(value: &Value) -> Result<Option<Self>, String> {
        match value {
            Value::Null => Ok(None),
            Value::String(text) if text.trim().is_empty() => Ok(None),
            Value::String(text) => Ok(Some(Self::Shell(text.clone()))),
            Value::Array(items) if items.is_empty() => Ok(None),
            Value::Array(items) => items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "an argument list holds only strings".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()
                .map(|arguments| Some(Self::Exec(arguments))),
            _ => Err("expected a string, an array of strings or an object of them".to_owned()),
        }
    }

    /// Applies `edit` to every string.
    fn map<E>(&self, edit: &mut impl FnMut(&str) -> Result<String, E>) -> Result<Self, E> {
        Ok(match self {
            Self::Shell(text) => Self::Shell(edit(text)?),
            Self::Exec(arguments) => Self::Exec(
                arguments
                    .iter()
                    .map(|argument| edit(argument))
                    .collect::<Result<_, E>>()?,
            ),
        })
    }

    /// Every string.
    fn strings(&self) -> Vec<&str> {
        match self {
            Self::Shell(text) => vec![text.as_str()],
            Self::Exec(arguments) => arguments.iter().map(String::as_str).collect(),
        }
    }
}
