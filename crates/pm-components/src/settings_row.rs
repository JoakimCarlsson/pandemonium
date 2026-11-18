//! A row of the settings window: what a setting is called and does beside, or above, its one control, and the button that puts it back as it comes once it was changed; with the controls a row is made of.

use pm_field::Field;
use rustle::{CursorIcon, Modifier};

use crate::Button;
use crate::button::Variant;
use crate::input::Input;
use crate::kit::{
    Flex, FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Side, Styled, Type, block,
    column, label, row,
};
use crate::parts::{clipped, glyph, line, wrap};
use crate::settings_ids::PICKS;
use crate::size::Size;
use crate::tile::Tile;

/// The room between what a setting says and its control, in logical pixels.
const ROW_GAP: f32 = 24.0;

/// The least room what a setting says is given beside its control, in logical pixels; with less it goes above the control.
const MIN_WORDS: f32 = 180.0;

/// How wide a character of the choices is taken to be, in logical pixels, generously so that the words beside never run under them.
const CHOICE_CHARACTER: f32 = 7.5;

/// How wide the button that resets a setting is, with the room before it, in logical pixels.
pub const RESET_WIDTH: f32 = 32.0;

/// How wide a box of text is beside what its setting says, in logical pixels.
const TEXT_WIDTH: f32 = 240.0;

/// How wide the button opening a list of choices is, in logical pixels, where there is room.
pub const PICKER_WIDTH: f32 = 200.0;

/// How wide a switch is, in logical pixels.
pub const SWITCH_WIDTH: f32 = 36.0;

/// How many choices an open list shows before it asks to be narrowed.
const MOST_LISTED: usize = 12;

const _: () = assert!(
    MOST_LISTED as u64 <= PICKS.len(),
    "PICKS has no room for every row an open list shows"
);

/// How many choices a list has before it offers a box to narrow it.
const FILTERED_FROM: usize = 10;

/// Emits a switch that is on while `on` is true, answering to `id`; with no name it is drawn but cannot be pressed.
#[track_caller]
pub fn switch(on: bool, id: Option<FocusId>) {
    let mut modifier = Modifier::new()
        .width(SWITCH_WIDTH)
        .h(5)
        .fill_rounded(if on { Role::Success } else { Role::Border }, Radius::Full);
    if let Some(id) = id {
        modifier = modifier.cursor(CursorIcon::Pointer).target(id);
    }
    let flex = if on {
        Flex::new().center().trailing()
    } else {
        Flex::new().center()
    };
    row(modifier.pad(0.5), flex, || {
        rustle::spacer(
            Modifier::new()
                .square(4)
                .fill_rounded(Role::Text, Radius::Full),
        );
    });
}

/// How wide the choices `names` are drawn side by side, in logical pixels, at most.
pub fn choices_width(names: &[&str]) -> f32 {
    8.0 + names
        .iter()
        .map(|name| 22.0 + name.chars().count() as f32 * CHOICE_CHARACTER)
        .sum::<f32>()
}

/// Emits the choices `names` side by side, `picked` marked, each answering to the name `id_of` gives its place; a choice given no name is left out.
#[track_caller]
pub fn choices(names: &[&str], picked: usize, id_of: impl Fn(usize) -> Option<FocusId>) {
    let modifier = Modifier::new()
        .h(8)
        .stroke(Role::Border, Radius::Md)
        .pad_x(0.5);
    row(modifier, Flex::new().center().gap(0.5), || {
        for (option, name) in names.iter().enumerate() {
            let Some(id) = id_of(option) else {
                continue;
            };
            let on = option == picked;
            Tile::new(id)
                .selected(on)
                .height(6.5)
                .radius(Radius::Sm)
                .inner(Modifier::new().pad_x(2.5))
                .flex(Flex::new().center().middle())
                .build_with(|| {
                    line(*name, 12, 1.4, if on { Role::Text } else { Role::Muted });
                });
        }
    });
}

/// How wide a number with its buttons is, in logical pixels.
pub const NUMBER_WIDTH: f32 = 136.0;

/// Emits the number `shown` between the buttons `smaller` and `bigger` that change it.
#[track_caller]
pub fn number(shown: String, smaller: FocusId, bigger: FocusId) {
    let button = |icon: IconName, id: FocusId| {
        Button::icon(icon, id)
            .size(Size::Sm)
            .tone(Role::Text)
            .build();
    };
    row(
        Modifier::new()
            .h(8)
            .stroke(Role::Border, Radius::Md)
            .pad_x(0.5),
        Flex::new().center().gap(1),
        || {
            button(IconName::Minus, smaller);
            row(Modifier::new().width(64.0), Flex::new().middle(), || {
                label(
                    shown,
                    Type::new(12).ratio(1.4).tone(Role::Text).truncate().mono(),
                );
            });
            button(IconName::Plus, bigger);
        },
    );
}

/// Emits the button saying `shown`, the choice picked, `width` logical pixels wide, that opens the list of choices, answering to `id`; its chevron points down while the list is `open`.
#[track_caller]
pub fn picker(shown: &str, width: f32, id: FocusId, open: bool) {
    Button::new(shown, id)
        .variant(Variant::Field)
        .size(Size::Sm)
        .width(width)
        .selected(open)
        .trailing_icon(if open {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        })
        .build();
}

/// The places among `options` of the ones the open list shows, narrowed to those holding what `filter` holds; the row of each is its place in what is returned.
pub fn picker_shown(options: &[String], filter: &Field) -> Vec<usize> {
    let needle = filter.text().trim().to_lowercase();
    options
        .iter()
        .enumerate()
        .filter(|(_, name)| needle.is_empty() || name.to_lowercase().contains(&needle))
        .map(|(place, _)| place)
        .collect()
}

/// Emits the open list of `options` under a row, `picked` marked, narrowed by what `filter` holds when they are many; each row shown answers to its row in [`PICKS`], so a list of any length names its rows the same way.
#[track_caller]
pub fn picker_list(options: &[String], picked: Option<usize>, filter: (&Field, FocusId)) {
    let shown = picker_shown(options, filter.0);
    let more = shown.len().saturating_sub(MOST_LISTED);
    column(
        Modifier::new()
            .fill_rounded(Role::Surface, Radius::Md)
            .stroke(Role::Border, Radius::Md)
            .pad(1),
        Flex::new().gap(0.5),
        || {
            if options.len() >= FILTERED_FROM {
                block(Modifier::new().pad(1), || {
                    Input::search(filter.0, filter.1)
                        .size(Size::Sm)
                        .placeholder("Type to narrow the list")
                        .build();
                });
            }
            for (row_place, place) in shown.into_iter().enumerate().take(MOST_LISTED) {
                let on = Some(place) == picked;
                let (Some(name), Some(id)) = (options.get(place), PICKS.id(row_place)) else {
                    continue;
                };
                Tile::new(id)
                    .selected(on)
                    .height(7.0)
                    .gap(2.0)
                    .radius(Radius::Sm)
                    .inner(Modifier::new().pad_x(2))
                    .flex(Flex::new().center().between())
                    .build_with(|| {
                        clipped(name.as_str(), 12, if on { Role::Text } else { Role::Muted });
                        if on {
                            glyph(IconName::Check, 3.5, Role::Text);
                        }
                    });
            }
            if more > 0 {
                row(Modifier::new().h(6).pad_x(2), Flex::new().center(), || {
                    line(
                        format!("{more} more; type to narrow"),
                        11,
                        1.4,
                        Role::Subtle,
                    );
                });
            }
        },
    );
}

/// Emits the box `field` is written in, answering to `id`, saying `placeholder` while empty.
#[track_caller]
pub fn text_box(field: &Field, id: FocusId, placeholder: &str) {
    Input::new(field, id)
        .size(Size::Sm)
        .placeholder(placeholder)
        .build();
}

/// Emits the `lines` one under another, each with a button that removes it answering to the name `remove_of` gives its place, then the box `field` answering to its name, saying `placeholder` while empty, with the button `add`, labelled with its words, that adds what is written in it; a line given no name has no remove button.
#[track_caller]
pub fn list(
    lines: &[String],
    remove_of: impl Fn(usize) -> Option<FocusId>,
    field: (&Field, FocusId),
    add: (FocusId, &str),
    placeholder: &str,
) {
    column(Modifier::new(), Flex::new().gap(1), || {
        for (place, text) in lines.iter().enumerate() {
            row(
                Modifier::new()
                    .h(6)
                    .looks(Looks::new().hover(Role::SurfaceHover).radius(Radius::Sm))
                    .pad_x(2),
                Flex::new().center().between().gap(2),
                || {
                    label(
                        text.as_str(),
                        Type::new(12).ratio(1.4).tone(Role::Text).truncate().mono(),
                    );
                    if let Some(remove) = remove_of(place) {
                        Button::icon(IconName::Close, remove).size(Size::Xs).build();
                    }
                },
            );
        }
        row(
            Modifier::new().pad_side(Side::Top, 1),
            Flex::new().center().gap(2),
            || {
                block(Modifier::new().width(TEXT_WIDTH), || {
                    text_box(field.0, field.1, placeholder);
                });
                Button::new(add.1.to_owned(), add.0).size(Size::Sm).build();
            },
        );
    });
}

/// Emits the button that puts a setting back as it comes, answering to `id`.
#[track_caller]
fn reset_button(id: FocusId) {
    Button::icon(IconName::Undo, id).size(Size::Xs).build();
}

/// What a row emits for its control or for what opens under it, kept until the row is built.
type Content<'a> = Box<dyn FnOnce() + 'a>;

/// A row being described: what the setting is called and does, its control and how wide that is, the button that resets it and what opens under it.
///
/// Describe it with [`SettingRow::new`], give it its control with
/// [`SettingRow::control`], and emit it `room` logical pixels across with
/// [`SettingRow::build`]: the words go beside the control while that leaves
/// them room enough, and above it when it does not, so they never run under it.
pub struct SettingRow<'a> {
    /// What the setting is called.
    title: String,
    /// What it does.
    about: String,
    /// What emits the control, and how wide it is drawn at most in logical pixels, which says whether the words fit beside it; nothing for a box of text, which is as wide as the row lets it.
    control: Option<(Content<'a>, Option<f32>)>,
    /// The button that resets it, while it was changed.
    reset: Option<FocusId>,
    /// What emits what opens under the row, such as a list of choices.
    below: Option<Content<'a>>,
    /// What the caller puts around the row's box.
    modifier: Modifier,
}

impl<'a> SettingRow<'a> {
    /// A row of the setting called `title` that does `about`, with no control yet.
    pub fn new(title: &str, about: &str) -> Self {
        Self {
            title: title.to_owned(),
            about: about.to_owned(),
            control: None,
            reset: None,
            below: None,
            modifier: Modifier::new(),
        }
    }

    /// The row with the control `control` emits, `width` logical pixels wide, or as wide as the row lets it when nothing.
    pub fn control(mut self, control: impl FnOnce() + 'a, width: Option<f32>) -> Self {
        self.control = Some((Box::new(control), width));
        self
    }

    /// The row with the button that resets it, answering to `id`, when it is given one.
    pub fn reset(mut self, id: Option<FocusId>) -> Self {
        self.reset = id;
        self
    }

    /// The row with what `below` emits open under it, when there is something.
    pub fn below(mut self, below: Option<impl FnOnce() + 'a>) -> Self {
        self.below = below.map(|below| Box::new(below) as Content<'a>);
        self
    }

    /// The row with `modifier` put around its box, outside its own.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the row, `room` logical pixels across.
    #[track_caller]
    pub fn build(self, room: f32) {
        let reset = self.reset;
        let reserved = if reset.is_some() { RESET_WIDTH } else { 0.0 };
        let width = self
            .control
            .as_ref()
            .map_or(0.0, |(_, width)| width.unwrap_or(TEXT_WIDTH));
        let beside = room - width - reserved - ROW_GAP;
        let stacked = beside < MIN_WORDS;
        let words = if stacked { room - reserved } else { beside };
        let (title, about) = (self.title, self.about);
        let control = self.control;
        let below = self.below;
        let modifier = self.modifier.chain(Modifier::new().pad_y(3));
        column(modifier, Flex::new().gap(2), move || {
            if stacked {
                column(Modifier::new().fill_max_width(), Flex::new().gap(2), || {
                    row(
                        Modifier::new().fill_max_width(),
                        Flex::new().start().between().gap(2),
                        || {
                            about_words(&title, &about, words, Modifier::new().grow());
                            if let Some(id) = reset {
                                reset_button(id);
                            }
                        },
                    );
                    if let Some((control, _)) = control {
                        block(Modifier::new().fill_max_width(), control);
                    }
                });
            } else {
                row(
                    Modifier::new(),
                    Flex::new().center().between().gap(6),
                    || {
                        about_words(&title, &about, words, Modifier::new().grow());
                        row(Modifier::new(), Flex::new().center().gap(2), || {
                            if let Some(id) = reset {
                                reset_button(id);
                            }
                            match control {
                                Some((control, Some(_))) => control(),
                                Some((control, None)) => {
                                    block(Modifier::new().width(TEXT_WIDTH), control);
                                }
                                None => {}
                            }
                        });
                    },
                );
            }
            if let Some(below) = below {
                below();
            }
        });
    }
}

/// Emits what a setting is called, `title`, over what it does, `about`, each wrapped to `words` logical pixels, in a column `modifier` sizes.
#[track_caller]
fn about_words(title: &str, about: &str, words: f32, modifier: Modifier) {
    column(modifier, Flex::new().gap(0.5), || {
        for part in wrap(title, 13, words) {
            line(part, 13, 1.4, Role::Text);
        }
        for part in wrap(about, 12, words) {
            line(part, 12, 1.4, Role::Muted);
        }
    });
}
