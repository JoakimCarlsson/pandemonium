//! The icons the editor draws, and the element that places one.
//!
//! Each icon is one piece of vector artwork shipped with the editor, drawn at
//! whatever size the caller asks for and tinted from the theme. Artwork is
//! never coloured in the file: an icon in a disabled row and the same icon in
//! a selected one are one drawing, tinted twice.

use pm_gfx::{Rect, Rgba, Size, Svg};

use crate::element::{Element, LayoutContext, PaintContext};
use crate::style::{Style, Styled};

/// Reads one icon's artwork out of the editor's own assets.
macro_rules! include_icon {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/icons/",
            $name,
            ".svg"
        ))
    };
}

/// One piece of artwork, at whatever size it is asked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IconName {
    /// A directory that is not showing what it holds.
    ChevronRight,
    /// A directory that is showing what it holds.
    ChevronDown,
    /// Close what the control is on.
    Close,
    /// A file.
    File,
    /// The branch a worktree is on.
    GitBranch,
    /// A worktree branched off another.
    GitFork,
    /// A directory.
    Folder,
    /// A directory that is open.
    FolderOpen,
    /// Add another of whatever the control is beside.
    Plus,
    /// Divide a pane.
    Split,
    /// A terminal.
    Terminal,
    /// Something the editor wants looked at.
    Warning,
}

impl IconName {
    /// The artwork this icon is drawn from.
    pub fn svg(self) -> Svg {
        match self {
            Self::ChevronRight => Svg::new("chevron_right", include_icon!("chevron_right")),
            Self::ChevronDown => Svg::new("chevron_down", include_icon!("chevron_down")),
            Self::Close => Svg::new("close", include_icon!("close")),
            Self::File => Svg::new("file", include_icon!("file")),
            Self::GitBranch => Svg::new("git_branch", include_icon!("git_branch")),
            Self::GitFork => Svg::new("git_fork", include_icon!("git_fork")),
            Self::Folder => Svg::new("folder", include_icon!("folder")),
            Self::FolderOpen => Svg::new("folder_open", include_icon!("folder_open")),
            Self::Plus => Svg::new("plus", include_icon!("plus")),
            Self::Split => Svg::new("split", include_icon!("split")),
            Self::Terminal => Svg::new("terminal", include_icon!("terminal")),
            Self::Warning => Svg::new("warning", include_icon!("warning")),
        }
    }
}

/// How large an icon is drawn, in the steps the editor uses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IconSize {
    /// 12px: inside a tab or a row of text.
    XSmall,
    /// 14px: the size a control carries.
    #[default]
    Small,
    /// 16px: the artwork's own size, for a standalone icon.
    Medium,
}

impl IconSize {
    /// The side of the square this size draws inside.
    pub const fn pixels(self) -> f32 {
        match self {
            Self::XSmall => 12.0,
            Self::Small => 14.0,
            Self::Medium => 16.0,
        }
    }
}

/// One icon, sized from the icon scale and coloured from the theme.
pub struct Icon {
    /// Which artwork to draw.
    name: IconName,
    /// How large to draw it.
    size: IconSize,
    /// The colour to tint it, or the theme's quietest text colour when unset.
    color: Option<Rgba>,
    /// How the box is sized.
    style: Style,
}

/// An icon of `name` at the default size, in the theme's subtle text colour.
pub fn icon(name: IconName) -> Icon {
    Icon {
        name,
        size: IconSize::default(),
        color: None,
        style: Style::default(),
    }
}

impl Icon {
    /// Returns this icon drawn at `size`.
    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    /// Returns this icon tinted `color`.
    pub fn color(mut self, color: Rgba) -> Self {
        self.color = Some(color);
        self
    }

    /// The square this icon occupies.
    fn box_(&self, bounds: Rect) -> Rect {
        let side = self.size.pixels();
        Rect::from_xywh(
            (bounds.left() + (bounds.size.width - side) / 2.0).round(),
            (bounds.top() + (bounds.size.height - side) / 2.0).round(),
            side,
            side,
        )
    }
}

impl Styled for Icon {
    /// How the box is sized.
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl<M> Element<M> for Icon {
    /// How the box is sized.
    fn layout_style(&self) -> Style {
        self.style
    }

    /// Takes the square the artwork is drawn inside.
    fn measure(&mut self, _available: Size, _cx: &mut LayoutContext<'_>) -> Size {
        let side = self.size.pixels();
        Size::new(side, side)
    }

    /// Paints the artwork centred in `bounds`.
    fn paint(&mut self, bounds: Rect, cx: &mut PaintContext<'_, '_, M>) {
        let color = self.color.unwrap_or(cx.theme().colors.text_subtle);
        let box_ = self.box_(bounds);
        cx.icon(box_, self.name.svg(), color);
    }
}
