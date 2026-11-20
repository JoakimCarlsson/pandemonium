//! The tile: a row of a list that holds its children side by side, answers to a press and fills under the pointer and while it is selected.

use crate::events::Interactive;
use crate::events::{Event, OnClick, send};
use crate::kit;
use crate::target::Target;
use crate::theme;
use crate::theme::Indicated;
use crate::theme::sp;
use crate::theme::{Radius, Role};
use rustle::Modifiers;
use rustle::{Alignment, Arrangement, row_arranged};
use rustle::{CursorIcon, Modifier};
use std::rc::Rc;

/// A tile being described: what it answers to, how tall it is and whether it is selected.
///
/// Build it with [`Tile::new`], change it with the methods that follow, and
/// emit it with [`Tile::build_with`]; the caller gives what
/// it holds and the room around it.
#[derive(Clone, Debug)]
pub struct Tile {
    /// The name it answers to.
    id: Target,
    /// How tall it is, in steps of four logical pixels.
    height: f32,
    /// How far apart its children sit, in steps of four logical pixels.
    gap: f32,
    /// How round its corners are.
    radius: Radius,
    /// Whether it is filled.
    selected: bool,
    /// How its children are spread along it, in place of `gap` steps between them, when the caller chose it.
    arrangement: Option<Arrangement>,
    /// Where its children sit across it.
    alignment: Alignment,
    /// The padding the caller gave in place of the tile's own, which has none.
    inner: Modifier,
    /// What the caller puts around its box.
    modifier: Modifier,
    /// What clicking it does, told the modifier keys held, when the caller said; the
    /// workspace's reducer takes the press otherwise.
    on_click: Option<Picked>,
    /// What a secondary press on it does, when it has a menu of its own.
    on_context: Option<OnClick>,
}

/// What clicking a tile does, told the modifier keys held, as its builder keeps it.
#[derive(Clone)]
struct Picked(Rc<dyn Fn(Modifiers)>);

impl std::fmt::Debug for Picked {
    /// Writes that there is a lambda, which has nothing else to show.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Picked")
    }
}

impl Tile {
    /// An empty row 36 logical pixels tall answering to `id`.
    pub fn new(id: Target) -> Self {
        Self {
            id,
            height: 9.0,
            gap: 3.0,
            radius: Radius::Md,
            selected: false,
            arrangement: None,
            alignment: Alignment::Center,
            inner: Modifier::new(),
            modifier: Modifier::new(),
            on_click: None,
            on_context: None,
        }
    }

    /// The tile running `on_context` when the secondary button goes down on it: what opens
    /// its menu, in place of the menu of the conversation's text.
    pub fn on_context(mut self, on_context: impl Fn() + 'static) -> Self {
        self.on_context = Some(OnClick::new(on_context));
        self
    }

    /// The modifier that makes the tile a control running its lambdas.
    #[track_caller]
    fn reacting(&self) -> Modifier {
        match self.on_context.clone() {
            Some(on_context) => {
                Modifier::new().control_menu(self.id, self.clicked(), on_context.lambda())
            }
            None => Modifier::new().control_with(self.id, self.clicked()),
        }
    }

    /// What clicking the tile does: the caller's lambda, or the workspace's reducer taking
    /// the press.
    fn clicked(&self) -> impl Fn(Modifiers) + 'static {
        let given = self.on_click.clone();
        let reduced = send(Event::pressing(self.id));
        move |held| match &given {
            Some(Picked(on_pick)) => on_pick(held),
            None => reduced(),
        }
    }

    /// The tile running `on_pick` with the modifier keys held when clicked, or activated
    /// from the keyboard, for a row the reader picks several of with Shift or Control.
    pub fn on_pick(mut self, on_pick: impl Fn(Modifiers) + 'static) -> Self {
        self.on_click = Some(Picked(Rc::new(on_pick)));
        self
    }

    /// The tile `height` steps tall.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The tile with its children `gap` steps apart.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// The tile with corners rounded by `radius`.
    pub fn radius(mut self, radius: Radius) -> Self {
        self.radius = radius;
        self
    }

    /// The tile filled while `selected` is true.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The tile with its children spread along it by `arrangement` in place of its gap.
    pub fn arrangement(mut self, arrangement: Arrangement) -> Self {
        self.arrangement = Some(arrangement);
        self
    }

    /// The tile with its children placed across it by `alignment` instead of centred.
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// The tile with `modifier` as its padding, in place of its own (none), inside its fill.
    pub fn inner(mut self, modifier: Modifier) -> Self {
        self.inner = modifier;
        self
    }

    /// The tile with `modifier` put around its box, outside its own: sizes, growth, margins, placement.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = self.modifier.chain(modifier);
        self
    }

    /// Emits the tile holding `content`: it fills under the pointer unless it is selected, and darker while pressed.
    #[track_caller]
    pub fn build_with(self, content: impl FnOnce()) {
        let looks = kit::selectable(self.selected)
            .active(Role::SurfaceActive)
            .radius(self.radius);
        let reacting = self.reacting();
        let modifier = self.modifier.chain(
            Modifier::new()
                .height(theme::sp(self.height))
                .indicated(looks, theme::Focused::Held(crate::focus::is_held(self.id)))
                .cursor(CursorIcon::Pointer)
                .chain(reacting)
                .chain(self.inner),
        );
        row_arranged(
            modifier,
            self.arrangement
                .unwrap_or(Arrangement::SpacedBy(sp(self.gap))),
            self.alignment,
            content,
        );
    }
}
