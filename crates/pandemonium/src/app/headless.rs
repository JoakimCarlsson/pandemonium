//! The app with no window: its interface laid out by rustle's embedded
//! interface instead of rustle's windows, the main window's composition and each
//! window's beside it composed from the same views the windows show, its
//! windows and clipboard stood in for by ones that keep what was asked of
//! them, and a rig that feeds each interface input the way rustle's event loop
//! does, so the roots hand the app its part of every event as they do in a
//! window, and wakes the app as the loop does when it comes to rest.

use pm_components::target::Target;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use pm_components::state::WorkspaceState;
use pm_components::tagged::{Tagged, focus_order, tagged_in};
use pm_pages::Screen;
use rustle::{Clipboard, ClipboardImage, Embedded, Offset, PixelRounding, PointerButton, Rect};
use rustle::{Key, KeyEvent, Modifiers};

use super::App;
use super::asides::Aside;
use super::host::{Shared, aside_view, main_view, slot};
use super::port::Port;
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
    pub(super) resized: RefCell<Vec<rustle::ResizeEdge>>,
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
        }
    }
}

impl Port for StandIn {
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
    fn drag_resize(&self, edge: rustle::ResizeEdge) {
        self.resized.borrow_mut().push(edge);
    }
}

/// A clipboard kept in memory, installed as rustle's clipboard in place of the system's and
/// shared with whoever holds the other end.
#[derive(Clone, Debug, Default)]
pub(super) struct Memory {
    /// The text on it.
    pub(super) text: Rc<RefCell<Option<String>>>,
    /// The files on it.
    pub(super) files: Rc<RefCell<Vec<PathBuf>>>,
    /// The picture on it.
    pub(super) image: Rc<RefCell<Option<ClipboardImage>>>,
}

impl Clipboard for Memory {
    /// The text kept.
    fn text(&self) -> Option<String> {
        self.text.borrow().clone()
    }

    /// Keeps `text`.
    fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = Some(text.to_owned());
    }

    /// The files kept.
    fn files(&self) -> Vec<PathBuf> {
        self.files.borrow().clone()
    }

    /// The picture kept.
    fn image(&self) -> Option<ClipboardImage> {
        self.image.borrow().clone()
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
    /// The time every interface's frame clock shows, which [`Rig::wait`] moves on, so
    /// what waits on the frame clock (a tooltip's delay, a caret's blink) does not wait on
    /// the real clock.
    pub(super) clock: Rc<Cell<Duration>>,
    /// The time the main window's interface spent composing and laying out
    /// since [`Rig::take_spent`] last took it.
    spent: rustle::UpdateTiming,
}

/// An interface laid out with no window, of `root`, at whole logical pixels, its frames
/// timed by `clock`, `size` logical pixels across.
fn embedded(
    root: impl FnMut() + 'static,
    clock: &Rc<Cell<Duration>>,
    size: (f32, f32),
) -> Embedded {
    let mut ui = Embedded::without_renderer(root, || {});
    ui.set_pixel_rounding(PixelRounding::Logical);
    let time = Rc::clone(clock);
    ui.set_clock(move || time.get());
    resize(&mut ui, size);
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
    /// Opens and closes the interface of each window beside the main one as
    /// `shared` says, and composes and lays out each, as rustle's loop does once
    /// state changed.
    fn update(&mut self, shared: &Shared) {
        self.main.update();
        let timing = self.main.update_timing();
        self.spent.compose += timing.compose;
        self.spent.layout += timing.layout;
        for aside in Aside::ALL {
            let open = shared.reads(|screen| aside.is_open(screen));
            let held = &mut self.asides[slot(aside)];
            if !open {
                *held = None;
                continue;
            }
            let clock = &self.clock;
            let ui = held.get_or_insert_with(|| {
                let aside_shared = shared.clone();
                embedded(
                    move || aside_view(&aside_shared, aside),
                    clock,
                    aside.opening_size(),
                )
            });
            ui.update();
        }
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

/// The app running headless, fed input the way rustle's event loop feeds it:
/// each event goes to the interface of its window, whose root hands the app its
/// part of it, and the app is woken as the loop wakes it when it comes to rest.
pub(super) struct Rig {
    /// The app, shared with the handlers its interface runs.
    pub(super) app: Rc<RefCell<App>>,
    /// The headless interfaces, held apart from the app so they can handle
    /// an event while the app is free for the handlers it runs.
    screens: Screens,
    /// What the roots of the windows read.
    shared: Shared,
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
        let mut app = App::assemble(screen, work, window_settings, editor, Arc::new(|| {}));
        let band = pm_components::shadow_margin();
        let outer = (size.0 + 2.0 * band, size.1 + 2.0 * band);
        app.shell.set_size(None, outer);
        let ports = [
            Rc::new(StandIn::new(outer)),
            Rc::new(StandIn::new(Aside::Settings.opening_size())),
            Rc::new(StandIn::new(Aside::Picker.opening_size())),
        ];
        app.window = Some(Rc::clone(&ports[0]) as Rc<dyn Port>);
        *app.shell.headless_ports.borrow_mut() = Some(ports.clone());
        let clipboard = Memory::default();
        rustle::set_clipboard(Rc::new(clipboard.clone()));
        app.apply_appearance();
        app.recompose = true;
        let shared = app.shell.shared.clone();
        let app = Rc::new(RefCell::new(app));
        shared.attach(&app);
        let clock = Rc::new(Cell::new(Duration::ZERO));
        let main_shared = shared.clone();
        let main = embedded(move || main_view(&main_shared), &clock, outer);
        let mut rig = Self {
            app,
            screens: Screens {
                main,
                asides: [None, None],
                ports,
                clock,
                spent: rustle::UpdateTiming::default(),
            },
            shared,
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

    /// The time the main window's interface spent composing and laying out
    /// since this was last asked, which it starts counting again from zero.
    pub(super) fn take_spent(&mut self) -> rustle::UpdateTiming {
        std::mem::take(&mut self.screens.spent)
    }

    /// Hands the open session's agent event `event` to the screen, as the
    /// work does when the agent says something, and comes to rest.
    pub(super) fn hear(&mut self, event: pm_harness::Event) {
        self.app
            .borrow()
            .model
            .update(|screen| *screen = std::mem::take(screen).heard(0, event));
        self.settle();
    }

    /// Composes and lays out every interface for what changed, as rustle's loop does.
    fn update(&mut self) {
        self.screens.update(&self.shared);
    }

    /// Lets the loop come to rest, as it does after every turn: the app is woken, as
    /// rustle's external handler wakes it, and every interface composed for what that
    /// changed, telling the app a frame was drawn; twice, so what the first turn asked
    /// for is composed and followed.
    pub(super) fn settle(&mut self) {
        for _ in 0..2 {
            self.app.borrow_mut().on_wake();
            self.update();
            self.app.borrow_mut().frame_composed();
        }
    }

    /// Whether the app last said it quits, closing the main window, as it says to the
    /// effect that closes it.
    pub(super) fn quits(&self) -> bool {
        self.shared.quits()
    }

    /// Moves the app's clock on by `ms` milliseconds, for what waits on time
    /// such as tooltips and the card about a word of code,
    /// and comes to rest.
    pub(super) fn wait(&mut self, ms: u64) {
        let by = Duration::from_millis(ms);
        let mut app = self.app.borrow_mut();
        if let Some((word, at)) = app.hover_rest {
            app.hover_rest = at.checked_sub(by).map(|at| (word, at));
        }
        if let Some(at) = app.hover_leaving {
            app.hover_leaving = at.checked_sub(by);
        }
        app.recompose = true;
        drop(app);
        self.screens.clock.set(self.screens.clock.get() + by);
        self.settle();
    }

    /// Hands what `feed` does to the interface of the main window, or of `aside`'s, whose
    /// root hands the app its part of the event, then lets the loop come to rest.
    fn feed(&mut self, aside: Option<Aside>, feed: impl FnOnce(&mut Embedded)) {
        if let Some(ui) = self.screens.ui(aside) {
            feed(ui);
        }
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
        self.feed(aside, |ui| {
            ui.pointer_moved(Offset::new(at.0, at.1));
        });
    }

    /// Takes the pointer out of the main window.
    pub(super) fn leave(&mut self) {
        self.feed(None, Embedded::pointer_left);
    }

    /// Presses `button` at `at` in the main window, or in `aside`'s, or lets
    /// it go when not `pressed`.
    pub(super) fn button_in(
        &mut self,
        aside: Option<Aside>,
        button: PointerButton,
        pressed: bool,
        at: (f32, f32),
    ) {
        let at = Self::outer(aside, at);
        self.feed(aside, |ui| {
            ui.pointer_moved(Offset::new(at.0, at.1));
            ui.pointer_button(button, pressed);
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
        self.button_in(aside, PointerButton::Primary, true, at);
        self.button_in(aside, PointerButton::Primary, false, at);
    }

    /// Presses and lets go of the secondary button at `at` in the main window.
    pub(super) fn right_click(&mut self, at: (f32, f32)) {
        self.move_to(at);
        self.button_in(None, PointerButton::Secondary, true, at);
        self.button_in(None, PointerButton::Secondary, false, at);
    }

    /// Presses the primary button at `from` in the main window, moves to
    /// `to` in `steps` steps holding it, and lets it go there.
    pub(super) fn drag(&mut self, from: (f32, f32), to: (f32, f32), steps: u32) {
        self.move_to(from);
        self.button_in(None, PointerButton::Primary, true, from);
        for step in 1..=steps.max(1) {
            let part = step as f32 / steps.max(1) as f32;
            let at = (
                from.0 + (to.0 - from.0) * part,
                from.1 + (to.1 - from.1) * part,
            );
            self.move_to(at);
        }
        self.button_in(None, PointerButton::Primary, false, to);
    }

    /// Turns the wheel where the pointer is in the main window so the content moves `down` logical pixels up.
    pub(super) fn wheel(&mut self, down: f32) {
        self.feed(None, |ui| {
            ui.wheel(Offset::new(0.0, -down));
        });
    }

    /// Holds `modifiers` down from now on, in the main window.
    pub(super) fn hold(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
        self.feed(None, |ui| ui.set_modifiers(modifiers));
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
                pressed,
                ..KeyEvent::press(key.clone(), self.modifiers)
            };
            self.feed(aside, |ui| {
                ui.key_event(event);
            });
        }
    }

    /// Presses `key` with `modifiers` held, then lets the modifiers go.
    pub(super) fn chord(&mut self, modifiers: Modifiers, key: Key) {
        self.hold(modifiers);
        self.key(key);
        self.hold(Modifiers::NONE);
    }

    /// Types `text` a key at a time into the main window.
    pub(super) fn type_text(&mut self, text: &str) {
        self.type_in(None, text);
    }

    /// Types `text` a key at a time into the main window, or into `aside`'s.
    pub(super) fn type_in(&mut self, aside: Option<Aside>, text: &str) {
        for character in text.chars() {
            let key = match character {
                '\n' => Key::Enter,
                other => Key::Character(other.to_string()),
            };
            self.key_in(aside, key);
        }
    }

    /// Lets an input method show `text` as being composed in the main window.
    pub(super) fn preedit(&mut self, text: &str) {
        self.feed(None, |ui| ui.preedit(text, None));
    }

    /// Lets an input method commit `text` in the main window.
    pub(super) fn commit(&mut self, text: &str) {
        self.feed(None, |ui| {
            ui.commit(text);
        });
    }

    /// Where the main window's interface tells the input method the caret is, in the
    /// window's logical pixels.
    pub(super) fn ime_area(&self) -> Option<Rect> {
        self.screens.main.ime_area()
    }

    /// Holds a drag of files from another application at `at` in the main window, as the
    /// desktop reports a drag it follows: the pointer itself is not heard of meanwhile.
    pub(super) fn hold_files(&mut self, at: (f32, f32)) {
        let outer = Self::outer(None, at);
        self.feed(None, |ui| {
            ui.files_dragged_over(Offset::new(outer.0, outer.1));
        });
    }

    /// Takes a drag of files from another application out of the main window without
    /// letting go of them.
    pub(super) fn files_leave(&mut self) {
        self.feed(None, Embedded::files_drag_left);
    }

    /// Lets go of a drag of `paths` from another application at `at` in the main window,
    /// held there first.
    pub(super) fn drop_files(&mut self, at: (f32, f32), paths: Vec<PathBuf>) {
        self.hold_files(at);
        let outer = Self::outer(None, at);
        self.feed(None, |ui| {
            ui.files_dropped(Offset::new(outer.0, outer.1), paths);
        });
    }

    /// The desktop asks the main window, or `aside`'s, to close, as rustle asks the main
    /// window's close handler and calls the `on_close_request` of a window beside it.
    pub(super) fn close(&mut self, aside: Option<Aside>) {
        match aside {
            None => {
                self.app.borrow_mut().main_close_requested();
            }
            Some(aside) => self.app.borrow_mut().aside_close_requested(aside),
        }
        self.settle();
    }

    /// The desktop resizes the main window's frame to `size`, and the window
    /// is drawn again at that size, as a window the desktop resized is.
    pub(super) fn resize(&mut self, size: (f32, f32)) {
        let band = pm_components::shadow_margin();
        let outer = (size.0 + 2.0 * band, size.1 + 2.0 * band);
        self.window().size.set(outer);
        resize(&mut self.screens.main, outer);
        self.settle();
        {
            let mut app = self.app.borrow_mut();
            app.recompose = true;
            app.flush();
        }
        self.update();
    }

    /// Where the caret before character `index` of the text `content`, as the
    /// main window draws it, goes: its x and the middle of its line, in the
    /// coordinates of the window's frame. Found from the text drawn last that
    /// says exactly `content`, laid out as it was drawn.
    pub(super) fn caret_in_text(&self, content: &str, index: usize) -> Option<(f32, f32)> {
        let commands = self.screens.main.commands();
        let (x, y, style, width) = commands.iter().rev().find_map(|command| match command {
            rustle::DrawCommand::Text {
                x,
                y,
                width,
                content: drawn,
                font_size,
                line_height,
                font_weight,
                font_family,
                letter_spacing,
                ligatures,
                wrap,
                ..
            } if drawn == content => Some((
                *x,
                *y,
                rustle::TextStyle {
                    font_size: *font_size,
                    line_height: *line_height,
                    font_weight: rustle::FontWeight(*font_weight),
                    font_family: *font_family,
                    letter_spacing: *letter_spacing,
                    ligatures: *ligatures,
                    wrap: *wrap,
                    ..rustle::TextStyle::default()
                },
                *width,
            )),
            _ => None,
        })?;
        let layout = rustle::layout_text(content, &style, Some(width));
        let byte = content
            .char_indices()
            .nth(index)
            .map_or(content.len(), |(byte, _)| byte);
        let caret = layout.caret(byte, rustle::Affinity::Downstream);
        let band = pm_components::shadow_margin();
        Some((x + caret.x - band, y + caret.y + caret.height / 2.0 - band))
    }

    /// The stand-in of the main window.
    pub(super) fn window(&self) -> Rc<StandIn> {
        Rc::clone(&self.screens.ports[0])
    }

    /// The stand-in of `aside`'s window.
    pub(super) fn aside_window(&self, aside: Aside) -> Rc<StandIn> {
        Rc::clone(&self.screens.ports[slot(aside) + 1])
    }

    /// Whether the window of `aside` is open.
    pub(super) fn aside_open(&self, aside: Aside) -> bool {
        self.screens.asides[slot(aside)].is_some()
    }

    /// The controls and areas the last frame of the main window, or of `aside`'s, laid
    /// out, named for tests by their [`Target`] tags, in paint order, in the coordinates of
    /// the window's frame: Compose's `onAllNodes` filtered to tagged nodes.
    pub(super) fn tagged_in(&self, aside: Option<Aside>) -> Vec<Tagged> {
        let screens = &self.screens;
        let ui = match aside {
            None => &screens.main,
            Some(aside) => match screens.asides[slot(aside)].as_ref() {
                Some(ui) => ui,
                None => return Vec::new(),
            },
        };
        let band = Self::outer(aside, (0.0, 0.0)).0;
        tagged_in(ui, -band, -band)
    }

    /// The controls and areas the last frame of the main window laid out, in paint order.
    pub(super) fn tagged(&self) -> Vec<Tagged> {
        self.tagged_in(None)
    }

    /// The first control or area named `id` the last frame of the main window, or of
    /// `aside`'s, laid out: Compose's `onNodeWithTag`.
    pub(super) fn node_in(&self, aside: Option<Aside>, id: Target) -> Option<Tagged> {
        self.tagged_in(aside).into_iter().find(|node| node.id == id)
    }

    /// The part of what `id` names in the main window that can be seen, if it was laid out.
    pub(super) fn region(&self, id: Target) -> Option<Rect> {
        self.node_in(None, id).map(|node| node.visible)
    }

    /// The controls of the main window, or of `aside`'s, in the order Tab walks them: the
    /// order their focus targets were painted in.
    pub(super) fn focus_order_in(&self, aside: Option<Aside>) -> Vec<Target> {
        focus_order(&self.tagged_in(aside))
    }

    /// The middle of what `id` names in the main window.
    ///
    /// # Panics
    ///
    /// When the last frame laid out nothing by that name.
    pub(super) fn center(&self, id: Target) -> (f32, f32) {
        self.center_in(None, id)
    }

    /// The middle of what `id` names in `aside`'s window.
    ///
    /// # Panics
    ///
    /// When the window's last frame laid out nothing by that name.
    pub(super) fn aside_center(&self, aside: Aside, id: Target) -> (f32, f32) {
        self.center_in(Some(aside), id)
    }

    /// The middle of the part of what `id` names in the main window, or in `aside`'s,
    /// that can be seen: where a test presses it.
    ///
    /// # Panics
    ///
    /// When the window's last frame laid out nothing by that name.
    fn center_in(&self, aside: Option<Aside>, id: Target) -> (f32, f32) {
        let rect = self
            .node_in(aside, id)
            .unwrap_or_else(|| panic!("nothing laid out as {id:?}"))
            .visible;
        (
            rect.origin.x + rect.size.width / 2.0,
            rect.origin.y + rect.size.height / 2.0,
        )
    }

    /// Clicks the middle of what `id` names in the main window.
    pub(super) fn click_on(&mut self, id: Target) {
        let at = self.center(id);
        self.click(at);
    }

    /// Whether the last frame of the main window laid out what `id` names.
    pub(super) fn drawn(&self, id: Target) -> bool {
        self.node_in(None, id).is_some()
    }

    /// What has the keyboard.
    pub(super) fn focused(&self) -> Option<Target> {
        self.app.borrow().focus.current()
    }

    /// Reads the screen through `read`.
    pub(super) fn read<T>(&self, read: impl FnOnce(&Screen) -> T) -> T {
        self.app.borrow().model.read(read)
    }

    /// Where the area whose bounds the screen keeps, as `area` reads them,
    /// is drawn in the main window, in its frame's coordinates.
    pub(super) fn area(
        &self,
        area: impl FnOnce(&Screen) -> Option<rustle::Rect>,
    ) -> Option<rustle::Rect> {
        let band = pm_components::shadow_margin();
        self.read(area).map(|rect| {
            rustle::Rect::new(
                Offset::new(rect.origin.x - band, rect.origin.y - band),
                rect.size,
            )
        })
    }

    /// Where the last frame of the main window, or of `aside`'s, drew text
    /// holding `words`, in its frame's coordinates.
    pub(super) fn text_at(&self, aside: Option<Aside>, words: &str) -> Option<(f32, f32)> {
        let screens = &self.screens;
        let ui = match aside {
            None => &screens.main,
            Some(aside) => screens.asides[slot(aside)].as_ref()?,
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

    /// The rectangles the main window fills with `color` now, in its outer logical pixels.
    pub(super) fn filled_with(&self, color: rustle::Color) -> Vec<Rect> {
        self.screens
            .main
            .commands()
            .iter()
            .filter_map(|command| match command {
                rustle::DrawCommand::Rect {
                    rect, color: fill, ..
                } if *fill == color => Some(*rect),
                _ => None,
            })
            .collect()
    }

    /// The pointer shape the main window shows.
    pub(super) fn cursor(&self) -> rustle::CursorIcon {
        let decided = self.app.borrow().shell.shared.cursor();
        decided.unwrap_or_else(|| self.screens.main.cursor())
    }
}
