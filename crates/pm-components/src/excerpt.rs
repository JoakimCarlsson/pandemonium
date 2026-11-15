//! An excerpt of a file: a header naming the file, with its icon, folder
//! and how many lines are shown, over those lines with their numbers and
//! the stretch of each that matched highlighted.

use std::ops::Range;
use std::rc::Rc;

use rustle::prelude::*;

use crate::badge::Badge;
use crate::file_icon::FileIcon;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How an excerpt is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExcerptVariant {
    /// Lines straight under the header.
    #[default]
    Plain,
    /// The whole on a sunken surface with a hairline border.
    Boxed,
}

/// A line of an excerpt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExcerptLine {
    /// Its number, counted from 1.
    pub number: usize,
    /// Its text.
    pub text: String,
    /// The characters of `text` highlighted.
    pub highlight: Range<usize>,
}

/// An excerpt with a variant and a size.
#[derive(Clone)]
pub struct Excerpt {
    /// The file, from the root of the project.
    path: String,
    /// The lines shown.
    lines: Vec<ExcerptLine>,
    /// Whether the lines are shown, or only the header.
    expanded: bool,
    /// How it is drawn.
    variant: ExcerptVariant,
    /// Its size.
    size: Size,
    /// What clicking the header does.
    on_toggle: Option<Rc<dyn Fn()>>,
    /// What clicking a line does, given its number.
    on_open: Option<Rc<dyn Fn(usize)>>,
}

impl Excerpt {
    /// An excerpt of the file at `path` showing `lines`.
    pub fn new(path: impl Into<String>, lines: Vec<ExcerptLine>) -> Self {
        Self {
            path: path.into(),
            lines,
            expanded: true,
            variant: ExcerptVariant::Plain,
            size: Size::Sm,
            on_toggle: None,
            on_open: None,
        }
    }

    /// The excerpt showing its lines when `expanded`, its header only otherwise.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// The excerpt in `variant`.
    pub fn variant(mut self, variant: ExcerptVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The excerpt at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The excerpt calling `on_toggle` when its header is clicked.
    pub fn on_toggle(mut self, on_toggle: impl Fn() + 'static) -> Self {
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }

    /// The excerpt calling `on_open` with a line's number when the line is clicked.
    pub fn on_open(mut self, on_open: impl Fn(usize) + 'static) -> Self {
        self.on_open = Some(Rc::new(on_open));
        self
    }

    /// Emits the excerpt.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let line_height = match self.size {
            Size::Xs => 20.0,
            Size::Sm => 22.0,
            Size::Md => 24.0,
            Size::Lg => 28.0,
        };
        let mut modifier = Modifier::new().fill_max_width();
        if self.variant == ExcerptVariant::Boxed {
            modifier = modifier
                .background_rounded(scheme.surface_container, 6.0)
                .border(1.0, scheme.outline_variant, 6.0)
                .padding(4.0);
        }
        let (folder, name) = self.path.rsplit_once('/').unwrap_or(("", &self.path));
        let (folder, name) = (folder.to_owned(), name.to_owned());
        let count = self.lines.len();
        let expanded = self.expanded;
        let on_toggle = self.on_toggle.clone();
        let on_open = self.on_open.clone();
        let lines = self.lines;
        column_with(modifier, move || {
            let mut header = Modifier::new()
                .fill_max_width()
                .height(line_height + 2.0)
                .padding_xy(4.0, 0.0);
            if let Some(on_toggle) = on_toggle {
                header = header
                    .cursor(CursorIcon::Pointer)
                    .clickable(move || on_toggle());
            }
            row_arranged(
                header,
                Arrangement::SpacedBy(6.0),
                Alignment::Center,
                || {
                    icon(
                        if expanded {
                            phosphor::CARET_DOWN
                        } else {
                            phosphor::CARET_RIGHT
                        },
                        10.0,
                        scheme.on_surface_variant,
                    );
                    FileIcon::new(name.clone()).size(Size::Sm).show();
                    Label::new(name.clone()).size(Size::Sm).show();
                    Label::new(folder.clone())
                        .variant(LabelVariant::Subtle)
                        .size(Size::Xs)
                        .modifier(Modifier::new().weight(1.0))
                        .show();
                    Badge::new(count.to_string()).size(Size::Xs).show();
                },
            );
            if !expanded {
                return;
            }
            for line in lines {
                key(line.number, || {
                    excerpt_line(line.clone(), line_height, on_open.clone())
                });
            }
        });
    }
}

/// One line of an excerpt, `height` tall, calling `on_open` with its number when clicked.
#[track_caller]
fn excerpt_line(line: ExcerptLine, height: f32, on_open: Option<Rc<dyn Fn(usize)>>) {
    let scheme = color_scheme();
    let hovered = remember(|| false);
    let fill = if hovered.get() {
        scheme.surface_container_high
    } else {
        Color::TRANSPARENT
    };
    let mut modifier = Modifier::new()
        .fill_max_width()
        .height(height)
        .background_rounded(fill, 4.0)
        .on_hover({
            let hovered = hovered.clone();
            move |inside| hovered.set(inside)
        })
        .padding_xy(4.0, 0.0);
    if let Some(on_open) = on_open {
        let number = line.number;
        modifier = modifier
            .cursor(CursorIcon::Pointer)
            .clickable(move || on_open(number));
    }
    let lead = line
        .text
        .chars()
        .take_while(|letter| letter.is_whitespace())
        .count();
    let letters: Vec<char> = line.text.chars().skip(lead).collect();
    let start = line.highlight.start.saturating_sub(lead).min(letters.len());
    let end = line
        .highlight
        .end
        .saturating_sub(lead)
        .clamp(start, letters.len());
    let before: String = letters[..start].iter().collect();
    let found: String = letters[start..end].iter().collect();
    let after: String = letters[end..].iter().collect();
    row_arranged(
        modifier,
        Arrangement::SpacedBy(0.0),
        Alignment::Center,
        || {
            stack_aligned(
                Modifier::new().width(40.0).padding_xy(0.0, 0.0),
                BoxAlignment::CenterEnd,
                || {
                    Label::new(line.number.to_string())
                        .variant(LabelVariant::Code)
                        .colour(scheme.on_surface_variant)
                        .size(Size::Xs)
                        .show();
                },
            );
            spacer(Modifier::new().width(10.0));
            row_with(Modifier::new().weight(1.0).clip(), || {
                Label::new(before)
                    .variant(LabelVariant::Code)
                    .colour(scheme.on_surface_variant)
                    .size(Size::Xs)
                    .show();
                if !found.is_empty() {
                    Label::new(found)
                        .variant(LabelVariant::Code)
                        .size(Size::Xs)
                        .modifier(Modifier::new().background_rounded(tinted(scheme.primary), 2.0))
                        .show();
                }
                Label::new(after)
                    .variant(LabelVariant::Code)
                    .colour(scheme.on_surface_variant)
                    .size(Size::Xs)
                    .show();
            });
        },
    );
}

/// `colour` faded to about a third, for a highlight behind text.
fn tinted(colour: Color) -> Color {
    Color::rgba(colour.r, colour.g, colour.b, 90)
}
