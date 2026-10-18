//! A project: a folder or repository the work happens in.

use pm_style::Color;

/// The perceptual lightness every avatar shares, so white letters read on all of them.
const LIGHTNESS: f32 = 0.55;

/// The chroma every avatar shares, so no hue looks louder than another.
const CHROMA: f32 = 0.14;

/// Where the FNV-1a hash of a path starts.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// What the FNV-1a hash is multiplied by after each byte.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A folder or repository whose sessions are listed together.
#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    /// What it is called: the last part of where it is.
    pub name: String,
    /// The folder or repository address it was added from.
    pub path: String,
}

impl Project {
    /// The project added from `source`, or nothing when `source` names no folder.
    pub fn from_source(source: &str) -> Option<Self> {
        let path = source.trim().trim_end_matches('/');
        let named = path.strip_suffix(".git").unwrap_or(path);
        let name = named.rsplit('/').next().unwrap_or(named);
        (!name.is_empty()).then(|| Self {
            name: name.to_owned(),
            path: path.to_owned(),
        })
    }

    /// The machine the project is on when it is not this one: the part of its address before the folder, such as the destination of an `ssh://destination/folder` address.
    pub fn host(&self) -> Option<String> {
        pm_protocol::host_of(&self.path).map(|(host, _)| host.to_owned())
    }

    /// The folder the project is, on its machine, with a leading `~` taken to be the reader's home when the machine is this one.
    pub fn directory(&self) -> std::path::PathBuf {
        if let Some((_, folder)) = pm_protocol::host_of(&self.path) {
            return if folder.starts_with('~') {
                folder.into()
            } else {
                format!("/{folder}").into()
            };
        }
        match (self.path.strip_prefix("~/"), std::env::home_dir()) {
            (Some(rest), Some(home)) => home.join(rest),
            _ => self.path.clone().into(),
        }
    }

    /// The branch the project's folder has checked out, or nothing when it is not a repository.
    pub fn branch(&self) -> String {
        if self.host().is_some() {
            return String::new();
        }
        std::fs::read_to_string(self.directory().join(".git/HEAD"))
            .ok()
            .and_then(|head| {
                head.trim()
                    .strip_prefix("ref: refs/heads/")
                    .map(str::to_owned)
            })
            .unwrap_or_default()
    }

    /// The colour of the project's avatar, which the same path always gets.
    pub fn color(&self) -> Color {
        let hash = self.path.bytes().fold(FNV_OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
        });
        Color::oklch(LIGHTNESS, CHROMA, (hash % 360) as f32)
    }
}
