//! Where yanked and deleted text is kept until it is put back.
//!
//! The unnamed register is the system clipboard as well, so text yanked here
//! pastes anywhere and text copied anywhere pastes here. A register holds
//! whole lines when its text ends in a line break; there is no separate flag
//! to fall out of step with the text.

use std::collections::HashMap;

/// The system clipboard, as the window reaches it.
pub trait Clipboard {
    /// What is on the clipboard, if it holds text.
    fn read(&mut self) -> Option<String>;
    /// Puts `text` on the clipboard.
    fn write(&mut self, text: String);
}

/// Why text is going into a register, which decides which ones it lands in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Filling {
    /// A yank, which also fills `"0`.
    Yank,
    /// A deletion, which fills `"1` or `"-`.
    Delete,
}

/// Every register, named and numbered.
#[derive(Debug, Default)]
pub(crate) struct Registers {
    /// The unnamed register, `""`.
    unnamed: String,
    /// The lettered registers, `"a` to `"z`.
    named: HashMap<char, String>,
    /// The numbered registers, `"0` to `"9`.
    numbered: [String; 10],
    /// The small-delete register, `"-`.
    small: String,
    /// What the clipboard held when the unnamed register was last filled.
    ///
    /// Putting text on the clipboard can take a moment to land, so a paste
    /// straight after a yank may still read what was there before. Text
    /// that is neither this nor the unnamed register's is something copied
    /// elsewhere since; anything else means the unnamed register is newest.
    displaced: Option<String>,
}

impl Registers {
    /// Stores `text` as `filling` does, in `register` when one was named.
    pub(crate) fn fill(
        &mut self,
        register: Option<char>,
        text: String,
        filling: Filling,
        clipboard: &mut dyn Clipboard,
    ) {
        match register {
            Some('_') => return,
            Some('+' | '*') => return clipboard.write(text),
            Some(name) if name.is_ascii_uppercase() => {
                let entry = self.named.entry(name.to_ascii_lowercase()).or_default();
                entry.push_str(&text);
            }
            Some(name) if name.is_ascii_lowercase() => {
                self.named.insert(name, text.clone());
            }
            Some(_) => {}
            None => match filling {
                Filling::Yank => self.numbered[0] = text.clone(),
                Filling::Delete if text.contains('\n') => {
                    self.numbered[1..].rotate_right(1);
                    self.numbered[1] = text.clone();
                }
                Filling::Delete => self.small = text.clone(),
            },
        }
        self.displaced = clipboard.read();
        clipboard.write(text.clone());
        self.unnamed = text;
    }

    /// The text `register` holds, or the unnamed register's.
    ///
    /// The unnamed register gives way to the clipboard when something else
    /// has been copied since, so a paste always puts back the latest copy.
    pub(crate) fn read(
        &self,
        register: Option<char>,
        clipboard: &mut dyn Clipboard,
    ) -> Option<String> {
        let text = match register {
            Some('+' | '*') => clipboard.read().unwrap_or_default(),
            None | Some('"') => match clipboard.read() {
                Some(copied)
                    if copied != self.unnamed && Some(&copied) != self.displaced.as_ref() =>
                {
                    copied
                }
                _ => self.unnamed.clone(),
            },
            Some('-') => self.small.clone(),
            Some(digit @ '0'..='9') => self.numbered[digit as usize - '0' as usize].clone(),
            Some(name) => self.named.get(&name.to_ascii_lowercase())?.clone(),
        };
        (!text.is_empty()).then_some(text)
    }
}
