//! The icons a file tree draws: the disclosure chevron, folders and files.
//!
//! Drawn from the draw list's own quads rather than loaded from an icon font
//! or an SVG, because [`pm_gfx`] draws quads and glyphs and nothing else.
//! Every icon is built inside a square box of [`ICON_SIZE`], so a row lines up
//! whether the thing on it is a folder, a file or neither.

use pm_gfx::{Quad, Rect, Rgba, Size};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Style, Styled};

/// The side of the square box every tree icon is drawn inside.
pub const ICON_SIZE: f32 = 16.0;

/// How thick a drawn line is.
const STROKE: f32 = 1.5;

/// Which icon a tree row draws.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeIcon {
    /// A directory that is not showing what it holds.
    Collapsed,
    /// A directory that is showing what it holds.
    Expanded,
    /// A directory, whether or not it is open.
    Folder {
        /// Whether the folder is showing what it holds.
        open: bool,
    },
    /// A file.
    File,
    /// Nothing, holding the column open so names line up.
    Blank,
}

/// An icon drawn in `color`, inside a square of [`ICON_SIZE`].
pub struct TreeIconElement {
    /// Which icon to draw.
    icon: TreeIcon,
    /// The colour to draw it in.
    color: Rgba,
    /// How the box is sized.
    style: Style,
}

/// Builds `icon` drawn in `color`.
pub fn tree_icon(icon: TreeIcon, color: Rgba) -> TreeIconElement {
    TreeIconElement {
        icon,
        color,
        style: Style::default(),
    }
    .size_px(ICON_SIZE)
}

impl Styled for TreeIconElement {
    /// How the box is sized.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M> Element<M> for TreeIconElement {
    /// How the box is sized.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes the square the icon is drawn inside.
    fn measure(&mut self, _available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        Size::new(ICON_SIZE, ICON_SIZE)
    }

    /// Paints the icon centred in `bounds`.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let box_ = Rect::from_xywh(
            (bounds.left() + (bounds.size.width - ICON_SIZE) / 2.0).round(),
            (bounds.top() + (bounds.size.height - ICON_SIZE) / 2.0).round(),
            ICON_SIZE,
            ICON_SIZE,
        );

        match self.icon {
            TreeIcon::Collapsed => self.chevron(box_, false, cx),
            TreeIcon::Expanded => self.chevron(box_, true, cx),
            TreeIcon::Folder { open } => self.folder(box_, open, cx),
            TreeIcon::File => self.file(box_, cx),
            TreeIcon::Blank => {}
        }
    }
}

impl TreeIconElement {
    /// Draws the disclosure chevron, pointing down when `open`.
    ///
    /// Three steps of a staircase make the arm of a chevron out of the
    /// axis-aligned quads the draw list has; at this size the steps read as
    /// one stroke.
    fn chevron<M>(&self, box_: Rect, open: bool, cx: &mut PaintContext<'_, '_, M>) {
        let steps: [(f32, f32); 3] = if open {
            [(4.0, 6.0), (6.5, 8.5), (9.0, 6.0)]
        } else {
            [(6.0, 4.0), (8.5, 6.5), (6.0, 9.0)]
        };

        for (x, y) in steps {
            cx.quad(Quad::filled(
                Rect::from_xywh(box_.left() + x, box_.top() + y, 2.5, 2.5),
                self.color,
            ));
        }
    }

    /// Draws a folder, its lid lifted when `open`.
    fn folder<M>(&self, box_: Rect, open: bool, cx: &mut PaintContext<'_, '_, M>) {
        let tab = Rect::from_xywh(box_.left() + 1.0, box_.top() + 3.0, 6.0, 2.5);
        cx.quad(Quad::filled(tab, self.color).corner_radius(1.0));

        let body = Rect::from_xywh(box_.left() + 1.0, box_.top() + 4.5, 14.0, 8.5);
        let fill = if open {
            self.color.alpha(0.55)
        } else {
            self.color
        };
        cx.quad(Quad::filled(body, fill).corner_radius(1.5));
    }

    /// Draws a sheet with a folded corner and two lines of writing.
    fn file<M>(&self, box_: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let sheet = Rect::from_xywh(box_.left() + 3.0, box_.top() + 2.0, 10.0, 12.0);
        cx.quad(
            Quad::filled(sheet, Rgba::TRANSPARENT)
                .corner_radius(1.5)
                .border(STROKE * 0.7, self.color),
        );

        for offset in [4.0, 7.0] {
            cx.quad(Quad::filled(
                Rect::from_xywh(sheet.left() + 2.5, sheet.top() + offset, 5.0, 1.0),
                self.color,
            ));
        }
    }
}
