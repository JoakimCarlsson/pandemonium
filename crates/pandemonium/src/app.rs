//! The application: the window, its renderer and the events that drive them.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use pm_acp::{AGENTS, Login, Standing};
use pm_components::{
    ADD, AgentStatus, BROWSE, MENU_COPY, MENU_CUT, MENU_PASTE, MENU_REDO, MENU_SELECT_ALL,
    MENU_UNDO, OnboardingState,
};
use pm_input::{Button, Focus, FocusId, Key, KeyEvent, Modifiers, PointerEvent, PointerState};
use pm_renderer::{DrawList, Renderer};
use pm_state::State;

use pm_text::{Edit, Shaper};
use pm_theme::{Appearance, ThemeFamily, default_family};
use pm_ui::{Cursor, Input, Output};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, Ime, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{CursorIcon, Window, WindowId};

use crate::keys::Clip;
use crate::{keys, scene};

/// The name the desktop knows the application by.
const APP_ID: &str = "pandemonium";

/// Logical width of the window when it opens.
const WIDTH: f32 = 1280.0;

/// Logical height of the window when it opens.
const HEIGHT: f32 = 800.0;

/// How soon after one press in a field a second one counts as a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// The window and what draws into it; both exist once the platform is ready.
pub struct App {
    /// The window, once it is open.
    window: Option<Arc<Window>>,
    /// What draws into the window, once it is open.
    renderer: Option<Renderer>,
    /// Shapes the text on screen.
    shaper: Shaper,
    /// The themes the app ships.
    family: ThemeFamily,
    /// Whether the desktop asks for light or dark.
    appearance: Appearance,
    /// The primitives of the frame being drawn, kept to reuse its allocation.
    list: DrawList,
    /// What the user can change on the screen.
    model: State<OnboardingState>,
    /// Set by the model whenever it changes, so the screen is drawn again.
    dirty: Rc<Cell<bool>>,
    /// The pointer as the events so far left it.
    pointer: PointerState,
    /// Which element holds keyboard focus.
    focus: Focus,
    /// Which modifier keys are held.
    modifiers: Modifiers,
    /// What the last frame found on screen.
    output: Output,
    /// The element the primary button went down on.
    pressed_on: Option<FocusId>,
    /// When the application started, which animations run on.
    started: Instant,
    /// Where the threads that start agents report what they found.
    standings: Receiver<(usize, Standing)>,
    /// Handed to each thread that starts an agent.
    reporter: Sender<(usize, Standing)>,
    /// The shape the pointer was last set to.
    cursor: Cursor,
    /// The system clipboard, once something has needed it.
    clipboard: Option<arboard::Clipboard>,
    /// The field a press began in, while the button is held, so dragging selects.
    dragging: Option<FocusId>,
    /// The last press that landed in a field, for noticing a double click.
    last_press: Option<(Instant, FocusId, u8)>,
    /// The logins each agent last offered.
    logins: [Vec<Login>; AGENTS.len()],
}

impl Default for App {
    /// An application with no window yet, whose clock starts now.
    fn default() -> Self {
        let model = State::new(OnboardingState::default());
        let dirty = Rc::new(Cell::new(false));
        let flag = Rc::clone(&dirty);
        model.subscribe(move |_| flag.set(true));
        let (reporter, standings) = mpsc::channel();
        Self {
            window: None,
            renderer: None,
            shaper: scene::shaper(),
            family: default_family(),
            appearance: Appearance::Dark,
            list: DrawList::new(),
            model,
            dirty,
            pointer: PointerState::default(),
            focus: Focus::new(),
            modifiers: Modifiers::default(),
            output: Output::default(),
            pressed_on: None,
            started: Instant::now(),
            standings,
            reporter,
            cursor: Cursor::Default,
            clipboard: None,
            dragging: None,
            last_press: None,
            logins: Default::default(),
        }
    }
}

impl App {
    /// The attributes the window is opened with.
    fn attributes() -> winit::window::WindowAttributes {
        let attributes = Window::default_attributes()
            .with_title("Pandemonium")
            .with_inner_size(LogicalSize::new(WIDTH, HEIGHT));
        #[cfg(target_os = "linux")]
        let attributes = {
            use winit::platform::wayland::WindowAttributesExtWayland;
            use winit::platform::x11::WindowAttributesExtX11;

            let attributes = WindowAttributesExtWayland::with_name(attributes, APP_ID, "");
            WindowAttributesExtX11::with_name(attributes, APP_ID, APP_ID)
        };
        attributes
    }

    /// Draws one frame and asks for the next one while anything is moving or
    /// the frame could not be presented.
    fn redraw(&mut self) {
        let (Some(window), Some(renderer)) = (self.window.as_ref(), self.renderer.as_mut()) else {
            return;
        };
        let input = Input {
            pointer: self.pointer,
            focused: self.focus.current(),
            elapsed_ms: self.started.elapsed().as_secs_f32() * 1000.0,
        };
        let (width, height) = renderer.size();
        let state = self.model.get();
        let theme = self.family.variant(self.appearance);
        self.output = scene::paint(
            width,
            height,
            &state,
            &input,
            theme,
            &mut self.shaper,
            &mut self.list,
        );
        self.focus
            .set_order(self.output.regions.iter().map(|region| region.id).collect());
        let mut scroll_to = None;
        if let Some(id) = self.focus.current()
            && let Some(field) = state.field(id)
            && let Some(held) = self.output.fields.iter().find(|held| held.id == id)
            && let Some(view) = held.view
        {
            let caret_x = held.run.caret_x(field.to_shown(field.caret()));
            let want = field.scroll_for(caret_x, held.run.width, view.width);
            if (want - field.scroll()).abs() > 0.5 {
                scroll_to = Some((id, want));
            }
            window.set_ime_cursor_area(
                LogicalPosition::new(held.x + caret_x, view.y),
                LogicalSize::new(1.0, view.height),
            );
        }
        if let Some((id, want)) = scroll_to {
            self.model
                .update(|state| *state = state.clone().edited(id, Edit::ScrollTo(want)));
        }
        if self.output.cursor != self.cursor {
            self.cursor = self.output.cursor;
            window.set_cursor(cursor_icon(self.cursor));
        }
        let presented = renderer.render(&self.list, &mut self.shaper, scene::ground(theme), || {
            window.pre_present_notify()
        });
        if self.output.animating || !presented {
            window.request_redraw();
        }
    }

    /// Carries out what pressing the element `id` means.
    fn activate(&mut self, id: FocusId) {
        if OnboardingState::is_menu_item(id) {
            self.menu_action(id);
            return;
        }
        if let Some(index) = OnboardingState::agent_of(id) {
            self.agent_action(index);
            return;
        }
        if id == BROWSE {
            self.browse();
            return;
        }
        self.model.update(|state| *state = state.clone().after(id));
    }

    /// Finds out where every agent stands, each on a thread of its own, since
    /// starting one can take a while.
    fn check_agents(&self) {
        for (index, agent) in AGENTS.iter().copied().enumerate() {
            let reporter = self.reporter.clone();
            std::thread::spawn(move || {
                let _ = reporter.send((index, agent.probe()));
            });
        }
    }

    /// Does what the row of the agent at `index` offers: signs it in,
    /// installs it or looks at it again, and reports where it ends up.
    fn agent_action(&mut self, index: usize) {
        let Some(agent) = AGENTS.get(index).copied() else {
            return;
        };
        let status = self.model.get().agents[index].clone();
        let login = self.logins[index]
            .iter()
            .find(|login| !login.id.contains("cached"))
            .or(self.logins[index].first())
            .cloned();
        self.model
            .update(|state| *state = state.clone().with_agent(index, AgentStatus::Working));
        let reporter = self.reporter.clone();
        std::thread::spawn(move || {
            let outcome = match (status, login) {
                (AgentStatus::SignedOut, Some(login)) => agent.log_in(&login),
                (AgentStatus::SignedOut, None) => Err("no way to sign in".to_owned()),
                (AgentStatus::Missing, _) => agent.install(),
                _ => Ok(()),
            };
            let standing = match outcome {
                Ok(()) => agent.probe(),
                Err(reason) => Standing::Failed(reason),
            };
            let _ = reporter.send((index, standing));
        });
    }

    /// Takes in what the agent threads have found.
    fn take_standings(&mut self) {
        while let Ok((index, standing)) = self.standings.try_recv() {
            let status = match standing {
                Standing::Ready => AgentStatus::Ready,
                Standing::SignedOut(logins) => {
                    self.logins[index] = logins;
                    AgentStatus::SignedOut
                }
                Standing::Missing => AgentStatus::Missing,
                Standing::Failed(reason) => AgentStatus::Failed(reason),
            };
            self.model
                .update(|state| *state = state.clone().with_agent(index, status));
        }
    }

    /// Asks the desktop for a folder and adds it as a project, unless the
    /// dialog is dismissed.
    fn browse(&mut self) {
        let Some(folder) = rfd::FileDialog::new()
            .set_title("Choose a project folder")
            .pick_folder()
        else {
            return;
        };
        let path = folder.to_string_lossy().into_owned();
        self.model
            .update(|state| *state = state.clone().with_project(&path));
    }

    /// Folds a pointer event in, and acts on a press that ends on the
    /// element it began on.
    fn pointer(&mut self, event: PointerEvent) {
        self.pointer.apply(event);
        match event {
            PointerEvent::Pressed(Button::Right) => {
                let on = self.pointer.position.and_then(|at| self.output.hit(at));
                match on {
                    Some(id) if OnboardingState::is_field(id) => self.open_menu(id),
                    _ => self
                        .model
                        .update(|state| *state = state.clone().without_menu()),
                }
            }
            PointerEvent::Pressed(Button::Left) => {
                let on = self.pointer.position.and_then(|at| self.output.hit(at));
                if self.model.get().menu.is_some() && !on.is_some_and(OnboardingState::is_menu_item)
                {
                    self.model
                        .update(|state| *state = state.clone().without_menu());
                }
                self.pressed_on = self.pointer.position.and_then(|at| self.output.hit(at));
                if let Some(id) = self.pressed_on
                    && OnboardingState::is_field(id)
                {
                    self.press_in_field(id);
                }
            }
            PointerEvent::Moved(x, _) => {
                if let Some(id) = self.dragging
                    && let Some(offset) = self.output.offset_in(id, x)
                {
                    self.edit_field(id, Edit::Place(offset, true));
                }
            }
            PointerEvent::Released(Button::Left) => {
                self.dragging = None;
                let released_on = self.pointer.position.and_then(|at| self.output.hit(at));
                if let (Some(pressed), Some(released)) = (self.pressed_on.take(), released_on)
                    && pressed == released
                {
                    self.focus.focus(pressed);
                    self.activate(pressed);
                }
            }
            _ => {}
        }
    }

    /// Carries out the menu row `row` on the field the menu is open over, and closes the menu.
    fn menu_action(&mut self, row: FocusId) {
        let Some(open) = self.model.get().menu else {
            return;
        };
        self.model
            .update(|state| *state = state.clone().without_menu());
        let field = open.field;
        self.focus.focus(field);
        match row {
            MENU_UNDO => self.edit_field(field, Edit::Undo),
            MENU_REDO => self.edit_field(field, Edit::Redo),
            MENU_CUT => self.clip(field, Clip::Cut),
            MENU_COPY => self.clip(field, Clip::Copy),
            MENU_PASTE => self.clip(field, Clip::Paste),
            MENU_SELECT_ALL => self.edit_field(field, Edit::SelectAll),
            _ => {}
        }
    }

    /// Opens the menu over the field `id` at the pointer, after putting the
    /// caret where the pointer is unless it is inside the selection.
    fn open_menu(&mut self, id: FocusId) {
        let Some((x, y)) = self.pointer.position else {
            return;
        };
        let Some(offset) = self.output.offset_in(id, x) else {
            return;
        };
        self.focus.focus(id);
        let inside = self.model.get().field(id).is_some_and(|field| {
            field
                .selection()
                .is_some_and(|range| range.contains(&field_text_offset(field, offset)))
        });
        if !inside {
            self.edit_field(id, Edit::Place(offset, false));
        }
        self.model
            .update(|state| *state = state.clone().with_menu(id, (x, y)));
    }

    /// Takes a press inside the field `id`: focuses it, puts the caret where
    /// the pointer is, and starts a selection that dragging extends. A second
    /// press soon after the first selects the word under it.
    fn press_in_field(&mut self, id: FocusId) {
        let Some((x, _)) = self.pointer.position else {
            return;
        };
        let Some(offset) = self.output.offset_in(id, x) else {
            return;
        };
        self.focus.focus(id);
        let clicks = match self.last_press {
            Some((at, last, clicks)) if last == id && at.elapsed() < DOUBLE_CLICK => clicks + 1,
            _ => 1,
        };
        self.last_press = Some((Instant::now(), id, clicks));
        self.dragging = Some(id);
        let edit = match clicks {
            1 => Edit::Place(offset, self.modifiers.shift),
            2 => Edit::SelectWordAt(offset),
            _ => Edit::SelectAll,
        };
        self.edit_field(id, edit);
    }

    /// Makes `edit` to the field `id`.
    fn edit_field(&mut self, id: FocusId, edit: Edit) {
        self.model
            .update(|state| *state = state.clone().edited(id, edit));
    }

    /// The system clipboard, opened on first use.
    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        self.clipboard.as_mut()
    }

    /// Carries out a clipboard shortcut in the field `id`.
    fn clip(&mut self, id: FocusId, action: Clip) {
        match action {
            Clip::Copy | Clip::Cut => {
                let selected = self
                    .model
                    .get()
                    .field(id)
                    .map(|field| field.selected().to_owned())
                    .unwrap_or_default();
                if selected.is_empty() {
                    return;
                }
                if let Some(clipboard) = self.clipboard() {
                    let _ = clipboard.set_text(selected);
                }
                if action == Clip::Cut {
                    self.edit_field(id, Edit::Backspace);
                }
            }
            Clip::Paste => {
                let pasted = self
                    .clipboard()
                    .and_then(|clipboard| clipboard.get_text().ok());
                if let Some(text) = pasted {
                    self.edit_field(id, Edit::Paste(text));
                }
            }
        }
    }

    /// Acts on a key: Tab walks the focus, Enter and Space press the focused
    /// element, Escape lets go of focus.
    fn key(&mut self, event: KeyEvent) {
        if !event.pressed {
            return;
        }
        match event.key {
            Key::Tab if event.modifiers.shift => self.focus.previous(),
            Key::Tab => self.focus.next(),
            Key::Escape if self.model.get().menu.is_some() => {
                self.model
                    .update(|state| *state = state.clone().without_menu());
            }
            Key::Escape => self.focus.clear(),
            _ => match self.focus.current() {
                Some(id) if OnboardingState::is_field(id) => {
                    if event.key == Key::Enter {
                        self.activate(ADD);
                    } else if let Some(action) = keys::clip(&event) {
                        self.clip(id, action);
                    } else if let Some(edit) = keys::edit(&event) {
                        self.edit_field(id, edit);
                    }
                }
                Some(id) if matches!(event.key, Key::Enter | Key::Space) => self.activate(id),
                _ => {}
            },
        }
    }
}

/// The window's pointer for the shape a style asks for.
fn cursor_icon(cursor: Cursor) -> CursorIcon {
    match cursor {
        Cursor::Default => CursorIcon::Default,
        Cursor::Pointer => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
        Cursor::Move => CursorIcon::Move,
        Cursor::NotAllowed => CursorIcon::NotAllowed,
        Cursor::Grab => CursorIcon::Grab,
        Cursor::Grabbing => CursorIcon::Grabbing,
        Cursor::Wait => CursorIcon::Wait,
        Cursor::Crosshair => CursorIcon::Crosshair,
        Cursor::ColResize => CursorIcon::ColResize,
        Cursor::RowResize => CursorIcon::RowResize,
    }
}

/// The byte offset in the text of `field` that the offset `shown` of what is shown stands for.
fn field_text_offset(field: &pm_text::Field, shown: usize) -> usize {
    (0..=field.text().len())
        .filter(|byte| field.text().is_char_boundary(*byte))
        .find(|byte| field.to_shown(*byte) >= shown)
        .unwrap_or(field.text().len())
}

/// The appearance the desktop asks for; dark when it does not say.
fn appearance(theme: Option<winit::window::Theme>) -> Appearance {
    match theme {
        Some(winit::window::Theme::Light) => Appearance::Light,
        _ => Appearance::Dark,
    }
}

/// Reports why the application cannot start and leaves.
fn fail_to_start(reason: &str) -> ! {
    eprintln!("pandemonium: {reason}");
    std::process::exit(1);
}

impl ApplicationHandler for App {
    /// Opens the window and builds its renderer once the platform is ready.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = match event_loop.create_window(Self::attributes()) {
            Ok(window) => Arc::new(window),
            Err(error) => fail_to_start(&format!("could not open a window: {error}")),
        };
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        match Renderer::new(window.clone(), size.width, size.height, scale) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => fail_to_start(&error.to_string()),
        }
        self.appearance = appearance(window.theme());
        self.check_agents();
        window.set_ime_allowed(true);
        window.request_redraw();
        self.window = Some(window);
    }

    /// Draws again when the model changed while no event asked for it.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.take_standings();
        let waiting = self
            .model
            .get()
            .agents
            .iter()
            .any(|status| matches!(status, AgentStatus::Checking | AgentStatus::Working));
        event_loop.set_control_flow(if waiting {
            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100))
        } else {
            ControlFlow::Wait
        });
        if self.dirty.replace(false)
            && let Some(window) = self.window.as_ref()
        {
            window.request_redraw();
        }
    }

    /// Routes window events to the renderer, the pointer and the keyboard.
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let scale = window.scale_factor() as f32;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ThemeChanged(theme) => self.appearance = appearance(Some(theme)),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height, scale);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = window.inner_size();
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height, scale);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer(PointerEvent::Moved(
                    position.x as f32 / scale,
                    position.y as f32 / scale,
                ));
            }
            WindowEvent::CursorLeft { .. } => self.pointer(PointerEvent::Left),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Right => Button::Right,
                    _ => return,
                };
                self.pointer(match state {
                    ElementState::Pressed => PointerEvent::Pressed(button),
                    ElementState::Released => PointerEvent::Released(button),
                });
            }
            WindowEvent::Ime(Ime::Commit(committed)) => {
                if let Some(id) = self.focus.current()
                    && OnboardingState::is_field(id)
                {
                    self.edit_field(id, Edit::Paste(committed));
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = keys::modifiers(modifiers.state());
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(event) =
                    keys::key_event(&event.logical_key, event.state, self.modifiers)
                {
                    self.key(event);
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                return;
            }
            _ => return,
        }
        window.request_redraw();
    }
}
