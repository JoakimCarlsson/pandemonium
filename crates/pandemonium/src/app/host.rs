//! The windows on rustle: the root composable the main window and the windows
//! beside it show, which rustle's own event loop runs ([`run`]); the app's part
//! of what happens in them: what the controls tell it through their typed
//! operations, and what each window's root hears of the window itself (its
//! focus, size, scale and modifier keys and the desktop's appearance, through
//! [`LOCAL_WINDOW_INFO`], [`LOCAL_DENSITY`](rustle::LOCAL_DENSITY) and
//! [`is_system_in_dark_theme`](rustle::is_system_in_dark_theme)); and the effects
//! that wake the app for the work that waits on time and close the main window
//! when the app quits.

use pm_components::areas::EditorEvent;
use pm_components::target::Target;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use pm_components::events::{self, Operations};
use pm_components::focus;
use pm_components::theme::Tokens;
use pm_pages::Screen;
use rustle::{
    LOCAL_WINDOW_INFO, Modifiers, ShadowLayer, WindowHandle, WindowOptions, WindowShadow,
};

use super::asides::{self, Aside};
use super::controller::Controller;
use super::port::{NativePort, Port};
use super::{App, frame as window_frame};
use crate::model::Model;
use crate::scene;

/// The operations of the main window's views and of each window's beside it.
type WindowOperations = [Rc<dyn Operations>; 3];

/// What the app waits on while it is idle, as it last said: for each thing that waits for a
/// pause, what it waits on, which the effect of the main window's root that owns it is keyed
/// on, so a change starts its delay again; how often the agents' work is looked at; whether
/// the browser's pages are pumped; and whether the app quits.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub(super) struct Idle {
    /// The word of code the pointer rests on, by where it starts, and how long it rests
    /// before the card about it is asked for.
    pub(super) hover: Option<(usize, Duration)>,
    /// Whether the pointer left the card about a word and its word while the card shows.
    pub(super) hover_leaving: bool,
    /// The file, version and caret waiting for the language server to be asked where the
    /// thing at the caret is used.
    pub(super) highlight: Option<(String, i32, usize)>,
    /// What waits for the language server to be asked for the marker beside the caret's line.
    pub(super) bulb: Option<super::editing::BulbKey>,
    /// The file shown and its version while it waits to be saved by itself.
    pub(super) autosave: Option<(String, i32)>,
    /// The chords recorded on the keymap page while recording waits for another.
    pub(super) recording: Option<Vec<String>>,
    /// Whether the app looks for newer releases.
    pub(super) release: bool,
    /// Whether the view changed since it was last written.
    pub(super) view_unsaved: bool,
    /// The fingerprint of the view as the screen shows it while it is not written, which
    /// the pause before writing it waits on, so a change of the view starts the pause again.
    pub(super) view_shown: Option<u64>,
    /// How often the agents' work is to be looked at while it is under way.
    pub(super) polling: Option<Duration>,
    /// Whether the browser's pages are being pumped.
    pub(super) pumping: bool,
    /// Whether the app quits, closing the main window.
    pub(super) quit: bool,
}

/// What the root composable reads that the app changes outside rustle's own state.
#[derive(Clone)]
pub(super) struct Shared {
    /// The screen the app holds.
    model: Model,
    /// How large the main window and each window beside it are, in logical pixels, as
    /// snapshot state each window's root reads.
    sizes: rustle::State<[(f32, f32); 3]>,
    /// The theme and appearance every window is drawn in, as snapshot state each window's
    /// root reads and provides to what it composes.
    tokens: rustle::State<Tokens>,
    /// The handles of the windows beside the main one, once composed.
    handles: Rc<RefCell<[Option<WindowHandle>; 2]>>,
    /// What the views of the main window and of each window beside it hand
    /// what the user does to them to, once the app is shared with them.
    operations: Rc<RefCell<Option<WindowOperations>>>,
    /// The rustle focus of every control, which the views of every window attach.
    focus: focus::FocusRequesters,
    /// The app, once it is shared with the windows.
    app: Rc<RefCell<Weak<RefCell<App>>>>,
    /// What the app waits for while it is idle, as it last said.
    idle: rustle::State<Idle>,
    /// The field the views give the keyboard back to as it appears, as the app last said.
    restored: rustle::State<Option<Target>>,
}

impl Shared {
    /// Hands what the user does to the views of every window to `app`.
    pub(super) fn attach(&self, app: &Rc<RefCell<App>>) {
        *self.app.borrow_mut() = Rc::downgrade(app);
        *self.operations.borrow_mut() = Some([
            Controller::operations(app, None),
            Controller::operations(app, Some(Aside::Settings)),
            Controller::operations(app, Some(Aside::Picker)),
        ]);
    }

    /// The app, while it runs and once it is shared with the windows.
    fn app(&self) -> Weak<RefCell<App>> {
        self.app.borrow().clone()
    }

    /// Composes `content` under the operations of the main window's views, or of
    /// `aside`'s, once the app is attached.
    fn under_operations(&self, aside: Option<Aside>, content: impl FnOnce()) {
        let operations = self
            .operations
            .borrow()
            .as_ref()
            .map(|operations| Rc::clone(&operations[aside.map_or(0, |aside| slot(aside) + 1)]));
        focus::provide(self.focus.clone(), || match operations {
            Some(operations) => events::provide(operations, content),
            None => content(),
        });
    }

    /// What `read` makes of the screen.
    #[cfg(test)]
    pub(super) fn reads<T>(&self, read: impl FnOnce(&pm_pages::Screen) -> T) -> T {
        self.model.read(read)
    }

    /// Whether the app last said it quits.
    #[cfg(test)]
    pub(super) fn quits(&self) -> bool {
        self.idle.get().quit
    }
}

/// The rustle side of the app: what the root of every window reads.
pub(super) struct Shell {
    /// What the root reads.
    pub(super) shared: Shared,
    /// The stand-ins of the windows laid out with no window, when the app runs headless.
    #[cfg(test)]
    pub(super) headless_ports: Rc<RefCell<Option<[Rc<super::headless::StandIn>; 3]>>>,
}

/// The index of `aside` among the windows beside the main one.
pub(super) fn slot(aside: Aside) -> usize {
    match aside {
        Aside::Settings => 0,
        Aside::Picker => 1,
    }
}

/// The window key of `aside`'s window.
pub(super) fn window_key(aside: Option<Aside>) -> scene::WindowKey {
    match aside {
        None => scene::MAIN_WINDOW,
        Some(Aside::Settings) => scene::SETTINGS_WINDOW,
        Some(Aside::Picker) => scene::PICKER_WINDOW,
    }
}

impl Shell {
    /// A shell for `model` and the focus of every control in `focus`, with no window yet.
    pub(super) fn new(model: Model, focus: focus::FocusRequesters) -> Self {
        Self {
            shared: Shared {
                model,
                sizes: rustle::mutable_state([(0.0, 0.0); 3]),
                tokens: rustle::mutable_state(Tokens::default()),
                handles: Rc::new(RefCell::new([None, None])),
                operations: Rc::new(RefCell::new(None)),
                focus,
                app: Rc::new(RefCell::new(Weak::new())),
                idle: rustle::mutable_state(Idle::default()),
                restored: rustle::mutable_state(None),
            },
            #[cfg(test)]
            headless_ports: Rc::new(RefCell::new(None)),
        }
    }

    /// The handle of `aside`'s window, while it is composed.
    pub(super) fn handle_of(&self, aside: Aside) -> Option<WindowHandle> {
        self.shared.handles.borrow()[slot(aside)]
    }

    /// The window of `aside`, while it is open.
    pub(super) fn port_of(&self, aside: Aside) -> Option<Rc<dyn Port>> {
        #[cfg(test)]
        if let Some(ports) = self.headless_ports.borrow().as_ref() {
            let open = self.shared.model.read(|screen| aside.is_open(screen));
            return open.then(|| Rc::clone(&ports[slot(aside) + 1]) as Rc<dyn Port>);
        }
        let port = NativePort::new(self.handle_of(aside)?);
        port.is_open().then(|| Rc::new(port) as Rc<dyn Port>)
    }

    /// Sets the theme every window is drawn in; the roots, which provide it, are composed
    /// again only when it changed.
    pub(super) fn set_tokens(&mut self, tokens: Tokens) {
        if self.shared.tokens.with(|held| *held != tokens) {
            self.shared.tokens.set(tokens);
        }
    }

    /// Records that the window of `aside`, or the main one, is `size` logical pixels across;
    /// the window's root is composed again only when that changed.
    pub(super) fn set_size(&mut self, aside: Option<Aside>, size: (f32, f32)) {
        let index = aside.map_or(0, |aside| slot(aside) + 1);
        if self.shared.sizes.with(|sizes| sizes[index] != size) {
            self.shared.sizes.update(|sizes| sizes[index] = size);
        }
    }

    /// Says what the app waits for while it is idle, which the root's effects follow; they
    /// are composed again only when that changed.
    pub(super) fn set_idle(&self, idle: Idle) {
        if self.shared.idle.get() != idle {
            self.shared.idle.set(idle);
        }
    }

    /// Says which field the views give the keyboard back to as it appears, the one the
    /// reader was typing in when the window last closed; the root composes again only
    /// when that changed.
    pub(super) fn set_restored(&self, restored: Option<Target>) {
        if self.shared.restored.get() != restored {
            self.shared.restored.set(restored);
        }
    }
}

/// Runs the app in rustle's event loop until its main window closes: the main window
/// opens as `options` says, its root and the windows beside it composed from `app`, then
/// the app captures the view it leaves and waits for that to be written.
pub fn run(app: App) {
    let options = app.main_options();
    let app = Rc::new(RefCell::new(app));
    let shared = app.borrow().shell.shared.clone();
    shared.attach(&app);
    let close = Rc::downgrade(&app);
    rustle::set_close_handler(move || {
        close
            .upgrade()
            .and_then(|app| {
                app.try_borrow_mut()
                    .ok()
                    .map(|mut app| app.main_close_requested())
            })
            .unwrap_or(false)
    });
    rustle::run_with(options, move || root(&shared));
    let mut app = app.borrow_mut();
    app.persist_view();
    app.view_writer.flush();
    app.browser_shutdown();
}

/// Runs `act` on the app behind `app`, while it runs and is not busy.
fn with_app(app: &Weak<RefCell<App>>, act: impl FnOnce(&mut App)) {
    if let Some(app) = app.upgrade()
        && let Ok(mut app) = app.try_borrow_mut()
    {
        act(&mut app);
    }
}

/// Emits every window for what `shared` holds: the handlers that wake the app, the main
/// window's root and, while what they show is open, the settings window and the folder
/// picker's, whose close button closes what they show.
fn root(shared: &Shared) {
    wake_handlers(shared);
    main_view(shared);
    for aside in Aside::ALL {
        if !shared.model.read(|screen| aside.is_open(screen)) {
            continue;
        }
        let handles = Rc::clone(&shared.handles);
        let app = shared.app();
        let shared = shared.clone();
        rustle::key(slot(aside), || {
            rustle::window(
                aside.options(),
                move || with_app(&app, |app| app.aside_close_requested(aside)),
                move || {
                    handles.borrow_mut()[slot(aside)] = Some(rustle::current_window());
                    aside_view(&shared, aside);
                },
            );
        });
    }
}

/// What rustle's loop runs for the app outside composition: its external handler, which
/// the agents' work and the servers' connections wake through `external_waker` and which
/// drains what they brought in, and the frame observer, which wakes it once a frame was
/// drawn so the app follows the frame.
fn wake_handlers(shared: &Shared) {
    let app = shared.app();
    rustle::disposable_effect((), move || {
        let woken = app.clone();
        rustle::set_external_handler(move || with_app(&woken, App::on_wake));
        let drawn = app.clone();
        let wake = rustle::external_waker();
        rustle::set_frame_observer(move |timing| {
            with_app(&drawn, |app| app.frame_drawn(timing));
            wake();
        });
        || {}
    });
}

/// Has the app follow each change of the screen: a skippable block reads the screen's
/// state and every size held as observable state, so it runs again only when the screen or
/// a size changed and not when the root is composed for anything else, and its side effect runs [`App::screen_changed`] once after each
/// such run. The root reads the screen itself, so the block needs no scope of its own.
fn screen_effect(shared: &Shared) {
    let model = shared.model.clone();
    let app = shared.app();
    rustle::skippable((), move || {
        model.read(|screen| screen.held_sizes());
        rustle::side_effect(move || with_app(&app, App::screen_changed));
    });
}

/// Has the app ask the language server for the names of the lines of the file shown that
/// are in view, as an effect keyed on those lines and the version of the file while they
/// are wanted: while the file shown can have them asked for at all, a block of its own
/// reads the screen, the code view's scroll and its size, so a scroll or an edit composes
/// only it again, and the effect waits for the frame that lays the view out before asking.
fn names_effect(shared: &Shared) {
    if !shared.model.read(|screen| screen.names_in_view_apply()) {
        return;
    }
    let model = shared.model.clone();
    let app = shared.app();
    rustle::restartable((), move || {
        let wanted = model.read(|screen| screen.names_in_view());
        let asking = app.clone();
        rustle::launched_effect(wanted.clone(), move |scope| async move {
            if wanted.is_none() {
                return;
            }
            scope.next_frame().await;
            with_app(&asking, |app| {
                app.ask_names_in_view();
                app.flush();
            });
        });
    });
}

/// The app's idle work as effects of the main window's root: what follows each change of
/// the screen; for each thing that waits for a pause, an effect keyed on what it waits on
/// that delays on the frame clock and then acts, the view written once changes of it pause
/// among them, or at the latest a while after they began; the agents' work looked at, the
/// browser's pages pumped and newer releases looked for at their intervals; the main window
/// closed once the app quits; and the view saved when the root leaves the composition.
fn idle_effects(shared: &Shared) {
    let app = shared.app();
    screen_effect(shared);
    names_effect(shared);
    let leaving = app.clone();
    rustle::disposable_effect((), move || {
        move || {
            with_app(&leaving, |app| {
                app.persist_view();
                app.view_writer.flush();
            });
        }
    });
    let idle = shared.idle.clone();
    rustle::restartable((), move || {
        let Idle {
            hover,
            hover_leaving,
            highlight,
            bulb,
            autosave,
            recording,
            release,
            view_unsaved,
            view_shown,
            polling,
            pumping,
            quit,
        } = idle.get();
        let rest = hover.map_or(Duration::ZERO, |(_, rest)| rest);
        rested(hover, rest, &app, |app, (word, _)| app.hover_rested(word));
        rested(
            hover_leaving.then_some(()),
            super::code::HOVER_GRACE,
            &app,
            |app, ()| app.hover_left(),
        );
        rested(
            highlight,
            super::code::HIGHLIGHT_REST,
            &app,
            App::highlight_rested,
        );
        rested(bulb, super::editing::BULB_REST, &app, App::bulb_rested);
        each_rest(
            autosave,
            super::code::AUTOSAVE_REST,
            &app,
            App::autosave_rested,
        );
        rested(
            recording,
            super::code::RECORD_PAUSE,
            &app,
            App::recording_rested,
        );
        rested(view_shown, super::VIEW_SAVE_PAUSE, &app, |app, _| {
            app.view_save_rested()
        });
        rested(
            view_unsaved.then_some(()),
            super::VIEW_SAVE_BURST,
            &app,
            |app, ()| app.view_save_rested(),
        );
        every(polling, &app, App::on_wake);
        every(
            pumping.then_some(super::browser::PUMP),
            &app,
            App::pump_browser,
        );
        every(release.then_some(super::RELEASE_RECHECK), &app, |app| {
            app.release_rechecked();
            app.flush();
        });
        rustle::disposable_effect(quit, move || {
            if quit {
                WindowHandle::MAIN.close();
            }
            || {}
        });
    });
}

/// A task keyed on `waiting`, what something waits on, that waits `rest` on the frame clock
/// with a delay while there is something and then runs `act` on `app` with it, and lets the
/// app follow what that did: a change of what is waited on starts the wait again.
#[track_caller]
fn rested<K: std::hash::Hash + Clone + 'static>(
    waiting: Option<K>,
    rest: Duration,
    app: &Weak<RefCell<App>>,
    act: fn(&mut App, K),
) {
    let app = app.clone();
    rustle::launched_effect(waiting.clone(), move |scope| async move {
        let Some(waiting) = waiting else {
            return;
        };
        scope.delay(rest).await;
        with_app(&app, |app| {
            act(app, waiting);
            app.flush();
        });
    });
}

/// A task keyed on `waiting`, what something waits on, that runs `act` on `app` with it each
/// time `rest` passed on the frame clock while there is something and it stays the same, and
/// lets the app follow what that did: a change of what is waited on starts the wait again.
#[track_caller]
fn each_rest<K: std::hash::Hash + Clone + 'static>(
    waiting: Option<K>,
    rest: Duration,
    app: &Weak<RefCell<App>>,
    act: fn(&mut App, K),
) {
    let app = app.clone();
    rustle::launched_effect(waiting.clone(), move |scope| async move {
        let Some(waiting) = waiting else {
            return;
        };
        loop {
            scope.delay(rest).await;
            with_app(&app, |app| {
                act(app, waiting.clone());
                app.flush();
            });
        }
    });
}

/// A task, keyed on `interval`, that runs `act` on `app` each `interval` with a delay
/// while there is one.
#[track_caller]
fn every(interval: Option<Duration>, app: &Weak<RefCell<App>>, act: fn(&mut App)) {
    let app = app.clone();
    rustle::launched_effect(interval, move |scope| async move {
        let Some(interval) = interval else {
            return;
        };
        loop {
            scope.delay(interval).await;
            with_app(&app, act);
        }
    });
}

/// Follows the reader's scrolls of the lists the screen keeps up with, the diff's and the
/// conversation's, by observing their scroll states: a block that reads how far both are
/// scrolled, composed again alone when either moves, whose effect keyed on the offsets has
/// the app catch the screen up with them.
fn scroll_effects(shared: &Shared) {
    let lists = shared.model.read(Screen::followed_lists);
    let app = shared.app();
    rustle::restartable(lists.clone(), move || {
        let (diff, chat) = &lists;
        let offsets = (
            diff.scroll().offset().to_bits(),
            chat.scroll().offset().to_bits(),
        );
        let scrolled = app.clone();
        rustle::disposable_effect(offsets, move || {
            with_app(&scrolled, |app| {
                app.follow_scrolls();
                app.flush();
            });
            || {}
        });
    });
}

/// Emits what the main window shows for what `shared` holds, under the operations its
/// views hand what the user does to, with what the app reads of the window; the field the
/// reader was typing in when the window last closed takes the keyboard back as it appears.
pub(super) fn main_view(shared: &Shared) {
    idle_effects(shared);
    scroll_effects(shared);
    watch_window(shared, None);
    let tokens = shared.tokens.get();
    let size = shared.sizes.with(|sizes| sizes[0]);
    let restored = shared.restored.get();
    shared.under_operations(None, || {
        focus::provide_restored(restored, || {
            pm_pages::observed::provide(shared.model.observed(), || {
                shared
                    .model
                    .read(|screen| scene::main_root(screen, size.0, size.1, tokens));
            });
        });
    });
}

/// Emits what the window of `aside` shows for what `shared` holds, under the operations its
/// views hand what the user does to, with what the app reads of the window; the folder
/// picker's window gives its filter the keyboard each time the picker opens.
pub(super) fn aside_view(shared: &Shared, aside: Aside) {
    watch_window(shared, Some(aside));
    if aside == Aside::Picker {
        asides::focus_picker_filter(
            &shared.focus,
            shared.model.read(Screen::folder_picker_serial),
        );
    }
    let tokens = shared.tokens.get();
    let (width, height) = aside_size(shared, aside);
    shared.under_operations(Some(aside), || {
        shared.model.read(|screen| {
            scene::aside_root(screen, window_key(Some(aside)), width, height, tokens);
        });
    });
}

/// How large the window of `aside` is, in logical pixels: as the desktop
/// last said, or as it opens.
pub(super) fn aside_size(shared: &Shared, aside: Aside) -> (f32, f32) {
    match shared.sizes.with(|sizes| sizes[slot(aside) + 1]) {
        (0.0, 0.0) => aside.opening_size(),
        size => size,
    }
}

/// Hands the app what the window it is composed in, the main one or `aside`'s, says of
/// itself, each in an effect keyed on what it says: whether it has the keyboard, how large
/// it is, how many physical pixels a logical one is, the modifier keys held and, for the
/// main window, whether the desktop is dark. The window's information is read in a block
/// of its own, so a change of it composes only that block again.
fn watch_window(shared: &Shared, aside: Option<Aside>) {
    let app = shared.app();
    let dark = rustle::is_system_in_dark_theme();
    let density = rustle::LOCAL_DENSITY.current().density;
    rustle::restartable((aside, dark, density), move || {
        let info = LOCAL_WINDOW_INFO.current();
        let focused = info.is_window_focused();
        let size = info.container_size();
        let held = info.keyboard_modifiers();
        let told = app.clone();
        rustle::disposable_effect(focused, move || {
            with_app(&told, |app| app.window_focused(aside, focused));
            || {}
        });
        let told = app.clone();
        rustle::disposable_effect((size.width.to_bits(), size.height.to_bits()), move || {
            if size.width > 0.0 && size.height > 0.0 {
                with_app(&told, |app| {
                    app.window_resized(aside, (size.width, size.height));
                });
            }
            || {}
        });
        let told = app.clone();
        rustle::disposable_effect(density.to_bits(), move || {
            with_app(&told, |app| app.scale_changed(aside, density));
            || {}
        });
        let told = app.clone();
        rustle::disposable_effect(modifier_keys(held), move || {
            with_app(&told, |app| app.modifiers_changed(aside, held));
            || {}
        });
        if aside.is_none() {
            let told = app.clone();
            rustle::disposable_effect(dark, move || {
                with_app(&told, |app| app.system_theme_changed(dark));
                || {}
            });
        }
    });
}

/// Which of the modifier keys `modifiers` holds, as a value effects can be keyed on.
fn modifier_keys(modifiers: Modifiers) -> (bool, bool, bool, bool) {
    (
        modifiers.shift,
        modifiers.ctrl,
        modifiers.alt,
        modifiers.meta,
    )
}

/// The shadow the main window casts around its frame while it floats.
fn window_shadow() -> WindowShadow {
    WindowShadow {
        margin: pm_components::SHADOW_MARGIN,
        radius: pm_components::FRAME_RADIUS,
        layers: pm_components::WINDOW_SHADOW
            .iter()
            .map(|shadow| {
                ShadowLayer::new(
                    shadow.offset.x,
                    shadow.offset.y,
                    shadow.radius,
                    shadow.spread,
                    shadow.alpha,
                )
            })
            .collect(),
    }
}

/// Whether the desktop the event loop will run on shows a window's shadow beside it, as
/// Wayland does: the platform rustle's loop picks, where a Wayland display is named.
fn on_wayland() -> bool {
    cfg!(target_os = "linux")
        && ["WAYLAND_DISPLAY", "WAYLAND_SOCKET"]
            .iter()
            .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
}

impl App {
    /// Readies the app for rustle's loop and the options its main window opens with: the
    /// renderer's settings, where the frame's shadow goes, and the frame's size, as large
    /// as it was when the window was last closed or the opening size until it fits the
    /// screen.
    fn main_options(&self) -> WindowOptions {
        pm_components::set_shadow_outside(on_wayland());
        rustle::set_legible_text(false);
        rustle::set_frame_latency(frame_latency());
        rustle::set_pixel_rounding(rustle::PixelRounding::Logical);
        let (width, height) = window_frame::with_band(self.opening_frame);
        WindowOptions {
            title: "Pandemonium".to_owned(),
            size: (width, height),
            min_size: (480.0, 320.0),
            decorations: !pm_components::title_bar::FRAMELESS,
            transparent: pm_components::title_bar::FRAMELESS,
            app_id: Some(pm_store::app_name().to_owned()),
            ime_allowed: true,
            macos_titlebar_transparent: true,
            macos_fullsize_content: true,
            macos_title_hidden: true,
            shadow: pm_components::title_bar::FRAMELESS.then(window_shadow),
            ..WindowOptions::default()
        }
    }

    /// Takes in the main window once rustle opened it, the first time the app is woken
    /// after that: whether the desktop shows the frame's shadow beside the window, the
    /// window fitted to its screen when no size was kept, and the fonts the renderer found.
    fn window_opened(&mut self) {
        let port = NativePort::new(WindowHandle::MAIN);
        if self.opened || !port.is_open() {
            return;
        }
        self.opened = true;
        crate::startup::mark("window-created");
        if pm_components::title_bar::FRAMELESS && port.floats() {
            let outside = WindowHandle::MAIN.shadow_band(pm_components::SHADOW_MARGIN) == 0.0;
            if outside != pm_components::shadow_outside() {
                let frame = window_frame::without_band(port.logical_size());
                pm_components::set_shadow_outside(outside);
                let (width, height) = window_frame::with_band(frame);
                WindowHandle::MAIN.set_size(rustle::Size::new(width, height));
            }
        }
        if self.fit_to_screen
            && let Some(screen) = WindowHandle::MAIN.screen_size()
        {
            let (width, height) =
                window_frame::with_band(window_frame::without_band((screen.width, screen.height)));
            WindowHandle::MAIN.set_size(rustle::Size::new(width, height));
        }
        pm_components::theme::fonts_changed();
        let (all, mono) = pm_components::theme::font_families();
        self.model.change(|state| state.with_fonts(all, mono));
        crate::startup::mark("renderer-ready");
    }

    /// What rustle's external handler runs whenever the loop is woken for the app: drains
    /// what woke it. The agents' work and the servers' answers are taken in and what the
    /// reader asked of them delivered; the app catches up with an event the interface
    /// finished handling (the text typed and the focus moved are followed) and with a frame
    /// drawn; then it follows what that changed.
    pub(super) fn on_wake(&mut self) {
        self.window_opened();
        self.work.discover();
        self.work.poll(&self.model);
        if self.model.read(Screen::has_outbox) {
            self.deliver();
        }
        self.pressed_in_conversation = false;
        if std::mem::take(&mut self.event_pending) {
            self.after_interface(None, |_| {});
        }
        if std::mem::take(&mut self.frame_pending) {
            self.browsing = self.browser_tick();
        }
        self.flush();
    }

    /// Pumps the browser's pages, as their effect does at its interval while they are shown.
    fn pump_browser(&mut self) {
        self.browsing = self.browser_tick();
        self.flush();
    }

    /// What follows each change of the screen, as the side effect of the block that reads
    /// the screen runs it: the rename box's focus, the settings the screen asked for, what
    /// the reader asked of the agents and the folders, the settings written and the view's
    /// save put off until typing pauses, the place recorded, the field the reader left, the
    /// windows beside the main one followed and the browser's work looked at.
    pub(super) fn screen_changed(&mut self) {
        self.settle_rename_focus();
        if self.model.read(Screen::editor_settings_wanted) {
            let settings = self.saved_editor.clone();
            self.model
                .change(|state| state.with_editor_settings(settings));
        }
        if self.model.read(Screen::window_settings_wanted) {
            let settings = self.saved_window.clone();
            let (all, mono) = pm_components::theme::font_families();
            self.model
                .change(|state| state.with_window_settings(settings).with_fonts(all, mono));
            self.work.show_release(&self.model);
        }
        if self.model.read(Screen::has_outbox) {
            self.deliver();
        }
        self.defer_view_save();
        self.persist_window();
        self.persist_editor();
        if self.model.read(Screen::place_unrecorded) {
            self.model
                .update(|screen| *screen = std::mem::take(screen).with_place_recorded());
        }
        self.tell_field_left();
        self.sync_asides();
        self.browsing = self.browser_tick();
        self.flush();
    }

    /// Says what the app waits on while it is idle, for the root's effects: what each thing
    /// that waits for a pause waits on, how often the agents' work is to be looked at,
    /// whether the browser's pages are pumped, and whether the app quits.
    fn publish_idle(&mut self) {
        let waiting = self.model.read(|screen| screen.waiting()) || self.work.busy();
        let polling = if self.work.hearing() {
            Some(Duration::from_millis(2))
        } else if waiting {
            Some(Duration::from_millis(50))
        } else {
            None
        };
        let idle = Idle {
            hover: self.hover_waiting(),
            hover_leaving: self.hover_leaving(),
            highlight: self.highlight_waiting(),
            bulb: self.bulb_waiting(),
            autosave: self.autosave_waiting(),
            recording: self.recording_waiting(),
            release: self.release_checked(),
            view_unsaved: self.view_unsaved,
            view_shown: self.view_unsaved.then(|| self.view_fingerprint()),
            polling,
            pumping: self.browsing,
            quit: self.closing || self.model.read(Screen::quitting),
        };
        self.shell.set_idle(idle);
    }

    /// Notes a frame rustle drew taking `timing`, so the app is caught up with it when it
    /// is next woken.
    pub(super) fn frame_drawn(&mut self, timing: rustle::FrameTiming) {
        self.frame_composed();
        self.timings.frame(timing);
    }

    /// Notes that the windows were composed and laid out, so the app is caught up with
    /// the frame when it is next woken.
    pub(super) fn frame_composed(&mut self) {
        self.frame_pending = true;
    }

    /// Runs what follows a composed frame with what was laid out last, and says what the
    /// app waits on while it is idle. Nothing is composed from here: what the windows show
    /// is snapshot state their composables read, so a change of it composes its readers.
    pub(super) fn flush(&mut self) {
        self.after_compose();
        self.publish_idle();
    }

    /// What the app does after the interface composed and laid out a frame:
    /// what the old frame did after painting, from what this one painted.
    pub(super) fn after_compose(&mut self) {
        self.sync_floating();
        self.fit_terminal();
        if self.window.is_none() {
            return;
        }
        let restored = Self::restored_focus(&mut self.refocus, &self.focus, self.work.resuming());
        self.shell.set_restored(restored);
        crate::startup::scene_painted();
        crate::startup::frame_presented();
        self.place_completion();
        self.place_signature();
        self.note_conversation(true);
    }

    /// Where the input method's candidates go for the code editor while it has the
    /// keyboard: at its caret, found where its lines were drawn, in the main window's
    /// logical pixels.
    pub(super) fn editor_caret_area(&self) -> Option<rustle::Rect> {
        let id = self
            .focus
            .focused()
            .filter(|id| *id == Target::Editor(EditorEvent::EditorField))?;
        let caret = self
            .model
            .read(|screen| screen.field(id).map(|field| field.to_shown(field.caret())))?;
        let rect = self.code_lines().caret_rect(id, caret)?;
        Some(self.unframed(rect))
    }

    /// Carries out `act`, the app's part of an event of the main window, or of `aside`'s,
    /// once the interface has seen it, against the window it happened in; then catches the
    /// screen up with what the interface did to it (the text typed, the scrolls) and runs
    /// what follows a frame.
    pub(super) fn after_interface(&mut self, aside: Option<Aside>, act: impl FnOnce(&mut Self)) {
        self.follow_typing();
        match aside {
            Some(aside) => self.in_aside(aside, act),
            None => act(self),
        }
        self.flush();
    }

    /// Carries out an event of a window that the interface does not handle itself, such as
    /// the window taking the keyboard or being resized: noted, carried out by `act` and
    /// caught up with as [`after_interface`](Self::after_interface) does.
    fn window_event(&mut self, aside: Option<Aside>, act: impl FnOnce(&mut Self)) {
        self.note_typing();
        self.after_interface(aside, act);
    }

    /// The window of `aside`, or the main one, took the keyboard (`focused`) or lost it:
    /// taking it leaves any field of another window.
    fn window_focused(&mut self, aside: Option<Aside>, focused: bool) {
        self.window_event(None, |app| {
            if !focused {
                return;
            }
            let elsewhere = match aside {
                None => app.focus.focused_where(asides::is_aside),
                Some(aside) => app.focus.focused_where(|id| !aside.owns(id)),
            };
            if let Some(id) = elsewhere {
                app.focus.free_focus_of(id);
            }
        });
    }

    /// The window of `aside`, or the main one, is now `size` logical pixels across: the
    /// windows are laid out for it, and the main window's size kept for the next launch.
    fn window_resized(&mut self, aside: Option<Aside>, size: (f32, f32)) {
        self.window_event(None, |app| {
            app.shell.set_size(aside, size);
            if aside.is_none()
                && app.keeps_to_disk
                && let Some(window) = app.window.clone()
            {
                Self::remember_size(&*window, size);
            }
        });
    }

    /// The window of `aside`, or the main one, now has `scale` physical pixels to a logical one.
    fn scale_changed(&mut self, aside: Option<Aside>, scale: f32) {
        if aside.is_none() {
            self.scale = scale;
        }
    }

    /// The modifier keys `modifiers` are now held, as the window of `aside`, or the main
    /// one, says.
    fn modifiers_changed(&mut self, aside: Option<Aside>, modifiers: Modifiers) {
        if modifiers == self.modifiers {
            return;
        }
        self.window_event(aside, |app| app.hold_modifiers(modifiers));
    }

    /// Holds `modifiers`, and follows what they change: the editor's and the shell's links
    /// that reach somewhere while a modifier is held.
    fn hold_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
        self.editor_modifiers_changed();
        self.reach_terminal_link();
    }

    /// The desktop is now dark, or light when not `dark`: the windows are drawn as it
    /// asks unless their own settings say otherwise.
    fn system_theme_changed(&mut self, dark: bool) {
        self.window_event(None, |app| {
            app.system_appearance = super::appearance(dark);
            app.apply_appearance();
        });
    }

    /// Notes a key of the window, before the interface handles it: what the keyboard
    /// holds, so the screen is caught up with what the key typed once the interface has
    /// handled it, which the app does when it is next woken. `pressed` says whether the key
    /// went down.
    pub(super) fn key_heard(&mut self, pressed: bool) {
        if !self.event_pending {
            self.note_typing();
            self.event_pending = true;
            (self.loop_wake)();
        }
        self.refocus = None;
        if pressed && self.on_aside.is_none() {
            self.timings.keyboard();
        }
    }

    /// Notes what an input method did in the window, while the interface hands it to the
    /// area holding the keyboard, so the app catches up with it when it is next woken.
    pub(super) fn input_method_heard(&mut self) {
        if !self.event_pending {
            self.note_typing();
            self.event_pending = true;
            (self.loop_wake)();
        }
        if self.on_aside.is_none() {
            self.timings.keyboard();
        }
    }

    /// The desktop or the window's close button asks the main window to close: says
    /// whether it closes now, which it does once the app quits or nothing is left to ask first.
    pub(super) fn main_close_requested(&mut self) -> bool {
        if self.shell.shared.idle.get().quit {
            return true;
        }
        self.window_event(None, |app| app.closing = app.close_window());
        self.closing
    }

    /// The desktop or the window's close button asks the window of `aside` to close, which
    /// closes what it shows.
    pub(super) fn aside_close_requested(&mut self, aside: Aside) {
        self.window_event(None, |app| {
            app.model
                .update(|state| *state = aside.closed(std::mem::take(state)));
        });
    }

    /// The tokens for the theme and appearance the window is drawn in.
    pub(super) fn tokens(&self) -> Tokens {
        Tokens::new(&self.family, self.appearance)
    }
}

/// How many frames the window's surface may queue: what `PANDEMONIUM_FRAME_LATENCY` says when it is `1` or `2`, and otherwise two.
fn frame_latency() -> u32 {
    std::env::var("PANDEMONIUM_FRAME_LATENCY")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| matches!(value, 1 | 2))
        .unwrap_or(2)
}
