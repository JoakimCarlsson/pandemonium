//! Where yanked and deleted text is kept until it is put back.
//!
//! How much the unnamed register shares with the system clipboard is the
//! reader's choice, as Zed's `use_system_clipboard` makes it: always, only
//! for yanks, or never. A register holds whole lines when its text ends in a
//! line break. What was yanked from several cursors is kept as one piece per
//! cursor too, so putting it back at as many cursors gives each its own.

use std::collections::HashMap;

/// The system clipboard, as the window reaches it.
pub trait Clipboard {
    /// What is on the clipboard, if it holds text.
    fn read(&mut self) -> Option<String>;
    /// Puts `text` on the clipboard.
    fn write(&mut self, text: String);
}

/// How much the unnamed register shares with the system clipboard.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ClipboardUse {
    /// Every yank and delete goes to the clipboard, and pastes read it.
    #[default]
    Always,
    /// Only yanks go to the clipboard; pastes still read it.
    OnYank,
    /// The clipboard is only reached through `"+` and `"*`.
    Never,
}

impl ClipboardUse {
    /// Every choice, in the order a toggle offers them.
    pub const ALL: [Self; 3] = [Self::Always, Self::OnYank, Self::Never];

    /// The choice's label in the toggle that picks it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Always => "Always",
            Self::OnYank => "On Yank",
            Self::Never => "Never",
        }
    }
}

/// Why text is going into a register, which decides which ones it lands in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Filling {
    /// A yank, which also fills `"0`.
    Yank,
    /// A deletion, which fills `"1` or `"-`.
    Delete,
}

/// What one register holds.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Register {
    /// The text, whole.
    pub text: String,
    /// The text each cursor contributed, when there was more than one, or
    /// each line of a block.
    pub pieces: Vec<String>,
    /// Whether it holds a block, a piece per line, to put back as a block.
    pub block: bool,
}

impl Register {
    /// A register holding `text` from one cursor.
    fn of(text: String) -> Self {
        Self {
            text,
            pieces: Vec::new(),
            block: false,
        }
    }

    /// A register holding what each cursor contributed, or each line of a
    /// block when `block`.
    ///
    /// Whole lines are laid end to end; spans within lines go one to a line.
    fn gathered(pieces: Vec<String>, block: bool) -> Self {
        let text = match !block && pieces.iter().all(|piece| piece.ends_with('\n')) {
            true => pieces.concat(),
            false => pieces.join("\n"),
        };
        Self {
            text,
            pieces: if pieces.len() > 1 || block {
                pieces
            } else {
                Vec::new()
            },
            block,
        }
    }

    /// Whether it holds whole lines.
    pub(crate) fn is_linewise(&self) -> bool {
        self.text.ends_with('\n')
    }
}

/// Every register, named and numbered.
#[derive(Debug, Default)]
pub(crate) struct Registers {
    /// The unnamed register, `""`.
    unnamed: Register,
    /// The lettered registers, `"a` to `"z`.
    named: HashMap<char, Register>,
    /// The numbered registers, `"0` to `"9`.
    numbered: [Register; 10],
    /// The small-delete register, `"-`.
    small: Register,
    /// The last text typed in insert mode, `".`.
    inserted: String,
    /// How much the unnamed register shares with the clipboard.
    sharing: ClipboardUse,
    /// What the clipboard held when the unnamed register was last filled.
    ///
    /// Putting text on the clipboard can take a moment to land, so a paste
    /// straight after a yank may still read what was there before. Text
    /// that is neither this nor the unnamed register's is something copied
    /// elsewhere since; anything else means the unnamed register is newest.
    displaced: Option<String>,
}

impl Registers {
    /// Shares the unnamed register with the clipboard as `sharing` says.
    pub(crate) fn share(&mut self, sharing: ClipboardUse) {
        self.sharing = sharing;
    }

    /// Stores what each cursor gave as `filling` does, in `register` when
    /// one was named, as a block when `block`.
    pub(crate) fn fill(
        &mut self,
        register: Option<char>,
        pieces: Vec<String>,
        filling: Filling,
        block: bool,
        clipboard: &mut dyn Clipboard,
    ) {
        let held = Register::gathered(pieces, block);
        match register {
            Some('_') => return,
            Some('+' | '*') => return clipboard.write(held.text),
            Some(name) if name.is_ascii_uppercase() => {
                let entry = self.named.entry(name.to_ascii_lowercase()).or_default();
                entry.text.push_str(&held.text);
                entry.pieces.clear();
            }
            Some(name) if name.is_ascii_lowercase() => {
                self.named.insert(name, held.clone());
            }
            Some(_) => {}
            None => match filling {
                Filling::Yank => self.numbered[0] = held.clone(),
                Filling::Delete if held.text.contains('\n') => {
                    self.numbered[1..].rotate_right(1);
                    self.numbered[1] = held.clone();
                }
                Filling::Delete => self.small = held.clone(),
            },
        }
        let shared = match self.sharing {
            ClipboardUse::Always => true,
            ClipboardUse::OnYank => filling == Filling::Yank,
            ClipboardUse::Never => false,
        };
        if shared {
            self.displaced = clipboard.read();
            clipboard.write(held.text.clone());
        }
        self.unnamed = held;
    }

    /// Keeps what was typed in insert mode, for `".`.
    pub(crate) fn typed(&mut self, text: String) {
        if !text.is_empty() {
            self.inserted = text;
        }
    }

    /// What `register` holds, or the unnamed register.
    ///
    /// The unnamed register gives way to the clipboard when something else
    /// has been copied since, so a paste always puts back the latest copy.
    pub(crate) fn read(
        &self,
        register: Option<char>,
        clipboard: &mut dyn Clipboard,
    ) -> Option<Register> {
        let held = match register {
            Some('+' | '*') => Register::of(clipboard.read().unwrap_or_default()),
            None | Some('"') if self.sharing != ClipboardUse::Never => match clipboard.read() {
                Some(copied)
                    if copied != self.unnamed.text && Some(&copied) != self.displaced.as_ref() =>
                {
                    Register::of(copied)
                }
                _ => self.unnamed.clone(),
            },
            None | Some('"') => self.unnamed.clone(),
            Some('-') => self.small.clone(),
            Some('.') => Register::of(self.inserted.clone()),
            Some(digit @ '0'..='9') => self.numbered[digit as usize - '0' as usize].clone(),
            Some(name) => self.named.get(&name.to_ascii_lowercase())?.clone(),
        };
        (!held.text.is_empty()).then_some(held)
    }
}
