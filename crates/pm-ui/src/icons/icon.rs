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
    /// Zed's loading circle, at one eighth-turn of its animation.
    LoadCircle,
    /// A directory that is not showing what it holds.
    ChevronRight,
    /// A directory that is showing what it holds.
    ChevronDown,
    /// The way back up through what a pane is showing.
    ChevronUp,
    /// The way down to what follows.
    ArrowDown,
    /// Go to what the control is beside.
    ArrowRight,
    /// The way up to what came before.
    ArrowUp,
    /// All of what the control is on.
    Check,
    /// Close what the control is on.
    Close,
    /// Open the remaining actions for the control.
    More,
    /// A file.
    File,
    /// The branch a worktree is on.
    GitBranch,
    /// A commit.
    GitCommit,
    /// One side of a change against the other.
    GitCompare,
    /// A worktree branched off another.
    GitFork,
    /// Fetch updates from a remote repository.
    GitFetch,
    /// Pull remote commits into the current branch.
    GitPull,
    /// Push local commits to a remote repository.
    GitPush,
    /// The current target in a history graph.
    Target,
    /// A directory.
    Folder,
    /// A directory that is open.
    FolderOpen,
    /// Take away one of whatever the control is beside.
    Minus,
    /// Add another of whatever the control is beside.
    Plus,
    /// Read again whatever the control is beside.
    Refresh,
    /// Put back the way it was whatever the control is beside.
    Undo,
    /// Keep what the control is on where it is.
    Pin,
    /// The same pin, filled, for what is being kept already.
    PinFilled,
    /// An agent, and what it says.
    Sparkle,
    /// Divide a pane.
    Split,
    /// The preferences, as a gear.
    Settings,
    /// A terminal.
    Terminal,
    /// Something the editor wants looked at.
    Warning,
}

impl IconName {
    /// The artwork this icon is drawn from.
    pub fn svg(self) -> Svg {
        match self {
            Self::LoadCircle => Svg::new("load_circle", include_icon!("load_circle")),
            Self::ChevronRight => Svg::new("chevron_right", include_icon!("chevron_right")),
            Self::ChevronDown => Svg::new("chevron_down", include_icon!("chevron_down")),
            Self::ChevronUp => Svg::new("chevron_up", include_icon!("chevron_up")),
            Self::ArrowDown => Svg::new("arrow_down", include_icon!("arrow_down")),
            Self::ArrowRight => Svg::new("arrow_right", include_icon!("arrow_right")),
            Self::ArrowUp => Svg::new("arrow_up", include_icon!("arrow_up")),
            Self::Check => Svg::new("check", include_icon!("check")),
            Self::Close => Svg::new("close", include_icon!("close")),
            Self::More => Svg::new("more", include_icon!("more")),
            Self::File => Svg::new("file", include_icon!("file")),
            Self::GitBranch => Svg::new("git_branch", include_icon!("git_branch")),
            Self::GitCommit => Svg::new("git_commit", include_icon!("git_commit")),
            Self::GitCompare => Svg::new("git_compare", include_icon!("git_compare")),
            Self::GitFork => Svg::new("git_fork", include_icon!("git_fork")),
            Self::GitFetch => Svg::new("git_fetch", include_icon!("git_fetch")),
            Self::GitPull => Svg::new("git_pull", include_icon!("git_pull")),
            Self::GitPush => Svg::new("git_push", include_icon!("git_push")),
            Self::Target => Svg::new("target", include_icon!("target")),
            Self::Folder => Svg::new("folder", include_icon!("folder")),
            Self::FolderOpen => Svg::new("folder_open", include_icon!("folder_open")),
            Self::Minus => Svg::new("minus", include_icon!("minus")),
            Self::Plus => Svg::new("plus", include_icon!("plus")),
            Self::Refresh => Svg::new("refresh", include_icon!("refresh")),
            Self::Undo => Svg::new("undo", include_icon!("undo")),
            Self::Pin => Svg::new("pin", include_icon!("pin")),
            Self::PinFilled => Svg::new("pin_filled", include_icon!("pin_filled")),
            Self::Sparkle => Svg::new("sparkle", include_icon!("sparkle")),
            Self::Split => Svg::new("split", include_icon!("split")),
            Self::Settings => Svg::new("settings", include_icon!("settings")),
            Self::Terminal => Svg::new("terminal", include_icon!("terminal")),
            Self::Warning => Svg::new("warning", include_icon!("warning")),
        }
    }
}

/// How large an icon is drawn, in the steps the editor uses.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IconSize {
    /// 12px: inside a tab or a row of text.
    ///
    /// The artwork is drawn on a grid of sixteen, so anything below
    /// [`IconSize::Medium`] rasterizes its strokes across two pixels rather
    /// than into one: a size for an icon that has to fit, not one to reach
    /// for because the icon is small.
    XSmall,
    /// 14px: a smaller icon, where one at its own size would crowd its row.
    #[default]
    Small,
    /// 16px: the artwork's own size, which is the size it is sharpest at.
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
    /// Clockwise rotation around the icon's centre, in radians.
    rotation: f32,
}

/// An icon of `name` at the default size, in the theme's subtle text colour.
pub fn icon(name: IconName) -> Icon {
    Icon {
        name,
        size: IconSize::default(),
        color: None,
        style: Style::default(),
        rotation: 0.0,
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

    /// Returns this icon rotated clockwise by `radians` around its centre.
    pub fn rotate(mut self, radians: f32) -> Self {
        self.rotation = radians;
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
        cx.rotated_icon(box_, self.name.svg(), color, self.rotation);
    }
}
