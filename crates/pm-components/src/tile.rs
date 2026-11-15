//! A row of a list: a leading mark, a label and trailing text, which
//! highlights under the pointer and when chosen.

use std::rc::Rc;

use rustle::prelude::*;

use crate::app_theme::extended_colors;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// What a tile leads with.
#[derive(Clone)]
pub enum Leading {
    /// Nothing.
    None,
    /// An icon in the muted tone.
    Icon(Icon),
    /// A small dot, filled or hollow.
    Dot {
        /// Whether the dot is filled.
        filled: bool,
    },
    /// A turning arc, for something under way.
    Spinner,
    /// A filled dot in the warning tone, for something waiting on an answer.
    Attention,
    /// A filled dot in the danger tone, for something that failed.
    Failed,
    /// Any composable, such as an avatar.
    Custom(Rc<dyn Fn()>),
}

impl Leading {
    /// Emits the mark in a 14 pixel square, painted from the scheme provided above it.
    #[track_caller]
    pub fn show(&self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        match self {
            Leading::None => {}
            Leading::Icon(glyph) => {
                stack_aligned(
                    Modifier::new().size(14.0, 14.0),
                    BoxAlignment::Center,
                    || icon(glyph.clone(), 14.0, scheme.on_surface_variant),
                );
            }
            Leading::Dot { filled } => {
                let filled = *filled;
                stack_aligned(
                    Modifier::new().size(14.0, 14.0),
                    BoxAlignment::Center,
                    move || {
                        let dot = Modifier::new().size(6.0, 6.0);
                        if filled {
                            spacer(dot.background_rounded(scheme.on_surface, 3.0));
                        } else {
                            spacer(dot.border(1.0, extended.subtle, 3.0));
                        }
                    },
                );
            }
            Leading::Spinner => {
                stack_aligned(
                    Modifier::new().size(14.0, 14.0),
                    BoxAlignment::Center,
                    || circular_progress_indicator_indeterminate(10.0),
                );
            }
            Leading::Attention | Leading::Failed => {
                let colour = if matches!(self, Leading::Attention) {
                    extended.warning
                } else {
                    scheme.error
                };
                stack_aligned(
                    Modifier::new().size(14.0, 14.0),
                    BoxAlignment::Center,
                    move || {
                        spacer(
                            Modifier::new()
                                .size(6.0, 6.0)
                                .background_rounded(colour, 3.0),
                        )
                    },
                );
            }
            Leading::Custom(content) => content(),
        }
    }
}

/// How a tile reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TileVariant {
    /// An ordinary row.
    #[default]
    Plain,
    /// A row whose label reads muted, for actions like "New session".
    Action,
    /// A header row, such as a project's name.
    Header,
}

/// A list row with a variant and a size.
#[derive(Clone)]
pub struct Tile {
    /// The text.
    label: String,
    /// What it leads with.
    leading: Leading,
    /// Faint text at the end, such as an age.
    trailing: Option<String>,
    /// How it reads.
    variant: TileVariant,
    /// Its size.
    size: Size,
    /// Space before the leading mark.
    indent: f32,
    /// Whether it is the chosen row.
    selected: bool,
    /// An icon shown at the end while hovered, such as "more".
    hover_icon: Option<Icon>,
    /// What a click on the hover icon does, instead of what a click on the row does.
    on_hover_icon: Option<Rc<dyn Fn()>>,
    /// What a click does.
    on_click: Option<Rc<dyn Fn()>>,
    /// What a right click does, such as opening the row's menu.
    on_menu: Option<Rc<dyn Fn()>>,
}

impl Tile {
    /// A plain tile reading `label`.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            leading: Leading::None,
            trailing: None,
            variant: TileVariant::Plain,
            size: Size::Sm,
            indent: 0.0,
            selected: false,
            hover_icon: None,
            on_hover_icon: None,
            on_click: None,
            on_menu: None,
        }
    }

    /// The tile calling `on_menu` when right-clicked.
    pub fn on_menu(mut self, on_menu: impl Fn() + 'static) -> Self {
        self.on_menu = Some(Rc::new(on_menu));
        self
    }

    /// The tile leading with `leading`.
    pub fn leading(mut self, leading: Leading) -> Self {
        self.leading = leading;
        self
    }

    /// The tile ending in faint `trailing` text.
    pub fn trailing(mut self, trailing: impl Into<String>) -> Self {
        self.trailing = Some(trailing.into());
        self
    }

    /// The tile in `variant`.
    pub fn variant(mut self, variant: TileVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The tile at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The tile indented by `indent` pixels.
    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent;
        self
    }

    /// The tile highlighted as chosen when `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The tile showing `icon` at its end while hovered.
    pub fn hover_icon(mut self, icon: Icon) -> Self {
        self.hover_icon = Some(icon);
        self
    }

    /// The tile calling `on_press` when its hover icon is clicked.
    pub fn on_hover_icon(mut self, on_press: impl Fn() + 'static) -> Self {
        self.on_hover_icon = Some(Rc::new(on_press));
        self
    }

    /// The tile calling `on_click` when clicked.
    pub fn on_click(mut self, on_click: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(on_click));
        self
    }

    /// Emits the tile.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let extended = extended_colors();
        let hovered = remember(|| false);
        let height = match self.size {
            Size::Xs => 24.0,
            Size::Sm => 28.0,
            Size::Md => 32.0,
            Size::Lg => 36.0,
        };
        let height = crate::row_height(height);
        let fill = if self.selected {
            scheme.secondary_container
        } else if hovered.get() && self.on_click.is_some() {
            scheme.surface_container_high
        } else {
            Color::TRANSPARENT
        };
        let mut modifier = Modifier::new()
            .fill_max_width()
            .height(height)
            .background_rounded(fill, 6.0)
            .padding_each(8.0 + self.indent, 0.0, 8.0, 0.0)
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
            modifier = modifier.on_secondary_click(move |_| on_menu());
        }
        let label_variant = match self.variant {
            TileVariant::Action => LabelVariant::Muted,
            _ => LabelVariant::Body,
        };
        let label_size = match self.variant {
            TileVariant::Header => Size::Md,
            _ => Size::Sm,
        };
        let show_hover_icon = hovered.get();
        row_arranged(
            modifier,
            Arrangement::SpacedBy(crate::spacing(8.0)),
            Alignment::Center,
            move || {
                self.leading.show();
                Label::new(self.label.clone())
                    .variant(label_variant)
                    .size(label_size)
                    .modifier(Modifier::new().weight(1.0))
                    .show();
                if let Some(trailing) = &self.trailing {
                    Label::new(trailing.clone())
                        .variant(LabelVariant::Subtle)
                        .size(Size::Xs)
                        .show();
                }
                if let Some(glyph) = &self.hover_icon {
                    let tint = if show_hover_icon {
                        scheme.on_surface_variant
                    } else {
                        extended.subtle
                    };
                    match &self.on_hover_icon {
                        Some(on_press) => {
                            let on_press = on_press.clone();
                            stack_aligned(
                                Modifier::new()
                                    .size(20.0, 20.0)
                                    .cursor(CursorIcon::Pointer)
                                    .clickable(move || on_press()),
                                BoxAlignment::Center,
                                || icon(glyph.clone(), 14.0, tint),
                            );
                        }
                        None => icon(glyph.clone(), 14.0, tint),
                    }
                }
            },
        );
    }
}
