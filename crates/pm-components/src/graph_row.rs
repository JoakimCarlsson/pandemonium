//! One commit of a commit graph: its slice of the lanes drawn as lines and
//! a dot, then its name, the references pointing at it and its summary.
//!
//! Each row paints only the lines crossing its own upper and lower halves,
//! so rows stacked on each other join into one picture without any row
//! knowing about its neighbours.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::{ExtendedColors, extended_colors};
use crate::badge::{Badge, BadgeVariant};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::{ColorScheme, color_scheme};

/// How wide one lane of the graph is.
const LANE: f32 = 12.0;

/// One line through half of a row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GraphLine {
    /// Whether it crosses the upper half, from the top edge to the dot, rather than the lower.
    pub upper: bool,
    /// The lane it starts in.
    pub from: usize,
    /// The lane it ends in.
    pub to: usize,
    /// The colour index of its branch.
    pub color: usize,
}

/// How a graph row reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GraphRowVariant {
    /// An ordinary commit.
    #[default]
    Plain,
    /// The commit checked out, its dot ringed.
    Head,
}

/// A commit graph row with a variant and a size.
#[derive(Clone)]
pub struct GraphRow {
    /// The commit's abbreviated name.
    id: String,
    /// The first line of its message.
    summary: String,
    /// The branches and tags pointing at it.
    refs: Vec<String>,
    /// The lane its dot sits in.
    lane: usize,
    /// The colour index of its branch.
    color: usize,
    /// How many lanes the graph is wide.
    columns: usize,
    /// The lines crossing its row.
    lines: Vec<GraphLine>,
    /// How it reads.
    variant: GraphRowVariant,
    /// Its height.
    size: Size,
    /// What a click does.
    on_click: Option<Rc<dyn Fn()>>,
    /// What a secondary click does.
    on_menu: Option<Rc<dyn Fn()>>,
}

impl GraphRow {
    /// A row for the commit `id` with `summary`, its dot in `lane`.
    pub fn new(id: impl Into<String>, summary: impl Into<String>, lane: usize) -> Self {
        Self {
            id: id.into(),
            summary: summary.into(),
            refs: Vec::new(),
            lane,
            color: lane,
            columns: lane + 1,
            lines: Vec::new(),
            variant: GraphRowVariant::Plain,
            size: Size::Sm,
            on_click: None,
            on_menu: None,
        }
    }

    /// The row showing `refs` before the summary.
    pub fn refs(mut self, refs: Vec<String>) -> Self {
        self.refs = refs;
        self
    }

    /// The row with its dot in the colour of index `color`.
    pub fn color(mut self, color: usize) -> Self {
        self.color = color;
        self
    }

    /// The row's lanes drawn `columns` wide, so every row lines up.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(self.lane + 1);
        self
    }

    /// The row drawing `lines` through it.
    pub fn lines(mut self, lines: Vec<GraphLine>) -> Self {
        self.lines = lines;
        self
    }

    /// The row in `variant`.
    pub fn variant(mut self, variant: GraphRowVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The row at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The row calling `on_click` when clicked.
    pub fn on_click(mut self, on_click: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// The row calling `on_menu` on a secondary click.
    pub fn on_menu(mut self, on_menu: impl Fn() + 'static) -> Self {
        self.on_menu = Some(Rc::new(on_menu));
        self
    }

    /// Emits the row.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let hovered = remember(|| false);
        let height = match self.size {
            Size::Xs => 20.0,
            Size::Sm => 22.0,
            Size::Md => 26.0,
            Size::Lg => 30.0,
        };
        let height = crate::row_height(height);
        let mut modifier = Modifier::new()
            .fill_max_width()
            .height(height)
            .background_rounded(
                if hovered.get() {
                    scheme.surface_container_high
                } else {
                    Color::TRANSPARENT
                },
                4.0,
            )
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            });
        if let Some(on_click) = self.on_click.clone() {
            modifier = modifier
                .cursor(CursorIcon::Pointer)
                .clickable(move || on_click());
        }
        if let Some(on_menu) = self.on_menu.clone() {
            modifier = modifier.on_context_menu(move |_| on_menu());
        }
        let cell = Cell {
            lines: self.lines.clone(),
            lane: self.lane,
            color: self.color,
            head: self.variant == GraphRowVariant::Head,
            scheme,
            extended,
        };
        let width = self.columns as f32 * LANE;
        row_arranged(
            modifier.padding_xy(4.0, 0.0),
            Arrangement::SpacedBy(crate::spacing(6.0)),
            Alignment::Center,
            move || {
                canvas(Modifier::new().size(width, height), move |canvas| {
                    cell.draw(canvas)
                });
                row_arranged(
                    Modifier::new().weight(1.0).clip(),
                    Arrangement::SpacedBy(crate::spacing(5.0)),
                    Alignment::Center,
                    || {
                        for (place, name) in self.refs.iter().enumerate() {
                            key(place, || reference(name));
                        }
                        Label::new(self.summary.clone()).size(Size::Xs).show();
                    },
                );
                Label::new(self.id.clone())
                    .variant(LabelVariant::Code)
                    .colour(scheme.on_surface_variant)
                    .size(Size::Xs)
                    .show();
            },
        );
    }
}

/// Emits a badge for the reference `name` as git decorates it, the checked out branch in the accent tone.
fn reference(name: &str) {
    let (shown, variant) = match name.strip_prefix("HEAD -> ") {
        Some(branch) => (branch.to_owned(), BadgeVariant::Accent),
        None => match name.strip_prefix("tag: ") {
            Some(tag) => (tag.to_owned(), BadgeVariant::Warning),
            None if name.contains('/') => (name.to_owned(), BadgeVariant::Neutral),
            None => (name.to_owned(), BadgeVariant::Success),
        },
    };
    Badge::new(shown).variant(variant).size(Size::Xs).show();
}

/// What a row's canvas draws, taken out of the builder.
struct Cell {
    /// The lines crossing the row.
    lines: Vec<GraphLine>,
    /// The lane of the dot.
    lane: usize,
    /// The colour index of the dot.
    color: usize,
    /// Whether the dot is ringed for the commit checked out.
    head: bool,
    /// The scheme the colours come from.
    scheme: ColorScheme,
    /// The extended colours the colours come from.
    extended: ExtendedColors,
}

impl Cell {
    /// The colour of the branch with index `color`.
    fn colour(&self, color: usize) -> Color {
        let tones = [
            self.scheme.primary,
            self.extended.success,
            self.extended.warning,
            self.scheme.error,
            self.scheme.on_surface_variant,
        ];
        tones[color % tones.len()]
    }

    /// Paints the lines and the dot onto `canvas`.
    fn draw(&self, canvas: &mut Canvas) {
        let height = canvas.height();
        let middle = height / 2.0;
        let x = |lane: usize| LANE / 2.0 + lane as f32 * LANE;
        for line in &self.lines {
            let (top, bottom) = if line.upper {
                (0.0, middle)
            } else {
                (middle, height)
            };
            let tone = self.colour(line.color);
            if line.from == line.to {
                canvas.line((x(line.from), top), (x(line.to), bottom), 1.5, tone);
            } else if line.upper {
                canvas.polyline(
                    &[
                        (x(line.from), top),
                        (x(line.from), top + (bottom - top) * 0.4),
                        (x(line.to), bottom),
                    ],
                    1.5,
                    tone,
                );
            } else {
                canvas.polyline(
                    &[
                        (x(line.from), top),
                        (x(line.to), top + (bottom - top) * 0.6),
                        (x(line.to), bottom),
                    ],
                    1.5,
                    tone,
                );
            }
        }
        let tone = self.colour(self.color);
        if self.head {
            canvas.circle(x(self.lane), middle, 4.5, self.scheme.background);
            canvas.stroke_circle(x(self.lane), middle, 4.0, 1.5, tone);
        } else {
            canvas.circle(x(self.lane), middle, 3.5, tone);
        }
    }
}
