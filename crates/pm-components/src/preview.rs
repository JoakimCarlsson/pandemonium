//! A file shown rendered rather than as its source: markdown laid out as a page, and pictures drawn whole.

use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_1, flex_none, gap, h_px, items_center,
    justify_center, min_h, px, py, rounded, w_px,
};
use pm_text::Shaper;
use pm_ui::{Node, Picture, node};

use crate::chip::{Chip, Size, Variant};
use crate::figures::Figures;
use crate::markdown::Markdown;
use crate::open_files::{Contents, STRIP, Viewer};
use crate::parts::line;

/// Shows the file shown rendered.
pub const PREVIEW_SHOW: FocusId = FocusId(7094);

/// Shows the source of the file shown.
pub const SOURCE_SHOW: FocusId = FocusId(7095);

/// The first link of a markdown file shown rendered; the rest follow it.
pub const PREVIEW_LINK_BASE: u64 = 4_200_000;

/// How many links of a markdown file shown rendered can be pressed.
pub const PREVIEW_LINKS: u64 = 100_000;

/// How many pixels of a drawn vector picture fall on one logical pixel, so it stays sharp on a dense screen.
pub const VECTOR_DENSITY: f32 = 2.0;

/// The widest a page of markdown is set, in logical pixels.
const PAGE_WIDTH: f32 = 760.0;

/// The room kept around a page or a picture, in logical pixels.
const MARGIN: f32 = 32.0;

/// How a file can be shown other than as its source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rendering {
    /// Markdown, laid out as a page.
    Markdown,
    /// A vector picture, drawn from its text, whose source can be shown and edited too.
    Vector,
    /// A picture of pixels, which has no source to show.
    Picture,
}

impl Rendering {
    /// How the file at `path` is rendered, as the end of its name says, or nothing for a file only shown as source.
    pub fn of(path: &str) -> Option<Self> {
        let name = path.rsplit('/').next().unwrap_or(path);
        let (_, extension) = name.rsplit_once('.')?;
        match extension.to_ascii_lowercase().as_str() {
            "md" | "markdown" | "mdown" | "mkd" => Some(Self::Markdown),
            "svg" => Some(Self::Vector),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" => Some(Self::Picture),
            _ => None,
        }
    }

    /// Whether the file also has a source the reader can switch to.
    pub fn has_source(self) -> bool {
        self != Self::Picture
    }
}

/// The address a link `address` of the file at `from` points to inside the repository, from its root, or nothing when it points elsewhere or nowhere.
pub fn linked_path(from: &str, address: &str) -> Option<String> {
    let address = address.split(['#', '?']).next().unwrap_or_default();
    if address.is_empty() || address.contains(':') {
        return None;
    }
    let (start, rest) = match address.strip_prefix('/') {
        Some(rest) => ("", rest),
        None => (
            from.rsplit_once('/').map_or("", |(folder, _)| folder),
            address,
        ),
    };
    let mut parts: Vec<&str> = start.split('/').filter(|part| !part.is_empty()).collect();
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The switch between the rendering of the file and its source, when the file has both and its text has arrived.
pub fn switch(viewer: &Viewer) -> Option<Node> {
    let rendering = Rendering::of(&viewer.path).filter(|rendering| rendering.has_source())?;
    if !matches!(viewer.contents, Contents::Text(_)) {
        return None;
    }
    let shown = viewer.rendering() == Some(rendering);
    Some(
        node()
            .styles([flex(), items_center(), gap(1), flex_none()])
            .child(
                Chip::new("Preview", PREVIEW_SHOW)
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .selected(shown)
                    .build(),
            )
            .child(
                Chip::new("Source", SOURCE_SHOW)
                    .variant(Variant::Ghost)
                    .size(Size::Sm)
                    .selected(!shown)
                    .build(),
            ),
    )
}

/// What the strip says about the picture of the file shown: how many pixels it has across and down.
pub fn dimensions(viewer: &Viewer) -> Option<Node> {
    let Contents::Picture(picture) = &viewer.contents else {
        return None;
    };
    Some(line(
        format!("{} × {}", picture.width, picture.height),
        11,
        1.4,
        Role::Muted,
    ))
}

/// The file shown rendered, its diagrams and images drawn as `figures` holds them, in a view `width` by `height` logical pixels, strip included, or nothing when it is shown as its source.
pub fn rendered(
    viewer: &Viewer,
    figures: &Figures,
    (width, height): (f32, f32),
    shaper: &mut Shaper,
) -> Option<Node> {
    let room = (width, (height - STRIP).max(0.0));
    match (viewer.rendering()?, &viewer.contents) {
        (Rendering::Markdown, Contents::Text(document)) => Some(page(
            &viewer.path,
            document.field.text(),
            figures,
            width,
            shaper,
        )),
        (Rendering::Vector, Contents::Text(_)) => Some(match &viewer.drawn {
            Some(Ok(picture)) => framed(picture, VECTOR_DENSITY, room),
            Some(Err(reason)) => note(reason),
            None => note("Drawing the picture."),
        }),
        (Rendering::Picture, Contents::Picture(picture)) => Some(framed(picture, 1.0, room)),
        _ => None,
    }
}

/// A line saying `message` where the rendering would be.
fn note(message: &str) -> Node {
    node()
        .styles([px(4), py(4)])
        .child(line(message, 13, 1.4, Role::Subtle))
}

/// The markdown `source` of the file at `path` laid out as a page in the middle of a view `width` logical pixels wide, its links pressable and its diagrams and images drawn as `figures` holds them.
fn page(path: &str, source: &str, figures: &Figures, width: f32, shaper: &mut Shaper) -> Node {
    let column = (width - 2.0 * MARGIN).clamp(120.0, PAGE_WIDTH);
    node()
        .styles([flex(), justify_center(), flex_none(), py(8)])
        .child(
            node().styles([w_px(column), flex_none()]).child(
                Markdown::new(source, column)
                    .links(PREVIEW_LINK_BASE)
                    .figures(figures, Some(path))
                    .build(shaper),
            ),
        )
}

/// `picture`, `density` of its pixels to a logical pixel, shrunk to fit a view `room` logical pixels across and down and set in its middle on a plain backing.
fn framed(picture: &Picture, density: f32, room: (f32, f32)) -> Node {
    let (across, down) = (
        picture.width as f32 / density,
        picture.height as f32 / density,
    );
    let (wide, tall) = (
        (room.0 - 2.0 * MARGIN).max(16.0),
        (room.1 - 2.0 * MARGIN).max(16.0),
    );
    let fit = (wide / across.max(1.0)).min(tall / down.max(1.0)).min(1.0);
    node()
        .styles([flex(), flex_1(), min_h(0), items_center(), justify_center()])
        .child(
            node()
                .styles([
                    w_px(across * fit),
                    h_px(down * fit),
                    flex_none(),
                    bg(Role::Surface),
                    border(),
                    border_color(Role::Border),
                    rounded(Radius::Sm),
                ])
                .picture(picture.clone()),
        )
}
