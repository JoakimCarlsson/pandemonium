//! Painting a laid-out node tree into a draw list.

use pm_input::{FocusId, PointerState};
use pm_renderer::{Affine, DrawList, Icon, Quad, Rect, Text};
use pm_style::color::Srgba;
use pm_style::layout::{Display, Overflow};
use pm_style::primitives::Side;
use pm_style::{Color, ComputedStyle, Context, Cursor, Role, State, StateSet, Transform};

use pm_style::layout::{Placed, solve};
use pm_text::{Run, Shaper};
use pm_theme::Theme;
use pm_typography::TextOverflow;

use crate::node::{Anchor, CaretShape, Mark, Node, Paint};
use crate::retain;
use crate::select::{DrawnPiece, TextPiece, TextSelection};
use crate::tooltip::{self, Showing};

/// How long the caret stays on, and then off, in milliseconds.
const CARET_BLINK_MS: f32 = 530.0;

/// The width of the caret in logical pixels.
const CARET_WIDTH: f32 = 1.5;

/// How far a shadow's edge fades, as a share of its blur radius, which is what makes it look as soft as a browser draws it.
const SHADOW_SOFTNESS: f32 = 1.3;

/// How long each piece of a wavy underline is, in logical pixels.
const WAVE_STEP: f32 = 2.0;

/// How wide the bar of a scrolling box is, in logical pixels.
const SCROLLBAR_WIDTH: f32 = 4.0;

/// How far the bar of a scrolling box stays from its edges, in logical pixels.
const SCROLLBAR_INSET: f32 = 3.0;

/// How short the bar of a scrolling box is at least, in logical pixels.
const SCROLLBAR_MINIMUM: f32 = 24.0;

/// How wide the bar of a scrolling box grows while the pointer is on it or holds it, in logical pixels.
const SCROLLBAR_WIDE: f32 = 7.0;

/// How wide the strip along the right edge of a scrolling box is where the pointer takes hold of its bar, in logical pixels.
pub const SCROLLBAR_GRIP: f32 = 14.0;

/// What the screen is being drawn for: the pointer, the focus and the clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Where the pointer is and which buttons are down.
    pub pointer: PointerState,
    /// The box holding keyboard focus, if any.
    pub focused: Option<FocusId>,
    /// Milliseconds since the application started, which animations run on.
    pub elapsed_ms: f32,
    /// The stretch of selectable text that is selected, if any.
    pub selection: Option<TextSelection>,
    /// The scrolling box whose bar the pointer holds, if any.
    pub held_bar: Option<FocusId>,
}

/// One box that answers to the user, where it ended up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    /// The name the box was given.
    pub id: FocusId,
    /// Where it is, in logical pixels.
    pub rect: Rect,
}

/// What a drawn frame reports back.
#[derive(Clone, Debug, Default)]
pub struct Output {
    /// Whether anything painted is animating, so another frame is wanted.
    pub animating: bool,
    /// The boxes that answer to the user, in tree order, which is the order
    /// Tab walks them in.
    pub regions: Vec<Region>,
    /// The pointer shape the box under the pointer asks for.
    pub cursor: Cursor,
    /// The text boxes that hold a caret, in tree order.
    pub fields: Vec<FieldBox>,
    /// The boxes that scroll from their end, in tree order.
    pub scrollers: Vec<Scroller>,
    /// The boxes made selectable, where they were seen, in tree order, those
    /// off screen included with no size.
    pub selectables: Vec<Region>,
    /// The lines of selectable text, in tree order, those off screen included.
    pub pieces: Vec<TextPiece>,
}

/// One box whose contents scroll from their end, where it ended up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroller {
    /// The name the box was given.
    pub id: FocusId,
    /// Where it is, in logical pixels.
    pub rect: Rect,
    /// How far back from the end its contents can be scrolled, in logical pixels.
    pub room: f32,
    /// The actual offset from the start, clamped to the available room.
    pub offset: f32,
}

/// One line of editable text, where it was drawn.
#[derive(Clone, Debug)]
pub struct FieldBox {
    /// The name of the box that owns the caret.
    pub id: FocusId,
    /// The byte offset of the field's text at which this line starts.
    pub start: usize,
    /// Where the line starts along x, in logical pixels.
    pub x: f32,
    /// Where the line is along y, in logical pixels.
    pub y: f32,
    /// How tall the line is, in logical pixels.
    pub height: f32,
    /// The line, shaped.
    pub run: Run,
    /// The rectangle the line is clipped to, if it is.
    pub view: Option<Rect>,
    /// How many bytes the line has.
    pub length: usize,
}

impl Output {
    /// The lines of the field `id` that were drawn, from the top.
    fn lines_of(&self, id: FocusId) -> Vec<&FieldBox> {
        let mut lines: Vec<&FieldBox> = self.fields.iter().filter(|field| field.id == id).collect();
        lines.sort_by(|a, b| a.y.total_cmp(&b.y));
        lines
    }

    /// The byte offset in the field `id` nearest to the point `x`, `y`
    /// logical pixels, if that field was drawn: on the line at `y`, or the
    /// nearest line to it.
    pub fn offset_in(&self, id: FocusId, x: f32, y: f32) -> Option<usize> {
        let lines = self.lines_of(id);
        let line = lines
            .iter()
            .find(|line| y < line.y + line.height)
            .or(lines.last())?;
        Some(line.start + line.run.byte_at(x - line.x, line.length))
    }

    /// The byte offset in the field `id` on the line `by` lines from the one
    /// the caret at byte `caret` is on, at the same distance from the left,
    /// if there is such a line.
    pub fn vertical(&self, id: FocusId, caret: usize, by: isize) -> Option<usize> {
        let lines = self.lines_of(id);
        let here = Self::line_at(&lines, caret)?;
        let target = lines.get(here.checked_add_signed(by)?)?;
        let x = lines[here].run.caret_x(caret - lines[here].start);
        Some(target.start + target.run.byte_at(x, target.length))
    }

    /// The byte offset at the start, or when `end` is set the end, of the
    /// line of the field `id` that the caret at byte `caret` is on.
    pub fn line_edge(&self, id: FocusId, caret: usize, end: bool) -> Option<usize> {
        let lines = self.lines_of(id);
        let line = lines[Self::line_at(&lines, caret)?];
        Some(if end {
            line.start + line.length
        } else {
            line.start
        })
    }

    /// The position among `lines` of the one the byte `caret` is on: the
    /// last line that starts at or before it.
    fn line_at(lines: &[&FieldBox], caret: usize) -> Option<usize> {
        lines.iter().rposition(|line| line.start <= caret)
    }

    /// The topmost region under `point`.
    pub fn hit(&self, point: (f32, f32)) -> Option<FocusId> {
        self.regions
            .iter()
            .rev()
            .find(|region| region.rect.contains(point.0, point.1))
            .map(|region| region.id)
    }
}

/// Lays `root` out in a viewport of `width` by `height` logical pixels and
/// paints it into `list`.
pub fn draw(
    root: &Node,
    width: f32,
    height: f32,
    input: &Input,
    theme: &Theme,
    shaper: &mut Shaper,
    list: &mut DrawList,
) -> Output {
    let laid = solve(&root.item(shaper), width, height);
    let mut frame = Frame {
        input,
        width,
        height,
        theme,
        shaper,
        list,
        output: Output::default(),
        clip: None,
        origin: (0.0, 0.0),
        culling: true,
        group_hovered: false,
        tooltip: None,
        selectable: None,
        counts: Vec::new(),
    };
    frame.node(
        root,
        &laid,
        Affine::IDENTITY,
        1.0,
        Cursor::Default,
        (0.0, 0.0),
    );
    match tooltip::settle(frame.tooltip.take(), input.elapsed_ms) {
        Showing::Shown(over, words) => frame.paint_tooltip(over, &words),
        Showing::Waiting => frame.output.animating = true,
        Showing::Nothing => {}
    }
    retain::end_frame();
    frame.output
}

/// The state of one painting pass.
struct Frame<'a> {
    /// What the pass is painting for.
    input: &'a Input,
    /// The viewport width, which breakpoints are resolved against.
    width: f32,
    /// The viewport height.
    height: f32,
    /// The theme roles are resolved against.
    theme: &'a Theme,
    /// Shapes the text the tree holds.
    shaper: &'a mut Shaper,
    /// Where primitives go.
    list: &'a mut DrawList,
    /// What the pass found out.
    output: Output,
    /// The rectangle everything now being painted is clipped to.
    clip: Option<Rect>,
    /// Where the boxes now being painted were laid out from: the top-left
    /// corner of the retained subtree they are in, or of the window.
    origin: (f32, f32),
    /// Whether boxes wholly outside the clip are passed over rather than painted.
    culling: bool,
    /// Whether the pointer is over the nearest group enclosing the boxes now being painted.
    group_hovered: bool,
    /// The words the innermost box under the pointer that has a tooltip asks to show, and where that box is.
    tooltip: Option<(Rect, String)>,
    /// The selectable text the boxes now being painted are part of.
    selectable: Option<FocusId>,
    /// How many pieces of each selectable text were met so far.
    counts: Vec<(FocusId, usize)>,
}

impl Frame<'_> {
    /// Paints `node` and its children, drawn through `parent` at `opacity`.
    fn node(
        &mut self,
        node: &Node,
        laid: &Placed,
        parent: Affine,
        opacity: f32,
        inherited: Cursor,
        scrolled: (f32, f32),
    ) {
        let rect = Rect::new(
            laid.x + self.origin.0,
            laid.y + self.origin.1,
            laid.width,
            laid.height,
        );
        let shown = rect.offset(-scrolled.0, -scrolled.1);
        if self.culling && self.clip.is_some_and(|clip| apart(clip, shown)) {
            self.unseen(node, laid, parent, opacity, inherited, scrolled);
            return;
        }
        let seen = intersect(self.clip, shown);
        let outer_group = self.group_hovered;
        if node.group {
            self.group_hovered = self.pointer_in(seen);
        }
        let computed = self.resolve(node, seen);
        if computed.layout.display.display == Display::None {
            self.group_hovered = outer_group;
            return;
        }

        let outer_selectable = self.selectable;
        if let Some(id) = node.selectable {
            self.selectable = Some(id);
            self.output.selectables.push(Region { id, rect: seen });
        }
        let cursor = match computed.decoration.effects.interactivity.cursor {
            Cursor::Default if self.selectable.is_some() && node.text.is_some() => Cursor::Text,
            Cursor::Default => inherited,
            own => own,
        };
        if self.pointer_in(seen) {
            self.output.cursor = cursor;
            if let Some(words) = &node.tooltip {
                self.tooltip = Some((seen, words.clone()));
            }
        }
        if let Some(id) = node.focus {
            self.output.regions.push(Region { id, rect: seen });
        }
        let (pose, pose_opacity) = self.pose(&computed, rect);
        let world = pose.then(parent);
        let opacity = opacity * computed.decoration.effects.opacity * pose_opacity;
        self.decoration(&computed, rect, world, opacity);
        let outer = self.clip;
        let display = computed.layout.display;
        if display.overflow_x != Overflow::Visible || display.overflow_y != Overflow::Visible {
            self.clip = Some(seen);
            self.list.set_clip(self.clip);
        }
        let (scroll_y, room) = match node.scroll_anchor {
            Some((id, anchor, position)) => self.scroll_anchored(
                id,
                (anchor, position),
                rect,
                seen,
                laid,
                computed.layout.spacing.padding.bottom,
            ),
            None => (node.scroll_y, 0.0),
        };
        let bar = node.scroll_anchor.map(|(id, ..)| id);
        let content = if node.scroll_x == 0.0 && scroll_y == 0.0 {
            world
        } else {
            Affine::translation(-node.scroll_x, -scroll_y).then(world)
        };
        let scrolled = (scrolled.0 + node.scroll_x, scrolled.1 + scroll_y);
        if let Some(text) = &node.text {
            self.text(text, node, &computed, (rect, seen), content, opacity);
        }
        if let Some(name) = node.icon {
            self.icon(name, &computed, rect, content, opacity);
        }
        if let Some((share, width, color, track)) = node.arc {
            self.arc(
                (share, width, color, track),
                &computed,
                rect,
                content,
                opacity,
            );
        }
        if let Some(picture) = &node.picture {
            let inset = computed.layout.spacing.padding;
            let (x, y) = content.apply(rect.x + inset.left, rect.y + inset.top);
            self.list.push_image(pm_renderer::Image {
                picture: picture.clone(),
                x,
                y,
                width: rect.width - inset.left - inset.right,
                height: rect.height - inset.top - inset.bottom,
                radius: computed.decoration.borders.radius.top_left,
            });
        }
        for (child, child_laid) in node.children.iter().zip(&laid.children) {
            self.node(child, child_laid, content, opacity, cursor, scrolled);
        }
        if let Some(retained) = &node.retained {
            let inset = computed.layout.spacing.padding;
            let border = computed.decoration.borders.width;
            let outer = self.origin;
            self.origin = (
                rect.x + inset.left + border.left,
                rect.y + inset.top + border.top,
            );
            self.node(
                &retained.root,
                &retained.placed,
                content,
                opacity,
                cursor,
                scrolled,
            );
            self.origin = outer;
        }
        if let Some(id) = bar.filter(|_| room > 0.0) {
            self.scrollbar(id, (rect, seen), scroll_y, room, world);
        }
        if self.clip != outer {
            self.clip = outer;
            self.list.set_clip(outer);
        }
        self.group_hovered = outer_group;
        self.selectable = outer_selectable;
    }

    /// The number of the next piece of the selectable text `id`, counted in tree order.
    fn next_piece(&mut self, id: FocusId) -> usize {
        match self.counts.iter_mut().find(|(owner, _)| *owner == id) {
            Some((_, count)) => {
                *count += 1;
                *count - 1
            }
            None => {
                self.counts.push((id, 1));
                0
            }
        }
    }

    /// Records `content`, laid out at `laid`, as the next piece of the
    /// selectable text the box is in, if it is in one, with where it was
    /// `drawn`, and gives back that text and the piece's number.
    fn record_piece(
        &mut self,
        content: &std::sync::Arc<str>,
        laid: Rect,
        drawn: Option<DrawnPiece>,
    ) -> Option<(FocusId, usize)> {
        let id = self.selectable?;
        let index = self.next_piece(id);
        self.output.pieces.push(TextPiece {
            id,
            index,
            content: content.clone(),
            laid,
            drawn,
        });
        Some((id, index))
    }

    /// Tints the byte range `range` of `run`, drawn at `x`, `y`, as selected.
    fn highlight(
        &mut self,
        run: &Run,
        range: std::ops::Range<usize>,
        at: (f32, f32),
        opacity: f32,
    ) {
        let left = run.caret_x(range.start);
        let right = run.caret_x(range.end);
        let tint = Color::from(Role::Focused)
            .resolve(self.theme)
            .alpha(0.45 * opacity);
        self.list.push(Quad::filled(
            Rect::new(at.0 + left, at.1, right - left, run.height),
            tint,
        ));
    }

    /// Paints a tooltip saying `words` over everything painted so far, above the box at `over`, or below it where there is no room above, kept inside the viewport.
    fn paint_tooltip(&mut self, over: Rect, words: &str) {
        let holder = tooltip::holder(words);
        let laid = solve(&holder.item(self.shaper), self.width, self.height);
        let (Some(card), Some(placed)) = (holder.children.first(), laid.children.first()) else {
            return;
        };
        let above = over.y - tooltip::GAP - placed.height;
        let top = match above >= 0.0 {
            true => above,
            false => over.y + over.height + tooltip::GAP,
        };
        let left = over.x.min(self.width - placed.width).max(0.0);
        self.clip = None;
        self.list.set_clip(None);
        self.origin = (left - placed.x, top - placed.y);
        self.node(
            card,
            placed,
            Affine::IDENTITY,
            1.0,
            Cursor::Default,
            (0.0, 0.0),
        );
        self.origin = (0.0, 0.0);
    }

    /// Whether the pointer is inside `rect`.
    fn pointer_in(&self, rect: Rect) -> bool {
        self.input
            .pointer
            .position
            .is_some_and(|(x, y)| rect.contains(x, y))
    }

    /// Walks `node`, which lies wholly outside the clip and so shows nothing,
    /// without painting it: the names its boxes answer to are still reported,
    /// in tree order, and the lines of text fields in it are painted as usual
    /// so the caret can still be moved through them.
    fn unseen(
        &mut self,
        node: &Node,
        laid: &Placed,
        parent: Affine,
        opacity: f32,
        inherited: Cursor,
        scrolled: (f32, f32),
    ) {
        if node.style.base().layout.display.display == Display::None {
            return;
        }
        let outer_selectable = self.selectable;
        if let Some(id) = node.selectable {
            self.selectable = Some(id);
            let rect = Rect::new(laid.x + self.origin.0, laid.y + self.origin.1, 0.0, 0.0);
            self.output.selectables.push(Region { id, rect });
        }
        if let Some(text) = &node.text
            && node.field_line.is_none()
            && node.caret.is_none()
        {
            let laid = Rect::new(
                laid.x + self.origin.0,
                laid.y + self.origin.1,
                laid.width,
                laid.height,
            );
            self.record_piece(text, laid, None);
        }
        self.unseen_inside(node, laid, parent, opacity, inherited, scrolled);
        self.selectable = outer_selectable;
    }

    /// Walks what `node`, which shows nothing, holds without painting it, as [`Frame::unseen`] does.
    fn unseen_inside(
        &mut self,
        node: &Node,
        laid: &Placed,
        parent: Affine,
        opacity: f32,
        inherited: Cursor,
        scrolled: (f32, f32),
    ) {
        if node.field_line.is_some() || node.caret.is_some() {
            self.culling = false;
            self.node(node, laid, parent, opacity, inherited, scrolled);
            self.culling = true;
            return;
        }
        if let Some(id) = node.focus {
            let rect = Rect::new(laid.x + self.origin.0, laid.y + self.origin.1, 0.0, 0.0);
            self.output.regions.push(Region { id, rect });
        }
        let scrolled = (scrolled.0 + node.scroll_x, scrolled.1 + node.scroll_y);
        for (child, child_laid) in node.children.iter().zip(&laid.children) {
            self.unseen(child, child_laid, parent, opacity, inherited, scrolled);
        }
        if let Some(retained) = &node.retained {
            let outer = self.origin;
            self.origin = (laid.x + outer.0, laid.y + outer.1);
            self.unseen(
                &retained.root,
                &retained.placed,
                parent,
                opacity,
                inherited,
                scrolled,
            );
            self.origin = outer;
        }
    }

    /// Draws a thin bar down the right edge of the scrolling box `id` at
    /// `rect`, shown at `seen`, that shows where its contents, scrolled
    /// `scrolled` pixels of `room`, are: wider and brighter while the pointer
    /// is on the strip that takes hold of it, and brighter still while held.
    fn scrollbar(
        &mut self,
        id: FocusId,
        (rect, seen): (Rect, Rect),
        scrolled: f32,
        room: f32,
        world: Affine,
    ) {
        let track = (rect.height - 2.0 * SCROLLBAR_INSET).max(0.0);
        let thumb = (track * rect.height / (rect.height + room)).max(SCROLLBAR_MINIMUM);
        let thumb = thumb.min(track);
        let top = rect.y + SCROLLBAR_INSET + (track - thumb) * (scrolled / room).clamp(0.0, 1.0);
        let grip = Rect::new(
            seen.x + seen.width - SCROLLBAR_GRIP,
            seen.y,
            SCROLLBAR_GRIP.min(seen.width),
            seen.height,
        );
        let held = self.input.held_bar == Some(id);
        let (width, role, alpha) = if held {
            (SCROLLBAR_WIDE, Role::Text, 0.6)
        } else if self.input.held_bar.is_none() && self.pointer_in(grip) {
            (SCROLLBAR_WIDE, Role::Muted, 0.85)
        } else {
            (SCROLLBAR_WIDTH, Role::Muted, 0.55)
        };
        let left = rect.x + rect.width - SCROLLBAR_INSET - width;
        let color = Color::from(role).resolve(self.theme).alpha(alpha);
        self.list.push(
            Quad::filled(Rect::new(left, top, width, thumb), color)
                .radii([width / 2.0; 4])
                .transformed(world),
        );
    }

    /// How far to shift the contents of the box at `rect` up so they are
    /// seen at `place`, measured from one of their ends; how far they can be
    /// scrolled is also reported.
    fn scroll_anchored(
        &mut self,
        id: FocusId,
        place: (Anchor, f32),
        rect: Rect,
        seen: Rect,
        laid: &Placed,
        padding: f32,
    ) -> (f32, f32) {
        let end = laid
            .children
            .iter()
            .map(|child| self.origin.1 + child.y + child.height)
            .fold(rect.y, f32::max)
            + padding;
        let room = (end - (rect.y + rect.height)).max(0.0);
        let (anchor, position) = place;
        let offset = match anchor {
            Anchor::Start => position.clamp(0.0, room),
            Anchor::End => room - position.clamp(0.0, room),
        };
        self.output.scrollers.push(Scroller {
            id,
            rect: seen,
            room,
            offset,
        });
        (offset, room)
    }

    /// The node's style for the pointer and the viewport.
    fn resolve(&self, node: &Node, rect: Rect) -> ComputedStyle {
        let pointer = self.input.pointer;
        let mut states = StateSet::EMPTY;
        if self.group_hovered {
            states = states.with(State::GroupHover);
        }
        if self.pointer_in(rect) {
            states = states.with(State::Hover);
            if pointer.left {
                states = states.with(State::Active);
            }
        }
        let focused = self.input.focused;
        if focused.is_some() && (node.focus == focused || node.focus_for == focused) {
            states = states.with(State::Focus);
        }
        node.style.resolve(&Context {
            states,
            viewport_width: self.width,
        })
    }

    /// The transform the node's own style and animation put on it, about its
    /// own pivot, and the opacity its animation asks for.
    fn pose(&mut self, computed: &ComputedStyle, rect: Rect) -> (Affine, f32) {
        let fixed = about(computed.animation.transform, rect);
        let Some(animation) = computed.animation.animation else {
            return (fixed, 1.0);
        };
        self.output.animating = true;
        match animation.sample(self.input.elapsed_ms) {
            Some(pose) => (fixed.then(about(pose.transform, rect)), pose.opacity),
            None => (fixed, 1.0),
        }
    }

    /// Paints a ring in the largest square that fits the content area of the box at `rect`: the track all round, and over it the filled part.
    fn arc(
        &mut self,
        arc: (f32, f32, Color, Color),
        computed: &ComputedStyle,
        rect: Rect,
        world: Affine,
        opacity: f32,
    ) {
        let (share, width, color, track) = arc;
        let inset = computed.layout.spacing.padding;
        let across =
            (rect.width - inset.left - inset.right).min(rect.height - inset.top - inset.bottom);
        let (x, y) = world.apply(rect.x + inset.left, rect.y + inset.top);
        let bounds = Rect::new(x, y, across, across);
        let theme = self.theme;
        self.list.push(Quad::arc(
            bounds,
            track.resolve(theme).alpha(opacity),
            width,
            1.0,
        ));
        if share > 0.0 {
            self.list.push(Quad::arc(
                bounds,
                color.resolve(theme).alpha(opacity),
                width,
                share,
            ));
        }
    }

    /// Paints `name` in the largest square that fits the content area of the box at `rect`.
    fn icon(
        &mut self,
        svg: pm_icon::Svg,
        computed: &ComputedStyle,
        rect: Rect,
        world: Affine,
        opacity: f32,
    ) {
        let inset = computed.layout.spacing.padding;
        let border = computed.decoration.borders.width;
        let width = rect.width - inset.left - inset.right - border.left - border.right;
        let height = rect.height - inset.top - inset.bottom - border.top - border.bottom;
        let size = width.min(height);
        let (middle_x, middle_y) = world.apply(
            rect.x + inset.left + border.left + width / 2.0,
            rect.y + inset.top + border.top + height / 2.0,
        );
        let color = computed
            .typography
            .color
            .unwrap_or(Role::Text.into())
            .resolve(self.theme)
            .alpha(opacity);
        self.list.push_icon(Icon {
            svg,
            x: middle_x - size / 2.0,
            y: middle_y - size / 2.0,
            size,
            color,
            rotation: world.b.atan2(world.a),
        });
    }

    /// Paints one line of text inside the box laid out at the first of
    /// `boxes` and seen at the second.
    fn text(
        &mut self,
        content: &std::sync::Arc<str>,
        node: &Node,
        computed: &ComputedStyle,
        boxes: (Rect, Rect),
        world: Affine,
        opacity: f32,
    ) {
        let (rect, seen) = boxes;
        let inset = computed.layout.spacing.padding;
        let border = computed.decoration.borders.width;
        let room = rect.width - inset.left - inset.right - border.left - border.right;
        let shortened = self.ellipsized(content, computed, room);
        let content = shortened.as_ref().unwrap_or(content);
        let run = self.shaper.shape(content, &computed.typography);
        let (x, y) = world.apply(
            rect.x + inset.left + border.left,
            rect.y + inset.top + border.top,
        );
        let color = computed
            .typography
            .color
            .unwrap_or(Role::Text.into())
            .resolve(self.theme)
            .alpha(opacity);
        if let Some((owner, start)) = node.field_line.or(node.caret.map(|(_, owner)| (owner, 0))) {
            self.output.fields.push(FieldBox {
                id: owner,
                start,
                x,
                y,
                height: run.height,
                run: run.clone(),
                view: self.clip,
                length: content.len(),
            });
        }
        let guides: Vec<&Mark> = node
            .marks
            .iter()
            .filter(|mark| mark.paint == Paint::Guide)
            .collect();
        if !guides.is_empty() {
            let space = self.shaper.shape(" ", &computed.typography).width;
            for mark in guides {
                self.list.push(Quad::filled(
                    Rect::new(x + space * mark.range.start as f32, y, 1.0, run.height),
                    mark.color.resolve(self.theme).alpha(opacity),
                ));
            }
        }
        for mark in node.marks.iter().filter(|mark| mark.paint == Paint::Fill) {
            let left = run.caret_x(mark.range.start);
            let right = run.caret_x(mark.range.end);
            self.list.push(Quad::filled(
                Rect::new(x + left, y, (right - left).max(0.0), run.height),
                mark.color.resolve(self.theme).alpha(opacity),
            ));
        }
        if let Some((range, owner)) = &node.selection
            && self.input.focused == Some(*owner)
        {
            self.highlight(&run, range.clone(), (x, y), opacity);
        }
        for mark in node
            .marks
            .iter()
            .filter(|mark| mark.paint == Paint::Selected)
        {
            self.highlight(&run, mark.range.clone(), (x, y), opacity);
        }
        let drawn = DrawnPiece {
            seen,
            x,
            run: run.clone(),
        };
        if let Some((id, index)) = self.record_piece(content, rect, Some(drawn))
            && let Some(range) = self
                .input
                .selection
                .filter(|selection| selection.id == id)
                .and_then(|selection| selection.covering(index, content.len()))
        {
            self.highlight(&run, range, (x, y), opacity);
        }
        if let Some((range, owner)) = &node.marked
            && self.input.focused == Some(*owner)
        {
            let left = run.caret_x(range.start);
            let right = run.caret_x(range.end);
            let thickness = (computed.typography.size / 14.0).max(1.0);
            self.list.push(Quad::filled(
                Rect::new(
                    x + left,
                    y + run.baseline + computed.typography.size * 0.12,
                    right - left,
                    thickness,
                ),
                color,
            ));
        }
        let mut carets: Vec<usize> = node
            .marks
            .iter()
            .filter(|mark| mark.paint == Paint::Caret)
            .map(|mark| mark.range.start)
            .collect();
        if let Some((byte, owner)) = node.caret
            && self.input.focused == Some(owner)
        {
            carets.push(byte);
        }
        if !carets.is_empty() {
            let look = node.caret_look;
            let lit =
                look.steady || ((self.input.elapsed_ms / CARET_BLINK_MS) as u32).is_multiple_of(2);
            if !look.steady {
                self.output.animating = true;
            }
            if lit {
                let size = computed.typography.size;
                for byte in carets {
                    let rect = caret_rect(content, &run, byte, look.shape, size);
                    let tone = match look.shape {
                        CaretShape::Block => color.alpha(0.45 * opacity.max(0.0)),
                        CaretShape::Bar | CaretShape::Underline => color,
                    };
                    self.list.push(Quad::filled(
                        Rect::new(x + rect.x, y + rect.y, rect.width, rect.height),
                        tone,
                    ));
                }
            }
        }
        self.lines(computed, &run, x, y, color);
        for mark in node.marks.iter().filter(|mark| mark.paint == Paint::Wave) {
            self.wave(&run, mark, (x, y), opacity);
        }
        for mark in node
            .marks
            .iter()
            .filter(|mark| mark.paint == Paint::Underline)
        {
            let left = run.caret_x(mark.range.start);
            let right = run.caret_x(mark.range.end);
            self.list.push(Quad::filled(
                Rect::new(
                    x + left,
                    y + run.baseline + computed.typography.size * 0.12,
                    (right - left).max(0.0),
                    (computed.typography.size / 14.0).max(1.0),
                ),
                mark.color.resolve(self.theme).alpha(opacity),
            ));
        }
        let mut inked: Vec<&Mark> = node
            .marks
            .iter()
            .filter(|mark| mark.paint == Paint::Ink)
            .collect();
        inked.sort_by_key(|mark| mark.range.start);
        if inked.is_empty() {
            self.list.push_text(Text { run, x, y, color });
            return;
        }
        let mut plain = Vec::new();
        let mut last = 0;
        for mark in &inked {
            if mark.range.start > last {
                plain.push(last..mark.range.start);
            }
            last = last.max(mark.range.end);
            self.list.push_text(Text {
                run: run.part(mark.range.clone()),
                x,
                y,
                color: mark.color.resolve(self.theme).alpha(opacity),
            });
        }
        plain.push(last..content.len().max(last));
        for range in plain {
            self.list.push_text(Text {
                run: run.part(range),
                x,
                y,
                color,
            });
        }
    }

    /// `content` cut short and ended with an ellipsis so that it fits in `room`
    /// logical pixels, when its typography says overflowing text ends in one and
    /// it does not fit; `None` when it is to be drawn as it is.
    fn ellipsized(
        &mut self,
        content: &std::sync::Arc<str>,
        computed: &ComputedStyle,
        room: f32,
    ) -> Option<std::sync::Arc<str>> {
        if computed.typography.overflow != TextOverflow::Ellipsis || content.contains('\n') {
            return None;
        }
        if self.shaper.measure(content, &computed.typography).width <= room + 1.0 {
            return None;
        }
        let boundaries: Vec<usize> = content
            .char_indices()
            .map(|(at, _)| at)
            .chain(std::iter::once(content.len()))
            .collect();
        let (mut low, mut high) = (0, boundaries.len() - 1);
        while low < high {
            let middle = (low + high).div_ceil(2);
            let candidate = format!("{}…", content[..boundaries[middle]].trim_end());
            if self.shaper.measure(&candidate, &computed.typography).width <= room {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        Some(format!("{}…", content[..boundaries[low]].trim_end()).into())
    }

    /// Paints the wavy line `mark` asks for under its stretch of `run`, the line drawn with its top-left corner at `at`.
    fn wave(&mut self, run: &Run, mark: &Mark, at: (f32, f32), opacity: f32) {
        let left = at.0 + run.caret_x(mark.range.start);
        let right = at.0 + run.caret_x(mark.range.end);
        let right = if right - left < WAVE_STEP * 2.0 {
            left + WAVE_STEP * 2.0
        } else {
            right
        };
        let color = mark.color.resolve(self.theme).alpha(opacity);
        let top = at.1 + run.baseline + 2.0;
        let mut step = 0;
        let mut from = left;
        while from < right {
            let lifted = if step % 2 == 0 { 0.0 } else { 1.0 };
            self.list.push(Quad::filled(
                Rect::new(from, top + lifted, WAVE_STEP.min(right - from), 1.0),
                color,
            ));
            from += WAVE_STEP;
            step += 1;
        }
    }

    /// Paints the underline and the line through that the typography of
    /// `computed` asks for, along `run` drawn at `x`, `y` in `color`.
    fn lines(&mut self, computed: &ComputedStyle, run: &Run, x: f32, y: f32, color: Srgba) {
        let typography = &computed.typography;
        let decoration = typography.decoration;
        if !decoration.underline && !decoration.line_through {
            return;
        }
        let thickness = decoration
            .thickness
            .unwrap_or((typography.size / 14.0).max(1.0));
        let color = decoration
            .color
            .map_or(color, |own| own.resolve(self.theme).alpha(color.a));
        let baseline = y + run.baseline;
        let mut rule = |top: f32| {
            self.list
                .push(Quad::filled(Rect::new(x, top, run.width, thickness), color));
        };
        if decoration.underline {
            rule(
                baseline
                    + decoration
                        .underline_offset
                        .unwrap_or(typography.size * 0.12),
            );
        }
        if decoration.line_through {
            rule(baseline - typography.size * 0.3);
        }
    }

    /// Paints the shadows, ring, fill and borders of one node.
    fn decoration(&mut self, computed: &ComputedStyle, rect: Rect, world: Affine, opacity: f32) {
        let decoration = &computed.decoration;
        let radii = [
            decoration.borders.radius.top_left,
            decoration.borders.radius.top_right,
            decoration.borders.radius.bottom_right,
            decoration.borders.radius.bottom_left,
        ];

        let shadow = decoration
            .shadows
            .color
            .unwrap_or(Color::Black)
            .resolve(self.theme);
        for layer in decoration
            .shadows
            .layers
            .iter()
            .filter(|layer| !layer.inset)
        {
            let bounds = rect.inflate(layer.spread).offset(layer.x, layer.y);
            self.list.push(
                Quad::filled(bounds, shadow.alpha(layer.alpha * opacity))
                    .radii(radii.map(|radius| (radius + layer.spread).max(0.0)))
                    .softness(layer.blur * SHADOW_SOFTNESS)
                    .transformed(world),
            );
        }

        if let Some(ring) = decoration.shadows.ring {
            let color = ring
                .color
                .unwrap_or(Role::Focused.into())
                .resolve(self.theme);
            self.list.push(
                Quad::filled(rect.inflate(ring.width), color.alpha(0.0))
                    .border(ring.width, color.alpha(opacity))
                    .radii(radii.map(|radius| radius + ring.width))
                    .transformed(world),
            );
        }

        let fill = decoration
            .background
            .color
            .unwrap_or(Color::Transparent)
            .resolve(self.theme)
            .alpha(opacity);
        let borders = decoration.borders;
        let widths = borders.width;
        let colors = borders.color;
        let uniform = widths.top == widths.right
            && widths.top == widths.bottom
            && widths.top == widths.left
            && colors.top == colors.right
            && colors.top == colors.bottom
            && colors.top == colors.left;
        let quad = Quad::filled(rect, fill).radii(radii).transformed(world);
        if uniform {
            let color = colors
                .top
                .unwrap_or(Color::Transparent)
                .resolve(self.theme)
                .alpha(opacity);
            self.list.push(quad.border(widths.top, color));
            return;
        }
        self.list.push(quad);
        for side in [Side::Top, Side::Right, Side::Bottom, Side::Left] {
            let width = widths.get(side);
            let color = colors
                .get(side)
                .unwrap_or(Color::Transparent)
                .resolve(self.theme)
                .alpha(opacity);
            if width > 0.0 && color.a > 0.0 {
                self.list
                    .push(Quad::filled(strip(side, rect, width), color).transformed(world));
            }
        }
    }
}

/// Whether `rect` lies wholly outside `clip`, sharing no point with it.
fn apart(clip: Rect, rect: Rect) -> bool {
    rect.x >= clip.x + clip.width
        || rect.y >= clip.y + clip.height
        || rect.x + rect.width <= clip.x
        || rect.y + rect.height <= clip.y
}

/// `rect` cut down to what `outer` leaves of it.
fn intersect(outer: Option<Rect>, rect: Rect) -> Rect {
    let Some(outer) = outer else {
        return rect;
    };
    let left = rect.x.max(outer.x);
    let top = rect.y.max(outer.y);
    let right = (rect.x + rect.width).min(outer.x + outer.width);
    let bottom = (rect.y + rect.height).min(outer.y + outer.height);
    Rect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0))
}

/// The transform as it acts on an element at `rect`: pivoting about its own
/// origin, in window coordinates.
fn about(transform: Transform, rect: Rect) -> Affine {
    let matrix = Affine::from_array(transform.matrix(rect.width, rect.height));
    Affine::translation(-rect.x, -rect.y)
        .then(matrix)
        .then(Affine::translation(rect.x, rect.y))
}

/// The strip `thickness` deep along `side` of `rect`.
fn strip(side: Side, rect: Rect, thickness: f32) -> Rect {
    match side {
        Side::Top => Rect::new(rect.x, rect.y, rect.width, thickness),
        Side::Right => Rect::new(
            rect.x + rect.width - thickness,
            rect.y,
            thickness,
            rect.height,
        ),
        Side::Bottom => Rect::new(
            rect.x,
            rect.y + rect.height - thickness,
            rect.width,
            thickness,
        ),
        Side::Left => Rect::new(rect.x, rect.y, thickness, rect.height),
    }
}

/// Where the caret at byte `byte` of `content`, shaped as `run`, is drawn in `shape`, from the start of the line, for type `size` logical pixels tall: a bar before the character, the character's cell filled, or a line under it.
fn caret_rect(content: &str, run: &Run, byte: usize, shape: CaretShape, size: f32) -> Rect {
    let at = run.caret_x(byte);
    let next = content
        .get(byte..)
        .and_then(|rest| rest.chars().next())
        .map(|character| byte + character.len_utf8());
    let cell = next
        .map(|next| run.caret_x(next) - at)
        .filter(|width| *width > 0.5)
        .unwrap_or(size * 0.6);
    match shape {
        CaretShape::Bar => Rect::new(at, 0.0, CARET_WIDTH, run.height),
        CaretShape::Block => Rect::new(at, 0.0, cell, run.height),
        CaretShape::Underline => Rect::new(at, run.height - 2.0, cell, 2.0),
    }
}
