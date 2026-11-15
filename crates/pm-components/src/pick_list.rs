//! A list to pick from with a field to narrow it: each row an icon, a title
//! with the characters typed highlighted, a muted detail and the keys bound
//! to it; Up and Down move the row chosen, Enter picks it, Escape closes the
//! list, and a footer can show those keys. It serves the quick input and the
//! lists of references, actions, history and problems.

use std::rc::Rc;

use rustle::prelude::*;
use rustle::{Key, KeyEvent, Modifiers};

use crate::divider::Divider;
use crate::key_cap::KeyCap;
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use crate::text_input::{TextInput, TextInputVariant};
use rustle::color_scheme;

/// A row of a pick list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PickItem {
    /// What it reads.
    pub title: String,
    /// The muted text after it.
    pub detail: String,
    /// Its icon.
    pub icon: Option<Icon>,
    /// The icon's colour, muted when not given.
    pub tint: Option<Color>,
    /// The keys bound to it, chords apart by spaces, such as `Ctrl+K Ctrl+T`.
    pub keys: Option<String>,
    /// The characters of `title` matching what is typed.
    pub matched: Vec<usize>,
    /// Whether it is the choice that now holds, marked with a check.
    pub current: bool,
}

/// How a pick list is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickListVariant {
    /// Bare, for a list inside a popover or modal.
    #[default]
    Plain,
    /// On a raised surface with a hairline border.
    Raised,
}

/// A pick list with a variant and a size.
#[derive(Clone)]
pub struct PickList {
    /// The rows.
    items: Vec<PickItem>,
    /// What is typed.
    query: String,
    /// What the field says while empty.
    placeholder: String,
    /// The row chosen.
    selected: usize,
    /// What it says when no row is listed.
    empty: String,
    /// The most rows shown at once.
    visible: usize,
    /// Whether the footer naming the keys shows.
    footer: bool,
    /// How it is drawn.
    variant: PickListVariant,
    /// Its size.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What typing does.
    on_query: Option<Rc<dyn Fn(String)>>,
    /// What picking a row does, given its place.
    on_pick: Option<Rc<dyn Fn(usize)>>,
    /// What Up and Down do, given how far to move.
    on_move: Option<Rc<dyn Fn(isize)>>,
    /// What Escape does.
    on_close: Option<Rc<dyn Fn()>>,
    /// What Control or Command with Enter does.
    on_prompt: Option<Rc<dyn Fn()>>,
}

impl PickList {
    /// A list of `items`.
    pub fn new(items: Vec<PickItem>) -> Self {
        Self {
            items,
            query: String::new(),
            placeholder: String::new(),
            selected: 0,
            empty: "Nothing matches.".to_owned(),
            visible: 12,
            footer: false,
            variant: PickListVariant::Plain,
            size: Size::Sm,
            modifier: Modifier::new(),
            on_query: None,
            on_pick: None,
            on_move: None,
            on_close: None,
            on_prompt: None,
        }
    }

    /// The list with `query` typed in its field.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    /// The list's field saying `placeholder` while empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The list with the row at `selected` chosen.
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }

    /// The list saying `empty` when no row is listed.
    pub fn empty(mut self, empty: impl Into<String>) -> Self {
        self.empty = empty.into();
        self
    }

    /// The list showing at most `visible` rows at once.
    pub fn visible(mut self, visible: usize) -> Self {
        self.visible = visible.max(1);
        self
    }

    /// The list with a footer naming its keys.
    pub fn footer(mut self) -> Self {
        self.footer = true;
        self
    }

    /// The list in `variant`.
    pub fn variant(mut self, variant: PickListVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The list at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The list laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The list calling `on_query` as its field is typed in.
    pub fn on_query(mut self, on_query: impl Fn(String) + 'static) -> Self {
        self.on_query = Some(Rc::new(on_query));
        self
    }

    /// The list calling `on_pick` with a row's place when it is picked.
    pub fn on_pick(mut self, on_pick: impl Fn(usize) + 'static) -> Self {
        self.on_pick = Some(Rc::new(on_pick));
        self
    }

    /// The list calling `on_move` with how far to move the row chosen on Up and Down.
    pub fn on_move(mut self, on_move: impl Fn(isize) + 'static) -> Self {
        self.on_move = Some(Rc::new(on_move));
        self
    }

    /// The list calling `on_close` on Escape.
    pub fn on_close(mut self, on_close: impl Fn() + 'static) -> Self {
        self.on_close = Some(Rc::new(on_close));
        self
    }

    /// The list calling `on_prompt` when Control or Command and Enter are pressed.
    pub fn on_prompt(mut self, on_prompt: impl Fn() + 'static) -> Self {
        self.on_prompt = Some(Rc::new(on_prompt));
        self
    }

    /// Emits the list.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let row_height = match self.size {
            Size::Xs => 24.0,
            Size::Sm => 28.0,
            Size::Md => 32.0,
            Size::Lg => 36.0,
        };
        let mut modifier = self.modifier;
        if self.variant == PickListVariant::Raised {
            modifier = modifier
                .background_rounded(scheme.surface_container, 8.0)
                .border(1.0, scheme.outline_variant, 8.0);
        }
        let selected = self.selected.min(self.items.len().saturating_sub(1));
        let keys = {
            let on_move = self.on_move.clone();
            let on_pick = self.on_pick.clone();
            let on_close = self.on_close.clone();
            let on_prompt = self.on_prompt.clone();
            let listed = !self.items.is_empty();
            move |event: &KeyEvent| {
                let call_move = |by: isize| {
                    if let Some(on_move) = &on_move {
                        on_move(by);
                    }
                    true
                };
                if event.is(Key::ArrowDown, Modifiers::NONE) {
                    return call_move(1);
                }
                if event.is(Key::ArrowUp, Modifiers::NONE) {
                    return call_move(-1);
                }
                if let Some(on_prompt) = &on_prompt
                    && (event.is(Key::Enter, Modifiers::CTRL)
                        || event.is(Key::Enter, Modifiers::META))
                {
                    on_prompt();
                    return true;
                }
                if event.is(Key::Enter, Modifiers::NONE) {
                    if let (Some(on_pick), true) = (&on_pick, listed) {
                        on_pick(selected);
                    }
                    return true;
                }
                if event.is(Key::Escape, Modifiers::NONE) {
                    if let Some(on_close) = &on_close {
                        on_close();
                    }
                    return true;
                }
                false
            }
        };
        let first = (selected + 1).saturating_sub(self.visible);
        column_with(modifier.on_preview_key_event(keys), || {
            let on_query = self.on_query.clone();
            stack_with(Modifier::new().fill_max_width().padding(8.0), || {
                TextInput::new(self.query.clone())
                    .placeholder(self.placeholder.clone())
                    .variant(TextInputVariant::Search)
                    .size(Size::Md)
                    .autofocus()
                    .modifier(Modifier::new().fill_max_width())
                    .on_change(move |text| {
                        if let Some(on_query) = &on_query {
                            on_query(text);
                        }
                    })
                    .show();
            });
            column_with(
                Modifier::new().fill_max_width().padding_xy(6.0, 0.0),
                || {
                    if self.items.is_empty() {
                        Label::new(self.empty.clone())
                            .variant(LabelVariant::Subtle)
                            .modifier(Modifier::new().padding_xy(10.0, 8.0))
                            .show();
                    }
                    for (place, item) in
                        self.items.iter().enumerate().skip(first).take(self.visible)
                    {
                        let on_pick = self.on_pick.clone();
                        key(place, || {
                            pick_row(item.clone(), place == selected, row_height, move || {
                                if let Some(on_pick) = &on_pick {
                                    on_pick(place);
                                }
                            });
                        });
                    }
                },
            );
            if self.footer {
                footer();
            } else {
                spacer(Modifier::new().height(6.0));
            }
        });
    }
}

/// One row of the list, `height` tall, highlighted when `chosen`, calling `on_click` when pressed.
#[track_caller]
fn pick_row(item: PickItem, chosen: bool, height: f32, on_click: impl Fn() + 'static) {
    let scheme = color_scheme();
    let hovered = remember(|| false);
    let fill = if chosen {
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
        Arrangement::SpacedBy(crate::spacing(8.0)),
        Alignment::Center,
        move || {
            if let Some(glyph) = item.icon.clone() {
                icon(glyph, 14.0, item.tint.unwrap_or(scheme.on_surface_variant));
            }
            row_with(Modifier::new().weight(1.0).clip(), || {
                matched_title(&item.title, &item.matched);
                if !item.detail.is_empty() {
                    spacer(Modifier::new().width(8.0));
                    Label::new(item.detail.clone())
                        .variant(LabelVariant::Subtle)
                        .size(Size::Xs)
                        .show();
                }
            });
            if item.current {
                icon(phosphor::CHECK, 12.0, scheme.primary);
            }
            if let Some(keys) = &item.keys {
                key_caps(keys);
            }
        },
    );
}

/// `title`, its characters at the places `matched` in the accent and bold.
fn matched_title(title: &str, matched: &[usize]) {
    let scheme = color_scheme();
    let mut runs: Vec<(bool, String)> = Vec::new();
    for (place, letter) in title.chars().enumerate() {
        let hit = matched.contains(&place);
        match runs.last_mut() {
            Some((was, run)) if *was == hit => run.push(letter),
            _ => runs.push((hit, letter.to_string())),
        }
    }
    for (hit, run) in runs {
        let mut label = Label::new(run);
        if hit {
            label = label.variant(LabelVariant::Heading).colour(scheme.primary);
        }
        label.show();
    }
}

/// The chords of `keys`, apart by spaces, each as a key cap.
pub fn key_caps(keys: &str) {
    row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(crate::spacing(4.0)),
        Alignment::Center,
        || {
            for chord in keys.split_whitespace() {
                KeyCap::new(chord).show();
            }
        },
    );
}

/// The footer naming the list's keys.
fn footer() {
    spacer(Modifier::new().height(6.0));
    Divider::new().show();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(34.0)
            .padding_xy(12.0, 0.0),
        Arrangement::SpacedBy(crate::spacing(6.0)),
        Alignment::Center,
        || {
            for (keys, what) in [
                ("↑ ↓", "to move"),
                ("Enter", "to pick"),
                ("Esc", "to close"),
            ] {
                key_caps(keys);
                Label::new(what)
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .show();
                spacer(Modifier::new().width(8.0));
            }
        },
    );
}
