//! What a rendered document keeps between one frame and the next.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use pm_gfx::{Image, Point};
use pm_ui::{Appearance, ResizeEvent, Scrolled, Theme, Zoom, Zoomed};

use crate::editor::FileId;
use crate::image::{Decodes, Decoding, read_file};
use crate::markdown::blocks::{self, Block};
use crate::markdown::diagram::Palette;

/// The endings of the files that read as markdown.
const MARKDOWN: &[&str] = &["md", "markdown", "mdown", "mkd"];

/// The blocks one file last parsed into, and the version it was at.
type Parsed = (i32, Rc<Vec<Block>>);

/// How far one press of a diagram's zoom buttons magnifies it.
const ZOOM_STEP: f32 = 1.5;

/// A step a diagram's zoom buttons take.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagramZoom {
    /// Magnify it a step.
    In,
    /// Shrink it a step, no smaller than at rest.
    Out,
    /// Put it back at rest, whole and fitted.
    Reset,
}

/// A diagram's source and all settings that affect its pixels.
#[derive(Clone, Eq, Hash, PartialEq)]
struct DiagramKey {
    /// The Mermaid source.
    source: String,
    /// Physical pixels per logical pixel.
    scale: u32,
    /// Whether the base Mermaid palette is dark.
    dark: bool,
    /// The editor theme colours used in the diagram.
    palette: Palette,
}

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
    /// Link destinations named by the rendered spans of each document.
    links: RefCell<BTreeMap<FileId, Vec<String>>>,
    /// The blocks each file last parsed into, and at which version.
    parsed: RefCell<BTreeMap<FileId, Parsed>>,
    /// The pictures the documents name, by where they are, decoded away
    /// from the window.
    pictures: Decodes<PathBuf>,
    /// Diagram images, including failed renders, scoped to their document.
    diagrams: Decodes<(FileId, DiagramKey)>,
    /// How far each diagram is zoomed and panned, by its file and its place
    /// among that file's diagrams.
    zooms: RefCell<BTreeMap<(FileId, usize), Zoomed>>,
}

impl Renders {
    /// Returns a stable message index for a rendered link destination.
    pub fn link(&self, file: FileId, target: &str) -> usize {
        let mut links = self.links.borrow_mut();
        let links = links.entry(file).or_default();
        if let Some(index) = links.iter().position(|link| link == target) {
            return index;
        }
        links.push(target.to_owned());
        links.len() - 1
    }

    /// Reads the destination named by a rendered span's message.
    pub fn linked(&self, file: FileId, index: usize) -> Option<String> {
        self.links.borrow().get(&file)?.get(index).cloned()
    }

    /// How far the rendering of `file` is scrolled.
    pub fn scroll(&self, file: FileId) -> Scrolled {
        self.scrolls.borrow_mut().entry(file).or_default().clone()
    }

    /// The blocks the text of `file` at `version` reads as, asking `source`
    /// for that text only when the version has not been parsed yet.
    pub fn blocks(
        &self,
        file: FileId,
        version: i32,
        source: impl FnOnce() -> String,
    ) -> Rc<Vec<Block>> {
        let mut parsed = self.parsed.borrow_mut();
        match parsed.get(&file) {
            Some((at, blocks)) if *at == version => blocks.clone(),
            _ => {
                let blocks = Rc::new(blocks::blocks(&source()));
                parsed.insert(file, (version, blocks.clone()));
                blocks
            }
        }
    }

    /// The picture at `path`, once it is decoded; the first asking starts
    /// the reading.
    pub fn picture(&self, path: &Path) -> Option<Image> {
        let path = path.to_path_buf();
        self.pictures.get(&path, read_file(path.clone())).ready()
    }

    /// Returns a diagram raster, rendering it only once per source, scale and theme.
    pub fn diagram(&self, file: FileId, source: &str, scale: f32, theme: &Theme) -> Decoding {
        let palette = Palette::from_theme(theme);
        let key = DiagramKey {
            source: source.to_owned(),
            scale: scale.to_bits(),
            dark: theme.appearance == Appearance::Dark,
            palette: palette.clone(),
        };
        let source = source.to_owned();
        let appearance = theme.appearance;
        self.diagrams.get_image(&(file, key), move || {
            palette
                .render(&source, scale, appearance)
                .ok_or_else(|| "Diagram could not be drawn".to_owned())
        })
    }

    /// How far the diagram at place `index` in `file` is zoomed.
    pub fn zoom(&self, file: FileId, index: usize) -> Zoomed {
        self.zooms
            .borrow_mut()
            .entry((file, index))
            .or_default()
            .clone()
    }

    /// Forgets the zoom of every diagram of `file` past the first `count`,
    /// the ones the document no longer has.
    pub fn keep_zooms(&self, file: FileId, count: usize) {
        self.zooms
            .borrow_mut()
            .retain(|(held, index), _| *held != file || *index < count);
    }

    /// Takes `step` on the zoom of the diagram at place `index` in `file`.
    pub fn step_zoom(&self, file: FileId, index: usize, step: DiagramZoom) {
        self.change_zoom(file, index, |zoom| match step {
            DiagramZoom::In => zoom.zoom_centred(ZOOM_STEP),
            DiagramZoom::Out => zoom.zoom_centred(1.0 / ZOOM_STEP),
            DiagramZoom::Reset => zoom.reset(),
        });
    }

    /// Moves the diagram at place `index` in `file` with the pointer dragging it.
    pub fn pan(&self, file: FileId, index: usize, event: ResizeEvent) {
        self.change_zoom(file, index, |zoom| zoom.pan(event));
    }

    /// Multiplies the zoom of the diagram of `file` under `pointer` by
    /// `multiplier`, around the pointer, saying whether there was one.
    pub fn zoom_under(&self, file: FileId, pointer: Point, multiplier: f32) -> bool {
        let zooms = self.zooms.borrow();
        let Some(zoomed) = zooms
            .iter()
            .find(|((held, _), zoomed)| *held == file && zoomed.get().contains(pointer))
            .map(|(_, zoomed)| zoomed)
        else {
            return false;
        };
        let mut zoom = zoomed.get();
        zoom.zoom_at(multiplier, pointer);
        zoomed.set(zoom);
        true
    }

    /// Applies `change` to the zoom of the diagram at place `index` in `file`.
    fn change_zoom(&self, file: FileId, index: usize, change: impl FnOnce(&mut Zoom)) {
        let zoomed = self.zoom(file, index);
        let mut zoom = zoomed.get();
        change(&mut zoom);
        zoomed.set(zoom);
    }

    /// Forgets what it kept for any file no pane is rendering any more, and
    /// the pictures, which a document shown again reads afresh.
    pub fn retain(&mut self, held: &BTreeSet<FileId>) {
        self.scrolls.get_mut().retain(|file, _| held.contains(file));
        self.links.get_mut().retain(|file, _| held.contains(file));
        self.parsed.get_mut().retain(|file, _| held.contains(file));
        self.diagrams.retain(|(file, _)| held.contains(file));
        self.zooms
            .get_mut()
            .retain(|(file, _), _| held.contains(file));
        if held.is_empty() {
            self.pictures.clear();
        }
    }
}
