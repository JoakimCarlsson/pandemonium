//! The node: a style, what it holds and whether it answers to the user.

use std::sync::Arc;

use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::layout::Item;
use pm_style::{Style, Utility};
use pm_text::Shaper;

use crate::retain::Retained;

/// How a caret is drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CaretShape {
    /// A thin upright bar before the character it is at.
    #[default]
    Bar,
    /// The character it is at, filled.
    Block,
    /// A line under the character it is at.
    Underline,
}

/// How the carets of a node are drawn: their shape, and whether they stay lit rather than blink.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CaretLook {
    /// The shape.
    pub shape: CaretShape,
    /// Whether they stay lit.
    pub steady: bool,
}

/// One box of the tree: how it is styled and what it holds.
#[derive(Clone, Debug, Default)]
pub struct Node {
    /// How the box is laid out, painted and typeset.
    pub(crate) style: Style,
    /// The boxes inside it, in layout order.
    pub(crate) children: Vec<Node>,
    /// A line of text the box holds instead of children.
    pub(crate) text: Option<Arc<str>>,
    /// What names the box to the user: it can be pressed and takes focus.
    pub(crate) focus: Option<FocusId>,
    /// Offscreen list entries kept in keyboard traversal order without child nodes.
    pub(crate) virtual_focus: Vec<FocusId>,
    /// An icon the box draws, tinted with the text colour, in the largest
    /// square that fits its content area.
    pub(crate) icon: Option<pm_icon::Svg>,
    /// A picture the box draws in its own colours, cropped to fill its content area.
    pub(crate) picture: Option<pm_renderer::Picture>,
    /// A ring the box draws in its content area: how much of it is filled, how thick it is, the colour of the filled part and the colour of the rest.
    pub(crate) arc: Option<(f32, f32, pm_style::Color, pm_style::Color)>,
    /// A blinking caret at this byte offset of the text, shown while the
    /// named box holds focus.
    pub(crate) caret: Option<(usize, FocusId)>,
    /// How its carets are drawn.
    pub(crate) caret_look: CaretLook,
    /// A highlighted byte range of this node's text, shown while the named
    /// box holds focus.
    pub(crate) selection: Option<(std::ops::Range<usize>, FocusId)>,
    /// A byte range of this node's text an input method is still composing,
    /// underlined while the named box holds focus.
    pub(crate) marked: Option<(std::ops::Range<usize>, FocusId)>,
    /// How far the contents are shifted left, in logical pixels, to scroll them.
    pub(crate) scroll_x: f32,
    /// How far the contents are shifted up, in logical pixels, to scroll them.
    pub(crate) scroll_y: f32,
    /// The text field this box shows one line of, and the byte offset of the
    /// text field at which the line starts, so presses and the caret can be
    /// placed in it.
    pub(crate) field_line: Option<(FocusId, usize)>,
    /// Complete wrapped byte ranges for fields whose offscreen lines have no child nodes.
    pub(crate) field_layout: Option<Arc<crate::FieldLayout>>,
    /// A box whose focus this one takes on: its focus styles apply while that box holds keyboard focus.
    pub(crate) focus_for: Option<FocusId>,
    /// Scrolls the contents from one end of them, and reports how far they
    /// can be scrolled under this name.
    pub(crate) scroll_anchor: Option<(FocusId, Anchor, f32)>,
    /// A subtree laid out before, which the box holds at its laid-out size
    /// instead of children.
    pub(crate) retained: Option<Arc<Retained>>,
    /// Whether the box is a group: the group-hover styles of the boxes inside
    /// it apply while the pointer is over it.
    pub(crate) group: bool,
    /// Words shown over the box once the pointer has rested on it for a moment.
    pub(crate) tooltip: Option<String>,
    /// Whether the box is moved, wherever it was laid out, to lie inside the window.
    pub(crate) keep_in_view: bool,
    /// The selectable text the lines inside the box are part of.
    pub(crate) selectable: Option<FocusId>,
    /// Stretches of the box's text painted apart from the rest.
    pub(crate) marks: Vec<Mark>,
}

/// How a stretch of a line of text is painted apart from the rest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Paint {
    /// Its letters in another colour.
    Ink,
    /// A box of colour behind it.
    Fill,
    /// A wavy line under it.
    Wave,
    /// A straight line under it.
    Underline,
    /// Shown as selected, as the selection of a field is.
    Selected,
    /// A blinking caret before its first byte, as the caret of a field is.
    Caret,
    /// A thin upright line the height of the line at the column its first number counts, in spaces from the left, which may lie past the end of the text.
    Guide,
    /// A steady upright line before its first byte, in its colour: where dragged text would be dropped.
    Drop,
}

/// A stretch of a line of text painted apart from the rest.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    /// The bytes of the text.
    pub range: std::ops::Range<usize>,
    /// The colour.
    pub color: pm_style::Color,
    /// How.
    pub paint: Paint,
}

/// The end of a box's contents that a scroll position is measured from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Anchor {
    /// The top: the position is how far down the contents are scrolled.
    Start,
    /// The bottom: the position is how far back up from the end they are.
    End,
}

/// An empty, unstyled box.
pub fn node() -> Node {
    Node::default()
}

/// A box holding one line of `content`; style it with typography utilities.
pub fn text(content: impl Into<String>) -> Node {
    Node {
        text: Some(Arc::from(content.into())),
        ..Node::default()
    }
}

impl Node {
    /// Preserves offscreen entries in keyboard traversal order without laying out their contents.
    pub fn virtual_focus(mut self, ids: Vec<FocusId>) -> Self {
        self.virtual_focus = ids;
        self
    }

    /// Makes the box draw `name` in the text colour; size the box to size the icon.
    pub fn icon(mut self, name: IconName) -> Self {
        self.icon = Some(name.svg());
        self
    }

    /// Makes the box draw the artwork `svg` in the text colour, as an icon is drawn.
    pub fn artwork(mut self, svg: pm_icon::Svg) -> Self {
        self.icon = Some(svg);
        self
    }

    /// Makes the box draw a ring `width` logical pixels thick, `share` of it, from nought to one, filled clockwise from the top in `color` and the rest in `track`.
    pub fn arc(
        mut self,
        share: f32,
        width: f32,
        color: impl Into<pm_style::Color>,
        track: impl Into<pm_style::Color>,
    ) -> Self {
        self.arc = Some((share.clamp(0.0, 1.0), width, color.into(), track.into()));
        self
    }

    /// Makes the box draw `picture` across its content area.
    pub fn picture(mut self, picture: pm_renderer::Picture) -> Self {
        self.picture = Some(picture);
        self
    }

    /// Applies utilities in order, after whatever was applied before.
    pub fn styles(mut self, utilities: impl IntoIterator<Item = Utility>) -> Self {
        self.style.extend(utilities);
        self
    }

    /// Appends one child.
    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    /// Appends every child of `children`.
    pub fn children(mut self, children: impl IntoIterator<Item = Node>) -> Self {
        self.children.extend(children);
        self
    }

    /// Names the box `id`: it can then be pressed, and takes keyboard focus in
    /// tree order.
    pub fn focusable(mut self, id: FocusId) -> Self {
        self.focus = Some(id);
        self
    }

    /// Makes the box a group: the boxes inside it take on their group-hover
    /// styles while the pointer is over it.
    pub fn group(mut self) -> Self {
        self.group = true;
        self
    }

    /// Shows `words` over the box, above everything else, once the pointer has rested on it for a moment.
    pub fn tooltip(mut self, words: impl Into<String>) -> Self {
        self.tooltip = Some(words.into());
        self
    }

    /// Moves the box, with what it holds, as far as it takes to lie inside the window when it was laid out partly outside it.
    pub fn keep_in_view(mut self) -> Self {
        self.keep_in_view = true;
        self
    }

    /// Makes the lines of text inside the box selectable with the pointer,
    /// as part of the text named `id`: every box marked with the same name
    /// is one text, its pieces counted in tree order, so a selection runs
    /// from one box into the next.
    pub fn selectable(mut self, id: FocusId) -> Self {
        self.selectable = Some(id);
        self
    }

    /// Takes away the name given by [`Node::focusable`]: the box stops
    /// answering to the user, for a control that is switched off.
    pub fn inert(mut self) -> Self {
        self.focus = None;
        self
    }

    /// Paints the bytes `range` of this node's text in `color` the way `paint` says, over or under the rest of the text.
    pub fn mark(
        mut self,
        range: std::ops::Range<usize>,
        color: impl Into<pm_style::Color>,
        paint: Paint,
    ) -> Self {
        self.marks.push(Mark {
            range,
            color: color.into(),
            paint,
        });
        self
    }

    /// Paints each stretch of `marks` apart from the rest of this node's text.
    pub fn marks(mut self, marks: impl IntoIterator<Item = Mark>) -> Self {
        self.marks.extend(marks);
        self
    }

    /// Shows a blinking caret at byte offset `byte` of this node's text while
    /// the box named `owner` holds focus.
    pub fn caret(mut self, byte: usize, owner: FocusId) -> Self {
        self.caret = Some((byte, owner));
        self
    }

    /// Draws the carets of this node's text as `look` says.
    pub fn caret_look(mut self, look: CaretLook) -> Self {
        self.caret_look = look;
        self
    }

    /// Highlights the byte range `range` of this node's text while the box
    /// named `owner` holds focus.
    pub fn selection(mut self, range: std::ops::Range<usize>, owner: FocusId) -> Self {
        self.selection = Some((range, owner));
        self
    }

    /// Underlines the byte range `range` of this node's text, which an input
    /// method is still composing, while the box named `owner` holds focus.
    pub fn marked(mut self, range: std::ops::Range<usize>, owner: FocusId) -> Self {
        self.marked = Some((range, owner));
        self
    }

    /// Marks the box as one line of the text field `owner` shows, the line
    /// that starts at byte `start` of the field's text.
    pub fn field_line(mut self, owner: FocusId, start: usize) -> Self {
        self.field_line = Some((owner, start));
        self
    }

    /// Gives a virtual input complete line metadata for lazy keyboard navigation.
    pub fn field_layout(mut self, layout: crate::FieldLayout) -> Self {
        self.field_layout = Some(Arc::new(layout));
        self
    }

    /// Makes the box take the focus styles of the box named `id` as well as
    /// its own: it looks focused while `id` holds keyboard focus.
    pub fn focused_with(mut self, id: FocusId) -> Self {
        self.focus_for = Some(id);
        self
    }

    /// Scrolls the contents `offset` logical pixels to the left; they are
    /// clipped to the box when its style says overflow is hidden.
    pub fn scroll_x(mut self, offset: f32) -> Self {
        self.scroll_x = offset;
        self
    }

    /// Scrolls the contents `offset` logical pixels upwards; they are
    /// clipped to the box when its style says overflow is hidden, and what is
    /// scrolled out of view cannot be pressed.
    pub fn scroll_y(mut self, offset: f32) -> Self {
        self.scroll_y = offset;
        self
    }

    /// Scrolls the contents to the bottom, then `back` logical pixels back up
    /// from it, kept within the contents. The frame reports how many pixels
    /// the contents can be scrolled back under the name `id`, which is not
    /// one that can be pressed.
    pub fn scroll_from_end(mut self, id: FocusId, back: f32) -> Self {
        self.scroll_anchor = Some((id, Anchor::End, back));
        self
    }

    /// Scrolls the contents `offset` logical pixels down from their start,
    /// kept within the contents. The frame reports how many pixels the
    /// contents can be scrolled under the name `id`, which is not one that
    /// can be pressed.
    pub fn scroll_from_start(mut self, id: FocusId, offset: f32) -> Self {
        self.scroll_anchor = Some((id, Anchor::Start, offset));
        self
    }

    /// How tall the subtree kept by [`crate::retained`] was laid out, when this node holds one.
    pub fn laid_height(&self) -> Option<f32> {
        self.retained
            .as_ref()
            .map(|retained| retained.placed.height)
    }

    /// The box this node and its children ask the layout for.
    pub(crate) fn item(&self, shaper: &mut Shaper) -> Item {
        let base = self.style.base();
        let intrinsic = match (&self.retained, self.text.as_deref()) {
            (Some(retained), _) => Some((retained.placed.width, retained.placed.height)),
            (None, Some(content)) => {
                let extent = shaper.measure(content, &base.typography);
                Some((extent.width, extent.height))
            }
            (None, None) => None,
        };
        Item {
            style: base.layout,
            border: base.decoration.borders.width,
            intrinsic,
            children: self
                .children
                .iter()
                .map(|child| child.item(shaper))
                .collect(),
        }
    }
}
