//! A row describing one item at length: an icon, a title with tags, lines
//! about it, a note in a tone of its own, and a button at its end.

use rustle::prelude::*;

use crate::button::Button;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How an item row reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemRowVariant {
    /// On the surface it sits on.
    #[default]
    Plain,
    /// Filled with the hover tone, to set it apart.
    Highlighted,
}

/// A line under an item row's title.
#[derive(Clone, Debug, PartialEq)]
struct Line {
    /// Its text.
    text: String,
    /// Its tone.
    variant: LabelVariant,
    /// A colour replacing the tone's.
    colour: Option<Color>,
}

/// A descriptive row with a variant and a size.
#[derive(Clone)]
pub struct ItemRow {
    /// The title.
    title: String,
    /// The icon before everything.
    icon: Option<Icon>,
    /// Faint words after the title, each with whether it is code.
    tags: Vec<(String, bool)>,
    /// The lines under the title.
    lines: Vec<Line>,
    /// The button at the end.
    action: Option<Button>,
    /// How it reads.
    variant: ItemRowVariant,
    /// Its padding.
    size: Size,
}

impl ItemRow {
    /// A plain row titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            icon: None,
            tags: Vec::new(),
            lines: Vec::new(),
            action: None,
            variant: ItemRowVariant::Plain,
            size: Size::Md,
        }
    }

    /// The row leading with `icon`.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The row with faint `tag` after its title.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push((tag.into(), false));
        self
    }

    /// The row with faint monospace `tag` after its title, such as a version.
    pub fn code_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push((tag.into(), true));
        self
    }

    /// The row with a muted line of `text` under its title.
    pub fn line(mut self, text: impl Into<String>) -> Self {
        self.lines.push(Line {
            text: text.into(),
            variant: LabelVariant::Muted,
            colour: None,
        });
        self
    }

    /// The row with a faint line of `text` under its title.
    pub fn faint_line(mut self, text: impl Into<String>) -> Self {
        self.lines.push(Line {
            text: text.into(),
            variant: LabelVariant::Subtle,
            colour: None,
        });
        self
    }

    /// The row with a line of `text` in `colour` under its title, such as a failure.
    pub fn note(mut self, text: impl Into<String>, colour: Color) -> Self {
        self.lines.push(Line {
            text: text.into(),
            variant: LabelVariant::Muted,
            colour: Some(colour),
        });
        self
    }

    /// The row ending in `button`.
    pub fn action(mut self, button: Button) -> Self {
        self.action = Some(button);
        self
    }

    /// The row in `variant`.
    pub fn variant(mut self, variant: ItemRowVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The row at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Emits the row.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let (padding_x, padding_y) = match self.size {
            Size::Xs => (10.0, 6.0),
            Size::Sm => (12.0, 8.0),
            Size::Md => (16.0, 12.0),
            Size::Lg => (20.0, 16.0),
        };
        let (padding_x, padding_y) = (crate::spacing(padding_x), crate::spacing(padding_y));
        let fill = match self.variant {
            ItemRowVariant::Plain => Color::TRANSPARENT,
            ItemRowVariant::Highlighted => scheme.surface_container_high,
        };
        row_arranged(
            Modifier::new()
                .fill_max_width()
                .background(fill)
                .padding_xy(padding_x, padding_y),
            Arrangement::SpacedBy(crate::spacing(12.0)),
            Alignment::Center,
            move || {
                if let Some(glyph) = &self.icon {
                    icon(glyph.clone(), 20.0, scheme.on_surface_variant);
                }
                column_arranged(
                    Modifier::new().weight(1.0),
                    Arrangement::SpacedBy(crate::spacing(2.0)),
                    Alignment::Start,
                    || {
                        row_arranged(
                            Modifier::new(),
                            Arrangement::SpacedBy(crate::spacing(8.0)),
                            Alignment::Center,
                            || {
                                Label::new(self.title.clone())
                                    .variant(LabelVariant::Heading)
                                    .show();
                                for (tag, code) in &self.tags {
                                    let variant = if *code {
                                        LabelVariant::Code
                                    } else {
                                        LabelVariant::Subtle
                                    };
                                    Label::new(tag.clone())
                                        .variant(variant)
                                        .colour(scheme.on_surface_variant)
                                        .size(Size::Xs)
                                        .show();
                                }
                            },
                        );
                        for line in &self.lines {
                            let mut label = Label::new(line.text.clone())
                                .variant(line.variant)
                                .size(Size::Xs);
                            if let Some(colour) = line.colour {
                                label = label.colour(colour);
                            }
                            label.show();
                        }
                    },
                );
                if let Some(button) = self.action.clone() {
                    button.show();
                }
            },
        );
    }
}
