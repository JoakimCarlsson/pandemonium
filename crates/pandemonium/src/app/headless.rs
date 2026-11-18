//! The app with no window: its interface laid out by rustle's embedded
//! interface instead of the windows of a [`rustle::Host`], its windows and
//! clipboard stood in for by ones that keep what was asked of them, and a
//! rig that feeds it input the way the event loop does, through the same
//! interface and the same handling of each event.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use pm_components::kit::report::Report;
use pm_components::kit::{FocusId, Key, KeyEvent, Modifiers};
use pm_components::state::WorkspaceState;
use pm_pages::Screen;
use rustle::{Embedded, Offset, PixelRounding, PointerButton};
use winit::event::MouseButton;
use winit::window::{ResizeDirection, Window};

use super::App;
use super::asides::Aside;
use super::host::{Input, Shared, aside_size, aside_view, main_view, slot};
use super::port::{Board, Port};
use crate::work::Work;

/// A window that is not there: it keeps its size and state, and records
/// what the app asked of it.
#[derive(Debug)]
pub(super) struct StandIn {
    /// How wide and tall its surface is, in logical pixels.
    pub(super) size: Cell<(f32, f32)>,
    /// Whether it is maximized.
    pub(super) maximized: Cell<bool>,
    /// Whether it is minimized.
    pub(super) minimized: Cell<bool>,
    /// Whether it has the keyboard.
    pub(super) focused: Cell<bool>,
    /// How often it was brought to the front.
    pub(super) raised: Cell<u32>,
    /// How often the reader's attention was drawn to it.
    pub(super) attention: Cell<u32>,
    /// How often the pointer was let move it.
    pub(super) moved: Cell<u32>,
    /// The edges the pointer was let resize it by, in order.
    pub(super) resized: RefCell<Vec<ResizeDirection>>,
    /// Where the input method was last told the caret is.
    pub(super) ime_area: Cell<Option<(f32, f32, f32, f32)>>,
}

impl StandIn {
    /// A window `size` logical pixels across that floats and has the keyboard.
    fn new(size: (f32, f32)) -> Self {
        Self {
            size: Cell::new(size),
            maximized: Cell::new(false),
            minimized: Cell::new(false),
            focused: Cell::new(true),
            raised: Cell::new(0),
            attention: Cell::new(0),
            moved: Cell::new(0),
            resized: RefCell::new(Vec::new()),
            ime_area: Cell::new(None),
        }
    }
}

impl Port for StandIn {
    /// None: there is no native window.
    fn native(&self) -> Option<Arc<dyn Window>> {
        None
    }

    /// One: a logical pixel is a physical one.
    fn scale(&self) -> f32 {
        1.0
    }

    /// The size it keeps.
    fn logical_size(&self) -> (f32, f32) {
        self.size.get()
    }

    /// Whether it was maximized.
    fn is_maximized(&self) -> bool {
        self.maximized.get()
    }

    /// Whether it is not maximized.
    fn floats(&self) -> bool {
        !self.maximized.get()
    }

    /// Whether it has the keyboard.
    fn has_focus(&self) -> bool {
        self.focused.get()
    }

    /// Keeps whether it is maximized.
    fn set_maximized(&self, maximized: bool) {
        self.maximized.set(maximized);
    }

    /// Keeps whether it is minimized.
    fn set_minimized(&self, minimized: bool) {
        self.minimized.set(minimized);
    }

    /// Counts it being brought forward, and gives it the keyboard.
    fn focus_window(&self) {
        self.raised.set(self.raised.get() + 1);
        self.focused.set(true);
    }

    /// Counts the reader's attention being drawn to it.
    fn request_attention(&self) {
        self.attention.set(self.attention.get() + 1);
    }

    /// Counts the pointer being let move it.
    fn drag_window(&self) {
        self.moved.set(self.moved.get() + 1);
    }

    /// Records the edge the pointer was let resize it by.
    fn drag_resize(&self, direction: ResizeDirection) {
        self.resized.borrow_mut().push(direction);
    }

    /// Keeps where the input method was told the caret is.
    fn set_ime_area(&self, area: (f32, f32, f32, f32)) {
        self.ime_area.set(Some(area));
    }
}

/// A clipboard kept in memory, shared with whoever holds the other end.
#[derive(Clone, Debug, Default)]
pub(super) struct Memory {
    /// The text on it.
    pub(super) text: Rc<RefCell<Option<String>>>,
    /// The files on it.
    pub(super) files: Rc<RefCell<Vec<PathBuf>>>,
}

impl Board for Memory {
    /// The text kept.
    fn text(&mut self) -> Option<String> {
        self.text.borrow().clone()
    }

    /// Keeps `text`.
    fn set_text(&mut self, text: String) {
        *self.text.borrow_mut() = Some(text);
    }

    /// The files kept.
    fn files(&mut self) -> Vec<PathBuf> {
        self.files.borrow().clone()
    }

    /// Never a picture.
    fn image(&mut self) -> Option<arboard::ImageData<'static>> {
        None
    }
}

/// The interface of every window laid out with no window: the main one's,
/// and that of each window beside it while what it shows is open.
pub(super) struct Screens {
    /// The main window's interface.
    pub(super) main: Embedded,
    /// The interface of each window beside the main one, while it is open.
    pub(super) asides: [Option<Embedded>; 2],
    /// The stand-in of the main window and of each window beside it.
    pub(super) ports: [Rc<StandIn>; 3],
}

/// An interface laid out with no window, of `root`, at whole logical pixels.
fn embedded(root: impl FnMut() + 'static) -> Embedded {
    let mut ui = Embedded::without_renderer(root, || {});
    ui.set_pixel_rounding(PixelRounding::Logical);
    ui
}

/// Gives `ui` the size `size`, in logical pixels, when it differs from the one it has.
fn resize(ui: &mut Embedded, size: (f32, f32)) {
    let whole = (size.0.round() as u32, size.1.round() as u32);
    let held = ui.size();
    if (held.width as u32, held.height as u32) != whole {
        ui.resize(whole, 1.0);
    }
}

impl Screens {
    /// The interfaces of what `shared` holds, the main window `size` across.
    fn new(shared: &Shared, size: (f32, f32)) -> Self {
        let main_shared = shared.clone();
        let mut main = embedded(move || main_view(&main_shared));
        resize(&mut main, size);
        Self {
            main,
            asides: [None, None],
            ports: [
                Rc::new(StandIn::new(size)),
                Rc::new(StandIn::new(Aside::Settings.opening_size())),
                Rc::new(StandIn::new(Aside::Picker.opening_size())),
            ],
        }
    }

    /// Composes every interface again at the next wake.
    pub(super) fn invalidate(&mut self) {
        self.main.invalidate();
        for ui in self.asides.iter_mut().flatten() {
            ui.invalidate();
        }
    }

    /// Opens and closes the interface of each window beside the main one as
    /// `shared` says, and composes and lays out each.
    pub(super) fn wake(&mut self, shared: &Shared) {
        resize(&mut self.main, shared.size_of(None));
        self.main.update();
        for aside in Aside::ALL {
            let open = shared.reads(|screen| aside.is_open(screen));
            let held = &mut self.asides[slot(aside)];
            if !open {
                *held = None;
                continue;
            }
            let ui = held.get_or_insert_with(|| {
                let aside_shared = shared.clone();
                embedded(move || aside_view(&aside_shared, aside))
            });
            resize(ui, aside_size(shared, aside));
            ui.update();
        }
    }

    /// The stand-in of the window of `aside`, while it is open.
    pub(super) fn port_of(&self, aside: Aside) -> Option<Rc<dyn Port>> {
        self.asides[slot(aside)].as_ref()?;
        Some(Rc::clone(&self.ports[slot(aside) + 1]) as Rc<dyn Port>)
    }

    /// The interface of the main window, or of `aside`'s.
    fn ui(&mut self, aside: Option<Aside>) -> Option<&mut Embedded> {
        match aside {
            None => Some(&mut self.main),
            Some(aside) => self.asides[slot(aside)].as_mut(),
        }
    }
}

/// The size of the main window's frame the rig opens it at, as the snapshots are.
pub(super) const SIZE: (f32, f32) = (1280.0, 800.0);

/// The app running headless, fed input the way the event loop feeds it:
/// each event goes to the interface of its window first, then to the app.
pub(super) struct Rig {
    /// The app.
    pub(super) app: App,
    /// The clipboard the app reads and writes.
    pub(super) clipboard: Memory,
    /// The modifier keys held.
    modifiers: Modifiers,
}

impl Rig {
    /// The app showing `screen`, its main window's frame `size` across, at rest.
    pub(super) fn new(screen: Screen, size: (f32, f32)) -> Self {
        pm_components::set_floating(true);
        let work = Work::offline(Arc::new(|| {}));
        let Screen::Workspace(held) = &screen;
        let window_settings = held.window.clone();
        let editor = held.editor.clone();
        let mut app = App::assemble(screen, work, window_settings, editor, None);
        let band = pm_components::shadow_margin();
        let outer = (size.0 + 2.0 * band, size.1 + 2.0 * band);
        app.shell.set_size(None, outer);
        let screens = Screens::new(&app.shell.shared, outer);
        app.window = Some(Rc::clone(&screens.ports[0]) as Rc<dyn Port>);
        app.shell.headless = Some(screens);
        let clipboard = Memory::default();
        app.clipboard = Some(Box::new(clipboard.clone()));
        app.apply_appearance();
        app.recompose = true;
        let mut rig = Self {
            app,
            clipboard,
            modifiers: Modifiers::default(),
        };
        rig.settle();
        rig
    }

    /// The app showing the screen of the snapshot `shot`, at the snapshots' size.
    pub(super) fn shot(shot: crate::snapshot::Shot) -> Self {
        Self::new(shot.screen(), SIZE)
    }

    /// Lets the event loop come to rest, as it does after every turn, twice
    /// so what the first turn asked for is composed.
    pub(super) fn settle(&mut self) {
        for _ in 0..2 {
            let _ = self.app.come_to_rest();
        }
    }

    /// Moves the app's clock on by `ms` milliseconds, for what waits on time
    /// such as tooltips, double presses and the card about a word of code,
    /// and comes to rest.
    pub(super) fn wait(&mut self, ms: u64) {
        let by = Duration::from_millis(ms);
        self.app.started = self.app.started.checked_sub(by).unwrap_or(self.app.started);
        if let Some(at) = self.app.title_press {
            self.app.title_press = at.checked_sub(by);
        }
        if let Some((id, at)) = self.app.last_activation {
            self.app.last_activation = at.checked_sub(by).map(|at| (id, at));
        }
        if let Some((word, at)) = self.app.hover_rest {
            self.app.hover_rest = at.checked_sub(by).map(|at| (word, at));
        }
        if let Some(at) = self.app.hover_leaving {
            self.app.hover_leaving = at.checked_sub(by);
        }
        self.app.recompose = true;
        self.settle();
    }

    /// Hands `input` of the main window, or of `aside`'s, to its interface
    /// through `feed`, then to the app, and lets the loop come to rest.
    fn send(&mut self, aside: Option<Aside>, input: Input, feed: impl FnOnce(&mut Embedded)) {
        self.app.set_frame();
        if let Some(ui) = self
            .app
            .shell
            .headless
            .as_mut()
            .and_then(|screens| screens.ui(aside))
        {
            feed(ui);
        }
        self.app.after_interface(aside, input);
        self.settle();
    }

    /// Where `at`, in the coordinates of the frame of the main window or of
    /// `aside`'s, is in the window's own: past the band that holds the main
    /// window's shadow.
    fn outer(aside: Option<Aside>, at: (f32, f32)) -> (f32, f32) {
        let band = if aside.is_none() {
            pm_components::shadow_margin()
        } else {
            0.0
        };
        (at.0 + band, at.1 + band)
    }

    /// Moves the pointer to `at` in the main window.
    pub(super) fn move_to(&mut self, at: (f32, f32)) {
        self.move_in(None, at);
    }

    /// Moves the pointer to `at` in the main window, or in `aside`'s.
    pub(super) fn move_in(&mut self, aside: Option<Aside>, at: (f32, f32)) {
        let at = Self::outer(aside, at);
        self.send(aside, Input::Moved(at), |ui| {
            ui.pointer_moved(Offset::new(at.0, at.1));
        });
    }

    /// Takes the pointer out of the main window.
    pub(super) fn leave(&mut self) {
        self.send(None, Input::Left, |ui| ui.pointer_left());
    }

    /// Presses `button` at `at` in the main window, or in `aside`'s, or lets
    /// it go when not `pressed`.
    pub(super) fn button_in(
        &mut self,
        aside: Option<Aside>,
        button: MouseButton,
        pressed: bool,
        at: (f32, f32),
    ) {
        let at = Self::outer(aside, at);
        let pointer = match button {
            MouseButton::Right => Some(PointerButton::Secondary),
            MouseButton::Middle => Some(PointerButton::Middle),
            MouseButton::Left => Some(PointerButton::Primary),
            MouseButton::Back => Some(PointerButton::Back),
            MouseButton::Forward => Some(PointerButton::Forward),
            _ => None,
        };
        self.send(aside, Input::Button(button, pressed, at), |ui| {
            ui.pointer_moved(Offset::new(at.0, at.1));
            if let Some(pointer) = pointer {
                ui.pointer_button(pointer, pressed);
            }
        });
    }

    /// Moves to `at` in the main window and presses and lets go of the primary button there.
    pub(super) fn click(&mut self, at: (f32, f32)) {
        self.click_in(None, at);
    }

    /// Moves to `at` in the main window, or in `aside`'s, and presses and
    /// lets go of the primary button there.
    pub(super) fn click_in(&mut self, aside: Option<Aside>, at: (f32, f32)) {
        self.move_in(aside, at);
        self.button_in(aside, MouseButton::Left, true, at);
        self.button_in(aside, MouseButton::Left, false, at);
    }

    /// Presses and lets go of the secondary button at `at` in the main window.
    pub(super) fn right_click(&mut self, at: (f32, f32)) {
        self.move_to(at);
        self.button_in(None, MouseButton::Right, true, at);
        self.button_in(None, MouseButton::Right, false, at);
    }

    /// Presses the primary button at `from` in the main window, moves to
    /// `to` in `steps` steps holding it, and lets it go there.
    pub(super) fn drag(&mut self, from: (f32, f32), to: (f32, f32), steps: u32) {
        self.move_to(from);
        self.button_in(None, MouseButton::Left, true, from);
        for step in 1..=steps.max(1) {
            let part = step as f32 / steps.max(1) as f32;
            let at = (
                from.0 + (to.0 - from.0) * part,
                from.1 + (to.1 - from.1) * part,
            );
            self.move_to(at);
        }
        self.button_in(None, MouseButton::Left, false, to);
    }

    /// Turns the wheel where the pointer is in the main window so the content moves `down` logical pixels up.
    pub(super) fn wheel(&mut self, down: f32) {
        self.send(None, Input::Wheel(0.0, -down), |ui| {
            ui.wheel(Offset::new(0.0, -down));
        });
    }

    /// Holds `modifiers` down from now on, in every window.
    pub(super) fn hold(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
        let held = rustle_modifiers(modifiers);
        self.send(None, Input::Modifiers(modifiers), |ui| {
            ui.set_modifiers(held)
        });
    }

    /// Presses and lets go of `key` in the main window with the modifiers held.
    pub(super) fn key(&mut self, key: Key) {
        self.key_in(None, key);
    }

    /// Presses and lets go of `key` in the main window, or in `aside`'s,
    /// with the modifiers held.
    pub(super) fn key_in(&mut self, aside: Option<Aside>, key: Key) {
        for pressed in [true, false] {
            let event = KeyEvent {
                key,
                modifiers: self.modifiers,
                pressed,
            };
            let window = rustle_key(event);
            self.send(aside, Input::Key(event), |ui| {
                ui.key_event(window);
            });
        }
    }

    /// Presses `key` with `modifiers` held, then lets the modifiers go.
    pub(super) fn chord(&mut self, modifiers: Modifiers, key: Key) {
        self.hold(modifiers);
        self.key(key);
        self.hold(Modifiers::default());
    }

    /// Types `text` a key at a time into the main window.
    pub(super) fn type_text(&mut self, text: &str) {
        self.type_in(None, text);
    }

    /// Types `text` a key at a time into the main window, or into `aside`'s.
    pub(super) fn type_in(&mut self, aside: Option<Aside>, text: &str) {
        for character in text.chars() {
            let key = match character {
                ' ' => Key::Space,
                '\n' => Key::Enter,
                other => Key::Character(other),
            };
            self.key_in(aside, key);
        }
    }

    /// Lets an input method show `text` as being composed in the main window.
    pub(super) fn preedit(&mut self, text: &str) {
        let held = text.to_owned();
        self.send(None, Input::Preedit(held), |ui| ui.preedit(text, None));
    }

    /// Lets an input method commit `text` in the main window.
    pub(super) fn commit(&mut self, text: &str) {
        let held = text.to_owned();
        self.send(None, Input::Commit(held), |ui| {
            ui.commit(text);
        });
    }

    /// Lets go of a drag of `paths` from another application at `at` in the main window.
    pub(super) fn drop_files(&mut self, at: (f32, f32), paths: Vec<PathBuf>) {
        let outer = Self::outer(None, at);
        self.send(None, Input::Moved(outer), |ui| {
            ui.pointer_moved(Offset::new(outer.0, outer.1));
            ui.files_dropped(Offset::new(outer.0, outer.1), paths);
        });
    }

    /// The desktop asks the main window, or `aside`'s, to close.
    pub(super) fn close(&mut self, aside: Option<Aside>) {
        self.send(aside, Input::Close, |_| {});
    }

    /// The desktop resizes the main window's frame to `size`.
    pub(super) fn resize(&mut self, size: (f32, f32)) {
        let band = pm_components::shadow_margin();
        let outer = (size.0 + 2.0 * band, size.1 + 2.0 * band);
        self.window().size.set(outer);
        self.send(None, Input::Resized(outer), |_| {});
    }

    /// The stand-in of the main window.
    pub(super) fn window(&self) -> Rc<StandIn> {
        Rc::clone(&self.screens().ports[0])
    }

    /// The stand-in of `aside`'s window.
    pub(super) fn aside_window(&self, aside: Aside) -> Rc<StandIn> {
        Rc::clone(&self.screens().ports[slot(aside) + 1])
    }

    /// The headless interfaces.
    fn screens(&self) -> &Screens {
        self.app
            .shell
            .headless
            .as_ref()
            .expect("the rig runs headless")
    }

    /// Whether the window of `aside` is open.
    pub(super) fn aside_open(&self, aside: Aside) -> bool {
        self.screens().asides[slot(aside)].is_some()
    }

    /// What the last frame of the main window found on screen, in its frame's coordinates.
    pub(super) fn report(&self) -> Report {
        self.app.report_of(None)
    }

    /// What the last frame of `aside`'s window found on screen.
    pub(super) fn aside_report(&self, aside: Aside) -> Report {
        self.app.report_of(Some(aside))
    }

    /// The middle of what `id` names in the main window.
    ///
    /// # Panics
    ///
    /// When the last frame drew nothing by that name.
    pub(super) fn center(&self, id: FocusId) -> (f32, f32) {
        center_in(&self.report(), id)
    }

    /// The middle of what `id` names in `aside`'s window.
    ///
    /// # Panics
    ///
    /// When the window's last frame drew nothing by that name.
    pub(super) fn aside_center(&self, aside: Aside, id: FocusId) -> (f32, f32) {
        center_in(&self.aside_report(aside), id)
    }

    /// Clicks the middle of what `id` names in the main window.
    pub(super) fn click_on(&mut self, id: FocusId) {
        let at = self.center(id);
        self.click(at);
    }

    /// Whether the last frame of the main window drew what `id` names.
    pub(super) fn drawn(&self, id: FocusId) -> bool {
        self.report().drawn(id)
    }

    /// What has the keyboard.
    pub(super) fn focused(&self) -> Option<FocusId> {
        self.app.focus.current()
    }

    /// Reads the screen through `read`.
    pub(super) fn read<T>(&self, read: impl FnOnce(&Screen) -> T) -> T {
        self.app.model.read(read)
    }

    /// Where the last frame of the main window, or of `aside`'s, drew text
    /// holding `words`, in its frame's coordinates.
    pub(super) fn text_at(&self, aside: Option<Aside>, words: &str) -> Option<(f32, f32)> {
        let ui = match aside {
            None => &self.screens().main,
            Some(aside) => self.screens().asides[slot(aside)].as_ref()?,
        };
        let band = Self::outer(aside, (0.0, 0.0)).0;
        ui.commands().iter().find_map(|command| match command {
            rustle::DrawCommand::Text { x, y, content, .. } if content.contains(words) => {
                Some((x - band, y - band))
            }
            rustle::DrawCommand::RichText { x, y, spans, .. }
                if spans
                    .iter()
                    .map(|span| span.text.as_str())
                    .collect::<String>()
                    .contains(words) =>
            {
                Some((x - band, y - band))
            }
            _ => None,
        })
    }

    /// Whether the last frame of the main window drew text holding `words`.
    pub(super) fn shows(&self, words: &str) -> bool {
        self.text_at(None, words).is_some()
    }

    /// Reads the workspace through `read`.
    pub(super) fn state<T>(&self, read: impl FnOnce(&WorkspaceState) -> T) -> T {
        self.read(|screen| {
            let Screen::Workspace(state) = screen;
            read(state)
        })
    }

    /// The pointer shape the main window shows.
    pub(super) fn cursor(&self) -> rustle::CursorIcon {
        let decided = self.app.shell.shared.cursor();
        decided.unwrap_or_else(|| self.screens().main.cursor())
    }
}

/// The middle of what `id` names in `report`: the area that takes presses,
/// or the view of the field of that name.
///
/// # Panics
///
/// When `report` has nothing by that name.
fn center_in(report: &Report, id: FocusId) -> (f32, f32) {
    let rect = report
        .region(id)
        .or_else(|| report.field(id).map(|field| field.view))
        .unwrap_or_else(|| panic!("nothing drawn as {id:?}"));
    (
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    )
}

/// Rustle's modifiers for the app's.
fn rustle_modifiers(modifiers: Modifiers) -> rustle::Modifiers {
    rustle::Modifiers {
        shift: modifiers.shift,
        ctrl: modifiers.control,
        alt: modifiers.alt,
        meta: modifiers.logo,
    }
}

/// Rustle's key event for the app's `event`.
fn rustle_key(event: KeyEvent) -> rustle::KeyEvent {
    use rustle::Key as Window;
    let key = match event.key {
        Key::Character(character) => Window::Character(character.to_string()),
        Key::Space => Window::Character(" ".to_owned()),
        Key::Enter => Window::Enter,
        Key::Escape => Window::Escape,
        Key::Tab => Window::Tab,
        Key::Backspace => Window::Backspace,
        Key::Delete => Window::Delete,
        Key::Home => Window::Home,
        Key::End => Window::End,
        Key::PageUp => Window::PageUp,
        Key::PageDown => Window::PageDown,
        Key::ArrowUp => Window::ArrowUp,
        Key::ArrowDown => Window::ArrowDown,
        Key::ArrowLeft => Window::ArrowLeft,
        Key::ArrowRight => Window::ArrowRight,
        Key::Function(number) => Window::F(number),
    };
    rustle::KeyEvent {
        pressed: event.pressed,
        ..rustle::KeyEvent::press(key, rustle_modifiers(event.modifiers))
    }
}
