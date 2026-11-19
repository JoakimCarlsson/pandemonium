//! A file shown rendered rather than as its source: markdown laid out as a page, and pictures drawn whole.

use crate::focus::FocusId;
use crate::theme;
use crate::theme::sp;
use crate::theme::{Radius, Role};
use rustle::Modifier;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};

use crate::Button;
use crate::button::Variant;
use crate::figures::Figures;
use crate::markdown::Markdown;
use crate::open_files::{Contents, STRIP, Viewer};
use crate::parts::line;
use crate::size::Size;

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

/// Emits the switch between the rendering of the file and its source, when the file has both and its text has arrived.
#[track_caller]
pub fn switch(viewer: &Viewer) {
    let Some(rendering) = Rendering::of(&viewer.path).filter(|rendering| rendering.has_source())
    else {
        return;
    };
    if !matches!(viewer.contents, Contents::Text(_)) {
        return;
    }
    let shown = viewer.rendering() == Some(rendering);
    row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(sp(1)),
        Alignment::Center,
        || {
            Button::new("Preview", PREVIEW_SHOW)
                .variant(Variant::Ghost)
                .size(Size::Xs)
                .selected(shown)
                .build();
            Button::new("Source", SOURCE_SHOW)
                .variant(Variant::Ghost)
                .size(Size::Xs)
                .selected(!shown)
                .build();
        },
    );
}

/// Emits what the strip says about the picture of the file shown: how many pixels it has across and down.
#[track_caller]
pub fn dimensions(viewer: &Viewer) {
    let Contents::Picture(picture) = &viewer.contents else {
        return;
    };
    line(
        format!("{} × {}", picture.width(), picture.height()),
        11,
        1.4,
        Role::Muted,
    );
}

/// Emits the file shown rendered, its diagrams and images drawn as `figures` holds them, in a view `width` by `height` logical pixels, strip included; emits nothing when it is shown as its source.
///
/// Returns whether it emitted the rendering.
#[track_caller]
pub fn rendered(viewer: &Viewer, figures: &Figures, (width, height): (f32, f32)) -> bool {
    let room = (width, (height - STRIP).max(0.0));
    let Some(rendering) = viewer.rendering() else {
        return false;
    };
    match (rendering, &viewer.contents) {
        (Rendering::Markdown, Contents::Text(document)) => {
            page(&viewer.path, document.field.text(), figures, width);
        }
        (Rendering::Vector, Contents::Text(_)) => match &viewer.drawn {
            Some(Ok(picture)) => framed(picture, VECTOR_DENSITY, room),
            Some(Err(reason)) => note(reason),
            None => note("Drawing the picture."),
        },
        (Rendering::Picture, Contents::Picture(picture)) => framed(picture, 1.0, room),
        _ => return false,
    }
    true
}

/// Emits a line saying `message` where the rendering would be.
#[track_caller]
fn note(message: &str) {
    column_arranged(
        Modifier::new()
            .padding_xy(theme::sp(4), 0.0)
            .padding_xy(0.0, theme::sp(4)),
        Arrangement::Start,
        Alignment::Start,
        || {
            line(message, 13, 1.4, Role::Subtle);
        },
    );
}

/// Emits the markdown `source` of the file at `path` laid out as a page in the middle of a view `width` logical pixels wide, its links pressable and its diagrams and images drawn as `figures` holds them.
#[track_caller]
fn page(path: &str, source: &str, figures: &Figures, width: f32) {
    let column = (width - 2.0 * MARGIN).clamp(120.0, PAGE_WIDTH);
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_xy(0.0, theme::sp(8)),
        Arrangement::Center,
        Alignment::Start,
        || {
            column_arranged(
                Modifier::new().width(column),
                Arrangement::Start,
                Alignment::Start,
                || {
                    Markdown::new(source, column)
                        .links(PREVIEW_LINK_BASE)
                        .figures(figures, Some(path))
                        .build();
                },
            );
        },
    );
}

/// Emits `picture`, `density` of its pixels to a logical pixel, shrunk to fit a view `room` logical pixels across and down and set in its middle on a plain backing.
#[track_caller]
fn framed(picture: &rustle::ImageData, density: f32, room: (f32, f32)) {
    let (across, down) = (
        picture.width() as f32 / density,
        picture.height() as f32 / density,
    );
    let (wide, tall) = (
        (room.0 - 2.0 * MARGIN).max(16.0),
        (room.1 - 2.0 * MARGIN).max(16.0),
    );
    let fit = (wide / across.max(1.0)).min(tall / down.max(1.0)).min(1.0);
    row_arranged(
        Modifier::new().weight(1.0).fill_max_width(),
        Arrangement::Center,
        Alignment::Center,
        || {
            rustle::image(
                Modifier::new()
                    .width(across * fit)
                    .height(down * fit)
                    .background_rounded(theme::color(Role::Surface), Radius::Sm.px())
                    .border(1.0, theme::color(Role::Border), Radius::Sm.px())
                    .padding(1.0),
                picture,
                rustle::ContentScale::Crop,
            );
        },
    );
}
