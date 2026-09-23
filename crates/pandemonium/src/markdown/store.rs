//! What a rendered document keeps between one frame and the next.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use pm_gfx::Image;
use pm_ui::Scrolled;

use crate::editor::FileId;
use crate::markdown::blocks::{self, Block};

/// The endings of the files that read as markdown.
const MARKDOWN: &[&str] = &["md", "markdown", "mdown", "mkd"];

/// The blocks one file last parsed into, and the version it was at.
type Parsed = (i32, Rc<Vec<Block>>);

/// Whether `path` is a markdown file, to be offered a rendered pane.
pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ending| ending.to_str())
        .is_some_and(|ending| MARKDOWN.contains(&ending.to_ascii_lowercase().as_str()))
}

/// The scroll, the parse and the pictures of every rendered document.
///
/// The screen is built from the window's state without changing it, and
/// what it builds from here is made the first time it is asked for, so the
/// maps are behind cells: asking for a scroll that is not there yet is how
/// one comes to be.
#[derive(Default)]
pub struct Renders {
    /// How far each rendered file is scrolled.
    scrolls: RefCell<BTreeMap<FileId, Scrolled>>,
    /// The blocks each file last parsed into, and at which version.
    parsed: RefCell<BTreeMap<FileId, Parsed>>,
    /// The pictures the documents name, by where they are, or none where a
    /// file could not be read as one.
    pictures: RefCell<BTreeMap<PathBuf, Option<Image>>>,
}

impl Renders {
    /// How far the rendering of `file` is scrolled.
    pub fn scroll(&self, file: FileId) -> Scrolled {
        self.scrolls.borrow_mut().entry(file).or_default().clone()
    }

    /// The blocks `source`, the text of `file` at `version`, reads as.
    pub fn blocks(&self, file: FileId, version: i32, source: &str) -> Rc<Vec<Block>> {
        let mut parsed = self.parsed.borrow_mut();
        match parsed.get(&file) {
            Some((at, blocks)) if *at == version => blocks.clone(),
            _ => {
                let blocks = Rc::new(blocks::blocks(source));
                parsed.insert(file, (version, blocks.clone()));
                blocks
            }
        }
    }

    /// The picture at `path`, read the first time it is asked for.
    pub fn picture(&self, path: &Path) -> Option<Image> {
        self.pictures
            .borrow_mut()
            .entry(path.to_path_buf())
            .or_insert_with(|| Image::decode(&std::fs::read(path).ok()?))
            .clone()
    }

    /// Forgets what it kept for any file no pane is rendering any more, and
    /// the pictures, which a document shown again reads afresh.
    pub fn retain(&mut self, held: &BTreeSet<FileId>) {
        self.scrolls.get_mut().retain(|file, _| held.contains(file));
        self.parsed.get_mut().retain(|file, _| held.contains(file));
        if held.is_empty() {
            self.pictures.get_mut().clear();
        }
    }
}
