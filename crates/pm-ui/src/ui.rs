//! The window's UI state: input, focus and the frame the tree is drawn into.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::SelectionDrag;
use crate::selection::{SelectionFrame, SelectionRegistry, SelectionSurface};

use pm_gfx::{DrawList, Point, Rect, Size, TextSystem};

use crate::element::{Element, Input, LayoutContext, PaintContext, Region, RegionAction};
use crate::resize::{ResizeEvent, ResizePhase};
use crate::theme::Theme;

/// How long the pointer rests on an element before its tooltip shows.
const TOOLTIP_DELAY: Duration = Duration::from_millis(150);

/// A tooltip the pointer is resting on, and since when.
struct HeldTooltip {
    /// Where the element asking for it was painted.
    bounds: Rect,
    /// What it says.
    text: String,
    /// When the pointer came to rest on it.
    since: Instant,
    /// Whether a frame has painted it yet.
    shown: bool,
}

impl HeldTooltip {
    /// Keeps the rest time of `held` while `asked` is the same tooltip, or starts it anew.
    fn hold(held: Option<Self>, asked: Option<(Rect, String)>, now: Instant) -> Option<Self> {
        asked.map(|(bounds, text)| {
            let (since, shown) = held
                .filter(|held| held.bounds == bounds && held.text == text)
                .map_or((now, false), |held| (held.since, held.shown));
            Self {
                bounds,
                text,
                since,
                shown,
            }
        })
    }
}

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

/// The handler and cursor captured for the lifetime of one pointer press.
struct CapturedDrag<M> {
    /// The original target's handler, independent of subsequent painted regions.
    handler: Arc<dyn Fn(ResizeEvent) -> M>,
    /// The cursor requested by the original target.
    cursor: PointerCursor,
    /// The latest position, also used for a release outside the window.
    current: Point,
    /// The ordinary click deferred for text nested in a clickable row.
    click: Option<M>,
    /// Whether the gesture has moved past the click threshold.
    moved: bool,
}

/// A text gesture and the ordinary click deferred until its release.
struct CapturedSelection<M> {
    /// The reading surface that owns the content boundaries.
    surface: Rc<RefCell<SelectionSurface>>,
    /// The anchor, pointer and edge scroll deadline.
    gesture: SelectionDrag,
    /// The ordinary message sent by an unmoved press.
    click: Option<M>,
    /// Whether the pointer has travelled far enough to select text.
    moved: bool,
    /// The window position where the press began.
    start: Point,
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
    drag: Option<CapturedDrag<M>>,
    /// Reading surfaces retained across frames.
    selections: Rc<RefCell<SelectionRegistry>>,
    /// Placed text that can start a selection in the latest frame.
    selection_frames: Vec<SelectionFrame>,
    /// The text gesture captured by the held primary button.
    selection_drag: Option<CapturedSelection<M>>,
    /// Where the region under the held primary button was painted.
    pressed_bounds: Option<Rect>,
    /// Where the region whose click the last release completed was painted.
    clicked_bounds: Option<Rect>,
    /// The tooltip under the pointer, shown once it has been held long enough.
    tooltip: Option<HeldTooltip>,
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
            selections: Rc::new(RefCell::new(SelectionRegistry::default())),
            selection_frames: Vec::new(),
            selection_drag: None,
            pressed_bounds: None,
            clicked_bounds: None,
            tooltip: None,
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
        if let Some(drag) = self.selection_drag.as_mut() {
            drag.gesture.pointer = pointer;
            drag.moved |=
                (pointer.x - drag.start.x).abs() + (pointer.y - drag.start.y).abs() >= 3.0;
            self.refresh_text_selection();
            return None;
        }
        if let Some(drag) = self.drag.as_mut() {
            drag.current = pointer;
            drag.moved |= self.input.pressed_at.is_some_and(|start| {
                (pointer.x - start.x).abs() + (pointer.y - start.y).abs() >= 3.0
            });
        }
        let start = self.input.pressed_at?;
        self.drag_message(start, pointer, ResizePhase::Moved)
    }

    /// Records the pointer leaving the window while preserving a held drag.
    pub fn pointer_left(&mut self) {
        self.input.pointer = None;
    }

    /// Records a press and leaves keyboard focus to keyboard navigation.
    pub fn pointer_pressed(&mut self) -> Option<M>
    where
        M: Clone,
    {
        self.input.pressed_at = self.input.pointer;
        self.pressed_bounds = None;
        self.clicked_bounds = None;
        let pointer = self.input.pointer?;
        let index = self.region_at(pointer);
        self.pressed_bounds = index.map(|index| self.regions[index].bounds);
        self.focus = None;
        if let Some(surface) = self.selection_surface_at(pointer) {
            let anchor = surface.borrow().spot_at(pointer)?;
            let click = index.and_then(|index| match &self.regions[index].action {
                RegionAction::Click(message) => Some(message.clone()),
                _ => None,
            });
            self.selections.borrow_mut().clear();
            surface.borrow_mut().focused = true;
            self.selection_drag = Some(CapturedSelection {
                surface,
                gesture: SelectionDrag::new(anchor, pointer),
                click,
                moved: false,
                start: pointer,
            });
            return None;
        }
        let text_parent = index.and_then(|index| {
            matches!(self.regions[index].action, RegionAction::Click(_))
                .then(|| {
                    self.regions[..index].iter().rposition(|region| {
                        region.bounds.contains(pointer)
                            && matches!(
                                region.action,
                                RegionAction::Drag {
                                    cursor: PointerCursor::Text,
                                    ..
                                }
                            )
                    })
                })
                .flatten()
        });
        let click = text_parent
            .and(index)
            .and_then(|index| match &self.regions[index].action {
                RegionAction::Click(message) => Some(message.clone()),
                _ => None,
            });
        self.drag = text_parent
            .or(index)
            .and_then(|index| match &self.regions[index].action {
                RegionAction::Drag { handler, cursor } => Some(CapturedDrag {
                    handler: handler.clone(),
                    cursor: *cursor,
                    current: pointer,
                    click,
                    moved: false,
                }),
                RegionAction::Click(_) | RegionAction::Inert => None,
            });
        self.drag_message(pointer, pointer, ResizePhase::Started)
    }

    /// Records a release, returning the message of the region it completed on.
    pub fn pointer_released(&mut self) -> Option<M>
    where
        M: Clone,
    {
        let released = self.release();
        self.clicked_bounds = released.as_ref().and(self.pressed_bounds.take());
        released
    }

    /// Where the control whose click the last release sent was painted, so
    /// that what it opens can be placed against it rather than the pointer.
    pub fn clicked_bounds(&self) -> Option<Rect> {
        self.clicked_bounds
    }

    /// Ends the primary press, returning the message of the region it
    /// completed on.
    fn release(&mut self) -> Option<M>
    where
        M: Clone,
    {
        let pressed_at = self.input.pressed_at.take()?;
        if self.selection_drag.is_some() {
            self.refresh_text_selection();
            let drag = self.selection_drag.take()?;
            return (!drag.moved && self.input.pointer.is_some())
                .then_some(drag.click)
                .flatten();
        }
        if self
            .drag
            .as_ref()
            .is_some_and(|drag| !drag.moved && drag.click.is_some())
        {
            return self
                .drag
                .take()
                .and_then(|drag| self.input.pointer.and(drag.click));
        }
        if self.drag.is_some() {
            return self.end_drag(pressed_at);
        }
        let pointer = self.input.pointer?;
        let index = self.region_at(pointer)?;
        let region = &self.regions[index];
        match &region.action {
            RegionAction::Click(message) if region.bounds.contains(pressed_at) => {
                Some(message.clone())
            }
            RegionAction::Click(_) | RegionAction::Drag { .. } | RegionAction::Inert => None,
        }
    }

    /// Ends a captured gesture on focus loss without activating a click target.
    pub fn pointer_cancelled(&mut self) -> Option<M> {
        self.selection_drag = None;
        let start = self.input.pressed_at.take()?;
        self.end_drag(start)
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
        self.clear_text_selection();
        self.focus = self.step_focus(1);
    }

    /// Moves focus to the previous region in tab order, wrapping around.
    pub fn focus_previous(&mut self) {
        self.clear_text_selection();
        self.focus = self.step_focus(-1);
    }

    /// Moves focus among visible click targets matching the caller's message predicate.
    pub fn focus_matching(&mut self, matches: impl Fn(&M) -> bool, backwards: bool) {
        self.clear_text_selection();
        let count = self.regions.len();
        let step = if backwards { -1 } else { 1 };
        let base = self
            .focus
            .map_or(if backwards { count as isize } else { -1 }, |index| {
                index as isize
            });
        self.focus = (1..=count).find_map(|distance| {
            let index = (base + step * distance as isize).rem_euclid(count as isize) as usize;
            let region = &self.regions[index];
            match &region.action {
                RegionAction::Click(message)
                    if region.bounds.size.width > 0.0
                        && region.bounds.size.height > 0.0
                        && matches(message) =>
                {
                    Some(index)
                }
                _ => None,
            }
        });
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
        if self.selection_drag.is_some() {
            return PointerCursor::Text;
        }
        if let Some(drag) = &self.drag {
            return drag.cursor;
        }
        if self
            .input
            .pointer
            .is_some_and(|pointer| self.selection_surface_at(pointer).is_some())
        {
            return PointerCursor::Text;
        }
        let index = self.input.pointer.and_then(|point| self.region_at(point));
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
        self.selections.borrow_mut().begin_frame();

        let mut layout = LayoutContext::new(&self.theme, text);
        let size = root.measure(offer, &mut layout);

        let mut cx = PaintContext::new(layout, list, self.input, self.focus, &mut self.regions);
        cx.selections = self.selections.clone();
        root.paint(Rect::new(origin, size), &mut cx);
        let now = Instant::now();
        self.tooltip = HeldTooltip::hold(self.tooltip.take(), cx.take_tooltip(), now);
        if let Some(held) = self
            .tooltip
            .as_mut()
            .filter(|held| held.since + TOOLTIP_DELAY <= now)
        {
            held.shown = true;
            cx.paint_tooltip(held.bounds, &held.text);
        }
        self.selection_frames = std::mem::take(&mut cx.selection_frames);

        if self.focus.is_some_and(|index| index >= self.regions.len()) {
            self.focus = None;
        }

        size
    }

    /// Returns text picked out of the focused reading surface.
    pub fn selected_text(&self) -> Option<String> {
        self.selections.borrow().focused()?.borrow().text()
    }

    /// Selects all content of the focused reading surface.
    pub fn select_all_text(&mut self) {
        if let Some(surface) = self.selections.borrow().focused() {
            surface.borrow_mut().select_all();
        }
    }

    /// Clears reading selections when focus moves to another pane or editor.
    pub fn clear_text_selection(&mut self) {
        self.selections.borrow_mut().clear();
        self.selection_drag = None;
    }

    /// Whether a held primary button belongs to a text selection.
    pub fn selecting_text(&self) -> bool {
        self.selection_drag.is_some()
    }

    /// Whether the secondary button is over selected text in its focused area.
    pub fn selected_text_under_pointer(&self) -> bool {
        let Some(point) = self.input.pointer else {
            return false;
        };
        let Some(surface) = self.selection_surface_at(point) else {
            return false;
        };
        let state = surface.borrow();
        state.focused
            && state.selection.range().is_some_and(|(first, last)| {
                state
                    .spot_at(point)
                    .is_some_and(|spot| spot >= first && spot <= last)
            })
    }

    /// Updates the head from newly painted placements after scrolling or resizing.
    pub fn refresh_text_selection(&mut self) -> bool {
        let Some(drag) = self.selection_drag.as_ref() else {
            return false;
        };
        if !drag.moved {
            return false;
        }
        let mut state = drag.surface.borrow_mut();
        if !state.visible {
            return false;
        }
        let Some(head) = state.spot_at(drag.gesture.head_point(state.bounds)) else {
            return false;
        };
        let before = state.selection.range();
        state.selection.select(drag.gesture.anchor, head);
        before != state.selection.range()
    }

    /// Returns when the tooltip under the pointer is due to show, until a frame shows it.
    pub fn next_tooltip(&self) -> Option<Instant> {
        self.tooltip
            .as_ref()
            .filter(|held| !held.shown)
            .map(|held| held.since + TOOLTIP_DELAY)
    }

    /// Returns the next wakeup for an edge scroll while the pointer is held.
    pub fn next_text_selection_scroll(&self) -> Option<Instant> {
        self.text_selection_scroll_step()?;
        Some(self.selection_drag.as_ref()?.gesture.next_scroll)
    }

    /// Scrolls the captured area toward the pointer at the shared selection interval.
    pub fn autoscroll_text_selection(&mut self) -> bool {
        let Some(step) = self.text_selection_scroll_step() else {
            return false;
        };
        let Some(drag) = self.selection_drag.as_mut() else {
            return false;
        };
        if !drag.gesture.scroll_due(Instant::now()) {
            return false;
        }
        let Some(scroll) = drag.surface.borrow().scroll.upgrade() else {
            return false;
        };
        let mut state = scroll.get();
        let before = state.offset();
        state.by(-step);
        scroll.set(state);
        state.offset() != before
    }

    /// Finds an unobscured text target without stealing a control's drag.
    fn selection_surface_at(&self, point: Point) -> Option<Rc<RefCell<SelectionSurface>>> {
        let region = self.region_at(point);
        self.selection_frames
            .iter()
            .rev()
            .find(|frame| {
                frame.bounds.contains(point)
                    && region.is_none_or(|index| {
                        index < frame.regions
                            && !matches!(self.regions[index].action, RegionAction::Drag { .. })
                    })
            })
            .map(|frame| frame.surface.clone())
    }

    /// Computes the edge scroll step from the captured surface's current extents.
    fn text_selection_scroll_step(&self) -> Option<f32> {
        let drag = self.selection_drag.as_ref()?;
        if !drag.moved || self.input.pointer.is_none() {
            return None;
        }
        let surface = drag.surface.borrow();
        if !surface.visible {
            return None;
        }
        let scroll = surface.scroll.upgrade()?.get();
        drag.gesture.scroll_step(
            surface.bounds,
            scroll.offset(),
            scroll.content_height() - scroll.viewport_height(),
        )
    }

    /// The topmost region containing `point`.
    fn region_at(&self, point: Point) -> Option<usize> {
        self.regions
            .iter()
            .rposition(|region| region.bounds.contains(point))
    }

    /// Builds a message from the captured handler without consulting painted regions.
    fn drag_message(&self, start: Point, pointer: Point, phase: ResizePhase) -> Option<M> {
        let drag = self.drag.as_ref()?;
        Some((drag.handler)(ResizeEvent {
            phase,
            start,
            current: pointer,
        }))
    }

    /// Releases the captured handler and sends its final pointer position.
    fn end_drag(&mut self, start: Point) -> Option<M> {
        let drag = self.drag.take()?;
        Some((drag.handler)(ResizeEvent {
            phase: ResizePhase::Ended,
            start,
            current: self.input.pointer.unwrap_or(drag.current),
        }))
    }

    /// Focus moved by `step` places in tab order, wrapping around.
    fn step_focus(&self, step: isize) -> Option<usize> {
        let count = self.regions.len();
        if count == 0 {
            return None;
        }

        let base = match self.focus {
            Some(index) => index as isize,
            None if step >= 0 => -1,
            None => count as isize,
        };
        (1..=count).find_map(|distance| {
            let next = (base + step * distance as isize).rem_euclid(count as isize) as usize;
            matches!(self.regions[next].action, RegionAction::Click(_)).then_some(next)
        })
    }
}
