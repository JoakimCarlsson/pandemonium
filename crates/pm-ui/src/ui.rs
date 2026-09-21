//! The window's UI state: input, focus and the frame the tree is drawn into.

use pm_gfx::{DrawList, Point, Rect, Size, TextSystem};

use crate::element::{Element, Input, LayoutContext, PaintContext, Region};
use crate::theme::Theme;

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
}

impl<M> Ui<M> {
    /// Creates UI state drawing in `theme`, with nothing focused.
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            input: Input::default(),
            focus: None,
            regions: Vec::new(),
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
    pub fn pointer_moved(&mut self, pointer: Point) {
        self.input.pointer = Some(pointer);
    }

    /// Records the pointer having left the window.
    pub fn pointer_left(&mut self) {
        self.input.pointer = None;
        self.input.pressed_at = None;
    }

    /// Records a press, which also moves focus to whatever is under it.
    pub fn pointer_pressed(&mut self) {
        self.input.pressed_at = self.input.pointer;
        if let Some(pointer) = self.input.pointer {
            self.focus = self.region_at(pointer);
        }
    }

    /// Records a release, returning the message of the region it completed on.
    pub fn pointer_released(&mut self) -> Option<M>
    where
        M: Clone,
    {
        let pressed_at = self.input.pressed_at.take()?;
        let pointer = self.input.pointer?;
        let index = self.region_at(pointer)?;
        let region = &self.regions[index];
        region
            .bounds
            .contains(pressed_at)
            .then(|| region.message.clone())
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
        self.regions.get(index).map(|region| region.message.clone())
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
