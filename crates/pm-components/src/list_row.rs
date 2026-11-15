//! A row of a table-like list: a leading mark, a title with a muted note
//! after it, then columns of fixed width, highlighted under the pointer.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use crate::tile::Leading;
use rustle::color_scheme;

/// The widest a row's title grows before it ends in an ellipsis.
const TITLE_WIDTH: f32 = 520.0;

/// How a list row reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListRowVariant {
    /// The title in body text.
    #[default]
    Plain,
    /// The title muted, for something closed or set aside.
    Dimmed,
}

/// How a column's text is drawn and placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColumnStyle {
    /// Muted text from the start.
    Text,
    /// Muted monospace text from the start.
    Code,
    /// Faint text against the end.
    End,
}

/// A column of a list row.
#[derive(Clone, Debug, PartialEq)]
struct Column {
    /// Its text.
    text: String,
    /// Its width, in logical pixels.
    width: f32,
    /// How its text is drawn and placed.
    style: ColumnStyle,
}

/// A table-like row with a variant and a size.
#[derive(Clone)]
pub struct ListRow {
    /// The title.
    title: String,
    /// What it leads with.
    leading: Leading,
    /// An icon before the note.
    note_icon: Option<Icon>,
    /// Muted text after the title, after a chevron.
    note: Option<String>,
    /// Text at the end of the title, in the success and danger tones, such as `+12 −3`.
    tally: Option<(String, String)>,
    /// The columns after the title.
    columns: Vec<Column>,
    /// How it reads.
    variant: ListRowVariant,
    /// Its height.
    size: Size,
    /// What a click does.
    on_click: Option<Rc<dyn Fn()>>,
}

impl ListRow {
    /// A plain row titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            leading: Leading::None,
            note_icon: None,
            note: None,
            tally: None,
            columns: Vec::new(),
            variant: ListRowVariant::Plain,
            size: Size::Md,
            on_click: None,
        }
    }

    /// The row leading with `leading`.
    pub fn leading(mut self, leading: Leading) -> Self {
        self.leading = leading;
        self
    }

    /// The row with muted `note` after its title, after a chevron.
    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// The row with `icon` before its note.
    pub fn note_icon(mut self, icon: Icon) -> Self {
        self.note_icon = Some(icon);
        self
    }

    /// The row with lines `added` and `removed` after its title.
    pub fn tally(mut self, added: usize, removed: usize) -> Self {
        self.tally = Some((format!("+{added}"), format!("−{removed}")));
        self
    }

    /// The row with a column `width` pixels wide of muted `text`.
    pub fn column(mut self, text: impl Into<String>, width: f32) -> Self {
        self.columns.push(Column {
            text: text.into(),
            width,
            style: ColumnStyle::Text,
        });
        self
    }

    /// The row with a column `width` pixels wide of muted monospace `text`.
    pub fn code_column(mut self, text: impl Into<String>, width: f32) -> Self {
        self.columns.push(Column {
            text: text.into(),
            width,
            style: ColumnStyle::Code,
        });
        self
    }

    /// The row with a column `width` pixels wide of faint `text` against its end.
    pub fn end_column(mut self, text: impl Into<String>, width: f32) -> Self {
        self.columns.push(Column {
            text: text.into(),
            width,
            style: ColumnStyle::End,
        });
        self
    }

    /// The row in `variant`.
    pub fn variant(mut self, variant: ListRowVariant) -> Self {
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

    /// Emits the row.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let hovered = remember(|| false);
        let height = match self.size {
            Size::Xs => 30.0,
            Size::Sm => 36.0,
            Size::Md => 44.0,
            Size::Lg => 48.0,
        };
        let height = crate::row_height(height);
        let fill = if hovered.get() && self.on_click.is_some() {
            scheme.surface_container_high
        } else {
            Color::TRANSPARENT
        };
        let mut modifier = Modifier::new()
            .fill_max_width()
            .height(height)
            .background_rounded(fill, 6.0)
            .padding_xy(12.0, 0.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            });
        if let Some(on_click) = self.on_click.clone() {
            modifier = modifier
                .cursor(CursorIcon::Pointer)
                .clickable(move || on_click());
        }
        let title_variant = match self.variant {
            ListRowVariant::Plain => LabelVariant::Body,
            ListRowVariant::Dimmed => LabelVariant::Muted,
        };
        row_arranged(
            modifier,
            Arrangement::SpacedBy(crate::spacing(12.0)),
            Alignment::Center,
            move || {
                if !matches!(self.leading, Leading::None) {
                    self.leading.show();
                }
                row_arranged(
                    Modifier::new().weight(1.0),
                    Arrangement::SpacedBy(crate::spacing(8.0)),
                    Alignment::Center,
                    || {
                        Label::new(self.title.clone())
                            .variant(title_variant)
                            .modifier(Modifier::new().width_in(0.0, TITLE_WIDTH))
                            .show();
                        if let Some(note) = &self.note {
                            icon(phosphor::CARET_RIGHT, 12.0, scheme.on_surface_variant);
                            if let Some(glyph) = &self.note_icon {
                                icon(glyph.clone(), 14.0, scheme.on_surface_variant);
                            }
                            Label::new(note.clone()).variant(LabelVariant::Muted).show();
                        }
                        if let Some((added, removed)) = &self.tally {
                            Label::new(added.clone())
                                .variant(LabelVariant::Code)
                                .colour(extended.success)
                                .size(Size::Xs)
                                .show();
                            Label::new(removed.clone())
                                .variant(LabelVariant::Code)
                                .colour(scheme.error)
                                .size(Size::Xs)
                                .show();
                        }
                        spacer(Modifier::new().weight(1.0));
                    },
                );
                for column in &self.columns {
                    let alignment = match column.style {
                        ColumnStyle::End => BoxAlignment::CenterEnd,
                        _ => BoxAlignment::CenterStart,
                    };
                    let (variant, size) = match column.style {
                        ColumnStyle::Text => (LabelVariant::Muted, Size::Sm),
                        ColumnStyle::Code => (LabelVariant::Code, Size::Xs),
                        ColumnStyle::End => (LabelVariant::Subtle, Size::Xs),
                    };
                    let text = column.text.clone();
                    stack_aligned(
                        Modifier::new().width(column.width).clip(),
                        alignment,
                        move || {
                            let mut label = Label::new(text.clone()).variant(variant).size(size);
                            if variant == LabelVariant::Code {
                                label = label.colour(scheme.on_surface_variant);
                            }
                            label.show();
                        },
                    );
                }
            },
        );
    }
}
