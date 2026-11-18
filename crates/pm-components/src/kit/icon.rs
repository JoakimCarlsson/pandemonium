//! Icons: the app's named icons, drawn from the Phosphor paths rustle
//! embeds and tinted with a tone like text, and artwork that arrives while
//! the app runs, rasterised into a coverage mask.
//!
//! Every [`IconName`] is a Phosphor drawing: the regular weight, or the
//! filled weight (rustle's `icons-fill` feature) for the variants that show
//! a selected state. The panels on the right are Phosphor's left sidebar
//! drawn mirrored. Only artwork that arrives at run time, such as an
//! extension's icon or a file icon ([`Artwork::arrived`]), goes through
//! resvg.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};

use rustle::{ImageData, Modifier, Path, PathCommand, phosphor};

use crate::kit::theme::{self, Tone, sp};

/// One of the app's icons, at whatever size it is asked for.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IconName {
    /// A branching line: version control.
    Git,
    /// A branching line with filled endpoints: selected version control.
    GitFilled,
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
    /// A filled magnifying glass: selected search.
    SearchFilled,
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
    /// A window with a panel along its left side.
    PanelLeft,
    /// A window with its left panel filled to show it is open.
    PanelLeftFilled,
    /// A window with a panel along its right side.
    PanelRight,
    /// A window with its right panel filled to show it is open.
    PanelRightFilled,
    /// A window with a panel along its bottom.
    PanelBottom,
    /// A window with its bottom panel filled to show it is open.
    PanelBottomFilled,
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
    /// A filled folder: selected files.
    FolderFilled,
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
    /// A cursor inside a dashed box: pick an element on a page.
    Pick,
    /// A globe: the web.
    Globe,
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
    /// A beetle: debugging.
    Bug,
    /// A triangle against a bar: run the paused program on.
    Continue,
    /// Two bars: pause the running program.
    Pause,
    /// An arrow down onto a line: step into a call.
    StepInto,
    /// An arrow up off a line: step out of a call.
    StepOut,
    /// An arc over a dot: step over a line.
    StepOver,
    /// A bulb: the language server offers changes here.
    Lightbulb,
    /// A camera: a screenshot.
    Camera,
    /// The corners of a frame: an area of a page.
    Scan,
    /// A closed padlock: locked.
    Lock,
    /// An open padlock: not locked.
    LockOpen,
}

impl IconName {
    /// The Phosphor outline this icon is drawn with.
    ///
    /// Port `name.svg()` to `name.path()`.
    pub fn path(self) -> Path {
        match self {
            Self::Git => phosphor::GIT_BRANCH,
            Self::GitFilled => phosphor::fill::GIT_BRANCH,
            Self::ArrowLeft => phosphor::ARROW_LEFT,
            Self::ArrowRight => phosphor::ARROW_RIGHT,
            Self::ArrowUp => phosphor::ARROW_UP,
            Self::ArrowDown => phosphor::ARROW_DOWN,
            Self::ChevronDown => phosphor::CARET_DOWN,
            Self::ChevronRight => phosphor::CARET_RIGHT,
            Self::Search => phosphor::MAGNIFYING_GLASS,
            Self::SearchFilled => phosphor::fill::MAGNIFYING_GLASS,
            Self::Settings => phosphor::GEAR,
            Self::Split => phosphor::SLIDERS_HORIZONTAL,
            Self::Close => phosphor::X,
            Self::Pin => phosphor::PUSH_PIN,
            Self::Plus => phosphor::PLUS,
            Self::Minus => phosphor::MINUS,
            Self::Check => phosphor::CHECK,
            Self::CheckCircle => phosphor::CHECK_CIRCLE,
            Self::Progress => phosphor::TIMER,
            Self::Sparkles => phosphor::SPARKLE,
            Self::Copy => phosphor::COPY,
            Self::PanelLeft => phosphor::SIDEBAR_SIMPLE,
            Self::PanelLeftFilled => phosphor::fill::SIDEBAR_SIMPLE,
            Self::PanelRight => MIRRORED_SIDEBAR.clone(),
            Self::PanelRightFilled => MIRRORED_SIDEBAR_FILLED.clone(),
            Self::PanelBottom => UPENDED_BROWSER.clone(),
            Self::PanelBottomFilled => phosphor::fill::SQUARE_HALF_BOTTOM,
            Self::SquareSplit => phosphor::SQUARE_SPLIT_HORIZONTAL,
            Self::Play => phosphor::PLAY,
            Self::More => phosphor::DOTS_THREE,
            Self::At => phosphor::AT,
            Self::File => phosphor::FILE,
            Self::FileCode => phosphor::CODE,
            Self::Folder => phosphor::FOLDER,
            Self::FolderFilled => phosphor::fill::FOLDER,
            Self::FolderOpen => phosphor::FOLDER_OPEN,
            Self::GitCompare => phosphor::GIT_DIFF,
            Self::GitFork => phosphor::GIT_FORK,
            Self::History => phosphor::CLOCK_COUNTER_CLOCKWISE,
            Self::Refresh => phosphor::ARROWS_CLOCKWISE,
            Self::Pick => phosphor::SELECTION,
            Self::Globe => phosphor::GLOBE,
            Self::Undo => phosphor::ARROW_COUNTER_CLOCKWISE,
            Self::Message => phosphor::CHAT_CIRCLE,
            Self::CircleUser => phosphor::USER_CIRCLE,
            Self::Plug => phosphor::PLUG,
            Self::Zap => phosphor::LIGHTNING,
            Self::SquareSlash => phosphor::COMMAND,
            Self::Terminal => phosphor::TERMINAL,
            Self::FilePlus => phosphor::FILE_PLUS,
            Self::FolderPlus => phosphor::FOLDER_PLUS,
            Self::Trash => phosphor::TRASH,
            Self::Pencil => phosphor::PENCIL_SIMPLE,
            Self::Collapse => phosphor::ARROWS_IN_LINE_VERTICAL,
            Self::Maximize => phosphor::ARROWS_OUT_SIMPLE,
            Self::Minimize => phosphor::ARROWS_IN_SIMPLE,
            Self::Loading => phosphor::CIRCLE_DASHED,
            Self::Stop => phosphor::STOP,
            Self::GitCommit => phosphor::GIT_COMMIT,
            Self::GitFetch => phosphor::CLOUD_ARROW_DOWN,
            Self::GitPull => phosphor::ARROW_LINE_DOWN,
            Self::GitPush => phosphor::ARROW_LINE_UP,
            Self::Target => phosphor::TARGET,
            Self::Bug => phosphor::BUG,
            Self::Continue => phosphor::SKIP_FORWARD,
            Self::Pause => phosphor::PAUSE,
            Self::StepInto => phosphor::ARROW_LINE_DOWN,
            Self::StepOut => phosphor::ARROW_LINE_UP,
            Self::StepOver => phosphor::ARROW_ARC_RIGHT,
            Self::Lightbulb => phosphor::LIGHTBULB,
            Self::Camera => phosphor::CAMERA,
            Self::Scan => phosphor::FRAME_CORNERS,
            Self::Lock => phosphor::LOCK_SIMPLE,
            Self::LockOpen => phosphor::LOCK_SIMPLE_OPEN,
        }
    }
}

/// Phosphor's regular sidebar mirrored left to right, made once.
static MIRRORED_SIDEBAR: LazyLock<Path> = LazyLock::new(|| flipped(&phosphor::SIDEBAR_SIMPLE));

/// Phosphor's browser window turned upside down, made once: a window with a
/// band along its bottom, the panel at the bottom of the screen.
static UPENDED_BROWSER: LazyLock<Path> = LazyLock::new(|| upended(&phosphor::BROWSER));

/// Phosphor's filled sidebar mirrored left to right, made once.
static MIRRORED_SIDEBAR_FILLED: LazyLock<Path> =
    LazyLock::new(|| flipped(&phosphor::fill::SIDEBAR_SIMPLE));

/// `path` mirrored left to right, kept as constant data so renderers cache it.
fn flipped(path: &Path) -> Path {
    let (width, _) = path.size();
    transformed(path, |x, y| (width - x, y))
}

/// `path` mirrored top to bottom, kept as constant data so renderers cache it.
fn upended(path: &Path) -> Path {
    let (_, height) = path.size();
    transformed(path, |x, y| (x, height - y))
}

/// `path` with every point moved by `point`, kept as constant data so renderers cache it.
fn transformed(path: &Path, point: impl Fn(f32, f32) -> (f32, f32)) -> Path {
    let (width, height) = path.size();
    let commands: Vec<PathCommand> = path
        .commands()
        .iter()
        .map(|command| match *command {
            PathCommand::MoveTo(x, y) => {
                let (x, y) = point(x, y);
                PathCommand::MoveTo(x, y)
            }
            PathCommand::LineTo(x, y) => {
                let (x, y) = point(x, y);
                PathCommand::LineTo(x, y)
            }
            PathCommand::QuadTo(cx, cy, x, y) => {
                let ((cx, cy), (x, y)) = (point(cx, cy), point(x, y));
                PathCommand::QuadTo(cx, cy, x, y)
            }
            PathCommand::CubicTo(ax, ay, bx, by, x, y) => {
                let ((ax, ay), (bx, by), (x, y)) = (point(ax, ay), point(bx, by), point(x, y));
                PathCommand::CubicTo(ax, ay, bx, by, x, y)
            }
            PathCommand::Close => PathCommand::Close,
        })
        .collect();
    Path::from_static(
        Box::leak(commands.into_boxed_slice()),
        width,
        height,
        path.fill_rule(),
    )
}

/// A piece of vector artwork, named so its rasterised masks can be kept.
///
/// The name is the identity: two pieces with the same name are the same
/// artwork, and the source is only read the first time one is rasterised.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Artwork {
    /// What it is called, unique across the artwork the app draws.
    pub name: &'static str,
    /// The SVG document it is drawn from.
    pub source: &'static str,
}

/// The artwork that arrived while the app runs, by name, each kept for as long as the app runs.
static ARRIVED: LazyLock<Mutex<HashMap<String, Artwork>>> = LazyLock::new(Default::default);

thread_local! {
    /// Masks rasterised so far, by artwork name and size in physical pixels.
    static MASKS: RefCell<HashMap<(&'static str, u32), Option<ImageData>>> = RefCell::new(HashMap::new());
    /// Physical pixels per logical pixel of the window being drawn, which masks are rasterised at.
    static PIXEL_SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

impl Artwork {
    /// The artwork `source` that arrived while the app runs, such as the icon
    /// of an extension, called `name`; artwork of one name is kept once, the
    /// first source given for it standing for every later one.
    pub fn arrived(name: &str, source: &str) -> Self {
        let mut held = ARRIVED.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(artwork) = held.get(name) {
            return *artwork;
        }
        let artwork = Self {
            name: Box::leak(name.to_owned().into_boxed_str()),
            source: Box::leak(source.to_owned().into_boxed_str()),
        };
        held.insert(name.to_owned(), artwork);
        artwork
    }

    /// The artwork's coverage in a square `pixels` physical pixels on a side,
    /// kept once made, or nothing when the size is zero or the source cannot be drawn.
    pub fn mask(&self, pixels: u32) -> Option<ImageData> {
        MASKS.with(|masks| {
            masks
                .borrow_mut()
                .entry((self.name, pixels))
                .or_insert_with(|| rasterise(self.source, pixels))
                .clone()
        })
    }
}

/// `source` drawn into a square `pixels` on a side, as coverage, or nothing when it cannot be drawn.
fn rasterise(source: &str, pixels: u32) -> Option<ImageData> {
    let tree = resvg::usvg::Tree::from_str(source, &resvg::usvg::Options::default()).ok()?;
    let mut canvas = resvg::tiny_skia::Pixmap::new(pixels, pixels)?;
    let scale = pixels as f32 / tree.size().width().max(tree.size().height());
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut canvas.as_mut(),
    );
    let coverage: Vec<u8> = canvas.pixels().iter().map(|pixel| pixel.alpha()).collect();
    ImageData::from_alpha(pixels, pixels, &coverage)
}

/// Rasterises artwork for a window drawn at `scale` physical pixels per logical pixel, so masks stay sharp.
pub fn set_pixel_scale(scale: f32) {
    PIXEL_SCALE.with(|held| held.set(scale.max(0.5)));
}

/// Emits `name` in a square `steps` of the spacing scale on a side, tinted
/// with `tone`: `glyph(name, steps, tone)`, or a box sized `size(steps)` with `.icon(name)`.
#[track_caller]
pub fn icon(name: IconName, steps: f32, tone: impl Into<Tone>) {
    icon_with(Modifier::new(), name, sp(steps), tone);
}

/// Emits `name` in a square `pixels` logical pixels on a side, tinted with `tone`, with `modifier` around it.
#[track_caller]
pub fn icon_with(modifier: Modifier, name: IconName, pixels: f32, tone: impl Into<Tone>) {
    path_with(modifier, name.path(), pixels, tone);
}

/// Emits the vector outline `path`, such as `phosphor::X`, in a square
/// `pixels` logical pixels on a side, tinted with `tone`, with `modifier` around it.
#[track_caller]
pub fn path_with(modifier: Modifier, path: Path, pixels: f32, tone: impl Into<Tone>) {
    let color = theme::color(tone);
    rustle::canvas(modifier.size(pixels, pixels), move |canvas| {
        let side = canvas.width().min(canvas.height());
        let (x, y) = (
            (canvas.width() - side) / 2.0,
            (canvas.height() - side) / 2.0,
        );
        canvas.path(&path, x, y, side, side, color);
    });
}

/// Emits `art` in a square `pixels` logical pixels on a side, tinted with
/// `tone`, with `modifier` around it: `.artwork(svg)`.
#[track_caller]
pub fn artwork_with(modifier: Modifier, art: Artwork, pixels: f32, tone: impl Into<Tone>) {
    let color = theme::color(tone);
    let scale = PIXEL_SCALE.with(std::cell::Cell::get);
    let physical = (pixels * scale).round().max(1.0) as u32;
    match art.mask(physical) {
        Some(mask) => rustle::mask_image(modifier.size(pixels, pixels), &mask, color),
        None => rustle::spacer(modifier.size(pixels, pixels)),
    }
}
