//! The app's own small design system: the rows, columns, lines of text and
//! dividing rules its views repeat, each a thin themed wrapper over the rustle
//! composable it names, so that a view says what it shows rather than which
//! arrangement, type size and colour role it is set in.
//!
//! Every gap is given in theme steps ([`theme::sp`]) and every type size in
//! interface pixels before the reader's text scale, as the theme counts them.
//! A gap of zero packs children from the start with no space between them.
//! The composables are `#[track_caller]`, so what they emit is keyed by the
//! view that calls them, as the rustle call they wrap would be.

use rustle::{
    Alignment, Arrangement, Canvas, Color, FontWeight, Modifier, TextOverflow, TextStyle,
};

use crate::button::{Button, Variant};
use crate::size::Size;
use crate::target::Target;
use crate::theme::{self, Radius, Role, Side};

/// The arrangement that puts `gap` theme steps between children, or packs them
/// from the start when `gap` is zero.
fn spaced(gap: impl Into<f64>) -> Arrangement {
    let gap = gap.into();
    if gap == 0.0 {
        Arrangement::Start
    } else {
        Arrangement::SpacedBy(theme::sp(gap))
    }
}

/// Emits a row of `content` with `modifier`, its children `gap` theme steps
/// apart and centred across the row: the app's usual row of an icon, a label
/// and buttons.
#[track_caller]
pub fn row(modifier: Modifier, gap: impl Into<f64>, content: impl FnOnce()) {
    rustle::row_arranged(modifier, spaced(gap), Alignment::Center, content);
}

/// Emits a row of `content` with `modifier`, its children `gap` theme steps
/// apart and hung from the top of the row, for children of several lines.
#[track_caller]
pub fn row_top(modifier: Modifier, gap: impl Into<f64>, content: impl FnOnce()) {
    rustle::row_arranged(modifier, spaced(gap), Alignment::Start, content);
}

/// Emits a column of `content` with `modifier`, its children `gap` theme steps
/// apart and set against its start edge.
#[track_caller]
pub fn column(modifier: Modifier, gap: impl Into<f64>, content: impl FnOnce()) {
    rustle::column_arranged(modifier, spaced(gap), Alignment::Start, content);
}

/// Emits `words` with `modifier` in `style`, cut short with an ellipsis where
/// they do not fit: how the app sets every line of interface text.
#[track_caller]
pub fn text(modifier: Modifier, words: impl Into<String>, style: TextStyle) {
    rustle::basic_text(modifier, words, style, TextOverflow::Ellipsis);
}

/// One line of interface text `pixels` tall, in the sans family and `tone`.
pub fn line(pixels: impl Into<f64>, tone: impl Into<Color>) -> TextStyle {
    theme::text_style(pixels, 1.4)
        .color(theme::color(tone))
        .max_lines(1)
}

/// One line of interface text `pixels` tall at medium weight, in the sans
/// family and `tone`: titles and names that lead a row.
pub fn strong(pixels: impl Into<f64>, tone: impl Into<Color>) -> TextStyle {
    line(pixels, tone).font_weight(FontWeight::MEDIUM)
}

/// One line of text `pixels` tall in the mono family and `tone`: paths,
/// hashes, keys and counts.
pub fn code(pixels: impl Into<f64>, tone: impl Into<Color>) -> TextStyle {
    theme::mono_style(pixels, 1.4)
        .color(theme::color(tone))
        .max_lines(1)
}

/// A drawing for [`Modifier::draw_behind`] that rules a one pixel line in the
/// border role along `side` of the box: how panels, bars and rows are divided.
pub fn rule(side: Side) -> impl Fn(&mut Canvas) + 'static {
    theme::edge(side, 1.0, theme::color(Role::Border))
}

/// The app's frames for a box, chained onto a [`Modifier`] like its own
/// methods: a dividing rule along one side, a one pixel outline and the raised
/// panel that cards and popups sit on.
pub trait Frame {
    /// Rules a one pixel border line along `side` (see [`rule`]) and keeps
    /// what the box holds clear of it.
    fn ruled(self, side: Side) -> Self;

    /// Outlines the box one pixel wide in the border role, its corners as
    /// round as `radius`, and keeps what it holds inside the outline.
    fn outlined(self, radius: Radius) -> Self;

    /// Fills the box with the surface role and outlines it, both with corners
    /// as round as `radius`: the panel cards, lists and popups are set on.
    fn panel(self, radius: Radius) -> Self;
}

impl Frame for Modifier {
    /// Draws [`rule`] behind the box and pads it by one pixel on `side`.
    fn ruled(self, side: Side) -> Self {
        let (left, top, right, bottom) = side.padding(1.0);
        self.draw_behind(rule(side))
            .padding_each(left, top, right, bottom)
    }

    /// Adds a one pixel border-role outline and pads the box by one pixel.
    fn outlined(self, radius: Radius) -> Self {
        self.border(1.0, theme::color(Role::Border), radius.px())
            .padding(1.0)
    }

    /// Adds a rounded surface background, then [`Frame::outlined`].
    fn panel(self, radius: Radius) -> Self {
        self.background_rounded(theme::color(Role::Surface), radius.px())
            .outlined(radius)
    }
}

/// A small button saying `label` in `variant`, answering to `id`: the size of
/// the actions in rows, bars, footers and dialogs. Finish it with
/// [`Button::build`] after any further settings.
pub fn small_button(label: impl Into<String>, id: Target, variant: Variant) -> Button {
    Button::new(label, id).variant(variant).size(Size::Sm)
}

/// Emits a small button saying `label` in `variant` that answers to `id`,
/// with no further settings: [`small_button`] built at once.
#[track_caller]
pub fn action(label: impl Into<String>, id: Target, variant: Variant) {
    small_button(label, id, variant).build();
}

/// Emits the buttons that end a form or a card, side by side: `label` in
/// the default variant answering to `confirm`, turned off unless `ready`,
/// then the ghost button saying "Cancel" that answers to `cancel`.
#[track_caller]
pub fn confirm_row(label: &str, confirm: Target, ready: bool, cancel: Target) {
    row(Modifier::new(), 2, || {
        small_button(label, confirm, Variant::Default)
            .disabled(!ready)
            .build();
        action("Cancel", cancel, Variant::Ghost);
    });
}

/// Emits one line of `words` `pixels` tall in `tone`, inset `x` theme steps
/// at either end and `y` above and below: what a list, menu or view says
/// when it has nothing to show, or is still busy.
#[track_caller]
pub fn note(
    x: impl Into<f64>,
    y: impl Into<f64>,
    words: impl Into<String>,
    pixels: u32,
    tone: impl Into<Color>,
) {
    column(
        Modifier::new().padding_xy(theme::sp(x), theme::sp(y)),
        0,
        || text(Modifier::new(), words, line(pixels, tone)),
    );
}

/// The caret that marks a foldable list or section: pointing down while it
/// is `open`, right while it is folded.
pub fn chevron(open: bool) -> rustle::Icon {
    if open {
        rustle::phosphor::CARET_DOWN
    } else {
        rustle::phosphor::CARET_RIGHT
    }
}

/// Emits a bar across its parent `height` theme steps tall, ruled along its
/// bottom and inset `inset` theme steps at either end, holding `content` in a
/// row `gap` theme steps apart: the header that tops panels, lists and steps.
#[track_caller]
pub fn bar(
    height: impl Into<f64>,
    inset: impl Into<f64>,
    gap: impl Into<f64>,
    content: impl FnOnce(),
) {
    row(
        Modifier::new()
            .fill_max_width()
            .height(theme::sp(height))
            .ruled(Side::Bottom)
            .padding_xy(theme::sp(inset), 0.0),
        gap,
        content,
    );
}

/// Emits `content` in a row that wraps onto further lines where it runs out
/// of room, its children two theme steps apart both ways and centred on each
/// line; while `spread` it fills its parent's width and pushes its children to
/// both ends: the line of a heading and its controls.
#[track_caller]
pub fn flow(spread: bool, content: impl FnOnce()) {
    let options =
        rustle::FlowOptions::new(theme::sp(2), theme::sp(2)).item_alignment(Alignment::Center);
    if spread {
        rustle::flow_row_with(
            Modifier::new().fill_max_width(),
            options.arrangement(Arrangement::SpaceBetween),
            content,
        );
    } else {
        rustle::flow_row_with(Modifier::new(), options, content);
    }
}

/// Emits `content` in a row across its parent that wraps onto further lines
/// where it runs out of room, its children two theme steps apart both ways,
/// centred on each line and pushed to its end: the actions under a card or
/// a question.
#[track_caller]
pub fn footer(content: impl FnOnce()) {
    rustle::flow_row_with(
        Modifier::new().fill_max_width(),
        rustle::FlowOptions::new(theme::sp(2), theme::sp(2))
            .arrangement(Arrangement::End)
            .item_alignment(Alignment::Center),
        content,
    );
}

/// Emits `content` in a column across its parent, its children `gap` theme
/// steps apart, at most `height` logical pixels tall and scrolled as `scroll`
/// holds once it is taller: the body of menus, popovers and lists.
#[track_caller]
pub fn scrolled(
    scroll: &rustle::ScrollState,
    height: f32,
    gap: impl Into<f64>,
    content: impl FnOnce(),
) {
    column(
        Modifier::new()
            .fill_max_width()
            .height_in(0.0, height)
            .vertical_scroll(scroll)
            .fill_max_width(),
        gap,
        content,
    );
}

/// `modifier` with the pointing hand over it, made the control `id` that
/// sends the press of `id` to the workspace when clicked: how links, chips and
/// counts that act at once are made pressable.
#[track_caller]
pub fn pressed(modifier: Modifier, id: Target) -> Modifier {
    use crate::events::Interactive;
    modifier
        .cursor(rustle::CursorIcon::Pointer)
        .control(id, crate::events::send(crate::events::Event::pressing(id)))
}

/// `modifier` made [`pressed`] as `id` when there is one, or as it is when
/// there is none: rows and controls that can be pressed only sometimes.
#[track_caller]
pub fn pressable(modifier: Modifier, id: Option<Target>) -> Modifier {
    match id {
        Some(id) => pressed(modifier, id),
        None => modifier,
    }
}

/// The fills of a row of a list that is filled while `selected`, and fills under the pointer only while it is not.
pub fn selectable(selected: bool) -> theme::InteractionTones {
    if selected {
        theme::InteractionTones::new().idle(Role::SurfaceSelected)
    } else {
        theme::InteractionTones::new().hover(Role::SurfaceHover)
    }
}
