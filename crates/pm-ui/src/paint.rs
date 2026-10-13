//! Painting a laid-out node tree into a draw list.

use pm_icon::IconName;
use pm_input::{FocusId, PointerState};
use pm_renderer::{Affine, DrawList, Icon, Quad, Rect, Text};
use pm_style::layout::{Display, Overflow};
use pm_style::primitives::Side;
use pm_style::{Color, ComputedStyle, Context, Cursor, Role, State, StateSet, Transform};

use pm_style::layout::{Placed, solve};
use pm_text::{Run, Shaper};
use pm_theme::Theme;

use crate::node::Node;

/// How long the caret stays on, and then off, in milliseconds.
const CARET_BLINK_MS: f32 = 530.0;

/// The width of the caret in logical pixels.
const CARET_WIDTH: f32 = 1.5;

/// What the screen is being drawn for: the pointer, the focus and the clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Where the pointer is and which buttons are down.
    pub pointer: PointerState,
    /// The box holding keyboard focus, if any.
    pub focused: Option<FocusId>,
    /// Milliseconds since the application started, which animations run on.
    pub elapsed_ms: f32,
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
}

/// One line of editable text, where it was drawn.
#[derive(Clone, Debug)]
pub struct FieldBox {
    /// The name of the box that owns the caret.
    pub id: FocusId,
    /// Where the line starts along x, in logical pixels.
    pub x: f32,
    /// The line, shaped.
    pub run: Run,
    /// The rectangle the line is clipped to, if it is.
    pub view: Option<Rect>,
    /// How many bytes the line has.
    pub length: usize,
}

impl Output {
    /// The byte offset in the field `id` nearest to `x` logical pixels, if
    /// that field was drawn.
    pub fn offset_in(&self, id: FocusId, x: f32) -> Option<usize> {
        let field = self.fields.iter().find(|field| field.id == id)?;
        Some(field.run.byte_at(x - field.x, field.length))
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
        theme,
        shaper,
        list,
        output: Output::default(),
        clip: None,
    };
    frame.node(root, &laid, Affine::IDENTITY, 1.0, Cursor::Default);
    frame.output
}

/// The state of one painting pass.
struct Frame<'a> {
    /// What the pass is painting for.
    input: &'a Input,
    /// The viewport width, which breakpoints are resolved against.
    width: f32,
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
    ) {
        let rect = Rect::new(laid.x, laid.y, laid.width, laid.height);
        let computed = self.resolve(node, rect);
        if computed.layout.display.display == Display::None {
            return;
        }

        let cursor = match computed.decoration.effects.interactivity.cursor {
            Cursor::Default => inherited,
            own => own,
        };
        if self
            .input
            .pointer
            .position
            .is_some_and(|(x, y)| rect.contains(x, y))
        {
            self.output.cursor = cursor;
        }
        if let Some(id) = node.focus {
            self.output.regions.push(Region { id, rect });
        }
        let (pose, pose_opacity) = self.pose(&computed, rect);
        let world = pose.then(parent);
        let opacity = opacity * computed.decoration.effects.opacity * pose_opacity;
        self.decoration(&computed, rect, world, opacity);
        let outer = self.clip;
        let display = computed.layout.display;
        if display.overflow_x != Overflow::Visible || display.overflow_y != Overflow::Visible {
            let held = intersect(outer, rect);
            self.clip = Some(held);
            self.list.set_clip(self.clip);
        }
        let content = if node.scroll_x == 0.0 {
            world
        } else {
            Affine::translation(-node.scroll_x, 0.0).then(world)
        };
        if let Some(text) = node.text.as_deref() {
            self.text(text, node, &computed, rect, content, opacity);
        }
        if let Some(name) = node.icon {
            self.icon(name, &computed, rect, content, opacity);
        }
        for (child, child_laid) in node.children.iter().zip(&laid.children) {
            self.node(child, child_laid, content, opacity, cursor);
        }
        if self.clip != outer {
            self.clip = outer;
            self.list.set_clip(outer);
        }
    }

    /// The node's style for the pointer and the viewport.
    fn resolve(&self, node: &Node, rect: Rect) -> ComputedStyle {
        let pointer = self.input.pointer;
        let hovered = pointer.position.is_some_and(|(x, y)| rect.contains(x, y));
        let mut states = StateSet::EMPTY;
        if hovered {
            states = states.with(State::Hover);
            if pointer.left {
                states = states.with(State::Active);
            }
        }
        if node.focus.is_some() && node.focus == self.input.focused {
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

    /// Paints `name` in the largest square that fits the content area of the box at `rect`.
    fn icon(
        &mut self,
        name: IconName,
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
        let (x, y) = world.apply(
            rect.x + inset.left + border.left + (width - size) / 2.0,
            rect.y + inset.top + border.top + (height - size) / 2.0,
        );
        let color = computed
            .typography
            .color
            .unwrap_or(Role::Text.into())
            .resolve(self.theme)
            .alpha(opacity);
        self.list.push_icon(Icon {
            svg: name.svg(),
            x,
            y,
            size,
            color,
        });
    }

    /// Paints one line of text inside the box at `rect`.
    fn text(
        &mut self,
        content: &str,
        node: &Node,
        computed: &ComputedStyle,
        rect: Rect,
        world: Affine,
        opacity: f32,
    ) {
        let run = self.shaper.shape(content, &computed.typography);
        let inset = computed.layout.spacing.padding;
        let border = computed.decoration.borders.width;
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
        if let Some((_, owner)) = node.caret {
            self.output.fields.push(FieldBox {
                id: owner,
                x,
                run: run.clone(),
                view: self.clip,
                length: content.len(),
            });
        }
        if let Some((range, owner)) = &node.selection
            && self.input.focused == Some(*owner)
        {
            let left = run.caret_x(range.start);
            let right = run.caret_x(range.end);
            let tint = Color::from(Role::Focused)
                .resolve(self.theme)
                .alpha(0.45 * opacity);
            self.list.push(Quad::filled(
                Rect::new(x + left, y, right - left, run.height),
                tint,
            ));
        }
        if let Some((byte, owner)) = node.caret
            && self.input.focused == Some(owner)
        {
            self.output.animating = true;
            if ((self.input.elapsed_ms / CARET_BLINK_MS) as u32).is_multiple_of(2) {
                let at = x + run.caret_x(byte);
                self.list.push(Quad::filled(
                    Rect::new(at, y, CARET_WIDTH, run.height),
                    color,
                ));
            }
        }
        self.list.push_text(Text { run, x, y, color });
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
                    .softness(layer.blur / 2.0)
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
