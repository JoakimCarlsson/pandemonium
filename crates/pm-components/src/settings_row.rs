//! A row of the settings window: what a setting is called and does beside, or above, its one control, and the button that puts it back as it comes once it was changed; with the controls a row is made of.

use crate::button;
use crate::events;
use crate::icons;
use crate::target::Target;
use crate::theme;
use crate::theme::Frame;
use crate::theme::Indicated;
use rustle::Modifier;
use rustle::TextFieldState;
use rustle::TextOverflow;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{Icon, phosphor};

use crate::Button;
use crate::button::Variant;
use crate::input::Input;
use crate::parts::{clipped, glyph, line, wrapping};
use crate::settings_rows::PICKS;
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
pub fn switch(on: bool, id: Option<Target>) {
    let modifier = events::pressable(Modifier::new().size(SWITCH_WIDTH, theme::sp(5)), id);
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
pub fn choices(names: &[&str], picked: usize, id_of: impl Fn(usize) -> Option<Target>) {
    let shown: Vec<(usize, &str, Target)> = names
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
                        events::pressed(Modifier::new(), id),
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
pub fn number(shown: String, smaller: Target, bigger: Target) {
    let button = |icon: Icon, id: Target| {
        Button::icon(icon, id).tone(Role::Text).build();
    };
    rustle::row_arranged(
        Modifier::new()
            .height(theme::sp(8))
            .outlined(Radius::Md)
            .padding_xy(theme::sp(0.5), 0.0),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            button(phosphor::MINUS, smaller);
            row_arranged(
                Modifier::new().width(64.0),
                Arrangement::Center,
                Alignment::Start,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        shown,
                        theme::code(12, Role::Text),
                        TextOverflow::Ellipsis,
                    );
                },
            );
            button(phosphor::PLUS, bigger);
        },
    );
}

/// Emits the button saying `shown`, the choice picked, `width` logical pixels wide, that opens the list of choices, answering to `id`; its chevron points down while the list is `open`.
#[track_caller]
pub fn picker(shown: &str, width: f32, id: Target, open: bool) {
    button::small_button(shown, id, Variant::Field)
        .width(width)
        .selected(open)
        .trailing_icon(icons::chevron(open))
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
pub fn picker_list(options: &[String], picked: Option<usize>, filter: (&TextFieldState, Target)) {
    let shown = picker_shown(options, filter.0);
    let more = shown.len().saturating_sub(MOST_LISTED);
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .panel(Radius::Md)
            .padding(theme::sp(1)),
        Arrangement::SpacedBy(theme::sp(0.5)),
        Alignment::Start,
        || {
            if options.len() >= FILTERED_FROM {
                rustle::column_arranged(
                    Modifier::new().fill_max_width().padding(theme::sp(1)),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        Input::search(filter.0, filter.1)
                            .size(Size::Sm)
                            .placeholder("Type to narrow the list")
                            .submits(Target::Settings(
                                crate::areas::SettingsEvent::SettingsPickFirst,
                            ))
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
                            glyph(phosphor::CHECK, 3.5, Role::Text);
                        }
                    });
            }
            if more > 0 {
                rustle::row_arranged(
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

/// Emits the box `field` is written in, answering to `id`, saying `placeholder` while
/// empty, handing in the press of `submit` on Enter.
#[track_caller]
pub fn text_box(field: &TextFieldState, id: Target, placeholder: &str, submit: Target) {
    Input::new(field, id)
        .size(Size::Sm)
        .placeholder(placeholder)
        .submits(submit)
        .build();
}

/// Emits the `lines` one under another, each with a button that removes it answering to the name `remove_of` gives its place, then the box `field` answering to its name, saying `placeholder` while empty, with the button `add`, labelled with its words, that adds what is written in it; a line given no name has no remove button.
#[track_caller]
pub fn list(
    lines: &[String],
    remove_of: impl Fn(usize) -> Option<Target>,
    field: (&TextFieldState, Target),
    add: (Target, &str),
    placeholder: &str,
) {
    rustle::column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Start,
        || {
            for (place, text) in lines.iter().enumerate() {
                rustle::row_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .height(theme::sp(6))
                        .indicated(
                            theme::InteractionTones::new()
                                .hover(Role::SurfaceHover)
                                .radius(Radius::Sm),
                            theme::Focused::Rustle,
                        )
                        .padding_xy(theme::sp(2), 0.0),
                    Arrangement::SpacedBy(theme::sp(2)),
                    Alignment::Center,
                    || {
                        rustle::row_arranged(
                            Modifier::new().weight(1.0),
                            Arrangement::Start,
                            Alignment::Center,
                            || {
                                rustle::basic_text(
                                    Modifier::new(),
                                    text.as_str(),
                                    theme::code(12, Role::Text),
                                    TextOverflow::Ellipsis,
                                );
                            },
                        );
                        if let Some(remove) = remove_of(place) {
                            Button::icon(phosphor::X, remove).size(Size::Xs).build();
                        }
                    },
                );
            }
            rustle::row_arranged(
                Modifier::new().padding_each(0.0, theme::sp(1), 0.0, 0.0),
                Arrangement::SpacedBy(theme::sp(2)),
                Alignment::Center,
                || {
                    rustle::column_arranged(
                        Modifier::new().width(TEXT_WIDTH),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            text_box(field.0, field.1, placeholder, add.0);
                        },
                    );
                    button::action(add.1, add.0, Variant::Default);
                },
            );
        },
    );
}

/// Emits the button that puts a setting back as it comes, answering to `id`, when there is one.
#[track_caller]
fn reset_button(id: Option<Target>) {
    if let Some(id) = id {
        Button::icon(phosphor::ARROW_COUNTER_CLOCKWISE, id)
            .size(Size::Xs)
            .build();
    }
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
    reset: Option<Target>,
    /// What emits what opens under the row, such as a list of choices.
    below: Option<Content<'a>>,
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
        }
    }

    /// The row with the control `control` emits, `width` logical pixels wide, or as wide as the row lets it when nothing.
    pub fn control(mut self, control: impl FnOnce() + 'a, width: Option<f32>) -> Self {
        self.control = Some((Box::new(control), width));
        self
    }

    /// The row with the button that resets it, answering to `id`, when it is given one.
    pub fn reset(mut self, id: Option<Target>) -> Self {
        self.reset = id;
        self
    }

    /// The row with what `below` emits open under it, when there is something.
    pub fn below(mut self, below: Option<impl FnOnce() + 'a>) -> Self {
        self.below = below.map(|below| Box::new(below) as Content<'a>);
        self
    }

    /// Emits the row, `room` logical pixels across.
    #[track_caller]
    pub fn build(self, room: f32) {
        let Self {
            title,
            about,
            control,
            reset,
            below,
        } = self;
        let reserved = if reset.is_some() { RESET_WIDTH } else { 0.0 };
        let width = control
            .as_ref()
            .map_or(0.0, |(_, width)| width.unwrap_or(TEXT_WIDTH));
        let beside = room - width - reserved - ROW_GAP;
        let stacked = beside < MIN_WORDS;
        let modifier = Modifier::new()
            .fill_max_width()
            .padding_xy(0.0, theme::sp(3));
        rustle::column_arranged(
            modifier,
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Start,
            move || {
                if stacked {
                    rustle::column_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(theme::sp(2)),
                        Alignment::Start,
                        || {
                            rustle::row_arranged(
                                Modifier::new().fill_max_width(),
                                Arrangement::SpacedBy(theme::sp(2)),
                                Alignment::Start,
                                || {
                                    about_words(&title, &about);
                                    reset_button(reset);
                                },
                            );
                            match control {
                                Some((control, Some(_))) => {
                                    rustle::row_arranged(
                                        Modifier::new().fill_max_width(),
                                        Arrangement::Start,
                                        Alignment::Start,
                                        control,
                                    );
                                }
                                Some((control, None)) => {
                                    rustle::column_arranged(
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
                    rustle::row_arranged(
                        Modifier::new().fill_max_width(),
                        Arrangement::SpacedBy(theme::sp(6)),
                        Alignment::Center,
                        || {
                            about_words(&title, &about);
                            rustle::row_arranged(
                                Modifier::new(),
                                Arrangement::SpacedBy(theme::sp(2)),
                                Alignment::Center,
                                || {
                                    reset_button(reset);
                                    match control {
                                        Some((control, Some(_))) => control(),
                                        Some((control, None)) => {
                                            rustle::column_arranged(
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

/// Emits what a setting is called, `title`, over what it does, `about`, each wrapped by the text layout to the room its row leaves, in a column taking that room.
#[track_caller]
fn about_words(title: &str, about: &str) {
    rustle::column_arranged(
        Modifier::new().weight(1.0),
        Arrangement::SpacedBy(theme::sp(0.5)),
        Alignment::Start,
        || {
            wrapping(Modifier::new(), title, 13, 1.4, Role::Text);
            wrapping(Modifier::new(), about, 12, 1.4, Role::Muted);
        },
    );
}
