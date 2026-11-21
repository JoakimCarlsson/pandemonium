//! What the user does to the controls the views draw, handed to the app as event lambdas.
//!
//! A control takes what clicking it does as a lambda, the way a composable takes its
//! `onClick`: [`Interactive::control`] names the control by its [`Target`] and runs its
//! lambda when it is clicked, or activated with Enter or Space while it holds the keyboard.
//! The lambdas call the app's state holder, the [`Operations`] the views are composed
//! under, provided once at the root with [`provide`]: [`Operations::send`] hands the
//! workspace's reducer, [`WorkspaceState::handle`], an [`Event`] (most controls send
//! an area event such as [`Event::Sidebar`] with [`send`]), [`Operations::update`] runs any other change of the
//! workspace, and the named operations do what only the app can, such as opening a menu
//! where the pointer is, moving the window or talking to the servers. The app does what
//! every event asks afterwards (saving, sending, the focus a reducer asked for) in one
//! place.
//!
//! Each control handles its own pointer gestures with rustle's modifiers and tells the app
//! what happened through a typed operation carrying what the app needs (see
//! [`crate::pointing`]): the press that went down on a control and where
//! ([`Operations::pressed`]), its click ([`Operations::click_lands`]), its secondary press
//! ([`Operations::context_pressed`]), a press nothing above took ([`background`]), a drag
//! of it ([`Operations::carry_started`]) and what the text, the shell and the web pages
//! did with the pointer. The app never sees the window's raw pointer events. Keyboard
//! focus is rustle's, held for each control in the registry of [`crate::focus`].

use crate::areas::ChatEvent;
use crate::pointing::{BrowserPointer, CodePointer, Over, Press, TerminalPointer};
use crate::target::Target;
use crate::tree_actions::TreeTarget;
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use rustle::{
    CompositionLocal, DragAndDropEvent, DragAndDropTarget, DragAndDropTransferData, DragData, Key,
    KeyEvent, LOCAL_VIEW_CONFIGURATION, Modifier, Modifiers, Offset, PointerButton, PointerEvent,
    PointerPhase, Rect, ResizeEdge,
};

use crate::browser_menu::BrowserCommand;
use crate::focus;
use crate::git_controls::Pick;
use crate::git_menus::MenuKind;
use crate::state::WorkspaceState;
use crate::terminal_menu::TerminalCommand;

/// Something the reader did to a control of the window, as the workspace's reducer,
/// [`WorkspaceState::handle`], takes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A control of the sidebar was clicked, or activated with Enter or Space.
    Sidebar(crate::areas::SidebarEvent),
    /// A control of the conversation was clicked, or activated with Enter or Space.
    Chat(crate::areas::ChatEvent),
    /// A control of the prompt box and its chips was clicked, or activated with Enter or Space.
    Composer(crate::areas::ComposerEvent),
    /// A control of the review of the changes was clicked, or activated with Enter or Space.
    Diff(crate::areas::DiffEvent),
    /// A control of the project's files was clicked, or activated with Enter or Space.
    Tree(crate::areas::TreeEvent),
    /// A control of the file shown was clicked, or activated with Enter or Space.
    Editor(crate::areas::EditorEvent),
    /// A control of the project search, the excerpts and the conflict resolver was clicked, or activated with Enter or Space.
    Search(crate::areas::SearchEvent),
    /// A control of source control was clicked, or activated with Enter or Space.
    Git(crate::areas::GitEvent),
    /// A control of the dock's shells and problems was clicked, or activated with Enter or Space.
    Terminal(crate::areas::TerminalEvent),
    /// A control of the debug tab was clicked, or activated with Enter or Space.
    Debug(crate::areas::DebugEvent),
    /// A control of the browser pane was clicked, or activated with Enter or Space.
    Browser(crate::areas::BrowserEvent),
    /// A control of the settings window was clicked, or activated with Enter or Space.
    Settings(crate::areas::SettingsEvent),
    /// A control of an open menu was clicked, or activated with Enter or Space.
    Menu(crate::areas::MenuEvent),
    /// A control of a dialog was clicked, or activated with Enter or Space.
    Dialog(crate::areas::DialogEvent),
    /// A control of the window's frame and panes was clicked, or activated with Enter or Space.
    Window(crate::areas::WindowEvent),
    /// Something was done to a text field.
    Field(FieldEvent),
}

/// What was done to a text field, named by its [`Target`].
#[derive(Clone, Debug, PartialEq)]
pub enum FieldEvent {
    /// The reader changed the text of the field through its text field.
    Changed(Target),
    /// The reader typed `text` into the field over its selection.
    Typed {
        /// The field typed in.
        field: Target,
        /// What was typed.
        text: String,
    },
    /// The reader left the field.
    Left(Target),
    /// `edit` was made to the code editor's field.
    Edited {
        /// The field edited.
        field: Target,
        /// The change made to it.
        edit: pm_editor::Edit,
    },
}

/// A change of the workspace: the state it was, made into the state it is after the change.
pub type Change = Box<dyn FnOnce(WorkspaceState) -> WorkspaceState>;

/// The app's state holder as the views see it: the operations the controls' lambdas call,
/// and what the controls' own gestures tell it. Positions are logical pixels from the
/// window's top left. Every operation but [`update`](Self::update) does nothing, or
/// declines, unless the holder overrides it.
pub trait Operations {
    /// Changes the workspace with `change`, then does what the change asks of the app.
    fn update(&self, change: Change);
    /// Hands `event` to the workspace's reducer, [`WorkspaceState::handle`], then does what
    /// it asks of the app.
    fn send(&self, event: Event) {
        self.update(Box::new(move |state| state.handle(event)));
    }
    /// The primary button went down on a control, as `press` tells: the app follows the
    /// press (what a press elsewhere closes closes, a list row is held for the keys that
    /// pick every row, a menu opened from the control hangs from it).
    fn pressed(&self, _press: Press) {}
    /// The primary button came up on the control it went down on, as `click` tells.
    /// Returns whether the click counts, which it does not after the press dragged something
    /// or was taken by the control's own gesture; a click that counts gives the keyboard to
    /// the field or page it is on.
    fn click_lands(&self, _click: Press) -> bool {
        false
    }
    /// The secondary button went down on a control that has a menu of its own, as `press`
    /// tells, before its menu opens.
    fn context_pressed(&self, _press: Press) {}
    /// The primary button went down at `at` where no control took it.
    fn pressed_elsewhere(&self, _at: (f32, f32)) {}
    /// The mouse's back button went down where nothing else took it, or its forward button
    /// when not `back`.
    fn history_pressed(&self, _back: bool) {}
    /// The primary button did what `pointer` tells on the code text of a field.
    fn code_pointer(&self, _pointer: CodePointer) {}
    /// The pointer is over the drawn character at this byte of the file shown, or over none
    /// of them, or left the file.
    fn code_hovered(&self, _byte: Option<usize>) {}
    /// The pointer came over the card about a word of the file shown, or left it when not `over`.
    fn hover_card_hovered(&self, _over: bool) {}
    /// The primary button went down on the card about a word of the file shown, or came up
    /// again when not `held`.
    fn hover_card_held(&self, _held: bool) {}
    /// The primary button did what `pointer` tells on the window of the shell.
    fn terminal_pointer(&self, _pointer: TerminalPointer) {}
    /// The pointer is over this cell of the window of the shell, by row and column, or left it.
    fn terminal_hovered(&self, _cell: Option<(usize, usize)>) {}
    /// The wheel turned over the window of the shell, `down` logical pixels downwards.
    fn terminal_wheel(&self, _down: f32) {}
    /// The pointer did what `pointer` tells over a web page or its tools.
    fn browser_pointer(&self, _pointer: BrowserPointer) {}
    /// The primary button went down inside the conversation, as the conversation hears
    /// before the press reaches what it landed on.
    fn conversation_pressed(&self) {}
    /// A key went down or up in the window, before whatever holds rustle's focus sees it.
    /// Returns whether the holder took it, in which case nothing else in the window does.
    fn key(&self, _event: &KeyEvent) -> bool {
        false
    }
    /// The input method is composing `text` for the area `id` that takes text the holder
    /// handles itself, or stopped composing when `text` is empty.
    fn composing(&self, id: Target, text: String) {
        let _ = (id, text);
    }
    /// The input method committed `text` for the area `id` that takes text the holder
    /// handles itself.
    fn committed(&self, id: Target, text: String) {
        let _ = (id, text);
    }
    /// Where the input method's candidates go for the area `id` that takes text the holder
    /// handles itself, in logical pixels from the window's top left, when the holder knows
    /// better than the area.
    fn caret_area(&self, id: Target) -> Option<Rect> {
        let _ = id;
        None
    }
    /// Files from another application are held over the window at `at`, over the row
    /// `over` of the project's files when they are over one, or left it, or were let go
    /// elsewhere, when `at` is `None`.
    fn files_held(&self, _at: Option<(f32, f32)>, _over: Option<TreeTarget>) {}
    /// Files from another application, at `paths`, were let go over the window at `at`,
    /// over the row `over` of the project's files when they were let go on one.
    fn files_dropped(&self, _paths: Vec<PathBuf>, _at: (f32, f32), _over: Option<TreeTarget>) {}
    /// The handle `id` on the edge of a panel is dragged `by` logical pixels across and
    /// down from where the drag began.
    fn sash_dragged(&self, _id: Target, _by: (f32, f32)) {}
    /// The drag of a panel's handle ended.
    fn sash_released(&self) {}
    /// The minimap is pressed or dragged `down` logical pixels from its top, of its `height`.
    fn minimap_dragged(&self, _down: f32, _height: f32) {}
    /// A press landed outside a popup that a press elsewhere closes: its
    /// `on_dismiss_request`. The holder closes what that press closes once the press tells
    /// what it landed on.
    fn dismiss_requested(&self) {}
    /// Whether the control `id` can be dragged elsewhere in the window: a project's header,
    /// a pane's tab or a row of the project's files.
    fn carries(&self, _id: Target) -> bool {
        false
    }
    /// A drag of the control `id` started, the pointer at `at` in the window.
    fn carry_started(&self, _id: Target, _at: (f32, f32)) {}
    /// The pointer carrying a control moved to `at` in the window, over `over` when the
    /// place under it takes what is carried.
    fn carry_moved(&self, _at: (f32, f32), _over: Option<Over>) {}
    /// The drag of a control ended where the pointer let go, with `modifiers` held.
    fn carry_ended(&self, _modifiers: Modifiers) {}
    /// The number of a line `id` of a diff is pressed, or the pointer dragged on from it.
    fn comment_dragged(&self, _id: Target) {}
    /// The pointer came over the number of the line `id` of a diff.
    fn comment_hovered(&self, _id: Target) {}
    /// The press or drag on the numbers of a diff's lines ended.
    fn comment_drag_ended(&self) {}
    /// The primary button went down on the edge or corner `edge` of the window's frame.
    fn edge_pressed(&self, _edge: ResizeEdge) {}
    /// Makes the window an icon on the desktop's bar.
    fn minimize_window(&self) {}
    /// Fills the screen with the window, or gives it back its size.
    fn toggle_maximized(&self) {}
    /// Asks the window to close, as its close button does.
    fn close_window(&self) {}
    /// Opens the list of commands over the file shown.
    fn open_command_palette(&self) {}
    /// Opens the list of the code actions at the caret of the file shown.
    fn show_code_actions(&self) {}
    /// Goes back (`-1`) or forward (`1`) through the places the reader went.
    fn navigate(&self, _delta: isize) {}
    /// Takes the row `row` of the list over the file, or its picked row when `None`.
    fn take_listing(&self, _row: Option<usize>) {}
    /// Opens the colour picker on the colour swatch `index` of the file shown.
    fn open_color_picker(&self, _index: usize) {}
    /// Carries out the row `row` of the menu open over a field, and closes the menu.
    fn field_menu_picked(&self, _row: Target) {}
    /// Carries out the row `row` of the menu open over the conversation's text.
    fn message_menu_picked(&self, _row: Target) {}
    /// Opens the menu of what the page's capture button takes.
    fn open_browser_capture_menu(&self) {}
    /// Carries out `command` of the menu open over a web page.
    fn browser_menu_command(&self, _command: BrowserCommand) {}
    /// Carries out `command` of the menu open over a shell.
    fn terminal_command(&self, _command: TerminalCommand) {}
    /// Asks the desktop for files to attach to the prompt.
    fn attach(&self) {}
    /// Opens the menu of the server `index` of the settings, where its button was clicked.
    fn open_settings_menu(&self, _index: usize) {}
    /// Opens the menu of the project `project` at `at`, or where its button was clicked.
    fn open_project_menu(&self, _project: usize, _at: Option<(f32, f32)>) {}
    /// Opens the menu `kind` of the changes, hanging from the control `trigger` when one
    /// opened it, or else at `at`.
    fn open_git_menu(&self, _kind: MenuKind, _trigger: Option<Target>, _at: Option<(f32, f32)>) {}
    /// Opens the menu of the pane that has the keyboard, where its button was clicked.
    fn open_pane_menu(&self) {}
    /// Opens the menu of the session `session` at `at`.
    fn open_session_menu(&self, _session: usize, _at: (f32, f32)) {}
    /// Opens the menu of the changed file `index` at `at`.
    fn open_change_menu(&self, _index: usize, _at: (f32, f32)) {}
    /// Opens the menu of the folder `position` of the changed files at `at`.
    fn open_folder_change_menu(&self, _position: usize, _at: (f32, f32)) {}
    /// Opens the menu of the tab of the open file `index` at `at`.
    fn open_tab_menu(&self, _index: usize, _at: (f32, f32)) {}
    /// Opens the menu of the row `target` of the project's files at `at`.
    fn open_tree_menu(&self, _target: TreeTarget, _at: (f32, f32)) {}
    /// Opens the menu of the field `field` at `at`, the caret going to the byte `offset` of
    /// the code text under the pointer first unless it is inside the selection.
    fn open_field_menu(&self, _field: Target, _at: (f32, f32), _offset: Option<usize>) {}
    /// Opens the menu of the shell showing at `at`.
    fn open_terminal_menu(&self, _at: (f32, f32)) {}
    /// Opens the menu of the web page showing at `at`.
    fn open_browser_menu(&self, _at: (f32, f32)) {}
    /// Opens the menu of the conversation's message `message` at `at`, or closes the menus
    /// open when the secondary button went down on no message.
    fn open_message_menu(&self, _message: Option<usize>, _at: (f32, f32)) {}
}

/// The operations of views composed with no app behind them, such as a component's own
/// previews: nothing happens.
struct Unattended;

impl Operations for Unattended {
    /// Does nothing.
    fn update(&self, _: Change) {}
}

/// The operations as a composition local holds them: equal to others when both are the
/// same holder, so views that skip see whether they changed.
#[derive(Clone)]
struct Provided(Rc<dyn Operations>);

impl PartialEq for Provided {
    /// Whether both are the same holder.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The operations the views composed below are under.
static LOCAL_OPERATIONS: CompositionLocal<Provided> =
    CompositionLocal::new(|| Provided(Rc::new(Unattended)));

/// Composes `content` under `operations`.
pub fn provide<R>(operations: Rc<dyn Operations>, content: impl FnOnce() -> R) -> R {
    LOCAL_OPERATIONS.provide(Provided(operations), content)
}

/// The operations the view being composed is under.
pub fn operations() -> Rc<dyn Operations> {
    LOCAL_OPERATIONS.current().0
}

/// What a control does when clicked, as a component's builder keeps it until it is built.
#[derive(Clone)]
pub struct OnClick(Rc<dyn Fn()>);

impl OnClick {
    /// What `on_click` does.
    pub fn new(on_click: impl Fn() + 'static) -> Self {
        Self(Rc::new(on_click))
    }

    /// What a click on the control `id` does when its builder was given nothing: the
    /// workspace's reducer takes the press, as [`send`] hands it [`Event::pressing`] of `id`.
    pub fn or_send(given: Option<Self>, id: Target) -> Self {
        given.unwrap_or_else(|| Self::new(send(Event::pressing(id))))
    }

    /// The lambda, to hand to [`Interactive::control`].
    pub fn lambda(self) -> impl Fn() + 'static {
        move || (self.0)()
    }
}

impl std::fmt::Debug for OnClick {
    /// Writes that there is a lambda, which has nothing else to show.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OnClick")
    }
}

/// The lambda that hands `event` to the workspace's reducer, [`WorkspaceState::handle`],
/// which is what most controls do when clicked, with the event of their area.
pub fn send(event: Event) -> impl Fn() + Clone + 'static {
    let operations = operations();
    move || operations.send(event.clone())
}

/// The lambda that hands a press of the row `id` of a list the reader picks several rows
/// of to the workspace: with Shift held `pick` picks the rows from the last one picked to
/// it ([`Pick::Range`]), with Control or Command it picks it or lets it go
/// ([`Pick::Toggle`]), and otherwise the row is pressed as [`send`] presses it.
pub fn picking(
    id: Target,
    pick: impl Fn(WorkspaceState, Pick) -> WorkspaceState + 'static,
) -> impl Fn(Modifiers) + 'static {
    let operations = operations();
    let pick = Rc::new(pick);
    move |held: Modifiers| {
        let pick = Rc::clone(&pick);
        let change: Change = if held.shift {
            Box::new(move |state| pick(state, Pick::Range))
        } else if held.ctrl || held.meta {
            Box::new(move |state| pick(state, Pick::Toggle))
        } else {
            Box::new(move |state| state.handle(Event::pressing(id)))
        };
        operations.update(change);
    }
}

/// Whether `event` activates the control holding the keyboard: Enter or the space bar
/// going down, or repeating while held.
fn activates(event: &KeyEvent) -> bool {
    event.pressed
        && match &event.key {
            Key::Enter => true,
            Key::Character(text) => text == " ",
            _ => false,
        }
}

/// The modifier that hands a drag of the handle `id` on a panel's edge to the app, as
/// `detectDragGestures` hands its `onDrag` and `onDragEnd` lambdas the distance dragged: from
/// the press on, how far the pointer went from where it went down, and the end of the drag.
pub fn sash_drag(id: Target) -> Modifier {
    let operations = operations();
    let ended = Rc::clone(&operations);
    let start: Rc<Cell<Option<Offset>>> = Rc::default();
    let reset = Rc::clone(&start);
    Modifier::new()
        .on_drag_end(move || {
            reset.set(None);
            ended.sash_released();
        })
        .on_drag(move |at| {
            let from = start.get().unwrap_or(at);
            start.set(Some(from));
            operations.sash_dragged(id, (at.x - from.x, at.y - from.y));
        })
}

/// The modifier that hands a press and drag along the minimap to the app: how far down
/// the pointer is from the minimap's top, and how tall the minimap is.
pub fn minimap_drag() -> Modifier {
    let operations = operations();
    let height = Rc::new(Cell::new(0.0_f32));
    let measured = Rc::clone(&height);
    Modifier::new()
        .on_size_changed(move |size| measured.set(size.height))
        .on_drag(move |at| operations.minimap_dragged(at.y, height.get()))
}

/// What a drag and drop inside the window carries: the control it started from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Carried(pub Target);

/// Where `event` is in the window.
fn event_at(event: &DragAndDropEvent) -> (f32, f32) {
    (event.position.x, event.position.y)
}

/// The window's root as the place every drag of a control is carried over: a drag and drop
/// target on the root, telling the app the drag started, where the pointer carries it
/// while it is over no place that takes it, and that it ended, with the keys held when it
/// was let go.
struct Carrying {
    /// The app's state holder.
    operations: Rc<dyn Operations>,
}

impl Carrying {
    /// The control `event` carries and where the pointer is, when it carries one.
    fn of(event: &DragAndDropEvent) -> Option<(Target, (f32, f32))> {
        let Carried(id) = *event.payload::<Carried>()?;
        Some((id, event_at(event)))
    }
}

impl DragAndDropTarget for Carrying {
    /// Takes the drop; the drag's end puts what is carried where it was let go.
    fn on_drop(&self, _: &DragAndDropEvent) -> bool {
        true
    }

    /// Tells the holder the drag started.
    fn on_started(&self, event: &DragAndDropEvent) {
        if let Some((id, at)) = Self::of(event) {
            self.operations.carry_started(id, at);
        }
    }

    /// Tells the holder where the pointer carries the control, over no place that takes it.
    fn on_moved(&self, event: &DragAndDropEvent) {
        if let Some((_, at)) = Self::of(event) {
            self.operations.carry_moved(at, None);
        }
    }

    /// Tells the holder the drag ended.
    fn on_ended(&self, event: &DragAndDropEvent) {
        if Self::of(event).is_some() {
            self.operations.carry_ended(event.modifiers);
        }
    }
}

/// The modifier for a window's root that follows every drag of a control in the window.
pub fn carry_target() -> Modifier {
    let operations = operations();
    Modifier::new().drag_and_drop_target(
        |event| event.payload::<Carried>().is_some(),
        Carrying { operations },
    )
}

/// What a place a carried control can be let go over makes of the pointer on it: the
/// [`Over`] for the pointer at a point of the window, the place laid out at a rectangle.
type Overs = Rc<dyn Fn((f32, f32), Rect) -> Over>;

/// A place of the window a carried control, or files from another application, can be let
/// go over: a drag and drop target on that place, telling the app the pointer is over it
/// and what it is over there.
struct Taking {
    /// The app's state holder.
    operations: Rc<dyn Operations>,
    /// What the place makes of the pointer on it.
    over: Overs,
    /// Where the place is laid out in the window.
    area: Rc<Cell<Rect>>,
}

impl Taking {
    /// What the place is for the pointer where `event` is.
    fn over(&self, event: &DragAndDropEvent) -> Over {
        (self.over)(event_at(event), self.area.get())
    }

    /// The row of the project's files the place is, for files held over it.
    fn row(&self, event: &DragAndDropEvent) -> Option<TreeTarget> {
        match self.over(event) {
            Over::Tree(row) => Some(row),
            _ => None,
        }
    }

    /// Tells the holder where the pointer is over the place, carrying a control or files.
    fn follow(&self, event: &DragAndDropEvent) {
        if event.payload::<Carried>().is_some() {
            self.operations
                .carry_moved(event_at(event), Some(self.over(event)));
        } else {
            self.operations
                .files_held(Some(event_at(event)), self.row(event));
        }
    }
}

impl DragAndDropTarget for Taking {
    /// Hands files let go over the place to the holder; a carried control is put down where
    /// it was let go by the drag's end.
    fn on_drop(&self, event: &DragAndDropEvent) -> bool {
        if let Some(DragData::FilesList(files)) = event.drag_data() {
            self.operations
                .files_dropped(files.clone(), event_at(event), self.row(event));
        }
        true
    }

    /// Tells the holder the pointer came over the place.
    fn on_entered(&self, event: &DragAndDropEvent) {
        self.follow(event);
    }

    /// Tells the holder where the pointer is over the place.
    fn on_moved(&self, event: &DragAndDropEvent) {
        self.follow(event);
    }

    /// Tells the holder files left the window when they leave it from over the place.
    fn on_exited(&self, event: &DragAndDropEvent) {
        if event.payload::<Carried>().is_none() {
            self.operations.files_held(None, None);
        }
    }
}

/// The modifier of a place of the window a carried control can be let go over, and files
/// from another application too when `files`: `over` makes of the pointer at a point of the
/// window, over the place laid out at a rectangle, what the app is told it is over.
pub fn carried_over(files: bool, over: impl Fn((f32, f32), Rect) -> Over + 'static) -> Modifier {
    let operations = operations();
    let area: Rc<Cell<Rect>> = Rc::default();
    let placed = Rc::clone(&area);
    Modifier::new()
        .on_globally_positioned(move |rect| placed.set(rect))
        .drag_and_drop_target(
            move |event| {
                event.payload::<Carried>().is_some()
                    || (files && matches!(event.drag_data(), Some(DragData::FilesList(_))))
            },
            Taking {
                operations,
                over: Rc::new(over),
                area,
            },
        )
}

/// The modifier of the numbers of a diff's lines that hands a press and drag on them to the
/// app: the press and each move, then the end;
/// and the pointer coming over the number `id`, which a drag stretches the lines to.
pub fn comment_drag(id: Target) -> Modifier {
    let operations = operations();
    let (ended, hovered) = (Rc::clone(&operations), Rc::clone(&operations));
    Modifier::new()
        .on_hover(move |over| {
            if over {
                hovered.comment_hovered(id);
            }
        })
        .on_drag_end(move || ended.comment_drag_ended())
        .on_drag(move |_| operations.comment_dragged(id))
}

/// The window's root as the place files dragged in from another application are held over
/// and let go: a drag and drop target taking [`DragData::FilesList`], telling the app where
/// the files are held, that they left, and where they were let go, over no row of the
/// project's files.
struct FilesFromOutside {
    /// The app's state holder.
    operations: Rc<dyn Operations>,
}

impl DragAndDropTarget for FilesFromOutside {
    /// Hands the files let go to the holder, with where they were let go.
    fn on_drop(&self, event: &DragAndDropEvent) -> bool {
        let Some(DragData::FilesList(files)) = event.drag_data() else {
            return false;
        };
        self.operations
            .files_dropped(files.clone(), event_at(event), None);
        true
    }

    /// Tells the holder where the files came over the window.
    fn on_entered(&self, event: &DragAndDropEvent) {
        self.operations.files_held(Some(event_at(event)), None);
    }

    /// Tells the holder where the files are held.
    fn on_moved(&self, event: &DragAndDropEvent) {
        self.operations.files_held(Some(event_at(event)), None);
    }

    /// Tells the holder the files left the window.
    fn on_exited(&self, _: &DragAndDropEvent) {
        self.operations.files_held(None, None);
    }
}

/// The modifier for a window's root that follows files dragged in from another application
/// and hands them to the app where they are let go.
pub fn file_drop() -> Modifier {
    let operations = operations();
    Modifier::new().drag_and_drop_target(
        |event| matches!(event.drag_data(), Some(DragData::FilesList(_))),
        FilesFromOutside { operations },
    )
}

/// The modifier for the bottom layer of a window's root, which gets the presses no control
/// above took: the primary button going down
/// there, the secondary one (which opens no message's menu and closes the menus open) and
/// the mouse's back and forward buttons.
pub fn background() -> Modifier {
    let operations = operations();
    let (secondary, back, forward) = (
        Rc::clone(&operations),
        Rc::clone(&operations),
        Rc::clone(&operations),
    );
    Modifier::new()
        .on_pointer_button(PointerButton::Primary, move |event| {
            if event.phase == PointerPhase::Press {
                operations.pressed_elsewhere(window_at(event));
            }
        })
        .on_pointer_button(PointerButton::Secondary, move |event| {
            if event.phase == PointerPhase::Press {
                secondary.open_message_menu(None, window_at(event));
            }
        })
        .on_pointer_button(PointerButton::Back, move |event| {
            if event.phase == PointerPhase::Press {
                back.history_pressed(true);
            }
        })
        .on_pointer_button(PointerButton::Forward, move |event| {
            if event.phase == PointerPhase::Press {
                forward.history_pressed(false);
            }
        })
}

/// Where the pointer of `event` is in the window.
pub fn window_at(event: &PointerEvent) -> (f32, f32) {
    (event.window_position.x, event.window_position.y)
}

/// What a control's secondary press runs: what opens its menu, told where the pointer is
/// in the window.
pub type OnContext = Rc<dyn Fn((f32, f32))>;

/// What a control does with the buttons beyond its click and menu, seeing the press, drag
/// and release of the primary button and the press and release of the secondary one on it,
/// after the app was told of the press.
pub type OnPointer = Rc<dyn Fn(&PointerEvent)>;

/// What a control reports and runs, as [`reporting`] makes it.
struct Reported {
    /// The control.
    id: Target,
    /// What a click runs, told the modifier keys held.
    on_click: Rc<dyn Fn(Modifiers)>,
    /// What a secondary press runs, when the control has a menu of its own.
    on_context: Option<OnContext>,
    /// What the control does itself with the buttons, when it does anything.
    on_pointer: Option<OnPointer>,
}

/// The press or click `event` makes on the control `id` laid out at `area`, with the touch
/// slop `touch_slop` in effect where it was composed.
fn press_of(id: Target, area: Rect, event: &PointerEvent, touch_slop: f32) -> Press {
    Press {
        id,
        area,
        at: window_at(event),
        clicks: event.click_count,
        modifiers: event.modifiers,
        touch_slop,
    }
}

/// The modifier that names the area `id` for tests, tells the app of the presses on it and
/// runs its lambdas: a primary press is reported, and `on_click` runs with the modifier keys
/// held when the button came up on the area it went down on and the app let the click
/// count, the press telling where it is and how many in a row it makes by the view
/// configuration in effect here; a drag of it is carried when the app
/// [carries](Operations::carries) it. A secondary press runs `on_context` when the control
/// has a menu of its own, and is left to what is beneath otherwise. `on_pointer` sees the buttons on the area after the app was told.
fn reporting(reported: Reported) -> Modifier {
    let Reported {
        id,
        on_click,
        on_context,
        on_pointer,
    } = reported;
    let operations = operations();
    let (carry, menus) = (Rc::clone(&operations), Rc::clone(&operations));
    let area: Rc<Cell<Rect>> = Rc::default();
    let (placed, context_area) = (Rc::clone(&area), Rc::clone(&area));
    let touch_slop = LOCAL_VIEW_CONFIGURATION.current().touch_slop;
    let secondary_pointer = on_pointer.clone();
    let modifier = Modifier::new()
        .test_tag(id.tag())
        .drag_and_drop_source(move |_| {
            carry
                .carries(id)
                .then(|| DragAndDropTransferData::new(Carried(id)))
        })
        .on_globally_positioned(move |rect| placed.set(rect))
        .on_pointer_button(PointerButton::Primary, move |event| {
            match event.phase {
                PointerPhase::Press => {
                    operations.pressed(press_of(id, area.get(), event, touch_slop));
                }
                PointerPhase::Click => {
                    if operations.click_lands(press_of(id, area.get(), event, touch_slop)) {
                        on_click(event.modifiers);
                    }
                }
                PointerPhase::Drag | PointerPhase::Release => {}
            }
            if let Some(on_pointer) = &on_pointer
                && event.phase != PointerPhase::Click
            {
                on_pointer(event);
            }
        });
    if on_context.is_none() && secondary_pointer.is_none() {
        return modifier;
    }
    modifier.on_pointer_button(PointerButton::Secondary, move |event| {
        if event.phase == PointerPhase::Press
            && let Some(on_context) = &on_context
        {
            menus.context_pressed(press_of(id, context_area.get(), event, touch_slop));
            on_context(window_at(event));
        }
        if let Some(on_pointer) = &secondary_pointer
            && event.phase != PointerPhase::Click
        {
            on_pointer(event);
        }
    })
}

/// Modifiers that name a control, report the presses on it and run its lambdas.
///
/// A secondary press on a control with no menu of its own, given with
/// [`control_menu`](Self::control_menu) or [`pressable_menu`](Self::pressable_menu), goes to
/// what is beneath it: the message it is in, which opens the message's menu, or the
/// window's [`background`].
pub trait Interactive: Sized {
    /// Names the control `id` and makes it clickable: it is a focus target Tab reaches in
    /// paint order, tests find it by its tag, the app is told of presses on it, and
    /// `on_click` runs when it is clicked, or activated with Enter or Space while it holds
    /// the keyboard. Dragged away when the app [carries](Operations::carries) it.
    #[track_caller]
    fn control(self, id: Target, on_click: impl Fn() + 'static) -> Self;
    /// Like [`control`](Self::control), with `on_click` told the modifier keys held, for a
    /// row the reader picks several of with Shift or Control.
    #[track_caller]
    fn control_with(self, id: Target, on_click: impl Fn(Modifiers) + 'static) -> Self;
    /// Like [`control_with`](Self::control_with), with `on_context` run when the secondary
    /// button goes down on the control, told where the pointer is in the window: what opens
    /// its menu.
    #[track_caller]
    fn control_menu(
        self,
        id: Target,
        on_click: impl Fn(Modifiers) + 'static,
        on_context: impl Fn((f32, f32)) + 'static,
    ) -> Self;
    /// Like [`control`](Self::control), with `on_pointer` seeing the buttons on the control
    /// after the app was told of the press: a control that selects or drags by itself.
    #[track_caller]
    fn control_pointing(
        self,
        id: Target,
        on_click: impl Fn() + 'static,
        on_pointer: impl Fn(&PointerEvent) + 'static,
    ) -> Self;
    /// Like [`control`](Self::control), without making the area a focus target, for a
    /// control whose keyboard focus is held by what it holds, such as a text field's box.
    fn pressable(self, id: Target, on_click: impl Fn() + 'static) -> Self;
    /// Like [`pressable`](Self::pressable), with `on_context` run when the secondary button
    /// goes down on the area, told where the pointer is in the window: what opens its menu.
    fn pressable_menu(
        self,
        id: Target,
        on_click: impl Fn() + 'static,
        on_context: impl Fn((f32, f32)) + 'static,
    ) -> Self;
    /// Like [`pressable_menu`](Self::pressable_menu), with `on_pointer` seeing the buttons on
    /// the area after the app was told of the press: an area that selects, or hands the
    /// pointer on, by itself.
    fn pressable_pointing(
        self,
        id: Target,
        on_click: impl Fn() + 'static,
        on_context: impl Fn((f32, f32)) + 'static,
        on_pointer: impl Fn(&PointerEvent) + 'static,
    ) -> Self;
    /// Marks the area as message `index` of the conversation: tests find it by the tag of
    /// [`CHAT_TEXT`](crate::areas::ChatEvent::ChatText), it tracks where it is among the
    /// places of [`ChatEvent::MessageArea`](crate::areas::ChatEvent::MessageArea), and a
    /// secondary press on it that no control in it took opens the message's menu.
    fn message_area(self, index: usize) -> Self;
}

/// Names the control `id` as `reported` reports it, a focus target activated with Enter or
/// Space while it holds the keyboard.
#[track_caller]
fn focus_control(modifier: Modifier, reported: Reported) -> Modifier {
    let id = reported.id;
    let keyed = Rc::clone(&reported.on_click);
    let focus = focus::focus_of(id);
    let held = focus.clone();
    modifier
        .on_key_event(move |event| {
            let taken = held.is_focused() && activates(event);
            if taken {
                keyed(event.modifiers);
            }
            taken
        })
        .focus_target(&focus)
        .chain(reporting(reported))
}

impl Interactive for Modifier {
    #[track_caller]
    fn control(self, id: Target, on_click: impl Fn() + 'static) -> Self {
        self.control_with(id, move |_| on_click())
    }

    #[track_caller]
    fn control_with(self, id: Target, on_click: impl Fn(Modifiers) + 'static) -> Self {
        focus_control(
            self,
            Reported {
                id,
                on_click: Rc::new(on_click),
                on_context: None,
                on_pointer: None,
            },
        )
    }

    #[track_caller]
    fn control_menu(
        self,
        id: Target,
        on_click: impl Fn(Modifiers) + 'static,
        on_context: impl Fn((f32, f32)) + 'static,
    ) -> Self {
        focus_control(
            self,
            Reported {
                id,
                on_click: Rc::new(on_click),
                on_context: Some(Rc::new(on_context)),
                on_pointer: None,
            },
        )
    }

    #[track_caller]
    fn control_pointing(
        self,
        id: Target,
        on_click: impl Fn() + 'static,
        on_pointer: impl Fn(&PointerEvent) + 'static,
    ) -> Self {
        focus_control(
            self,
            Reported {
                id,
                on_click: Rc::new(move |_| on_click()),
                on_context: None,
                on_pointer: Some(Rc::new(on_pointer)),
            },
        )
    }

    fn pressable(self, id: Target, on_click: impl Fn() + 'static) -> Self {
        self.chain(reporting(Reported {
            id,
            on_click: Rc::new(move |_| on_click()),
            on_context: None,
            on_pointer: None,
        }))
    }

    fn pressable_menu(
        self,
        id: Target,
        on_click: impl Fn() + 'static,
        on_context: impl Fn((f32, f32)) + 'static,
    ) -> Self {
        self.chain(reporting(Reported {
            id,
            on_click: Rc::new(move |_| on_click()),
            on_context: Some(Rc::new(on_context)),
            on_pointer: None,
        }))
    }

    fn pressable_pointing(
        self,
        id: Target,
        on_click: impl Fn() + 'static,
        on_context: impl Fn((f32, f32)) + 'static,
        on_pointer: impl Fn(&PointerEvent) + 'static,
    ) -> Self {
        self.chain(reporting(Reported {
            id,
            on_click: Rc::new(move |_| on_click()),
            on_context: Some(Rc::new(on_context)),
            on_pointer: Some(Rc::new(on_pointer)),
        }))
    }

    fn message_area(self, index: usize) -> Self {
        let operations = operations();
        crate::bounds::placed(
            Target::Chat(ChatEvent::MessageArea(index)),
            self.test_tag(Target::Chat(ChatEvent::ChatText).tag())
                .on_pointer_button(PointerButton::Secondary, move |event| {
                    if event.phase == PointerPhase::Press {
                        operations.open_message_menu(Some(index), window_at(event));
                    }
                }),
        )
    }
}

/// `modifier` with the pointing hand over it, made the control `id` that
/// sends the press of `id` to the workspace when clicked: how links, chips and
/// counts that act at once are made pressable.
#[track_caller]
pub fn pressed(modifier: Modifier, id: Target) -> Modifier {
    modifier
        .cursor(rustle::CursorIcon::Pointer)
        .control(id, send(Event::pressing(id)))
}

/// `modifier` made [`pressed`] as `id` when there is one, or as it is when
/// there is none: rows and controls that can be pressed only sometimes.
#[track_caller]
pub fn pressable(modifier: Modifier, id: Option<Target>) -> Modifier {
    match id {
        Some(id) => pressed(modifier, id),
        None => modifier,
    }
}

#[cfg(test)]
mod tests;
