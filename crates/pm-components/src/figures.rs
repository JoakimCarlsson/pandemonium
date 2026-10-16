//! The pictures a markdown source shows: diagrams written as fenced mermaid code, formulas written in TeX and images it names, each drawn or fetched into pixels and kept under a key.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use pm_input::FocusId;
use pm_markdown::{Block, Image};
use pm_style::{
    Role, cursor_pointer, flex, flex_none, flex_wrap, gap, h_px, items_end, justify_center, w_full,
    w_px,
};
use pm_ui::{Node, Picture, node};

use crate::parts::line;
use crate::preview::linked_path;
use crate::state::{Outgoing, WorkspaceState};

/// Pixels a figure was drawn into, and how many of them fall on one logical pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    /// The pixels.
    pub picture: Picture,
    /// How many pixels fall on one logical pixel across: more than one for a picture drawn sharp for a dense screen.
    pub density: f32,
}

impl Drawing {
    /// How wide and tall the drawing is at its own size, in logical pixels.
    fn size(&self) -> (f32, f32) {
        (
            self.picture.width as f32 / self.density,
            self.picture.height as f32 / self.density,
        )
    }
}

/// The figures drawn or fetched so far, by their key, each a drawing or why there is none.
pub type Figures = HashMap<u64, Result<Drawing, String>>;

/// What has to be done for a figure to be shown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Wanted {
    /// A diagram drawn from what it is written as.
    Diagram(String),
    /// A formula set from the TeX it is written in.
    Formula(String),
    /// A file of the repository read, by its path from the root.
    File(String),
    /// A picture fetched from the web, by its address.
    Address(String),
}

/// Whether a fence naming `language` holds a diagram.
pub fn is_diagram(language: &str) -> bool {
    language.eq_ignore_ascii_case("mermaid")
}

/// `value` hashed into the key a figure is kept under.
fn key_of(value: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// The key a diagram written as `source` is kept under.
pub fn diagram_key(source: &str) -> u64 {
    key_of(("diagram", source))
}

/// The key a formula written in TeX as `source` is kept under.
pub fn formula_key(source: &str) -> u64 {
    key_of(("formula", source))
}

/// The key the image at `source`, named by a markdown file at `place` or by text in no file, is kept under.
pub fn image_key(place: Option<&str>, source: &str) -> u64 {
    key_of(("image", place, source))
}

/// What showing the image at `source` takes, named by a markdown file at `place` or by text in no file: a web address is fetched, and a path is read from the repository when there is a file to read it from.
fn wanted_image(place: Option<&str>, source: &str) -> Option<Wanted> {
    if source.starts_with("https://") || source.starts_with("http://") {
        return Some(Wanted::Address(source.to_owned()));
    }
    linked_path(place?, source).map(Wanted::File)
}

/// Every figure of the markdown `source`, named by a file at `place` or by text in no file, nested ones included, each with its key and what showing it takes, in the order they are written.
pub fn figures_of(source: &str, place: Option<&str>) -> Vec<(u64, Wanted)> {
    let mut found = Vec::new();
    collect(&pm_markdown::parse(source), place, &mut found);
    found
}

/// Adds the figures among `blocks` and the blocks inside them, named by a file at `place`, to `found`.
fn collect(blocks: &[Block], place: Option<&str>, found: &mut Vec<(u64, Wanted)>) {
    for block in blocks {
        match block {
            Block::Code { language, text } if is_diagram(language) => {
                found.push((diagram_key(text), Wanted::Diagram(text.clone())));
            }
            Block::Math(text) => found.push((formula_key(text), Wanted::Formula(text.clone()))),
            Block::Images { images, .. } => found.extend(images.iter().filter_map(|image| {
                wanted_image(place, &image.source)
                    .map(|wanted| (image_key(place, &image.source), wanted))
            })),
            Block::List { items, .. } => {
                for item in items {
                    collect(item, place, found);
                }
            }
            Block::Quote(inner) | Block::Details { blocks: inner, .. } => {
                collect(inner, place, found)
            }
            _ => {}
        }
    }
}

/// What `figures` holds for each figure of the markdown `source` named by a file at `place`, as one number each that changes when its drawing arrives or fails, so a layout kept of the source is made again then.
pub(crate) fn figure_marks(source: &str, place: Option<&str>, figures: &Figures) -> Vec<u64> {
    figures_of(source, place)
        .iter()
        .map(|(key, _)| match figures.get(key) {
            None => 0,
            Some(Ok(drawing)) => drawing.picture.key,
            Some(Err(_)) => 1,
        })
        .collect()
}

/// `drawing` `across` by `down` logical pixels.
fn drawn(drawing: &Drawing, across: f32, down: f32) -> Node {
    node()
        .styles([w_px(across), h_px(down), flex_none()])
        .picture(drawing.picture.clone())
}

/// `drawing`, a drawn diagram or formula, at its own size or shrunk to `width` logical pixels, in the middle of a row that wide.
pub(crate) fn diagram_node(drawing: &Drawing, width: f32) -> Node {
    let (across, down) = drawing.size();
    let fit = (width / across.max(1.0)).min(1.0);
    node()
        .styles([flex(), justify_center(), w_full(), flex_none()])
        .child(drawn(drawing, across * fit, down * fit))
}

/// How big `image` is drawn from `drawing`: the size the source asks for, the other side following the picture's shape when only one is asked, or else the drawing's own size, shrunk to `width` logical pixels.
fn image_size(image: &Image, drawing: &Drawing, width: f32) -> (f32, f32) {
    let (across, down) = drawing.size();
    let shape = down / across.max(1.0);
    let (across, down) = match (image.width, image.height) {
        (Some(wide), Some(tall)) => (wide as f32, tall as f32),
        (Some(wide), None) => (wide as f32, wide as f32 * shape),
        (None, Some(tall)) => (tall as f32 / shape.max(f32::EPSILON), tall as f32),
        (None, None) => (across, down),
    };
    let fit = (width / across.max(1.0)).min(1.0);
    (across * fit, down * fit)
}

/// `images` side by side, wrapping onto more rows within `width` logical pixels and set in the middle when `centered`, each drawn as `figures` holds it under `key`, and its words in `pixels` tall type in its place until then; an image that is the face of a link answers to the link's name from `links` up.
pub(crate) fn images_node(
    images: &[Image],
    centered: bool,
    figures: Option<&Figures>,
    key: impl Fn(&Image) -> u64,
    (links, pixels, width): (Option<u64>, u32, f32),
) -> Node {
    let mut row = node().styles([flex(), flex_wrap(), items_end(), gap(1.5), w_full()]);
    if centered {
        row = row.styles([justify_center()]);
    }
    row.children(images.iter().filter_map(|image| {
        let shown = match figures.and_then(|figures| figures.get(&key(image))) {
            Some(Ok(drawing)) => {
                let (across, down) = image_size(image, drawing, width);
                drawn(drawing, across, down)
            }
            _ if image.alt.is_empty() => return None,
            _ => line(&image.alt, pixels.saturating_sub(1), 1.4, Role::Subtle),
        };
        Some(match (links, image.target) {
            (Some(base), target) if target > 0 => shown
                .styles([cursor_pointer()])
                .focusable(FocusId(base + u64::from(target) - 1)),
            _ => shown,
        })
    }))
}

impl WorkspaceState {
    /// Asks for each of the figures `wanted` that is neither drawn nor asked for yet; a file of a repository is read from the folder of the session at `session`, and not asked for without one.
    pub(crate) fn ask_figures(&mut self, wanted: Vec<(u64, Wanted)>, session: Option<usize>) {
        for (key, wanted) in wanted {
            if self.figures.contains_key(&key) || self.figures_asked.contains(&key) {
                continue;
            }
            let asked = match (wanted, session) {
                (Wanted::Diagram(source), _) => Outgoing::DrawDiagram { key, source },
                (Wanted::Formula(source), _) => Outgoing::DrawFormula { key, source },
                (Wanted::Address(address), _) => Outgoing::FetchFigure { key, address },
                (Wanted::File(path), Some(session)) => Outgoing::ReadFigure { key, session, path },
                (Wanted::File(_), None) => continue,
            };
            self.figures_asked.insert(key);
            self.outbox.push(asked);
        }
    }

    /// Asks for the figures of the last reply of the session at `session`, once it is finished.
    pub(crate) fn ask_reply_figures(&mut self, session: usize) {
        let wanted = self
            .sessions
            .get(session)
            .and_then(|held| {
                held.messages
                    .iter()
                    .rev()
                    .find(|message| !message.from_user)
            })
            .map(|message| {
                message
                    .chunks()
                    .into_iter()
                    .flat_map(|(_, piece)| figures_of(piece, None))
                    .collect()
            })
            .unwrap_or_default();
        self.ask_figures(wanted, None);
    }

    /// The state after the figure kept under `key` was drawn or fetched, or could not be.
    pub fn with_figure(mut self, key: u64, drawn: Result<Drawing, String>) -> Self {
        self.figures_asked.remove(&key);
        self.figures.insert(key, drawn);
        self
    }

    /// The state with every figure let go, as when the colours diagrams are drawn in change, and those of the file shown asked for again.
    pub fn without_figures(mut self) -> Self {
        self.figures.clear();
        self.figures_asked.clear();
        self.ask_shown_figures();
        self
    }
}
