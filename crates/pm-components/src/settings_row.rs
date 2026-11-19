//! A row of the settings window: what a setting is called and does beside, or above, its one control, and the button that puts it back as it comes once it was changed; with the controls a row is made of.

use crate::events::reduce;
use crate::theme;
use crate::theme::sp;
use rustle::TextFieldState;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};

use crate::Button;
use crate::button::Variant;
use crate::events::Interactive;
use crate::focus::FocusId;
use crate::icons::IconName;
use crate::input::Input;
use crate::parts::{clipped, glyph, line, wrap};
use crate::settings_ids::PICKS;
use crate::size::Size;
use crate::theme::{Radius, Role};
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

/// Emits a switch that is on while `on` is true, answering to `id`; with no name it is drawn
/// but cannot be pressed. It is rustle's switch with a text-coloured knob on the success
/// colour while on and on the border colour while off.
#[track_caller]
pub fn switch(on: bool, id: Option<FocusId>) {
    let mut modifier = Modifier::new().size(SWITCH_WIDTH, theme::sp(5));
    if let Some(id) = id {
        modifier = modifier.cursor(CursorIcon::Pointer).control(id, reduce(id));
    }
    let colors = rustle::SwitchColors {
        checked_thumb_color: theme::color(Role::Text),
        checked_track_color: theme::color(Role::Success),
        unchecked_thumb_color: theme::color(Role::Text),
        unchecked_track_color: theme::color(Role::Border),
    };
    rustle::switch_with(modifier, on, None, true, colors);
}

/// How wide the choices `names` are drawn side by side, in logical pixels, at most.
pub fn choices_width(names: &[&str]) -> f32 {
    8.0 + names
        .iter()
        .map(|name| 22.0 + name.chars().count() as f32 * CHOICE_CHARACTER)
        .sum::<f32>()
}

/// Emits the choices `names` side by side as rustle's single-choice segmented buttons, `picked`
/// marked, each answering to the name `id_of` gives its place; a choice given no name is left out.
#[track_caller]
pub fn choices(names: &[&str], picked: usize, id_of: impl Fn(usize) -> Option<FocusId>) {
    let shown: Vec<(usize, &str, FocusId)> = names
        .iter()
        .enumerate()
        .filter_map(|(option, name)| id_of(option).map(|id| (option, *name, id)))
        .collect();
    let count = shown.len();
    let colors = choice_colors();
    let base = rustle::Shape::rounded(Radius::Md.px());
    let border_width = rustle::SegmentedButtonDefaults::BORDER_WIDTH;
    rustle::single_choice_segmented_button_row(
        Modifier::new().height(theme::sp(8)),
        border_width,
        move || {
            for (index, (option, name, id)) in shown.into_iter().enumerate() {
                let on = option == picked;
                rustle::key(option, || {
                    rustle::segmented_button(
                        on,
                        || {},
                        rustle::SegmentedButtonDefaults::item_shape(index, count, base),
                        Modifier::new()
                            .cursor(CursorIcon::Pointer)
                            .control(id, reduce(id)),
                        true,
                        colors,
                        rustle::SegmentedButtonDefaults::border_stroke(
                            colors.border_color(true, on),
                            border_width,
                        ),
                        (theme::sp(2.5), 0.0),
                        || {},
                        || line(name, 12, 1.4, if on { Role::Text } else { Role::Muted }),
                    );
                });
            }
        },
    );
}

/// The colours of the choices: the selected surface behind the one picked, nothing behind the
/// others, the text colour on the one picked and the muted one on the others, and the border
/// colour around every one.
fn choice_colors() -> rustle::SegmentedButtonColors {
    let border = theme::color(Role::Border);
    rustle::SegmentedButtonColors {
        active_container_color: theme::color(Role::SurfaceSelected),
        active_content_color: theme::color(Role::Text),
        active_border_color: border,
        inactive_container_color: rustle::Color::TRANSPARENT,
        inactive_content_color: theme::color(Role::Muted),
        inactive_border_color: border,
        ..rustle::SegmentedButtonDefaults::colors()
    }
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
    row_arranged(
        Modifier::new()
            .height(theme::sp(8))
            .border(1.0, theme::color(Role::Border), Radius::Md.px())
            .padding(1.0)
            .padding_xy(theme::sp(0.5), 0.0),
        Arrangement::SpacedBy(sp(1)),
        Alignment::Center,
        || {
            button(IconName::Minus, smaller);
            row_arranged(
                Modifier::new().width(64.0),
                Arrangement::Center,
                Alignment::Start,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        shown,
                        theme::mono_style(12, 1.4)
                            .color(theme::color(Role::Text))
                            .max_lines(1),
                        rustle::TextOverflow::Ellipsis,
                    );
                },
            );
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
pub fn picker_shown(options: &[String], filter: &TextFieldState) -> Vec<usize> {
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
pub fn picker_list(options: &[String], picked: Option<usize>, filter: (&TextFieldState, FocusId)) {
    let shown = picker_shown(options, filter.0);
    let more = shown.len().saturating_sub(MOST_LISTED);
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .background_rounded(theme::color(Role::Surface), Radius::Md.px())
            .border(1.0, theme::color(Role::Border), Radius::Md.px())
            .padding(1.0)
            .padding(theme::sp(1)),
        Arrangement::SpacedBy(sp(0.5)),
        Alignment::Start,
        || {
            if options.len() >= FILTERED_FROM {
                column_arranged(
                    Modifier::new().fill_max_width().padding(theme::sp(1)),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        Input::search(filter.0, filter.1)
                            .size(Size::Sm)
                            .placeholder("Type to narrow the list")
                            .build();
                    },
                );
            }
            for (row_place, place) in shown.into_iter().enumerate().take(MOST_LISTED) {
                let on = Some(place) == picked;
                let (Some(name), Some(id)) = (options.get(place), PICKS.id(row_place)) else {
                    continue;
                };
                Tile::new(id)
                    .selected(on)
                    .modifier(Modifier::new().fill_max_width())
                    .height(7.0)
                    .gap(2.0)
                    .radius(Radius::Sm)
                    .inner(Modifier::new().padding_xy(theme::sp(2), 0.0))
                    .arrangement(Arrangement::SpaceBetween)
                    .build_with(|| {
                        clipped(name.as_str(), 12, if on { Role::Text } else { Role::Muted });
                        if on {
                            glyph(IconName::Check, 3.5, Role::Text);
                        }
                    });
            }
            if more > 0 {
                row_arranged(
                    Modifier::new()
                        .height(theme::sp(6))
                        .padding_xy(theme::sp(2), 0.0),
                    Arrangement::Start,
                    Alignment::Center,
                    || {
                        line(
                            format!("{more} more; type to narrow"),
                            11,
                            1.4,
                            Role::Subtle,
                        );
                    },
                );
            }
        },
    );
}

/// Emits the box `field` is written in, answering to `id`, saying `placeholder` while empty.
#[track_caller]
pub fn text_box(field: &TextFieldState, id: FocusId, placeholder: &str) {
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
    field: (&TextFieldState, FocusId),
    add: (FocusId, &str),
    placeholder: &str,
) {
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(sp(1)),
        Alignment::Start,
        || {
            for (place, text) in lines.iter().enumerate() {
                row_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .height(theme::sp(6))
                        .interaction_background(
                            (theme::InteractionTones::new()
                                .hover(Role::SurfaceHover)
                                .radius(Radius::Sm))
                            .colors(),
                        )
                        .padding_xy(theme::sp(2), 0.0),
                    Arrangement::SpacedBy(sp(2)),
                    Alignment::Center,
                    || {
                        row_arranged(
                            Modifier::new().weight(1.0),
                            Arrangement::Start,
                            Alignment::Center,
                            || {
                                rustle::basic_text(
                                    Modifier::new(),
                                    text.as_str(),
                                    theme::mono_style(12, 1.4)
                                        .color(theme::color(Role::Text))
                                        .max_lines(1),
                                    rustle::TextOverflow::Ellipsis,
                                );
                            },
                        );
                        if let Some(remove) = remove_of(place) {
                            Button::icon(IconName::Close, remove).size(Size::Xs).build();
                        }
                    },
                );
            }
            row_arranged(
                Modifier::new().padding_each(0.0, theme::sp(1), 0.0, 0.0),
                Arrangement::SpacedBy(sp(2)),
                Alignment::Center,
                || {
                    column_arranged(
                        Modifier::new().width(TEXT_WIDTH),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            text_box(field.0, field.1, placeholder);
                        },
                    );
                    Button::new(add.1.to_owned(), add.0).size(Size::Sm).build();
                },
            );
        },
    );
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
        let modifier = Modifier::new()
            .fill_max_width()
            .chain(self.modifier)
            .chain(Modifier::new().padding_xy(0.0, theme::sp(3)));
        column_arranged(
            modifier,
            Arrangement::SpacedBy(sp(2)),
            Alignment::Start,
            move || {
                if stacked {
                    column_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(sp(2)),
                        Alignment::Start,
                        || {
                            row_arranged(
                                Modifier::new().fill_max_width(),
                                Arrangement::SpacedBy(sp(2)),
                                Alignment::Start,
                                || {
                                    about_words(&title, &about, words, Modifier::new().weight(1.0));
                                    if let Some(id) = reset {
                                        reset_button(id);
                                    }
                                },
                            );
                            match control {
                                Some((control, Some(_))) => {
                                    row_arranged(
                                        Modifier::new().fill_max_width(),
                                        Arrangement::Start,
                                        Alignment::Start,
                                        control,
                                    );
                                }
                                Some((control, None)) => {
                                    column_arranged(
                                        Modifier::new().fill_max_width(),
                                        Arrangement::Start,
                                        Alignment::Start,
                                        control,
                                    );
                                }
                                None => {}
                            }
                        },
                    );
                } else {
                    row_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(sp(6)),
                        Alignment::Center,
                        || {
                            about_words(&title, &about, words, Modifier::new().weight(1.0));
                            row_arranged(
                                Modifier::new(),
                                Arrangement::SpacedBy(sp(2)),
                                Alignment::Center,
                                || {
                                    if let Some(id) = reset {
                                        reset_button(id);
                                    }
                                    match control {
                                        Some((control, Some(_))) => control(),
                                        Some((control, None)) => {
                                            column_arranged(
                                                Modifier::new().width(TEXT_WIDTH),
                                                Arrangement::Start,
                                                Alignment::Start,
                                                control,
                                            );
                                        }
                                        None => {}
                                    }
                                },
                            );
                        },
                    );
                }
                if let Some(below) = below {
                    below();
                }
            },
        );
    }
}

/// Emits what a setting is called, `title`, over what it does, `about`, each wrapped to `words` logical pixels, in a column `modifier` sizes.
#[track_caller]
fn about_words(title: &str, about: &str, words: f32, modifier: Modifier) {
    column_arranged(
        modifier,
        Arrangement::SpacedBy(sp(0.5)),
        Alignment::Start,
        || {
            for part in wrap(title, 13, words) {
                line(part, 13, 1.4, Role::Text);
            }
            for part in wrap(about, 12, words) {
                line(part, 12, 1.4, Role::Muted);
            }
        },
    );
}
