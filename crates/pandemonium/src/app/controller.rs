//! The app as the views' [`Operations`]: what the controls' lambdas ask of it and what
//! the views report of the pointer, called by the interface while it handles the event.
//!
//! The interface runs these while the app is free, because the event loop
//! hands an event to the interface before it borrows the app for its own
//! handling of it (see [`Running`](super::Running)). What an operation finds is
//! kept in [`Claims`] for the rest of the app's handling of the same event,
//! which clears them when it is done.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use pm_components::browser_menu::BrowserCommand;
use pm_components::events::{Change, Operations};
use pm_components::focus::FocusId;
use pm_components::git_menus::MenuKind;
use pm_components::terminal_menu::TerminalCommand;
use pm_components::tree_actions::TreeTarget;
use pm_editor::Action;

use super::App;
use super::asides::Aside;

/// What the controls under the pointer said about the event being handled.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Claims {
    /// The control the primary button went down on.
    pub(super) press: Option<FocusId>,
    /// Whether a control's lambda opened a menu for the secondary button.
    pub(super) context: bool,
    /// The control the primary button went down and came up on.
    pub(super) click: Option<FocusId>,
    /// Whether an operation changed the app, so a frame is due even for an
    /// event the app's own logic does not act on.
    pub(super) acted: bool,
}

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

    /// Runs `act`, which opens a menu, on the app, in the window the views are in, and
    /// keeps that a menu was opened so the app's handling of a secondary press does not
    /// open the menu of the conversation's text as well.
    fn with_menu(&self, act: impl FnOnce(&mut App)) {
        self.with(|app| {
            app.claimed.context = true;
            act(app);
        });
    }

    /// Runs `act` on the app, in the window the views are in.
    ///
    /// # Panics
    ///
    /// When the app is already borrowed, which means the interface handled an
    /// event while the app was busy, against the order [`Running`](super::Running) keeps.
    fn with(&self, act: impl FnOnce(&mut App)) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let mut app = app.borrow_mut();
        app.claimed.acted = true;
        match self.aside {
            Some(aside) => app.in_aside(aside, act),
            None => act(&mut app),
        }
    }
}

impl Operations for Controller {
    /// Changes the workspace and does what the change asks of the app.
    fn update(&self, change: Change) {
        self.with(|app| app.update(change));
    }

    /// Takes the press, and keeps the control pressed so the app's handling
    /// of the press does not look for another.
    fn pressed(&self, id: FocusId, area: rustle::Rect) {
        self.with(|app| {
            app.claimed.press = Some(id);
            app.anchor = Some((id, area));
            app.press(id);
        });
    }

    /// Lets the click count when the press began on the control and dragged nothing, and
    /// keeps the control clicked, whether it counts or not, so the app's handling of the
    /// release does not look for another.
    fn click_lands(&self, id: FocusId, area: rustle::Rect) -> bool {
        let mut lands = false;
        self.with(|app| {
            app.claimed.click = Some(id);
            app.anchor = Some((id, area));
            lands = app.click_lands(id);
        });
        lands
    }

    /// Offers the key to the app's own keys before whatever holds rustle's
    /// focus in the window sees it.
    fn key(&self, event: &rustle::KeyEvent) -> bool {
        let mut taken = false;
        self.with(|app| taken = app.key_event(event));
        taken
    }

    /// Follows the pointer coming over the control `id` or leaving it, which is
    /// how the app knows what the pointer is over.
    fn hovered(&self, id: FocusId, over: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let Ok(mut app) = app.try_borrow_mut() else {
            return;
        };
        let note = |app: &mut App| {
            app.hovered.retain(|held| *held != id);
            if over {
                app.hovered.push(id);
            }
        };
        match self.aside {
            Some(aside) => app.in_aside(aside, note),
            None => note(&mut app),
        }
    }

    /// Makes the panel whose handle `id` is dragged as big as the drag says.
    fn sash_dragged(&self, id: FocusId, by: (f32, f32)) {
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
    fn carries(&self, id: FocusId) -> bool {
        let Some(app) = self.app.upgrade() else {
            return false;
        };
        let Ok(app) = app.try_borrow() else {
            return false;
        };
        app.carries(id)
    }

    /// Starts carrying the control.
    fn carry_started(&self, id: FocusId, at: (f32, f32)) {
        self.with(|app| app.carry_started(id, at));
    }

    /// Follows the pointer carrying a control.
    fn carry_moved(&self, at: (f32, f32)) {
        self.with(|app| app.carry_moved(at));
    }

    /// Puts what was carried where it was let go.
    fn carry_ended(&self, modifiers: rustle::Modifiers) {
        self.with(|app| app.carry_ended(modifiers));
    }

    /// Starts or stretches the lines selected for a comment.
    fn comment_dragged(&self, id: FocusId) {
        self.with(|app| app.comment_dragged(id));
    }

    /// Ends the selection of lines for a comment.
    fn comment_drag_ended(&self) {
        self.with(App::end_comment_drag);
    }

    /// Asks the desktop to resize the window by the edge pressed.
    fn edge_pressed(&self, edge: rustle::ResizeEdge) {
        self.with(|app| app.edge_pressed(edge));
    }

    /// Takes a file dropped on the window, unless the drag is the desktop's
    /// own one the app follows itself.
    fn dropped(&self, path: PathBuf) {
        self.with(|app| {
            if !app.outside_offered() {
                app.file_dropped(path);
            }
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
    fn field_menu_picked(&self, row: FocusId) {
        self.with(|app| app.menu_action(row));
    }

    /// Carries out a row of the menu over the conversation's text.
    fn message_menu_picked(&self, row: FocusId) {
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

    /// Opens the menu of a server of the settings.
    fn open_settings_menu(&self, index: usize) {
        self.with(|app| app.open_settings_menu(index));
    }

    /// Opens the menu of a project.
    fn open_project_menu(&self, project: usize) {
        self.with_menu(|app| app.open_project_menu(project));
    }

    /// Opens a menu of the changes.
    fn open_git_menu(&self, kind: MenuKind, trigger: Option<FocusId>) {
        self.with_menu(|app| app.open_git_menu(kind, trigger));
    }

    /// Opens the menu of the pane that has the keyboard.
    fn open_pane_menu(&self) {
        self.with(App::open_pane_menu);
    }

    /// Opens the menu of a session.
    fn open_session_menu(&self, session: usize) {
        self.with_menu(|app| app.open_session_menu(session));
    }

    /// Opens the menu of a changed file.
    fn open_change_menu(&self, index: usize) {
        self.with_menu(|app| app.open_change_menu(index));
    }

    /// Opens the menu of a folder of changed files.
    fn open_folder_change_menu(&self, position: usize) {
        self.with_menu(|app| app.open_folder_change_menu(position));
    }

    /// Opens the menu of a file's tab.
    fn open_tab_menu(&self, index: usize) {
        self.with_menu(|app| app.open_tab_menu(index));
    }

    /// Opens the menu of a row of the project's files.
    fn open_tree_menu(&self, target: TreeTarget) {
        self.with_menu(|app| app.open_tree_menu(target));
    }

    /// Opens the menu of a field.
    fn open_field_menu(&self, field: FocusId) {
        self.with_menu(|app| app.open_menu(field));
    }

    /// Opens the menu of the shell.
    fn open_terminal_menu(&self) {
        self.with_menu(App::open_terminal_menu);
    }

    /// Opens the menu of the web page.
    fn open_browser_menu(&self) {
        self.with_menu(App::open_browser_menu);
    }

    /// Opens the menu of the conversation's text.
    fn open_message_menu(&self) {
        self.with_menu(App::open_message_menu);
    }
}
