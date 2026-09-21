//! The keymap a first launch starts from.

use serde::{Deserialize, Serialize};

use crate::keymap::binding::{Keymap, Row};
use crate::keymap::tables;

/// A keymap the editor can start from, named after the editor it comes from.
///
/// Each one is [`tables::BASE`] with that editor's overlay on top; picking one
/// is picking an overlay, not a table of its own.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum BaseKeymap {
    /// The editor's own bindings, with nothing laid over them.
    #[default]
    Pandemonium,
    /// VS Code's bindings.
    VsCode,
    /// Zed's bindings.
    Zed,
    /// JetBrains' bindings.
    JetBrains,
    /// Vim's window commands.
    Vim,
    /// Emacs' window commands.
    Emacs,
    /// Helix's space menu and window commands.
    Helix,
    /// Sublime Text's bindings.
    SublimeText,
}

impl BaseKeymap {
    /// Every keymap, in the order the setup screen offers them.
    pub const ALL: [Self; 8] = [
        Self::Pandemonium,
        Self::VsCode,
        Self::Zed,
        Self::JetBrains,
        Self::Vim,
        Self::Emacs,
        Self::Helix,
        Self::SublimeText,
    ];

    /// The keymap's name, as the setup screen shows it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pandemonium => "Pandemonium",
            Self::VsCode => "VS Code",
            Self::Zed => "Zed",
            Self::JetBrains => "JetBrains",
            Self::Vim => "Vim",
            Self::Emacs => "Emacs",
            Self::Helix => "Helix",
            Self::SublimeText => "Sublime Text",
        }
    }

    /// The overlay the keymap lays over [`tables::BASE`].
    const fn overlay(self) -> &'static [Row] {
        match self {
            Self::Pandemonium => &[],
            Self::VsCode => tables::VS_CODE,
            Self::Zed => tables::ZED,
            Self::JetBrains => tables::JETBRAINS,
            Self::Vim => tables::VIM,
            Self::Emacs => tables::EMACS,
            Self::Helix => tables::HELIX,
            Self::SublimeText => tables::SUBLIME,
        }
    }

    /// The keymap itself: the base table with this editor's overlay on it.
    pub fn keymap(self) -> Keymap {
        let mut keymap = Keymap::from_table(tables::BASE);
        keymap.layer(Keymap::from_table(self.overlay()));
        keymap
    }

    /// The keymap the setup screen's `index`-th option stands for.
    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or_default()
    }

    /// Which of the setup screen's options this keymap is.
    pub fn index(self) -> usize {
        Self::ALL
            .into_iter()
            .position(|keymap| keymap == self)
            .unwrap_or_default()
    }
}
