//! The open list of choices under a select box, narrowed by a box above it
//! when it is long.

use std::rc::Rc;

use rustle::prelude::*;

use crate::label::{Label, LabelVariant};
use crate::size::Size;
use crate::text_input::TextInput;
use rustle::color_scheme;

/// How many choices a list holds before it offers a box to narrow it.
const FILTER_FROM: usize = 8;

/// How many matching choices the list shows at most.
const SHOWN_AT_MOST: usize = 200;

/// How a list of choices is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OptionListVariant {
    /// A raised surface with a hairline border.
    #[default]
    Raised,
    /// No surface of its own, for a list inside a card.
    Flat,
}

/// A list of choices with a variant and a size.
#[derive(Clone)]
pub struct OptionList {
    /// The choices, as they read.
    options: Vec<String>,
    /// The place of the choice picked, when one is.
    picked: Option<usize>,
    /// What the list is narrowed to.
    filter: String,
    /// How it is drawn.
    variant: OptionListVariant,
    /// Its size.
    size: Size,
    /// The tallest it grows before it scrolls.
    max_height: f32,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What typing in the narrowing box does.
    on_filter: Option<Rc<dyn Fn(String)>>,
    /// What picking a choice does, given its place among all the options.
    on_pick: Option<Rc<dyn Fn(usize)>>,
}

impl OptionList {
    /// A list of `options` with the one at `picked` marked.
    pub fn new(options: Vec<String>, picked: Option<usize>) -> Self {
        Self {
            options,
            picked,
            filter: String::new(),
            variant: OptionListVariant::Raised,
            size: Size::Sm,
            max_height: 240.0,
            modifier: Modifier::new(),
            on_filter: None,
            on_pick: None,
        }
    }

    /// The list narrowed to choices holding `filter`.
    pub fn filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = filter.into();
        self
    }

    /// The list in `variant`.
    pub fn variant(mut self, variant: OptionListVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The list at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The list growing to `max_height` before it scrolls.
    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = max_height;
        self
    }

    /// The list laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The list calling `on_filter` with what is typed in its narrowing box.
    pub fn on_filter(mut self, on_filter: impl Fn(String) + 'static) -> Self {
        self.on_filter = Some(Rc::new(on_filter));
        self
    }

    /// The list calling `on_pick` with the place of the choice pressed among all the options.
    pub fn on_pick(mut self, on_pick: impl Fn(usize) + 'static) -> Self {
        self.on_pick = Some(Rc::new(on_pick));
        self
    }

    /// The places of the options that hold every word of `filter`, ignoring case.
    pub fn matching(options: &[String], filter: &str) -> Vec<usize> {
        let needle = filter.trim().to_lowercase();
        options
            .iter()
            .enumerate()
            .filter(|(_, option)| {
                let option = option.to_lowercase();
                needle.split_whitespace().all(|word| option.contains(word))
            })
            .map(|(place, _)| place)
            .collect()
    }

    /// Emits the list.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let scroll = remember_scroll_state();
        let row_height = match self.size {
            Size::Xs => 22.0,
            Size::Sm => 26.0,
            Size::Md => 30.0,
            Size::Lg => 34.0,
        };
        let mut modifier = self.modifier.padding(4.0);
        if self.variant == OptionListVariant::Raised {
            modifier = modifier
                .background_rounded(scheme.surface_container, 8.0)
                .border(1.0, scheme.outline_variant, 8.0);
        }
        let matching = Self::matching(&self.options, &self.filter);
        let filtering = self.options.len() > FILTER_FROM;
        column_arranged(
            modifier,
            Arrangement::SpacedBy(4.0),
            Alignment::Start,
            || {
                if filtering {
                    let on_filter = self.on_filter.clone();
                    let pick = self.on_pick.clone();
                    let first = matching.first().copied();
                    TextInput::new(self.filter.clone())
                        .placeholder("Filter")
                        .size(Size::Xs)
                        .modifier(Modifier::new().fill_max_width())
                        .on_change(move |text| {
                            if let Some(on_filter) = &on_filter {
                                on_filter(text);
                            }
                        })
                        .on_submit(move |_| {
                            if let (Some(pick), Some(first)) = (&pick, first) {
                                pick(first);
                            }
                        })
                        .show();
                }
                if matching.is_empty() {
                    Label::new("No choice matches.")
                        .variant(LabelVariant::Subtle)
                        .modifier(Modifier::new().padding_xy(8.0, 4.0))
                        .show();
                    return;
                }
                column_with(
                    Modifier::new()
                        .fill_max_width()
                        .height_in(0.0, self.max_height)
                        .vertical_scroll(&scroll),
                    || {
                        for place in matching.iter().copied().take(SHOWN_AT_MOST) {
                            let picked = self.picked == Some(place);
                            let on_pick = self.on_pick.clone();
                            option_row(
                                self.options[place].clone(),
                                picked,
                                row_height,
                                move || {
                                    if let Some(on_pick) = &on_pick {
                                        on_pick(place);
                                    }
                                },
                            );
                        }
                    },
                );
            },
        );
    }
}

/// One choice of the list, reading `label`, marked when `picked`, `height` tall, calling `on_click` when pressed.
#[track_caller]
fn option_row(label: String, picked: bool, height: f32, on_click: impl Fn() + 'static) {
    let scheme = color_scheme();
    let hovered = remember(|| false);
    let fill = if picked {
        scheme.secondary_container
    } else if hovered.get() {
        scheme.surface_container_high
    } else {
        Color::TRANSPARENT
    };
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(height)
            .background_rounded(fill, 4.0)
            .on_hover({
                let hovered = hovered.clone();
                move |inside| hovered.set(inside)
            })
            .cursor(CursorIcon::Pointer)
            .clickable(on_click)
            .padding_xy(8.0, 0.0),
        Arrangement::SpacedBy(8.0),
        Alignment::Center,
        move || {
            Label::new(label)
                .modifier(Modifier::new().weight(1.0))
                .show();
            if picked {
                icon(phosphor::CHECK, 12.0, scheme.primary);
            }
        },
    );
}
