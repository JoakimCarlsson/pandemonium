//! The app as the views' [`Operations`]: what the controls' lambdas ask of it and what
//! the controls' own gestures tell it, called by the interface while it handles the event.
//!
//! The interface runs these while the app is free, because rustle's event loop hands an
//! event to the interface, whose controls hand the app what happened to them as typed
//! operations (see [`run`](super::run)). The first operation of an event notes what the
//! keyboard holds and wakes the loop, so the app is caught up with what the event did
//! once it is handled: the text typed, the scrolls, the focus moved.

use pm_components::areas::EditorEvent;
use pm_components::target::Target;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use pm_components::browser_menu::BrowserCommand;
use pm_components::events::{Change, Operations};
use pm_components::git_menus::MenuKind;
use pm_components::pointing::{BrowserPointer, CodePointer, Over, Press, TerminalPointer};
use pm_components::terminal_menu::TerminalCommand;
use pm_components::tree_actions::TreeTarget;
use pm_editor::Action;

use super::App;
use super::asides::Aside;

/// The app as the views of one window see it.
pub(super) struct Controller {
    /// The app, while it runs.
    app: Weak<RefCell<App>>,
    /// The window beside the main one the views are in, or none for the main window.
    aside: Option<Aside>,
}

impl Controller {
    /// The operations of the views of the main window, or of `aside`'s, for `app`.
    pub(super) fn operations(app: &Rc<RefCell<App>>, aside: Option<Aside>) -> Rc<dyn Operations> {
        Rc::new(Self {
            app: Rc::downgrade(app),
            aside,
        })
    }

    /// Runs `act` on the app, in the window the views are in; the first operation of an
    /// event notes what the keyboard holds before it acts and wakes rustle's loop, so the
    /// app catches up with the event once it is handled.
    ///
    /// # Panics
    ///
    /// When the app is already borrowed, which means the interface handled an
    /// event while the app was busy, against the order [`run`](super::run) keeps.
    fn with(&self, act: impl FnOnce(&mut App)) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let mut app = app.borrow_mut();
        if !app.event_pending {
            app.note_typing();
            app.event_pending = true;
            (app.loop_wake)();
        }
        app.acted = true;
        match self.aside {
            Some(aside) => app.in_aside(aside, act),
            None => act(&mut app),
        }
    }

    /// Runs `act` on the app, in the window the views are in, timed as a move of the
    /// pointer in the main window.
    fn with_move(&self, act: impl FnOnce(&mut App)) {
        let main = self.aside.is_none();
        self.with(|app| {
            let received = app.timings.start();
            act(app);
            if main {
                app.timings.pointer(received);
            }
        });
    }
}

impl Operations for Controller {
    /// Changes the workspace and does what the change asks of the app.
    fn update(&self, change: Change) {
        self.with(|app| app.update(change));
    }

    /// Takes the press of the primary button on a control.
    fn pressed(&self, press: Press) {
        self.with(|app| app.pressed(press));
    }

    /// Lets the click count when the press was not taken by the control's own gesture and
    /// dragged nothing.
    fn click_lands(&self, click: Press) -> bool {
        let mut lands = false;
        self.with(|app| lands = app.click_lands(click));
        lands
    }

    /// Takes the press of the secondary button on a control with a menu of its own.
    fn context_pressed(&self, press: Press) {
        self.with(|app| app.context_pressed(press));
    }

    /// Takes the press of the primary button that no control took.
    fn pressed_elsewhere(&self, at: (f32, f32)) {
        self.with(|app| app.pressed_elsewhere(at));
    }

    /// Steps back or forward through history, as the mouse's buttons for that ask.
    fn history_pressed(&self, back: bool) {
        self.with(|app| app.history_pressed(back));
    }

    /// Places the caret, selects or carries text of the code as the button did.
    fn code_pointer(&self, pointer: CodePointer) {
        match pointer.stroke {
            pm_components::pointing::Stroke::Move => {
                self.with_move(|app| app.code_pointer(pointer));
            }
            _ => self.with(|app| app.code_pointer(pointer)),
        }
    }

    /// Follows the pointer over the characters of the file shown.
    fn code_hovered(&self, byte: Option<usize>) {
        self.with_move(|app| app.code_hovered(byte));
    }

    /// Follows the pointer coming over the card about a word, or leaving it.
    fn hover_card_hovered(&self, over: bool) {
        self.with(|app| app.hover_card_hovered(over));
    }

    /// Follows the primary button held on the card about a word.
    fn hover_card_held(&self, held: bool) {
        self.with(|app| app.hover_card_held(held));
    }

    /// Opens a link or marks text of the shell as the button did.
    fn terminal_pointer(&self, pointer: TerminalPointer) {
        match pointer.stroke {
            pm_components::pointing::Stroke::Move => {
                self.with_move(|app| app.terminal_pointer(pointer));
            }
            _ => self.with(|app| app.terminal_pointer(pointer)),
        }
    }

    /// Follows the pointer over the cells of the shell.
    fn terminal_hovered(&self, cell: Option<(usize, usize)>) {
        self.with_move(|app| app.terminal_hovered(cell));
    }

    /// Scrolls the shell back or forward.
    fn terminal_wheel(&self, down: f32) {
        self.with(|app| app.terminal_wheel(down));
    }

    /// Hands the pointer on to a web page.
    fn browser_pointer(&self, pointer: BrowserPointer) {
        self.with_move(|app| app.browser_pointer(pointer));
    }

    /// Notes that the press being handled is inside the conversation.
    fn conversation_pressed(&self) {
        self.with(App::conversation_pressed);
    }

    /// Offers the key to the app's own keys before whatever holds rustle's
    /// focus in the window sees it.
    fn key(&self, event: &rustle::KeyEvent) -> bool {
        let mut taken = false;
        self.with(|app| {
            app.key_heard(event.pressed);
            taken = app.key_event(event);
        });
        taken
    }

    /// Shows what the input method is composing in the field or shell holding the
    /// keyboard, which the area `id` of the window hands on.
    fn composing(&self, _id: Target, text: String) {
        self.with(|app| {
            app.input_method_heard();
            app.compose(text);
        });
    }

    /// Hands what the input method committed, which the area `id` of the window hands on,
    /// to the shell, the web page or the code editor holding the keyboard.
    fn committed(&self, _id: Target, text: String) {
        self.with(|app| {
            app.input_method_heard();
            app.commit(&text);
        });
    }

    /// Where the input method's candidates go for the area `id`: at the code editor's
    /// caret while it has the keyboard, and where the area says otherwise.
    fn caret_area(&self, id: Target) -> Option<rustle::Rect> {
        if id != Target::Editor(EditorEvent::EditorField) {
            return None;
        }
        let app = self.app.upgrade()?;
        let app = app.try_borrow().ok()?;
        app.editor_caret_area()
    }

    /// Makes the panel whose handle `id` is dragged as big as the drag says.
    fn sash_dragged(&self, id: Target, by: (f32, f32)) {
        self.with(|app| app.sash_dragged(id, by));
    }

    /// Lets go of the handle being dragged.
    fn sash_released(&self) {
        self.with(|app| app.sash_drag = None);
    }

    /// Scrolls the file shown to the place of the minimap pressed or dragged over.
    fn minimap_dragged(&self, down: f32, height: f32) {
        self.with(|app| app.minimap_dragged(down, height));
    }

    /// Closes what a press outside a popup closes.
    fn dismiss_requested(&self) {
        self.with(App::dismiss_requested);
    }

    /// Whether the control can be carried elsewhere in the window.
    fn carries(&self, id: Target) -> bool {
        let Some(app) = self.app.upgrade() else {
            return false;
        };
        let Ok(app) = app.try_borrow() else {
            return false;
        };
        app.carries(id)
    }

    /// Starts carrying the control.
    fn carry_started(&self, id: Target, at: (f32, f32)) {
        self.with(|app| app.carry_started(id, at));
    }

    /// Follows the pointer carrying a control.
    fn carry_moved(&self, at: (f32, f32), over: Option<Over>) {
        self.with(|app| app.carry_moved(at, over));
    }

    /// Puts what was carried where it was let go.
    fn carry_ended(&self, modifiers: rustle::Modifiers) {
        self.with(|app| app.carry_ended(modifiers));
    }

    /// Starts or stretches the lines selected for a comment.
    fn comment_dragged(&self, id: Target) {
        self.with(|app| app.comment_dragged(id));
    }

    /// Stretches the lines selected for a comment.
    fn comment_hovered(&self, id: Target) {
        self.with(|app| app.comment_hovered(id));
    }

    /// Ends the selection of lines for a comment.
    fn comment_drag_ended(&self) {
        self.with(App::end_comment_drag);
    }

    /// Asks the desktop to resize the window by the edge pressed.
    fn edge_pressed(&self, edge: rustle::ResizeEdge) {
        self.with(|app| app.edge_pressed(edge));
    }

    /// Shows where files held over the window would land in the project's files.
    fn files_held(&self, at: Option<(f32, f32)>, over: Option<TreeTarget>) {
        self.with(|app| {
            let at = at.map(|at| app.framed_point(at));
            app.files_hovered(at, over);
        });
    }

    /// Takes in the files let go over the window where they were let go.
    fn files_dropped(&self, paths: Vec<PathBuf>, _at: (f32, f32), over: Option<TreeTarget>) {
        self.with(|app| {
            let paths = paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            app.files_dropped(paths, over);
        });
    }

    /// Makes the window an icon on the desktop's bar.
    fn minimize_window(&self) {
        self.with(App::minimize_window);
    }

    /// Fills the screen with the window, or gives it back its size.
    fn toggle_maximized(&self) {
        self.with(App::toggle_maximized);
    }

    /// Closes the window, or what the window beside the main one shows.
    fn close_window(&self) {
        self.with(App::close_button_pressed);
    }

    /// Opens the list of commands.
    fn open_command_palette(&self) {
        self.with(|app| app.perform(Action::CommandPalette));
    }

    /// Opens the list of code actions at the caret.
    fn show_code_actions(&self) {
        self.with(|app| app.perform(Action::CodeActions));
    }

    /// Moves along the places visited.
    fn navigate(&self, delta: isize) {
        self.with(|app| app.navigate(delta));
    }

    /// Takes a row of the list over the file.
    fn take_listing(&self, row: Option<usize>) {
        self.with(|app| app.take_listing(row));
    }

    /// Opens the colour picker on a swatch.
    fn open_color_picker(&self, index: usize) {
        self.with(|app| app.open_color_picker(index));
    }

    /// Carries out a row of a field's menu.
    fn field_menu_picked(&self, row: Target) {
        self.with(|app| app.menu_action(row));
    }

    /// Carries out a row of the menu over the conversation's text.
    fn message_menu_picked(&self, row: Target) {
        self.with(|app| app.message_menu_action(row));
    }

    /// Opens the menu of what the page's capture takes.
    fn open_browser_capture_menu(&self) {
        self.with(App::open_browser_capture_menu);
    }

    /// Carries out a command of the menu over a web page.
    fn browser_menu_command(&self, command: BrowserCommand) {
        self.with(|app| app.browser_menu_command(command));
    }

    /// Carries out a command of the menu over a shell.
    fn terminal_command(&self, command: TerminalCommand) {
        self.with(|app| app.terminal_command(command));
    }

    /// Asks for files to attach.
    fn attach(&self) {
        self.with(App::attach);
    }

    /// Opens the menu of a server of the settings where its button was clicked.
    fn open_settings_menu(&self, index: usize) {
        self.with(|app| {
            if let Some(at) = app.clicked_at() {
                app.open_settings_menu(index, at);
            }
        });
    }

    /// Opens the menu of a project at the pointer, or where its button was clicked.
    fn open_project_menu(&self, project: usize, at: Option<(f32, f32)>) {
        self.with(|app| {
            let at = match at {
                Some(at) => Some(app.framed_point(at)),
                None => app.clicked_at(),
            };
            if let Some(at) = at {
                app.open_project_menu(project, at);
            }
        });
    }

    /// Opens a menu of the changes.
    fn open_git_menu(&self, kind: MenuKind, trigger: Option<Target>, at: Option<(f32, f32)>) {
        self.with(|app| {
            let at = at.map(|at| app.framed_point(at));
            app.open_git_menu(kind, trigger, at);
        });
    }

    /// Opens the menu of the pane that has the keyboard where its button was clicked.
    fn open_pane_menu(&self) {
        self.with(|app| {
            if let Some(at) = app.clicked_at() {
                app.open_pane_menu(at);
            }
        });
    }

    /// Opens the menu of a session.
    fn open_session_menu(&self, session: usize, at: (f32, f32)) {
        self.with(|app| {
            let at = app.framed_point(at);
            app.open_session_menu(session, at);
        });
    }

    /// Opens the menu of a changed file.
    fn open_change_menu(&self, index: usize, at: (f32, f32)) {
        self.with(|app| {
            let at = app.framed_point(at);
            app.open_change_menu(index, at);
        });
    }

    /// Opens the menu of a folder of changed files.
    fn open_folder_change_menu(&self, position: usize, at: (f32, f32)) {
        self.with(|app| {
            let at = app.framed_point(at);
            app.open_folder_change_menu(position, at);
        });
    }

    /// Opens the menu of a file's tab.
    fn open_tab_menu(&self, index: usize, at: (f32, f32)) {
        self.with(|app| {
            let at = app.framed_point(at);
            app.open_tab_menu(index, at);
        });
    }

    /// Opens the menu of a row of the project's files.
    fn open_tree_menu(&self, target: TreeTarget, at: (f32, f32)) {
        self.with(|app| {
            let at = app.framed_point(at);
            app.open_tree_menu(target, at);
        });
    }

    /// Opens the menu of a field.
    fn open_field_menu(&self, field: Target, at: (f32, f32), offset: Option<usize>) {
        self.with(|app| {
            let at = app.framed_point(at);
            app.open_menu(field, at, offset);
        });
    }

    /// Opens the menu of the shell.
    fn open_terminal_menu(&self, at: (f32, f32)) {
        self.with(|app| {
            let at = app.framed_point(at);
            app.open_terminal_menu(at);
        });
    }

    /// Opens the menu of the web page.
    fn open_browser_menu(&self, at: (f32, f32)) {
        self.with(|app| {
            let at = app.framed_point(at);
            app.open_browser_menu(at);
        });
    }

    /// Opens the menu of a message of the conversation, or closes the menus open.
    fn open_message_menu(&self, message: Option<usize>, at: (f32, f32)) {
        self.with(|app| app.open_message_menu(message, at));
    }
}
