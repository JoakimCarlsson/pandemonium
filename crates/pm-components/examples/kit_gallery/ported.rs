//! The exemplar components ported onto the kit, as the porting guide says to
//! port them: builders keep `new`, their setters and `build`, which emits;
//! `modifier` adds to the outermost box; `build_with` takes the children.

use pm_components::kit::{
    FieldText, FocusId, Hovered, IconName, Interactive, Looks, Radius, Role, Side, Styled, Tone,
    Type, at_window, icon, label, label_with, sp, spinner, spinning_icon,
};
use pm_field::Field;
use rustle::{Alignment, Arrangement, column_arranged, row_arranged};
use rustle::{CursorIcon, Modifier};

pub use pm_components::size::Size;

/// How a button looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Variant {
    /// Filled with the accent.
    #[default]
    Default,
    /// Filled with a quiet surface.
    Secondary,
    /// Filled with the danger colour.
    Destructive,
    /// Outlined.
    Outline,
    /// Only filled under the pointer.
    Ghost,
    /// Underlined text.
    Link,
    /// A field that opens a list.
    Field,
}

/// How wide a button is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Reach {
    /// As wide as what it holds.
    #[default]
    Content,
    /// A fixed width in logical pixels.
    Fixed(f32),
    /// The room left in its row.
    Fill,
}

/// A button being described.
#[derive(Clone, Debug)]
pub struct Button {
    /// The words on it.
    label: Option<String>,
    /// The icon before the words.
    icon: Option<IconName>,
    /// The icon after the words.
    trailing: Option<IconName>,
    /// How it looks.
    variant: Variant,
    /// How big it is.
    size: Size,
    /// What it answers to.
    id: FocusId,
    /// Whether it is switched off.
    disabled: bool,
    /// Whether it is a pill.
    round: bool,
    /// Whether its words are set in the code family.
    monospace: bool,
    /// Whether its icon turns.
    spinning: bool,
    /// Whether it is picked.
    selected: bool,
    /// The tone of its words and icons, instead of the variant's.
    tone: Option<Role>,
    /// How wide it is.
    reach: Reach,
    /// What the caller adds to its outermost box.
    modifier: Modifier,
}

impl Button {
    /// A medium default button saying `label`, answering to `id`.
    pub fn new(label: impl Into<String>, id: FocusId) -> Self {
        Self {
            label: Some(label.into()),
            icon: None,
            trailing: None,
            variant: Variant::Default,
            size: Size::Md,
            id,
            disabled: false,
            round: false,
            monospace: false,
            spinning: false,
            selected: false,
            tone: None,
            reach: Reach::Content,
            modifier: Modifier::new(),
        }
    }

    /// A small ghost button showing only `icon`, answering to `id`.
    pub fn icon(icon: IconName, id: FocusId) -> Self {
        Self {
            label: None,
            icon: Some(icon),
            variant: Variant::Ghost,
            size: Size::Sm,
            ..Self::new("", id)
        }
    }

    /// The button looking like `variant`.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// The button as a pill.
    pub fn round(mut self) -> Self {
        self.round = true;
        self
    }

    /// The words set in the code family.
    pub fn monospace(mut self) -> Self {
        self.monospace = true;
        self
    }

    /// The button `size` big.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// `icon` before the words.
    pub fn with_icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// `icon` after the words.
    pub fn trailing_icon(mut self, icon: IconName) -> Self {
        self.trailing = Some(icon);
        self
    }

    /// Picked while `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Words and icons in `tone`.
    pub fn tone(mut self, tone: Role) -> Self {
        self.tone = Some(tone);
        self
    }

    /// `width` logical pixels wide.
    pub fn width(mut self, width: f32) -> Self {
        self.reach = Reach::Fixed(width);
        self
    }

    /// Taking the room left in its row, its words cut short at its edge.
    pub fn fill(mut self) -> Self {
        self.reach = Reach::Fill;
        self
    }

    /// Switched off while `disabled`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Its icon turning while `spinning`.
    pub fn spinning(mut self, spinning: bool) -> Self {
        self.spinning = spinning;
        self
    }

    /// `modifier` put around the button's box, outside its own: sizes, weight, placement, a tooltip.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// The height and side padding in steps, the type size in pixels and the side of an icon button in steps.
    fn measure(&self) -> (f32, f32, u32, f32) {
        match self.size {
            Size::Xs => (7.0, 2.5, 12, 6.0),
            Size::Sm => (8.0, 3.0, 12, 7.0),
            Size::Md => (9.0, 4.0, 13, 8.0),
            Size::Lg => (10.0, 5.0, 13, 9.0),
        }
    }

    /// The tone of its words and icons.
    fn ink(&self) -> Role {
        self.tone.unwrap_or(match self.variant {
            Variant::Default => Role::OnAccent,
            Variant::Destructive => Role::OnDanger,
            Variant::Secondary | Variant::Outline | Variant::Link | Variant::Field => Role::Text,
            Variant::Ghost => Role::Muted,
        })
    }

    /// The fills of the button at rest, under the pointer and pressed.
    fn looks(&self, radius: Radius) -> Looks {
        let (resting, hovered, pressed): (Option<Tone>, Option<Tone>, Option<Tone>) =
            match self.variant {
                Variant::Default => (
                    Some(Role::Accent.into()),
                    Some(Role::AccentHover.into()),
                    Some(Tone::from(Role::Accent).alpha(0.8)),
                ),
                Variant::Secondary => (
                    Some(Role::SurfaceHover.into()),
                    Some(Role::SurfaceActive.into()),
                    Some(Role::SurfaceSelected.into()),
                ),
                Variant::Destructive => (
                    Some(Role::Danger.into()),
                    Some(Tone::from(Role::Danger).alpha(0.9)),
                    Some(Tone::from(Role::Danger).alpha(0.8)),
                ),
                Variant::Outline | Variant::Ghost => (
                    None,
                    Some(Role::SurfaceHover.into()),
                    Some(Role::SurfaceActive.into()),
                ),
                Variant::Field => (
                    Some(Role::Surface.into()),
                    Some(Role::SurfaceHover.into()),
                    Some(Role::SurfaceActive.into()),
                ),
                Variant::Link => (None, None, None),
            };
        let resting = match (self.variant, self.selected) {
            (Variant::Field, _) => resting,
            (_, true) => Some(Role::SurfaceSelected.into()),
            _ => resting,
        };
        let mut looks = Looks::new().radius(radius);
        if let Some(tone) = resting {
            looks = looks.idle(tone);
        }
        if !self.disabled {
            if let Some(tone) = hovered {
                looks = looks.hover(tone);
            }
            if let Some(tone) = pressed {
                looks = looks.active(tone);
            }
        }
        looks
    }

    /// Emits the button.
    #[track_caller]
    pub fn build(self) {
        let ink = self.ink();
        let (height, side, font, square) = self.measure();
        let spread = self.reach != Reach::Content;
        let radius = if self.round { Radius::Full } else { Radius::Md };
        let mut modifier = match self.reach {
            Reach::Content => Modifier::new(),
            Reach::Fixed(width) => Modifier::new().width(width),
            Reach::Fill => Modifier::new().weight(1.0),
        };
        modifier = if self.label.is_none() {
            modifier.square(square)
        } else {
            modifier.h(height)
        };
        modifier = modifier.looks_of(self.looks(radius), self.id);
        modifier = match (self.variant, self.selected) {
            (Variant::Outline, true) => modifier.stroke(Role::Muted, radius),
            (Variant::Outline, false) | (Variant::Field, false) => {
                modifier.stroke(Role::Border, radius)
            }
            (Variant::Field, true) => modifier.stroke(Role::Focused, radius),
            _ => modifier,
        };
        modifier = modifier.focus_ring(self.id, 2.0, radius);
        modifier = if self.disabled {
            modifier.opacity(40.0).cursor(CursorIcon::NotAllowed)
        } else {
            modifier.cursor(CursorIcon::Pointer).target(self.id)
        };
        if self.label.is_some() {
            modifier = modifier.pad_x(side);
        }
        let modifier = self.modifier.chain(modifier);
        let arrangement = if spread {
            Arrangement::SpaceBetween
        } else {
            Arrangement::SpacedByAligned(sp(2), Alignment::Center)
        };
        let mut kind = Type::new(font).ratio(1.4).tone(ink);
        if self.variant != Variant::Field {
            kind = kind.medium();
        }
        if self.variant == Variant::Link {
            kind = kind.underline();
        }
        if self.monospace {
            kind = kind.mono();
        }
        let (icon_name, trailing, spinning, label_text) =
            (self.icon, self.trailing, self.spinning, self.label);
        row_arranged(modifier, arrangement, Alignment::Center, move || {
            if let Some(name) = icon_name {
                glyph(name, ink, spinning && trailing.is_none());
            }
            if let Some(words) = label_text {
                if spread {
                    label_with(Modifier::new().weight(1.0), words, kind.truncate());
                } else {
                    label(words, kind);
                }
            }
            if let Some(name) = trailing {
                glyph(name, ink, spinning);
            }
        });
    }
}

/// Emits `name` four steps across in `tone`, turning while `spinning`.
fn glyph(name: IconName, tone: Role, spinning: bool) {
    match (spinning, name) {
        (true, IconName::Loading) => spinner(4.0, tone),
        (true, name) => spinning_icon(name, 4.0, tone),
        (false, name) => icon(name, 4.0, tone),
    }
}

/// How a check box looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CheckVariant {
    /// A small square with a tick.
    #[default]
    Box,
    /// A track with a knob that sits at the end while it is on.
    Switch,
}

/// A check box being described.
#[derive(Clone, Copy, Debug)]
pub struct Checkbox {
    /// Whether it is ticked.
    checked: bool,
    /// Whether it is dashed, standing for some of what it covers being ticked.
    mixed: bool,
    /// What it answers to.
    id: FocusId,
    /// How it looks.
    variant: CheckVariant,
}

impl Checkbox {
    /// A check box answering to `id`, ticked while `checked`.
    pub fn new(checked: bool, id: FocusId) -> Self {
        Self {
            checked,
            mixed: false,
            id,
            variant: CheckVariant::default(),
        }
    }

    /// Dashed instead of ticked while `mixed` and not `checked`.
    pub fn mixed(mut self, mixed: bool) -> Self {
        self.mixed = mixed;
        self
    }

    /// Looking like `variant`.
    pub fn variant(mut self, variant: CheckVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Emits the check box.
    #[track_caller]
    pub fn build(self) {
        match self.variant {
            CheckVariant::Box => self.build_box(),
            CheckVariant::Switch => self.build_switch(),
        }
    }

    /// Emits the square.
    #[track_caller]
    fn build_box(self) {
        let filled = self.checked || self.mixed;
        let mut modifier = Modifier::new().square(4);
        modifier = if filled {
            modifier
                .fill_rounded(Role::Accent, Radius::Sm)
                .stroke(Role::Accent, Radius::Sm)
        } else {
            modifier.stroke(Role::Subtle, Radius::Sm)
        };
        let modifier = modifier
            .cursor(CursorIcon::Pointer)
            .focus_ring(self.id, 2.0, Radius::Sm)
            .target(self.id);
        let (checked, mixed) = (self.checked, self.mixed);
        row_arranged(
            modifier,
            Arrangement::Center,
            Alignment::Center,
            move || {
                if checked {
                    icon(IconName::Check, 3.0, Role::OnAccent);
                } else if mixed {
                    icon(IconName::Minus, 3.0, Role::OnAccent);
                }
            },
        );
    }

    /// Emits the switch.
    #[track_caller]
    fn build_switch(self) {
        let on = self.checked;
        let modifier = Modifier::new()
            .w(9)
            .h(5)
            .fill_rounded(
                if on { Role::Text } else { Role::SurfaceHover },
                Radius::Full,
            )
            .cursor(CursorIcon::Pointer)
            .target(self.id)
            .pad_x(0.5);
        let arrangement = if on {
            Arrangement::End
        } else {
            Arrangement::Start
        };
        row_arranged(modifier, arrangement, Alignment::Center, move || {
            rustle::spacer(Modifier::new().square(4).fill_rounded(
                if on { Role::Background } else { Role::Muted },
                Radius::Full,
            ));
        });
    }
}

/// Emits the settings switch, on while `on`, answering to `id`: `settings_row::switch`.
#[track_caller]
pub fn switch(on: bool, id: Option<FocusId>) {
    let mut modifier = Modifier::new()
        .width(36.0)
        .h(5)
        .fill_rounded(if on { Role::Success } else { Role::Border }, Radius::Full);
    if let Some(id) = id {
        modifier = modifier.cursor(CursorIcon::Pointer).target(id);
    }
    let arrangement = if on {
        Arrangement::End
    } else {
        Arrangement::Start
    };
    row_arranged(modifier.pad(0.5), arrangement, Alignment::Center, || {
        rustle::spacer(
            Modifier::new()
                .square(4)
                .fill_rounded(Role::Text, Radius::Full),
        );
    });
}

/// Emits the choices `names` side by side, `picked` marked, in a strip with `modifier` put around it: `settings_row::choices`.
///
/// `modifier` goes outside the strip's own box: sizes such as `fill_max_width`. `id_of` gives the id each
/// choice answers to, or `None` to leave it out.
#[track_caller]
pub fn choices(
    modifier: Modifier,
    names: &[&str],
    picked: usize,
    id_of: impl Fn(usize) -> Option<FocusId>,
) {
    let modifier = modifier.h(8).stroke(Role::Border, Radius::Md).pad_x(0.5);
    row_arranged(
        modifier,
        Arrangement::SpacedBy(sp(0.5)),
        Alignment::Center,
        || {
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
                    .arrangement(Arrangement::Center)
                    .build_with(|| {
                        label(
                            *name,
                            Type::new(12).ratio(1.4).tone(if on {
                                Role::Text
                            } else {
                                Role::Muted
                            }),
                        )
                    });
            }
        },
    );
}

/// A tile being described: a row of a list that answers to a press and fills under the pointer and while selected.
#[derive(Clone, Debug)]
pub struct Tile {
    /// What it answers to.
    id: FocusId,
    /// How tall it is, in steps.
    height: f32,
    /// How far apart its children sit, in steps.
    gap: f32,
    /// How round its corners are.
    radius: Radius,
    /// Whether it is filled.
    selected: bool,
    /// How its children are spread along it, in place of its gap, when the caller chose.
    arrangement: Option<Arrangement>,
    /// The padding the caller gave in place of the tile's own, which has none.
    inner: Modifier,
    /// What the caller puts around its box.
    modifier: Modifier,
}

impl Tile {
    /// An empty row nine steps tall answering to `id`.
    pub fn new(id: FocusId) -> Self {
        Self {
            id,
            height: 9.0,
            gap: 3.0,
            radius: Radius::Md,
            selected: false,
            arrangement: None,
            inner: Modifier::new(),
            modifier: Modifier::new(),
        }
    }

    /// `modifier`, padding, in place of the tile's own padding, inside its fill.
    pub fn inner(mut self, modifier: Modifier) -> Self {
        self.inner = modifier;
        self
    }

    /// `height` steps tall.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Children `gap` steps apart.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Corners rounded by `radius`.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// Filled while `selected`.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Children spread by `arrangement` instead of the tile's gap apart.
    pub fn arrangement(mut self, arrangement: Arrangement) -> Self {
        self.arrangement = Some(arrangement);
        self
    }

    /// `modifier` put around the tile's box, outside its own: sizes, weight, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the tile holding `content`.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let looks = if self.selected {
            Looks::new().idle(Role::SurfaceSelected)
        } else {
            Looks::new().hover(Role::SurfaceHover)
        }
        .active(Role::SurfaceActive)
        .radius(self.radius);
        let modifier = self.modifier.chain(
            Modifier::new()
                .h(self.height)
                .looks_of(looks, self.id)
                .cursor(CursorIcon::Pointer)
                .target(self.id)
                .chain(self.inner),
        );
        let arrangement = self
            .arrangement
            .unwrap_or(Arrangement::SpacedBy(sp(self.gap)));
        row_arranged(modifier, arrangement, Alignment::Center, content);
    }
}

/// How a surface looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SurfaceVariant {
    /// Small corners and a short shadow, for a list of actions.
    #[default]
    Compact,
    /// Large corners and a tall, soft shadow, for a panel that floats.
    Floating,
    /// Large corners, a short shadow and a clip at its edge.
    Dialog,
}

/// A surface being described: the raised, bordered column menus and popovers are drawn on.
#[derive(Clone, Debug, Default)]
pub struct Surface {
    /// How it looks.
    variant: SurfaceVariant,
    /// The padding the caller gave in place of the surface's own, which has none.
    inner: Modifier,
    /// What the caller puts around its box.
    modifier: Modifier,
}

impl Surface {
    /// A compact surface.
    pub fn new() -> Self {
        Self::default()
    }

    /// Looking like `variant`.
    pub fn variant(mut self, variant: SurfaceVariant) -> Self {
        self.variant = variant;
        self
    }

    /// `modifier`, padding, in place of the surface's own padding, inside its fill.
    pub fn inner(mut self, modifier: Modifier) -> Self {
        self.inner = modifier;
        self
    }

    /// `modifier` put around the surface's box, outside its own: sizes, weight, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the surface holding `content` in a column.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let modifier = match self.variant {
            SurfaceVariant::Compact => Modifier::new().shadow_md(Radius::Md),
            SurfaceVariant::Floating => Modifier::new().shadow_floating(Radius::Xl),
            SurfaceVariant::Dialog => Modifier::new().shadow_md(Radius::Xl),
        };
        let radius = match self.variant {
            SurfaceVariant::Compact => Radius::Md,
            _ => Radius::Xl,
        };
        let mut modifier = modifier
            .fill_rounded(Role::Surface, radius)
            .stroke(Role::Border, radius);
        if self.variant == SurfaceVariant::Dialog {
            modifier = modifier.clip_to(radius);
        }
        column_arranged(
            self.modifier.chain(modifier.chain(self.inner)),
            Arrangement::Start,
            Alignment::Start,
            content,
        );
    }
}

/// A popover being described: a floating surface with a heading and rows of choices.
#[derive(Clone, Debug, Default)]
pub struct Popover {
    /// How wide it is, when fixed.
    width: Option<f32>,
}

impl Popover {
    /// A popover as wide as what it holds.
    pub fn new() -> Self {
        Self::default()
    }

    /// `pixels` wide.
    pub fn width(mut self, pixels: f32) -> Self {
        self.width = Some(pixels);
        self
    }

    /// Emits the popover answering to `id`, holding `content`.
    #[track_caller]
    pub fn build_with(self, id: FocusId, content: impl FnOnce()) {
        let mut modifier = Modifier::new();
        if let Some(width) = self.width {
            modifier = modifier.width(width);
        }
        Surface::new()
            .variant(SurfaceVariant::Floating)
            .modifier(modifier.target(id))
            .inner(Modifier::new().pad(1.5))
            .build_with(content);
    }
}

/// Emits a popover row: `name` over `description`, filled while `current` and ticked, answering to `id`.
#[track_caller]
pub fn popover_row(name: &str, description: &str, current: bool, id: FocusId) {
    let looks = if current {
        Looks::new().idle(Role::SurfaceSelected)
    } else {
        Looks::new().hover(Role::SurfaceHover)
    }
    .radius(Radius::Lg);
    let modifier = Modifier::new()
        .fill_max_width()
        .looks_of(looks, id)
        .cursor(CursorIcon::Pointer)
        .target(id)
        .pad_x(2.5)
        .pad_y(1.5);
    row_arranged(
        modifier,
        Arrangement::SpacedBy(sp(3)),
        Alignment::Center,
        || {
            column_arranged(
                Modifier::new().weight(1.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    label(name, Type::new(13).ratio(1.4).truncate());
                    if !description.is_empty() {
                        label(
                            description,
                            Type::new(12).ratio(1.4).tone(Role::Subtle).truncate(),
                        );
                    }
                },
            );
            if current {
                icon(IconName::Check, 4.0, Role::Text);
            }
        },
    );
}

/// Emits a popover heading: `title`, and a key hint at its end.
#[track_caller]
pub fn popover_heading(title: &str, hint: Option<(&str, &str)>) {
    row_arranged(
        Modifier::new().fill_max_width().h(8).pad_x(2.5),
        Arrangement::SpaceBetween,
        Alignment::Center,
        || {
            label(title, Type::new(12).ratio(1.4).tone(Role::Subtle));
            if let Some((key, does)) = hint {
                row_arranged(
                    Modifier::new(),
                    Arrangement::SpacedBy(sp(1.5)),
                    Alignment::Center,
                    || {
                        row_arranged(
                            Modifier::new()
                                .fill_rounded(Role::SurfaceHover, Radius::Md)
                                .pad_x(1.5),
                            Arrangement::Start,
                            Alignment::Start,
                            || label(key, Type::new(11).ratio(1.6).mono().tone(Role::Muted)),
                        );
                        label(does, Type::new(12).ratio(1.4).tone(Role::Subtle));
                    },
                );
            }
        },
    );
}

/// One entry of a menu.
#[derive(Clone, Debug)]
enum Entry {
    /// An action.
    Item {
        /// Its words.
        label: String,
        /// Its shortcut.
        shortcut: Option<String>,
        /// What it answers to.
        id: FocusId,
        /// Whether it is switched off.
        disabled: bool,
    },
    /// A line between groups.
    Separator,
}

/// A menu being described: a floating list of actions opened at a point.
#[derive(Clone, Debug)]
pub struct Menu {
    /// Where its top left is in the window.
    at: (f32, f32),
    /// What it lists.
    entries: Vec<Entry>,
    /// How wide it is, in steps.
    width: f32,
    /// How tall each row is, in steps.
    row: f32,
}

impl Menu {
    /// An empty menu with its top left at `x`, `y` in the window.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            at: (x, y),
            entries: Vec::new(),
            width: 52.0,
            row: 8.0,
        }
    }

    /// Adds an action.
    pub fn item(
        mut self,
        label: &str,
        shortcut: Option<&str>,
        id: FocusId,
        disabled: bool,
    ) -> Self {
        self.entries.push(Entry::Item {
            label: label.to_owned(),
            shortcut: shortcut.map(str::to_owned),
            id,
            disabled,
        });
        self
    }

    /// Adds a line between groups.
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }

    /// Emits the menu over everything, kept inside the window.
    #[track_caller]
    pub fn build(self) {
        let (x, y) = self.at;
        let (width, height) = (self.width, self.row);
        let entries = self.entries;
        at_window(x, y, Modifier::new(), move || {
            Surface::new()
                .modifier(Modifier::new().w(width))
                .inner(Modifier::new().pad(1))
                .build_with(move || {
                    for entry in entries {
                        menu_row(entry, height);
                    }
                });
        });
    }
}

/// Emits one entry of a menu, `height` steps tall.
#[track_caller]
fn menu_row(entry: Entry, height: f32) {
    match entry {
        Entry::Separator => {
            let color = pm_components::kit::role(Role::Border);
            rustle::spacer(
                Modifier::new()
                    .fill_max_width()
                    .height(sp(0.25) + 2.0 * sp(1))
                    .draw_behind(move |canvas| {
                        let (width, height) = (canvas.width(), canvas.height());
                        canvas.rect(-sp(1), height / 2.0 - 0.5, width + 2.0 * sp(1), 1.0, color);
                    }),
            )
        }
        Entry::Item {
            label: words,
            shortcut,
            id,
            disabled,
        } => {
            let mut modifier = Modifier::new().fill_max_width().h(height);
            modifier = if disabled {
                modifier.opacity(40.0).cursor(CursorIcon::NotAllowed)
            } else {
                modifier
                    .looks(
                        Looks::new()
                            .hover(Role::SurfaceHover)
                            .active(Role::SurfaceActive)
                            .radius(Radius::Sm),
                    )
                    .cursor(CursorIcon::Pointer)
                    .target(id)
            };
            row_arranged(
                modifier.pad_x(2),
                Arrangement::SpacedBy(sp(4)),
                Alignment::Center,
                move || {
                    label_with(Modifier::new().weight(1.0), words, Type::new(13).ratio(1.4));
                    if let Some(shortcut) = shortcut {
                        label(shortcut, Type::new(12).ratio(1.4).tone(Role::Subtle));
                    }
                },
            );
        }
    }
}

/// How an input looks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InputVariant {
    /// A filled, bordered box with code type.
    #[default]
    Default,
    /// A filled, bordered box with interface type.
    Bordered,
    /// No box.
    Ghost,
    /// Wrapped lines.
    Multiline,
}

/// An input being described: a box holding editable text.
#[derive(Clone, Debug)]
pub struct Input<'a> {
    /// The model.
    field: &'a Field,
    /// What it shows while empty.
    placeholder: &'a str,
    /// What it answers to.
    id: FocusId,
    /// How it looks.
    variant: InputVariant,
    /// How big it is.
    size: Size,
    /// The icon before the text.
    leading: Option<IconName>,
    /// Whether it is switched off.
    disabled: bool,
    /// Whether its content is wrong.
    invalid: bool,
    /// How many lines tall it is at least.
    rows: u32,
    /// The width its lines wrap at.
    width: f32,
    /// What the caller adds to its outermost box.
    modifier: Modifier,
}

impl<'a> Input<'a> {
    /// An input for `field` answering to `id`.
    pub fn new(field: &'a Field, id: FocusId) -> Self {
        Self {
            field,
            placeholder: "",
            id,
            variant: InputVariant::Default,
            size: Size::Md,
            leading: None,
            disabled: false,
            invalid: false,
            rows: 1,
            width: 400.0,
            modifier: Modifier::new(),
        }
    }

    /// `icon` before the text.
    pub fn leading_icon(mut self, icon: IconName) -> Self {
        self.leading = Some(icon);
        self
    }

    /// At least `rows` lines tall.
    pub fn rows(mut self, rows: u32) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// Lines wrapped for a box `width` pixels across.
    pub fn wrap(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Shows `placeholder` while empty.
    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Looking like `variant`.
    pub fn variant(mut self, variant: InputVariant) -> Self {
        self.variant = variant;
        self
    }

    /// `size` big.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// Switched off while `disabled`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `modifier` put around the input's box, outside its own: sizes, weight, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Marked wrong while `invalid`.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Emits the input.
    #[track_caller]
    pub fn build(self) {
        if self.variant == InputVariant::Multiline {
            return self.build_multiline();
        }
        let (height, font) = match (self.variant, self.size) {
            (InputVariant::Bordered, Size::Xs) => (7.0, 12),
            (InputVariant::Bordered, Size::Sm) => (8.0, 12),
            (InputVariant::Bordered, Size::Md) => (9.0, 12),
            (InputVariant::Bordered, Size::Lg) => (10.0, 12),
            (_, Size::Xs) => (7.0, 12),
            (_, Size::Sm) => (8.0, 12),
            (_, Size::Md) => (10.0, 13),
            (_, Size::Lg) => (12.0, 14),
        };
        let id = self.id;
        let invalid = self.invalid;
        let boxed = matches!(self.variant, InputVariant::Default | InputVariant::Bordered);
        let mut modifier = Modifier::new().h(height);
        if boxed {
            modifier = modifier.fill_rounded(Role::Surface, Radius::Lg);
        }
        let border = theme_colors();
        modifier = modifier.focus_of(id).styled_by(move |canvas, interaction| {
            let tone = match (invalid, interaction.focused) {
                (true, _) => border.2,
                (false, true) => border.1,
                (false, false) => border.0,
            };
            if boxed || invalid {
                let (w, h) = (canvas.width(), canvas.height());
                canvas.stroke_rect(0.0, 0.0, w, h, Radius::Lg.px(), 1.0, tone);
            }
        });
        modifier = modifier.focus_ring(id, 2.0, Radius::Lg);
        if self.disabled {
            modifier = modifier.opacity(40.0).cursor(CursorIcon::NotAllowed);
        }
        let mut kind = Type::new(font).ratio(1.4);
        kind = if self.variant == InputVariant::Default {
            kind.mono()
        } else {
            kind.ratio(1.57)
        };
        let (field, placeholder, leading) = (self.field, self.placeholder, self.leading);
        let modifier = self
            .modifier
            .chain(Modifier::new().fill_max_width().chain(modifier));
        row_arranged(
            modifier.pad_x(3),
            Arrangement::SpacedBy(sp(2)),
            Alignment::Center,
            move || {
                if let Some(name) = leading {
                    icon(name, 4.0, Role::Muted);
                }
                FieldText::new(field, id, kind)
                    .placeholder(placeholder, Role::Subtle)
                    .build(Modifier::new().weight(1.0));
            },
        );
    }

    /// Emits the wrapped lines of a multiline input.
    #[track_caller]
    fn build_multiline(self) {
        let kind = Type::new(14).ratio(1.57);
        let inner = self.width - 2.0 * sp(4);
        let minimum = self.rows as f32 * 22.0 + 18.0;
        let modifier = Modifier::new()
            .fill_max_width()
            .height_in(minimum, f32::INFINITY)
            .cursor(CursorIcon::Text)
            .pad_x(4)
            .pad_side(Side::Top, 3.5)
            .pad_side(Side::Bottom, 1);
        let (field, id, placeholder) = (self.field, self.id, self.placeholder);
        column_arranged(modifier, Arrangement::Start, Alignment::Start, move || {
            FieldText::new(field, id, kind)
                .placeholder(placeholder, Role::Subtle)
                .wrap(inner, Some(8))
                .build(Modifier::new().fill_max_width());
        });
    }
}

/// The border colours of an input at rest, focused and invalid.
fn theme_colors() -> (rustle::Color, rustle::Color, rustle::Color) {
    (
        pm_components::kit::role(Role::Border),
        pm_components::kit::role(Role::Focused),
        pm_components::kit::role(Role::Danger),
    )
}

/// Emits one row of a settings page: `title` over `about` on the left and `control` on the right: `SettingRow::new(..).control(..).build(room)`.
#[track_caller]
pub fn setting_row(title: &str, about: &str, control: impl FnOnce()) {
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .stroke_side(Side::Bottom, 1.0, Role::Border)
            .pad_y(3),
        Arrangement::SpacedBy(sp(2)),
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(sp(6)),
                Alignment::Center,
                || {
                    column_arranged(
                        Modifier::new().weight(1.0),
                        Arrangement::SpacedBy(sp(0.5)),
                        Alignment::Start,
                        || {
                            label(title, Type::new(13).ratio(1.4));
                            pm_components::kit::paragraph(
                                Modifier::new().width_in(0.0, 360.0),
                                about,
                                Type::new(12).ratio(1.4).tone(Role::Muted),
                            );
                        },
                    );
                    row_arranged(
                        Modifier::new(),
                        Arrangement::SpacedBy(sp(2)),
                        Alignment::Center,
                        control,
                    );
                },
            );
        },
    );
}
