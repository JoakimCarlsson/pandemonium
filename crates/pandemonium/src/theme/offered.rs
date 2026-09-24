//! The theme families on offer: the ones the editor ships and the reader's
//! own.
//!
//! A family is a name, the family it builds on and what it paints there,
//! which is how both kinds are written down. The editor ships each of its
//! families in full; a family the reader writes names any of them to build
//! on, and one that names none builds on the family a first launch starts
//! in. Like the keymaps, the list is installed once at launch and again when
//! the reader writes or reloads one, and everything that offers a family
//! reads it from here.

use std::sync::RwLock;

use pm_ui::{Appearance, Theme, ThemeFamily};

use crate::theme::paint::Paint;

/// Index of the family a first launch starts in.
pub const DEFAULT_FAMILY: usize = 0;

/// How many families a chain of `extends` is followed through before it is
/// taken to go round in a circle.
const DEEPEST: usize = 8;

/// A family as it is written: its name, what it builds on and what it
/// paints in either appearance.
#[derive(Clone, Debug)]
pub struct ThemeFile {
    /// The name the family is offered by.
    pub name: &'static str,
    /// The name of the family it builds on, if it names one.
    pub extends: Option<String>,
    /// What it paints over the dark variant beneath.
    pub dark: Paint,
    /// What it paints over the light variant beneath.
    pub light: Paint,
}

impl ThemeFile {
    /// What it paints over the `appearance` variant beneath.
    fn paint(&self, appearance: Appearance) -> &Paint {
        match appearance {
            Appearance::Dark => &self.dark,
            Appearance::Light => &self.light,
        }
    }
}

/// The families on offer, resolved.
static OFFERED: RwLock<&'static [ThemeFamily]> = RwLock::new(&[]);

/// Puts `files` on offer in place of whatever was offered before, each
/// resolved through the families it builds on.
///
/// The handful of lists a session installs live as long as it does, so a
/// frame still holding the last one never sees it go.
pub fn install(files: Vec<ThemeFile>) {
    let families = (0..files.len())
        .map(|index| resolve(&files, index))
        .collect::<Vec<_>>()
        .leak();
    if let Ok(mut offered) = OFFERED.write() {
        *offered = families;
    }
}

/// Every family on offer, the shipped ones first.
pub fn families() -> &'static [ThemeFamily] {
    OFFERED.read().map_or(&[], |families| *families)
}

/// The family at `index`, or the default one when there is none there.
pub fn family(index: usize) -> ThemeFamily {
    let families = families();
    families
        .get(index)
        .or_else(|| families.get(DEFAULT_FAMILY))
        .copied()
        .unwrap_or_else(unpainted)
}

/// The index of the family called `name`.
pub fn find(name: &str) -> Option<usize> {
    families().iter().position(|family| family.name == name)
}

/// The family `files[index]` describes, painted over every family it builds
/// on from the root of the chain up.
fn resolve(files: &[ThemeFile], index: usize) -> ThemeFamily {
    let chain = chain(files, index);
    let name = files[index].name;
    ThemeFamily {
        name,
        dark: painted(&chain, name, Appearance::Dark),
        light: painted(&chain, name, Appearance::Light),
    }
}

/// The `appearance` variant of the family called `family`, painted by
/// `chain` from its far end to its near one.
fn painted(chain: &[&ThemeFile], family: &str, appearance: Appearance) -> Theme {
    let mut theme = Theme::unpainted(appearance);
    for file in chain.iter().rev() {
        file.paint(appearance).apply(&mut theme);
    }
    let suffix = match appearance {
        Appearance::Dark => "Dark",
        Appearance::Light => "Light",
    };
    theme.name = format!("{family} {suffix}").leak();
    theme
}

/// `files[index]` and every file it builds on, nearest first.
///
/// A file that names nothing builds on the default family. A name that is
/// not on offer ends the chain where it is, and so does a file already in
/// it: the default family builds on nothing more than itself.
fn chain(files: &[ThemeFile], index: usize) -> Vec<&ThemeFile> {
    let mut chain = Vec::new();
    let mut next = files.get(index);
    while let Some(file) = next {
        if chain.len() == DEEPEST
            || chain
                .iter()
                .any(|held: &&ThemeFile| std::ptr::eq(*held, file))
        {
            break;
        }
        chain.push(file);
        next = match &file.extends {
            Some(name) => files.iter().find(|beneath| beneath.name == name),
            None => files.get(DEFAULT_FAMILY),
        };
    }
    chain
}

/// The family drawn when none is on offer at all.
fn unpainted() -> ThemeFamily {
    ThemeFamily {
        name: "",
        dark: Theme::unpainted(Appearance::Dark),
        light: Theme::unpainted(Appearance::Light),
    }
}
