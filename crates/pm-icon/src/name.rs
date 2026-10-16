//! The names of the app's icons and the artwork each is drawn from.
//!
//! The artwork is from Phosphor Icons, used under the MIT licence kept in
//! `assets/LICENSE-phosphor`.

use crate::Svg;

/// Reads one icon's artwork out of the crate's own assets.
macro_rules! include_icon {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/",
            $name,
            ".svg"
        ))
    };
}

/// One piece of artwork, at whatever size it is asked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IconName {
    /// A branching line: version control.
    Git,
    /// An arrow pointing left: back.
    ArrowLeft,
    /// An arrow pointing right: forward.
    ArrowRight,
    /// An arrow pointing up.
    ArrowUp,
    /// An arrow pointing down.
    ArrowDown,
    /// A small chevron pointing down: expanded, or a menu.
    ChevronDown,
    /// A small chevron pointing right: collapsed, or deeper.
    ChevronRight,
    /// A magnifying glass: search.
    Search,
    /// A gear: settings.
    Settings,
    /// Horizontal sliders: view options.
    Split,
    /// A cross: remove or dismiss.
    Close,
    /// A push pin: keep in place.
    Pin,
    /// A plus: add.
    Plus,
    /// A minus: take away.
    Minus,
    /// A tick: done.
    Check,
    /// A tick in a circle: finished and healthy.
    CheckCircle,
    /// A timer: work in progress.
    Progress,
    /// A spark: an agent.
    Sparkles,
    /// Two sheets: copy.
    Copy,
    /// A window with a side panel.
    PanelRight,
    /// A square cut down the middle: panes.
    SquareSplit,
    /// A triangle pointing right: run.
    Play,
    /// Three dots: more actions.
    More,
    /// An at sign: mention.
    At,
    /// A sheet of paper: a file.
    File,
    /// Angle brackets: code.
    FileCode,
    /// A closed folder.
    Folder,
    /// An open folder.
    FolderOpen,
    /// Two branches with a change between them: a diff to review.
    GitCompare,
    /// A line forking in two.
    GitFork,
    /// A clock with a counter-clockwise arrow: history.
    History,
    /// Two arrows chasing each other: refresh.
    Refresh,
    /// An arrow turning back on itself: undo.
    Undo,
    /// A speech bubble: a chat message.
    Message,
    /// A person in a circle: an account.
    CircleUser,
    /// A plug: a connection.
    Plug,
    /// A lightning bolt: quick.
    Zap,
    /// The command key: commands.
    SquareSlash,
    /// A prompt in a window: the terminal.
    Terminal,
    /// A file with a plus: new file.
    FilePlus,
    /// A folder with a plus: new folder.
    FolderPlus,
    /// A bin: delete.
    Trash,
    /// A pencil: edit.
    Pencil,
    /// Arrows pressing together vertically: collapse.
    Collapse,
    /// Arrows pointing out: enlarge.
    Maximize,
    /// Arrows pointing in: shrink.
    Minimize,
    /// An open ring: loading.
    Loading,
    /// A square: stop what is running.
    Stop,
    /// A dot on a line: a commit.
    GitCommit,
    /// A cloud with an arrow down: fetch.
    GitFetch,
    /// An arrow down onto a line: pull.
    GitPull,
    /// An arrow up off a line: push.
    GitPush,
    /// Rings around a dot: the current place.
    Target,
}

impl IconName {
    /// The artwork this icon is drawn from.
    pub fn svg(self) -> Svg {
        match self {
            Self::Git => Svg::new("git", include_icon!("git")),
            Self::ArrowLeft => Svg::new("arrow_left", include_icon!("arrow_left")),
            Self::ArrowRight => Svg::new("arrow_right", include_icon!("arrow_right")),
            Self::ArrowUp => Svg::new("arrow_up", include_icon!("arrow_up")),
            Self::ArrowDown => Svg::new("arrow_down", include_icon!("arrow_down")),
            Self::ChevronDown => Svg::new("chevron_down", include_icon!("chevron_down")),
            Self::ChevronRight => Svg::new("chevron_right", include_icon!("chevron_right")),
            Self::Search => Svg::new("search", include_icon!("search")),
            Self::Settings => Svg::new("settings", include_icon!("settings")),
            Self::Split => Svg::new("split", include_icon!("split")),
            Self::Close => Svg::new("close", include_icon!("close")),
            Self::Pin => Svg::new("pin", include_icon!("pin")),
            Self::Plus => Svg::new("plus", include_icon!("plus")),
            Self::Minus => Svg::new("minus", include_icon!("minus")),
            Self::Check => Svg::new("check", include_icon!("check")),
            Self::CheckCircle => Svg::new("check_circle", include_icon!("check_circle")),
            Self::Progress => Svg::new("progress", include_icon!("progress")),
            Self::Sparkles => Svg::new("sparkles", include_icon!("sparkles")),
            Self::Copy => Svg::new("copy", include_icon!("copy")),
            Self::PanelRight => Svg::new("panel_right", include_icon!("panel_right")),
            Self::SquareSplit => Svg::new("square_split", include_icon!("square_split")),
            Self::Play => Svg::new("play", include_icon!("play")),
            Self::More => Svg::new("more", include_icon!("more")),
            Self::At => Svg::new("at", include_icon!("at")),
            Self::File => Svg::new("file", include_icon!("file")),
            Self::FileCode => Svg::new("file_code", include_icon!("file_code")),
            Self::Folder => Svg::new("folder", include_icon!("folder")),
            Self::FolderOpen => Svg::new("folder_open", include_icon!("folder_open")),
            Self::GitCompare => Svg::new("git_compare", include_icon!("git_compare")),
            Self::GitFork => Svg::new("git_fork", include_icon!("git_fork")),
            Self::History => Svg::new("history", include_icon!("history")),
            Self::Refresh => Svg::new("refresh", include_icon!("refresh")),
            Self::Undo => Svg::new("undo", include_icon!("undo")),
            Self::Message => Svg::new("message", include_icon!("message")),
            Self::CircleUser => Svg::new("circle_user", include_icon!("circle_user")),
            Self::Plug => Svg::new("plug", include_icon!("plug")),
            Self::Zap => Svg::new("zap", include_icon!("zap")),
            Self::SquareSlash => Svg::new("square_slash", include_icon!("square_slash")),
            Self::Terminal => Svg::new("terminal", include_icon!("terminal")),
            Self::FilePlus => Svg::new("file_plus", include_icon!("file_plus")),
            Self::FolderPlus => Svg::new("folder_plus", include_icon!("folder_plus")),
            Self::Trash => Svg::new("trash", include_icon!("trash")),
            Self::Pencil => Svg::new("pencil", include_icon!("pencil")),
            Self::Collapse => Svg::new("collapse", include_icon!("collapse")),
            Self::Maximize => Svg::new("maximize", include_icon!("maximize")),
            Self::Minimize => Svg::new("minimize", include_icon!("minimize")),
            Self::Loading => Svg::new("loading", include_icon!("loading")),
            Self::Stop => Svg::new("stop", include_icon!("stop")),
            Self::GitCommit => Svg::new("git_commit", include_icon!("git_commit")),
            Self::GitFetch => Svg::new("git_fetch", include_icon!("git_fetch")),
            Self::GitPull => Svg::new("git_pull", include_icon!("git_pull")),
            Self::GitPush => Svg::new("git_push", include_icon!("git_push")),
            Self::Target => Svg::new("target", include_icon!("target")),
        }
    }
}
