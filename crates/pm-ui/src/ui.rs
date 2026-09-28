//! The window's UI state: input, focus and the frame the tree is drawn into.

use pm_gfx::{DrawList, Point, Rect, Size, TextSystem};

use crate::element::{Element, Input, LayoutContext, PaintContext, Region, RegionAction};
use crate::resize::{ResizeEvent, ResizePhase};
use crate::theme::Theme;

/// The cursor shape requested by the element under the pointer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PointerCursor {
    /// The platform's ordinary pointer.
    #[default]
    Default,
    /// A hand indicating that the region can be activated.
    Pointer,
    /// Horizontal resizing across a vertical divider.
    ResizeHorizontal,
    /// Vertical resizing across a horizontal divider.
    ResizeVertical,
    /// An I-beam over text that can be selected.
    Text,
    /// An open hand over something that can be taken hold of and moved.
    Grab,
}

/// Everything that survives between frames: the theme, the pointer and focus.
///
/// The element tree does not survive: the caller rebuilds it every frame and
/// hands it to [`Ui::draw`]. What the tree leaves behind is a list of regions,
/// in paint order, which is how a click or a keypress after the frame turns
/// into one of the caller's messages.
pub struct Ui<M> {
    /// The tokens frames are drawn from.
    theme: Theme,
    /// What the pointer is doing.
    input: Input,
    /// The region holding keyboard focus, as a tab index.
    focus: Option<usize>,
    /// The regions painted by the last frame, in paint order.
    regions: Vec<Region<M>>,
    /// The drag target captured by the current pointer press.
    drag: Option<usize>,
}

impl<M> Ui<M> {
    /// Creates UI state drawing in `theme`, with nothing focused.
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            input: Input::default(),
            focus: None,
            regions: Vec::new(),
            drag: None,
        }
    }

    /// The tokens frames are drawn from.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Draws later frames in `theme`.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Records the pointer at `pointer`.
    pub fn pointer_moved(&mut self, pointer: Point) -> Option<M> {
        self.input.pointer = Some(pointer);
        let start = self.input.pressed_at?;
        self.drag_message(start, pointer, ResizePhase::Moved)
    }

    /// Records the pointer having left the window.
    pub fn pointer_left(&mut self) {
        self.input.pointer = None;
        self.input.pressed_at = None;
        self.drag = None;
    }

    /// Records a press and leaves keyboard focus to keyboard navigation.
    pub fn pointer_pressed(&mut self) -> Option<M> {
        self.input.pressed_at = self.input.pointer;
        let pointer = self.input.pointer?;
        let index = self.region_at(pointer);
        self.focus = None;
        self.drag =
            index.filter(|index| matches!(self.regions[*index].action, RegionAction::Drag { .. }));
        self.drag_message(pointer, pointer, ResizePhase::Started)
    }

    /// Records a release, returning the message of the region it completed on.
    pub fn pointer_released(&mut self) -> Option<M>
    where
        M: Clone,
    {
        let pressed_at = self.input.pressed_at.take()?;
        let pointer = self.input.pointer?;
        if self.drag.is_some() {
            let message = self.drag_message(pressed_at, pointer, ResizePhase::Ended);
            self.drag = None;
            return message;
        }
        let index = self.region_at(pointer)?;
        let region = &self.regions[index];
        match &region.action {
            RegionAction::Click(message) if region.bounds.contains(pressed_at) => {
                Some(message.clone())
            }
            RegionAction::Click(_) | RegionAction::Drag { .. } | RegionAction::Inert => None,
        }
    }

    /// The message of the region under a press of the secondary button.
    ///
    /// A menu opens under the pointer the moment the button goes down, the
    /// way every other editor opens one, so there is no release to wait for.
    pub fn secondary_pressed(&self) -> Option<M>
    where
        M: Clone,
    {
        let pointer = self.input.pointer?;
        let index = self.region_at(pointer)?;
        self.regions[index].secondary.clone()
    }

    /// Moves focus to the next region in tab order, wrapping around.
    pub fn focus_next(&mut self) {
        self.focus = self.step_focus(1);
    }

    /// Moves focus to the previous region in tab order, wrapping around.
    pub fn focus_previous(&mut self) {
        self.focus = self.step_focus(-1);
    }

    /// Gives up focus entirely.
    pub fn clear_focus(&mut self) {
        self.focus = None;
    }

    /// The message of the focused region, for a key that activates it.
    pub fn activate_focused(&self) -> Option<M>
    where
        M: Clone,
    {
        let index = self.focus?;
        match &self.regions.get(index)?.action {
            RegionAction::Click(message) => Some(message.clone()),
            RegionAction::Drag { .. } | RegionAction::Inert => None,
        }
    }

    /// Returns the cursor requested by the captured or hovered region.
    pub fn pointer_cursor(&self) -> PointerCursor {
        let index = self
            .drag
            .or_else(|| self.input.pointer.and_then(|point| self.region_at(point)));
        match index.and_then(|index| self.regions.get(index)) {
            Some(Region {
                action: RegionAction::Click(_),
                ..
            }) => PointerCursor::Pointer,
            Some(Region {
                action: RegionAction::Inert,
                ..
            }) => PointerCursor::Default,
            Some(Region {
                action: RegionAction::Drag { cursor, .. },
                ..
            }) => *cursor,
            _ => PointerCursor::Default,
        }
    }

    /// Whether the pointer is over an interactive region from the last frame.
    pub fn pointer_over_region(&self) -> bool {
        self.input
            .pointer
            .is_some_and(|pointer| self.region_at(pointer).is_some())
    }

    /// Measures `root` against `offer`, paints it at `origin` and reports its size.
    ///
    /// The size comes back because it is what the caller needs to clamp a
    /// scroll offset: a page taller than the window is drawn by passing an
    /// `origin` above it.
    pub fn draw(
        &mut self,
        text: &mut TextSystem,
        list: &mut DrawList,
        offer: Size,
        origin: Point,
        mut root: impl Element<M>,
    ) -> Size {
        self.regions.clear();

        let mut layout = LayoutContext::new(&self.theme, text);
        let size = root.measure(offer, &mut layout);

        let mut cx = PaintContext::new(layout, list, self.input, self.focus, &mut self.regions);
        root.paint(Rect::new(origin, size), &mut cx);
        cx.paint_tooltip();

        if self.focus.is_some_and(|index| index >= self.regions.len()) {
            self.focus = None;
        }

        size
    }

    /// The topmost region containing `point`.
    fn region_at(&self, point: Point) -> Option<usize> {
        self.regions
            .iter()
            .rposition(|region| region.bounds.contains(point))
    }

    /// Builds the message for the captured drag from `start` to `pointer`.
    ///
    /// Where the gesture began is passed in rather than read back out of the
    /// input, because the release that ends a drag has already let go of the
    /// press it began with by the time the last event is built.
    fn drag_message(&self, start: Point, pointer: Point, phase: ResizePhase) -> Option<M> {
        let region = self.regions.get(self.drag?)?;
        match &region.action {
            RegionAction::Drag { handler, .. } => Some(handler(ResizeEvent {
                phase,
                start,
                current: pointer,
            })),
            RegionAction::Click(_) | RegionAction::Inert => None,
        }
    }

    /// Focus moved by `step` places in tab order, wrapping around.
    fn step_focus(&self, step: isize) -> Option<usize> {
        let count = self.regions.len();
        if count == 0 {
            return None;
        }

        let next = match self.focus {
            Some(index) => (index as isize + step).rem_euclid(count as isize),
            None if step >= 0 => 0,
            None => count as isize - 1,
        };
        Some(next as usize)
    }
}
