//! A row of the settings window: what a setting is called and does beside, or above, its one control, and the button that puts it back as it comes once it was changed; with the controls a row is made of.

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, cursor_pointer, flex, flex_col, flex_none, font_mono,
    gap, h, items_center, items_start, justify_between, justify_center, justify_end, min_w_px, p,
    px, rounded, rounded_full, size, w_full, w_px,
};
use pm_text::Field;
use pm_ui::{Node, node};

use crate::Button;
use crate::button::Variant;
use crate::input::Input;
use crate::parts::{clipped, glyph, line, wrap};
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

/// How many choices a list has before it offers a box to narrow it.
const FILTERED_FROM: usize = 10;

/// A switch that is on while `on` is true, answering to `id`.
pub fn switch(on: bool, id: FocusId) -> Node {
    let knob = node().styles([size(4), rounded_full(), bg(Role::Text)]);
    let track = node()
        .styles([
            flex(),
            items_center(),
            flex_none(),
            w_px(36),
            h(5),
            p(0.5),
            rounded_full(),
            cursor_pointer(),
            bg(if on { Role::Success } else { Role::Border }),
        ])
        .focusable(id);
    if on {
        track.styles([justify_end()]).child(knob)
    } else {
        track.child(knob)
    }
}

/// How wide the choices `names` are drawn side by side, in logical pixels, at most.
pub fn choices_width(names: &[&str]) -> f32 {
    8.0 + names
        .iter()
        .map(|name| 22.0 + name.chars().count() as f32 * CHOICE_CHARACTER)
        .sum::<f32>()
}

/// The choices `names` side by side, `picked` marked, each answering to the name `id_of` gives its place.
pub fn choices(names: &[&str], picked: usize, id_of: impl Fn(usize) -> FocusId) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            flex_none(),
            gap(0.5),
            px(0.5),
            h(8),
            rounded(Radius::Md),
            border(),
            border_color(Role::Border),
        ])
        .children(names.iter().enumerate().map(|(option, name)| {
            let on = option == picked;
            Tile::new(id_of(option))
                .selected(on)
                .height(6.5)
                .radius(Radius::Sm)
                .build()
                .styles([justify_center(), px(2.5)])
                .child(line(
                    *name,
                    12,
                    1.4,
                    if on { Role::Text } else { Role::Muted },
                ))
        }))
}

/// How wide a number with its buttons is, in logical pixels.
pub const NUMBER_WIDTH: f32 = 136.0;

/// The number `shown` between the buttons `smaller` and `bigger` that change it.
pub fn number(shown: String, smaller: FocusId, bigger: FocusId) -> Node {
    let button = |icon: IconName, id: FocusId| {
        Button::icon(icon, id)
            .size(Size::Sm)
            .tone(Role::Text)
            .build()
    };
    node()
        .styles([
            flex(),
            items_center(),
            flex_none(),
            gap(1),
            px(0.5),
            h(8),
            rounded(Radius::Md),
            border(),
            border_color(Role::Border),
        ])
        .child(button(IconName::Minus, smaller))
        .child(
            node()
                .styles([flex(), justify_center(), w_px(64.0)])
                .child(line(shown, 12, 1.4, Role::Text).styles([font_mono()])),
        )
        .child(button(IconName::Plus, bigger))
}

/// The button saying `shown`, the choice picked, `width` logical pixels wide, that opens the list of choices, answering to `id`; its chevron points down while the list is `open`.
pub fn picker(shown: &str, width: f32, id: FocusId, open: bool) -> Node {
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
        .build()
}

/// The open list of `options` under a row, `picked` marked, each answering to the name `id_of` gives its place among all of them, narrowed by what `filter` holds when they are many.
pub fn picker_list(
    options: &[String],
    picked: Option<usize>,
    id_of: impl Fn(usize) -> FocusId,
    filter: (&Field, FocusId),
) -> Node {
    let needle = filter.0.text().trim().to_lowercase();
    let shown: Vec<(usize, &String)> = options
        .iter()
        .enumerate()
        .filter(|(_, name)| needle.is_empty() || name.to_lowercase().contains(&needle))
        .collect();
    let more = shown.len().saturating_sub(MOST_LISTED);
    let mut list = node().styles([
        flex(),
        flex_col(),
        gap(0.5),
        flex_none(),
        p(1),
        rounded(Radius::Md),
        border(),
        border_color(Role::Border),
        bg(Role::Surface),
    ]);
    if options.len() >= FILTERED_FROM {
        list = list.child(
            node().styles([flex_none(), p(1)]).child(
                Input::search(filter.0, filter.1)
                    .size(Size::Sm)
                    .placeholder("Type to narrow the list")
                    .build(),
            ),
        );
    }
    list = list.children(shown.into_iter().take(MOST_LISTED).map(|(place, name)| {
        let on = Some(place) == picked;
        Tile::new(id_of(place))
            .selected(on)
            .height(7.0)
            .gap(2.0)
            .radius(Radius::Sm)
            .build()
            .styles([justify_between(), px(2)])
            .child(clipped(
                name.as_str(),
                12,
                if on { Role::Text } else { Role::Muted },
            ))
            .children(on.then(|| glyph(IconName::Check, 3.5, Role::Text)))
    }));
    if more > 0 {
        list = list.child(
            node()
                .styles([flex(), items_center(), h(6), px(2)])
                .child(line(
                    format!("{more} more; type to narrow"),
                    11,
                    1.4,
                    Role::Subtle,
                )),
        );
    }
    list
}

/// The box `field` is written in, answering to `id`, saying `placeholder` while empty.
pub fn text_box(field: &Field, id: FocusId, placeholder: &str) -> Node {
    Input::new(field, id)
        .size(Size::Sm)
        .placeholder(placeholder)
        .build()
}

/// The `lines` one under another, each with a button that removes it answering to the name `remove_of` gives its place, then the box `field` answering to `field_id`, saying `placeholder` while empty, with the button `add`, labelled `label`, that adds what is written in it.
pub fn list(
    lines: &[String],
    remove_of: impl Fn(usize) -> FocusId,
    field: (&Field, FocusId),
    add: (FocusId, &str),
    placeholder: &str,
) -> Node {
    let rows = lines.iter().enumerate().map(|(place, text)| {
        node()
            .styles([
                flex(),
                items_center(),
                justify_between(),
                gap(2),
                px(2),
                h(6),
                rounded(Radius::Sm),
                bg(Role::SurfaceHover).hover(),
            ])
            .child(line(text.as_str(), 12, 1.4, Role::Text).styles([font_mono()]))
            .child(
                Button::icon(IconName::Close, remove_of(place))
                    .size(Size::Xs)
                    .build(),
            )
    });
    node()
        .styles([flex(), flex_col(), gap(1), flex_none()])
        .children(rows)
        .child(
            node()
                .styles([flex(), items_center(), gap(2), pm_style::pt(1)])
                .child(
                    node()
                        .styles([w_px(TEXT_WIDTH), flex_none()])
                        .child(text_box(field.0, field.1, placeholder)),
                )
                .child(Button::new(add.1.to_owned(), add.0).size(Size::Sm).build()),
        )
}

/// The button that puts a setting back as it comes, answering to `id`.
fn reset_button(id: FocusId) -> Node {
    Button::icon(IconName::Undo, id).size(Size::Xs).build()
}

/// A row being described: what the setting is called and does, its control and how wide that is, the button that resets it and what opens under it.
///
/// Build it with [`SettingRow::new`], give it its control with
/// [`SettingRow::control`], and turn it into a node `room` logical pixels
/// across with [`SettingRow::build`]: the words go beside the control while
/// that leaves them room enough, and above it when it does not, so they
/// never run under it.
pub struct SettingRow {
    /// What the setting is called.
    title: String,
    /// What it does.
    about: String,
    /// The control, and how wide it is drawn at most in logical pixels, which says whether the words fit beside it; nothing for a box of text, which is as wide as the row lets it.
    control: Option<(Node, Option<f32>)>,
    /// The button that resets it, while it was changed.
    reset: Option<FocusId>,
    /// What opens under the row, such as a list of choices.
    below: Option<Node>,
}

impl SettingRow {
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

    /// The row with `control`, `width` logical pixels wide, or as wide as the row lets it when nothing.
    pub fn control(mut self, control: Node, width: Option<f32>) -> Self {
        self.control = Some((control, width));
        self
    }

    /// The row with the button that resets it, answering to `id`, when it is given one.
    pub fn reset(mut self, id: Option<FocusId>) -> Self {
        self.reset = id;
        self
    }

    /// The row with `below` open under it.
    pub fn below(mut self, below: Option<Node>) -> Self {
        self.below = below;
        self
    }

    /// The node of the row, `room` logical pixels across.
    pub fn build(self, room: f32) -> Node {
        let reset = self.reset.map(reset_button);
        let reserved = if reset.is_some() { RESET_WIDTH } else { 0.0 };
        let width = self
            .control
            .as_ref()
            .map_or(0.0, |(_, width)| width.unwrap_or(TEXT_WIDTH));
        let beside = room - width - reserved - ROW_GAP;
        let stacked = beside < MIN_WORDS;
        let words = if stacked { room - reserved } else { beside };
        let about = node()
            .styles([flex(), flex_col(), gap(0.5), min_w_px(0)])
            .children(
                wrap(&self.title, 13, words)
                    .into_iter()
                    .map(|words| line(words, 13, 1.4, Role::Text)),
            )
            .children(
                wrap(&self.about, 12, words)
                    .into_iter()
                    .map(|words| line(words, 12, 1.4, Role::Muted)),
            );
        let control = self.control.map(|(control, width)| match width {
            Some(_) => control.styles([flex_none()]),
            None if stacked => control.styles([w_full()]),
            None => control.styles([w_px(TEXT_WIDTH), flex_none()]),
        });
        let row = if stacked {
            let head = node()
                .styles([flex(), items_start(), justify_between(), gap(2), w_full()])
                .child(about)
                .children(reset);
            node()
                .styles([flex(), flex_col(), gap(2), w_full(), flex_none()])
                .child(head)
                .children(control.map(|control| {
                    node()
                        .styles([flex(), w_full(), min_w_px(0)])
                        .child(control)
                }))
        } else {
            node()
                .styles([
                    flex(),
                    items_center(),
                    justify_between(),
                    gap(6),
                    flex_none(),
                ])
                .child(about)
                .child(
                    node()
                        .styles([flex(), items_center(), gap(2), flex_none()])
                        .children(reset)
                        .children(control),
                )
        };
        node()
            .styles([flex(), flex_col(), gap(2), pm_style::py(3), flex_none()])
            .child(row)
            .children(self.below)
    }
}
