//! What the user does to the controls the views draw, handed to the app as event lambdas.
//!
//! A control takes what clicking it does as a lambda, the way a composable takes its
//! `onClick`: [`Interactive::control`] names the control and runs its lambda when it is
//! clicked, or activated with Enter or Space while it holds the keyboard. The lambdas call
//! the app's state holder, the [`Operations`] the views are composed under, provided once
//! at the root with [`provide`]: [`Operations::update`] runs a reducer of the workspace
//! (most controls hand their press to [`WorkspaceState::after`] with [`reduce`]), and the
//! named operations do what only the app can, such as opening a menu where the pointer is,
//! moving the window or talking to the servers. The app does what every event asks
//! afterwards (saving, sending, the focus a reducer asked for) in one place.
//!
//! What the views report besides, so the app knows where the pointer and the presses are,
//! keeps the control's [`FocusId`]: the press that went down on a control
//! ([`Operations::pressed`]), the pointer over it ([`Operations::hovered`]) and a drag of
//! it ([`Operations::carry_started`]). Keyboard focus is rustle's, followed by the app
//! through [`crate::focus`].

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use rustle::{
    CompositionLocal, DragAndDropEvent, DragAndDropTarget, DragAndDropTransferData, DragData, Key,
    KeyEvent, LOCAL_VIEW_CONFIGURATION, Modifier, Modifiers, Offset, PointerButton, PointerPhase,
    Rect, ResizeEdge, ViewConfiguration,
};

use crate::browser_menu::BrowserCommand;
use crate::focus::{self, FocusId};
use crate::git_controls::Pick;
use crate::git_menus::MenuKind;
use crate::state::WorkspaceState;
use crate::terminal_menu::TerminalCommand;
use crate::tree_actions::TreeTarget;

/// A change of the workspace: the state it was, made into the state it is after the change.
pub type Change = Box<dyn FnOnce(WorkspaceState) -> WorkspaceState>;

/// The app's state holder as the views see it: the operations the controls' lambdas call,
/// and what the views report of the pointer. Positions are logical pixels from the
/// window's top left.
pub trait Operations {
    /// Changes the workspace with `change`, then does what the change asks of the app.
    fn update(&self, change: Change);
    /// The primary button went down on the control `id`, laid out at `area` in the window,
    /// making `clicks` presses in a row as rustle counted them by `configuration`, the view
    /// configuration in effect where the control was composed: the app follows the press
    /// (fields take the keyboard, a handle takes hold of the pointer, a menu opened from the
    /// control hangs from `area`).
    fn pressed(&self, id: FocusId, area: Rect, clicks: u32, configuration: ViewConfiguration);
    /// The primary button came up on the control `id` it went down on, laid out at `area`.
    /// Returns whether the click counts, which it does not after the press dragged something
    /// or was taken by a handle; a click that counts gives the keyboard to the field or page
    /// it is on.
    fn click_lands(&self, id: FocusId, area: Rect) -> bool;
    /// A key went down or up in the window, before whatever holds rustle's focus sees it.
    /// Returns whether the holder took it, in which case nothing else in the window does.
    fn key(&self, event: &KeyEvent) -> bool;
    /// Files from another application are held over the window at `at`, or left it, or
    /// were let go elsewhere, when `at` is `None`.
    fn files_held(&self, at: Option<(f32, f32)>);
    /// Files from another application, at `paths`, were let go over the window at `at`.
    fn files_dropped(&self, paths: Vec<PathBuf>, at: (f32, f32));
    /// The pointer came over the control `id`, or left it when not `over`.
    fn hovered(&self, id: FocusId, over: bool);
    /// The handle `id` on the edge of a panel is dragged `by` logical pixels across and
    /// down from where the drag began.
    fn sash_dragged(&self, id: FocusId, by: (f32, f32));
    /// The drag of a panel's handle ended.
    fn sash_released(&self);
    /// The minimap is pressed or dragged `down` logical pixels from its top, of its `height`.
    fn minimap_dragged(&self, down: f32, height: f32);
    /// A press landed outside a popup that a press elsewhere closes: its
    /// `on_dismiss_request`. The holder closes what that press closes.
    fn dismiss_requested(&self);
    /// Whether the control `id` can be dragged elsewhere in the window: a project's header,
    /// a pane's tab or a row of the project's files.
    fn carries(&self, id: FocusId) -> bool;
    /// A drag of the control `id` started, the pointer at `at` in the window.
    fn carry_started(&self, id: FocusId, at: (f32, f32));
    /// The pointer carrying a control moved to `at` in the window.
    fn carry_moved(&self, at: (f32, f32));
    /// The drag of a control ended where the pointer let go, with `modifiers` held.
    fn carry_ended(&self, modifiers: Modifiers);
    /// The number of a line `id` of a diff is pressed, or the pointer dragged on from it.
    fn comment_dragged(&self, id: FocusId);
    /// The press or drag on the numbers of a diff's lines ended.
    fn comment_drag_ended(&self);
    /// The primary button went down on the edge or corner `edge` of the window's frame.
    fn edge_pressed(&self, edge: ResizeEdge);
    /// Makes the window an icon on the desktop's bar.
    fn minimize_window(&self);
    /// Fills the screen with the window, or gives it back its size.
    fn toggle_maximized(&self);
    /// Asks the window to close, as its close button does.
    fn close_window(&self);
    /// Opens the list of commands over the file shown.
    fn open_command_palette(&self);
    /// Opens the list of the code actions at the caret of the file shown.
    fn show_code_actions(&self);
    /// Goes back (`-1`) or forward (`1`) through the places the reader went.
    fn navigate(&self, delta: isize);
    /// Takes the row `row` of the list over the file, or its picked row when `None`.
    fn take_listing(&self, row: Option<usize>);
    /// Opens the colour picker on the colour swatch `index` of the file shown.
    fn open_color_picker(&self, index: usize);
    /// Carries out the row `row` of the menu open over a field, and closes the menu.
    fn field_menu_picked(&self, row: FocusId);
    /// Carries out the row `row` of the menu open over the conversation's text.
    fn message_menu_picked(&self, row: FocusId);
    /// Opens the menu of what the page's capture button takes.
    fn open_browser_capture_menu(&self);
    /// Carries out `command` of the menu open over a web page.
    fn browser_menu_command(&self, command: BrowserCommand);
    /// Carries out `command` of the menu open over a shell.
    fn terminal_command(&self, command: TerminalCommand);
    /// Asks the desktop for files to attach to the prompt.
    fn attach(&self);
    /// Opens the menu of the server `index` of the settings, under the pointer.
    fn open_settings_menu(&self, index: usize);
    /// Opens the menu of the project `project`, under the pointer.
    fn open_project_menu(&self, project: usize);
    /// Opens the menu `kind` of the changes, hanging from the control `trigger` when one
    /// opened it, or under the pointer.
    fn open_git_menu(&self, kind: MenuKind, trigger: Option<FocusId>);
    /// Opens the menu of the pane that has the keyboard.
    fn open_pane_menu(&self);
    /// Opens the menu of the session `session`, under the pointer.
    fn open_session_menu(&self, session: usize);
    /// Opens the menu of the changed file `index`, under the pointer.
    fn open_change_menu(&self, index: usize);
    /// Opens the menu of the folder `position` of the changed files, under the pointer.
    fn open_folder_change_menu(&self, position: usize);
    /// Opens the menu of the tab of the open file `index`, under the pointer.
    fn open_tab_menu(&self, index: usize);
    /// Opens the menu of the row `target` of the project's files, under the pointer.
    fn open_tree_menu(&self, target: TreeTarget);
    /// Opens the menu of the field `field` at the pointer.
    fn open_field_menu(&self, field: FocusId);
    /// Opens the menu of the shell showing.
    fn open_terminal_menu(&self);
    /// Opens the menu of the web page showing.
    fn open_browser_menu(&self);
    /// Opens the menu of the conversation's text at the pointer.
    fn open_message_menu(&self);
}

/// The operations of views composed with no app behind them, such as a component's own
/// previews: nothing happens.
struct Unattended;

impl Operations for Unattended {
    /// Does nothing.
    fn update(&self, _: Change) {}

    /// Does nothing.
    fn pressed(&self, _: FocusId, _: Rect, _: u32, _: ViewConfiguration) {}

    /// Lets no click count.
    fn click_lands(&self, _: FocusId, _: Rect) -> bool {
        false
    }

    /// Takes nothing, so every key goes on to what holds focus.
    fn key(&self, _: &KeyEvent) -> bool {
        false
    }

    /// Does nothing.
    fn files_held(&self, _: Option<(f32, f32)>) {}

    /// Does nothing.
    fn files_dropped(&self, _: Vec<PathBuf>, _: (f32, f32)) {}

    /// Does nothing.
    fn hovered(&self, _: FocusId, _: bool) {}

    /// Does nothing.
    fn sash_dragged(&self, _: FocusId, _: (f32, f32)) {}

    /// Does nothing.
    fn sash_released(&self) {}

    /// Does nothing.
    fn minimap_dragged(&self, _: f32, _: f32) {}

    /// Does nothing.
    fn dismiss_requested(&self) {}

    /// Carries nothing.
    fn carries(&self, _: FocusId) -> bool {
        false
    }

    /// Does nothing.
    fn carry_started(&self, _: FocusId, _: (f32, f32)) {}

    /// Does nothing.
    fn carry_moved(&self, _: (f32, f32)) {}

    /// Does nothing.
    fn carry_ended(&self, _: Modifiers) {}

    /// Does nothing.
    fn comment_dragged(&self, _: FocusId) {}

    /// Does nothing.
    fn comment_drag_ended(&self) {}

    /// Does nothing.
    fn edge_pressed(&self, _: ResizeEdge) {}

    /// Does nothing.
    fn minimize_window(&self) {}

    /// Does nothing.
    fn toggle_maximized(&self) {}

    /// Does nothing.
    fn close_window(&self) {}

    /// Does nothing.
    fn open_command_palette(&self) {}

    /// Does nothing.
    fn show_code_actions(&self) {}

    /// Does nothing.
    fn navigate(&self, _: isize) {}

    /// Does nothing.
    fn take_listing(&self, _: Option<usize>) {}

    /// Does nothing.
    fn open_color_picker(&self, _: usize) {}

    /// Does nothing.
    fn field_menu_picked(&self, _: FocusId) {}

    /// Does nothing.
    fn message_menu_picked(&self, _: FocusId) {}

    /// Does nothing.
    fn open_browser_capture_menu(&self) {}

    /// Does nothing.
    fn browser_menu_command(&self, _: BrowserCommand) {}

    /// Does nothing.
    fn terminal_command(&self, _: TerminalCommand) {}

    /// Does nothing.
    fn attach(&self) {}

    /// Does nothing.
    fn open_settings_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_project_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_git_menu(&self, _: MenuKind, _: Option<FocusId>) {}

    /// Does nothing.
    fn open_pane_menu(&self) {}

    /// Does nothing.
    fn open_session_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_change_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_folder_change_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_tab_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_tree_menu(&self, _: TreeTarget) {}

    /// Does nothing.
    fn open_field_menu(&self, _: FocusId) {}

    /// Does nothing.
    fn open_terminal_menu(&self) {}

    /// Does nothing.
    fn open_browser_menu(&self) {}

    /// Does nothing.
    fn open_message_menu(&self) {}
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
    /// workspace's reducer takes the press, as [`reduce`] hands it.
    pub fn or_reduce(given: Option<Self>, id: FocusId) -> Self {
        given.unwrap_or_else(|| Self::new(reduce(id)))
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

/// The lambda that hands a press of the control `id` to the workspace's reducer,
/// [`WorkspaceState::after`], which is what most controls do when clicked.
pub fn reduce(id: FocusId) -> impl Fn() + Clone + 'static {
    let operations = operations();
    move || operations.update(Box::new(move |state| state.after(id)))
}

/// The lambda that hands a press of the row `id` of a list the reader picks several rows
/// of to the workspace: with Shift held `pick` picks the rows from the last one picked to
/// it ([`Pick::Range`]), with Control or Command it picks it or lets it go
/// ([`Pick::Toggle`]), and otherwise the row is pressed as [`reduce`] presses it.
pub fn picking(
    id: FocusId,
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
            Box::new(move |state| state.after(id))
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
pub fn sash_drag(id: FocusId) -> Modifier {
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
pub struct Carried(pub FocusId);

/// The window's root as the place every drag of a control is carried over: Compose's
/// `dragAndDropTarget` on the root, telling the app the drag started, where the pointer
/// carries it, and that it ended, with the keys held when it was let go.
struct Carrying {
    /// The app's state holder.
    operations: Rc<dyn Operations>,
}

impl Carrying {
    /// The control `event` carries and where the pointer is, when it carries one.
    fn of(event: &DragAndDropEvent) -> Option<(FocusId, (f32, f32))> {
        let Carried(id) = *event.payload::<Carried>()?;
        Some((id, (event.position.x, event.position.y)))
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

    /// Tells the holder where the pointer carries the control.
    fn on_moved(&self, event: &DragAndDropEvent) {
        if let Some((_, at)) = Self::of(event) {
            self.operations.carry_moved(at);
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

/// The modifier of the numbers of a diff's lines that hands a press and drag on them to the
/// app, as `detectDragGestures` hands its lambdas: the press and each move, then the end.
pub fn comment_drag(id: FocusId) -> Modifier {
    let operations = operations();
    let ended = Rc::clone(&operations);
    Modifier::new()
        .on_drag_end(move || ended.comment_drag_ended())
        .on_drag(move |_| operations.comment_dragged(id))
}

/// The window's root as the place files dragged in from another application are held over
/// and let go: Compose's `dragAndDropTarget` taking `DragData.FilesList`, telling the app
/// where the files are held, that they left, and where they were let go.
struct FilesFromOutside {
    /// The app's state holder.
    operations: Rc<dyn Operations>,
}

impl FilesFromOutside {
    /// Where `event` is, as the holder takes a place in the window.
    fn at(event: &DragAndDropEvent) -> (f32, f32) {
        (event.position.x, event.position.y)
    }
}

impl DragAndDropTarget for FilesFromOutside {
    /// Hands the files let go to the holder, with where they were let go.
    fn on_drop(&self, event: &DragAndDropEvent) -> bool {
        let Some(DragData::FilesList(files)) = event.drag_data() else {
            return false;
        };
        self.operations
            .files_dropped(files.clone(), Self::at(event));
        true
    }

    /// Tells the holder where the files came over the window.
    fn on_entered(&self, event: &DragAndDropEvent) {
        self.operations.files_held(Some(Self::at(event)));
    }

    /// Tells the holder where the files are held.
    fn on_moved(&self, event: &DragAndDropEvent) {
        self.operations.files_held(Some(Self::at(event)));
    }

    /// Tells the holder the files left the window.
    fn on_exited(&self, _: &DragAndDropEvent) {
        self.operations.files_held(None);
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

/// What a secondary press on a control does when it has no menu of its own: the menu of
/// the conversation's text opens, as it does on no control at all.
fn message_menu() -> Rc<dyn Fn()> {
    let operations = operations();
    Rc::new(move || operations.open_message_menu())
}

/// The modifier that names the area `id` for tests, tells the app where the pointer and the
/// presses are on it and runs its lambdas: a primary press is reported, and `on_click` runs
/// with the modifier keys held when the button came up on the area it went down on and
/// the app let the click count, the press telling how many in a row it makes by the view
/// configuration in effect here; the pointer coming over it or leaving it is reported, and
/// a drag of it when the app [carries](Operations::carries) it. A secondary press runs
/// `on_context`, as a composable's `onContextMenu` lambda.
fn reporting(id: FocusId, on_click: Rc<dyn Fn(Modifiers)>, on_context: Rc<dyn Fn()>) -> Modifier {
    let operations = operations();
    let (hover, carry) = (Rc::clone(&operations), Rc::clone(&operations));
    let area: Rc<Cell<Rect>> = Rc::default();
    let placed = Rc::clone(&area);
    let configuration = LOCAL_VIEW_CONFIGURATION.current();
    Modifier::new()
        .test_tag(id.tag())
        .drag_and_drop_source(move |_| {
            carry
                .carries(id)
                .then(|| DragAndDropTransferData::new(Carried(id)))
        })
        .on_globally_positioned(move |rect| placed.set(rect))
        .on_hover(move |over| hover.hovered(id, over))
        .on_pointer_button(PointerButton::Primary, move |event| match event.phase {
            PointerPhase::Press => {
                operations.pressed(id, area.get(), event.click_count, configuration);
            }
            PointerPhase::Click => {
                if operations.click_lands(id, area.get()) {
                    on_click(event.modifiers);
                }
            }
            PointerPhase::Drag | PointerPhase::Release => {}
        })
        .on_pointer_button(PointerButton::Secondary, move |event| {
            if event.phase == PointerPhase::Press {
                on_context();
            }
        })
}

/// Modifiers that name a control, report where the pointer is on it and run its lambdas.
///
/// A secondary press on a control opens the menu of the conversation's text unless the
/// control has a menu of its own, given with [`control_menu`](Self::control_menu) or
/// [`pressable_menu`](Self::pressable_menu).
pub trait Interactive: Sized {
    /// Names the control `id` and makes it clickable: it is a focus target Tab reaches in
    /// paint order, tests find it by its tag, the app is told of presses and the pointer
    /// over it, and `on_click` runs when it is clicked, or activated with Enter or Space
    /// while it holds the keyboard. Dragged away when the app [carries](Operations::carries) it.
    #[track_caller]
    fn control(self, id: FocusId, on_click: impl Fn() + 'static) -> Self;
    /// Like [`control`](Self::control), with `on_click` told the modifier keys held, for a
    /// row the reader picks several of with Shift or Control.
    #[track_caller]
    fn control_with(self, id: FocusId, on_click: impl Fn(Modifiers) + 'static) -> Self;
    /// Like [`control_with`](Self::control_with), with `on_context` run when the secondary
    /// button goes down on the control: what opens its menu.
    #[track_caller]
    fn control_menu(
        self,
        id: FocusId,
        on_click: impl Fn(Modifiers) + 'static,
        on_context: impl Fn() + 'static,
    ) -> Self;
    /// Like [`control`](Self::control), without making the area a focus target, for a
    /// control whose keyboard focus is held by what it holds, such as a text field's box.
    fn pressable(self, id: FocusId, on_click: impl Fn() + 'static) -> Self;
    /// Like [`pressable`](Self::pressable), with `on_context` run when the secondary button
    /// goes down on the area: what opens its menu.
    fn pressable_menu(
        self,
        id: FocusId,
        on_click: impl Fn() + 'static,
        on_context: impl Fn() + 'static,
    ) -> Self;
    /// Marks the area as message `index` of the conversation: tests find it by the tag of
    /// [`CHAT_TEXT`](crate::state::CHAT_TEXT), and the app finds the message under a point
    /// among the places of [`message_area_id`](crate::message_menu::message_area_id).
    fn message_area(self, index: usize) -> Self;
}

/// Names the control `id`, a focus target activated with Enter or Space while it holds the
/// keyboard, and makes `on_click` its click and `on_context` its secondary press.
#[track_caller]
fn focus_control(
    modifier: Modifier,
    id: FocusId,
    on_click: Rc<dyn Fn(Modifiers)>,
    on_context: Rc<dyn Fn()>,
) -> Modifier {
    let keyed = Rc::clone(&on_click);
    let focus = focus::follow_focus(id, true);
    let held = focus.clone();
    modifier
        .on_key_event(move |event| {
            let taken = held.is_focused() && activates(event);
            if taken {
                keyed(event.modifiers);
            }
            taken
        })
        .chain(focus::reports_focus(id))
        .focus_target(&focus)
        .chain(reporting(id, on_click, on_context))
}

impl Interactive for Modifier {
    #[track_caller]
    fn control(self, id: FocusId, on_click: impl Fn() + 'static) -> Self {
        focus_control(self, id, Rc::new(move |_| on_click()), message_menu())
    }

    #[track_caller]
    fn control_with(self, id: FocusId, on_click: impl Fn(Modifiers) + 'static) -> Self {
        focus_control(self, id, Rc::new(on_click), message_menu())
    }

    #[track_caller]
    fn control_menu(
        self,
        id: FocusId,
        on_click: impl Fn(Modifiers) + 'static,
        on_context: impl Fn() + 'static,
    ) -> Self {
        focus_control(self, id, Rc::new(on_click), Rc::new(on_context))
    }

    fn pressable(self, id: FocusId, on_click: impl Fn() + 'static) -> Self {
        self.pressable_menu(id, on_click, {
            let open = message_menu();
            move || open()
        })
    }

    fn pressable_menu(
        self,
        id: FocusId,
        on_click: impl Fn() + 'static,
        on_context: impl Fn() + 'static,
    ) -> Self {
        self.chain(reporting(
            id,
            Rc::new(move |_| on_click()),
            Rc::new(on_context),
        ))
    }

    fn message_area(self, index: usize) -> Self {
        crate::bounds::placed(
            crate::message_menu::message_area_id(index),
            self.test_tag(crate::state::CHAT_TEXT.tag()),
        )
    }
}

#[cfg(test)]
mod tests;
