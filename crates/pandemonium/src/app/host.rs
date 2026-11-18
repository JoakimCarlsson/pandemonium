//! The windows on rustle: the event loop's handler, which hands every event
//! to the interface and then to the app's own logic, the root composable
//! the main window and the windows beside it show, and what the app does
//! after the interface composed a frame.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use pm_components::events::{self, Handler};
use pm_components::kit::FocusId;
use pm_components::kit::frame::{self, Frame};
use pm_components::kit::report::{self, Report};
use pm_components::kit::theme::Appearance;
use pm_components::kit::theme::Tokens;
use pm_components::kit::{Button, KeyEvent, Modifiers, PointerEvent};
use pm_field::Edit;
use pm_pages::Screen;
use rustle::{CursorIcon, Host, ShadowLayer, WindowHandle, WindowOptions, WindowShadow};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::window::{
    ImeCapabilities, ImeEnableRequest, ImeRequest, ImeRequestData, Window, WindowId,
};

use super::asides::Aside;
use super::controller::{Claims, Controller};
use super::port::{NativePort, Port};
use super::{App, WHEEL_LINE, appearance, fail_to_start, frame as window_frame};
use crate::model::Model;
use crate::{keys, scene};

/// The handler of the main window's views and of each window's beside it.
type Handlers = [Rc<dyn Handler>; 3];

/// What the root composable reads that the app changes outside rustle's own state.
#[derive(Clone)]
pub(super) struct Shared {
    /// The screen the app holds.
    model: Model,
    /// How large the main window and each window beside it are, in logical pixels.
    sizes: Rc<RefCell<[(f32, f32); 3]>>,
    /// The theme and appearance every window is drawn in.
    tokens: Rc<RefCell<Tokens>>,
    /// The pointer shape the app decided on, over whatever the interface shows.
    cursor: Rc<Cell<Option<CursorIcon>>>,
    /// The handles of the windows beside the main one, once composed.
    handles: Rc<RefCell<[Option<WindowHandle>; 2]>>,
    /// The windows beside the main one their close button asked to close.
    closed: Rc<RefCell<Vec<Aside>>>,
    /// What the views of the main window and of each window beside it hand
    /// what the user does to them to, once the app is shared with them.
    handlers: Rc<RefCell<Option<Handlers>>>,
}

impl Shared {
    /// Hands what the user does to the views of every window to `app`.
    pub(super) fn attach(&self, app: &Rc<RefCell<App>>) {
        *self.handlers.borrow_mut() = Some([
            Controller::handler(app, None),
            Controller::handler(app, Some(Aside::Settings)),
            Controller::handler(app, Some(Aside::Picker)),
        ]);
    }

    /// Composes `content` under the handler of the main window's views, or of
    /// `aside`'s, once the app is attached.
    fn under_handler(&self, aside: Option<Aside>, content: impl FnOnce()) {
        let handler = self
            .handlers
            .borrow()
            .as_ref()
            .map(|handlers| Rc::clone(&handlers[aside.map_or(0, |aside| slot(aside) + 1)]));
        match handler {
            Some(handler) => events::provide(handler, content),
            None => content(),
        }
    }

    /// How large the main window is, or `aside`'s, as the desktop last said, in logical pixels.
    #[cfg(test)]
    pub(super) fn size_of(&self, aside: Option<Aside>) -> (f32, f32) {
        self.sizes.borrow()[aside.map_or(0, |aside| slot(aside) + 1)]
    }

    /// What `read` makes of the screen.
    #[cfg(test)]
    pub(super) fn reads<T>(&self, read: impl FnOnce(&Screen) -> T) -> T {
        self.model.read(read)
    }

    /// The pointer shape the app decided on over the main window, if it decided.
    #[cfg(test)]
    pub(super) fn cursor(&self) -> Option<CursorIcon> {
        self.cursor.get()
    }
}

/// The rustle side of the app: the host that owns the windows, and what its root reads.
pub(super) struct Shell {
    /// The interface and its windows, once the platform could make surfaces,
    /// shared so it can handle an event while the app is free for the
    /// handlers the event runs.
    pub(super) host: Rc<RefCell<Option<Host>>>,
    /// Whether the windows were asked to compose while they were busy with
    /// an event, so they compose once it is handled.
    deferred: Rc<Cell<bool>>,
    /// The event loop's proxy, which wakes it for the host and for the app's work,
    /// when the app runs in one.
    proxy: Option<EventLoopProxy>,
    /// What the root reads.
    pub(super) shared: Shared,
    /// The interface laid out with no window, when the app runs headless.
    #[cfg(test)]
    pub(super) headless: Rc<RefCell<Option<super::headless::Screens>>>,
}

/// The index of `aside` among the windows beside the main one.
pub(super) fn slot(aside: Aside) -> usize {
    match aside {
        Aside::Settings => 0,
        Aside::Picker => 1,
    }
}

/// The report key of `aside`'s window.
pub(super) fn report_key(aside: Option<Aside>) -> report::WindowKey {
    match aside {
        None => report::MAIN,
        Some(Aside::Settings) => scene::SETTINGS_WINDOW,
        Some(Aside::Picker) => scene::PICKER_WINDOW,
    }
}

impl Shell {
    /// A shell for `model`, its event loop woken through `proxy` when there is one, with no window yet.
    pub(super) fn new(model: Model, proxy: Option<EventLoopProxy>) -> Self {
        Self {
            host: Rc::new(RefCell::new(None)),
            deferred: Rc::new(Cell::new(false)),
            proxy,
            shared: Shared {
                model,
                sizes: Rc::new(RefCell::new([(0.0, 0.0); 3])),
                tokens: Rc::new(RefCell::new(Tokens::default())),
                cursor: Rc::new(Cell::new(None)),
                handles: Rc::new(RefCell::new([None, None])),
                closed: Rc::new(RefCell::new(Vec::new())),
                handlers: Rc::new(RefCell::new(None)),
            },
            #[cfg(test)]
            headless: Rc::new(RefCell::new(None)),
        }
    }

    /// Composes everything again at the next wake-up, or once the windows
    /// have handled the event they are busy with.
    pub(super) fn invalidate(&mut self) {
        match self.host.try_borrow_mut() {
            Ok(mut host) => {
                if let Some(host) = host.as_mut() {
                    host.invalidate();
                }
            }
            Err(_) => self.deferred.set(true),
        }
        #[cfg(test)]
        match self.headless.try_borrow_mut() {
            Ok(mut screens) => {
                if let Some(screens) = screens.as_mut() {
                    screens.invalidate();
                }
            }
            Err(_) => self.deferred.set(true),
        }
    }

    /// Composes and lays out what changed now, in the windows or headless,
    /// or once the windows have handled the event they are busy with.
    pub(super) fn wake(&mut self) {
        match self.host.try_borrow_mut() {
            Ok(mut host) => {
                if let Some(host) = host.as_mut() {
                    host.wake();
                }
            }
            Err(_) => self.deferred.set(true),
        }
        #[cfg(test)]
        match self.headless.try_borrow_mut() {
            Ok(mut screens) => {
                if let Some(screens) = screens.as_mut() {
                    screens.wake(&self.shared);
                }
            }
            Err(_) => self.deferred.set(true),
        }
    }

    /// Whether the windows were asked to compose while they were busy, clearing it.
    pub(super) fn take_deferred(&self) -> bool {
        self.deferred.replace(false)
    }

    /// The handle of `aside`'s window, while it is composed.
    pub(super) fn handle_of(&self, aside: Aside) -> Option<WindowHandle> {
        self.shared.handles.borrow()[slot(aside)]
    }

    /// The window of `aside`, while it is open.
    pub(super) fn port_of(&self, aside: Aside) -> Option<Rc<dyn Port>> {
        #[cfg(test)]
        if let Some(screens) = self.headless.borrow().as_ref() {
            return screens.port_of(aside);
        }
        let handle = self.handle_of(aside)?;
        let window = handle.native()?;
        Some(Rc::new(NativePort::new(window, handle)))
    }

    /// Sets the theme every window is drawn in.
    pub(super) fn set_tokens(&mut self, tokens: Tokens) {
        *self.shared.tokens.borrow_mut() = tokens;
        self.invalidate();
    }

    /// Sets the pointer shape the app decided on over the main window, composing again when it changed.
    pub(super) fn set_cursor(&mut self, cursor: Option<CursorIcon>) {
        if self.shared.cursor.replace(cursor) != cursor {
            self.invalidate();
        }
    }

    /// Records that the window of `aside`, or the main one, is `size` logical pixels across.
    pub(super) fn set_size(&mut self, aside: Option<Aside>, size: (f32, f32)) {
        let index = aside.map_or(0, |aside| slot(aside) + 1);
        let changed = std::mem::replace(&mut self.shared.sizes.borrow_mut()[index], size) != size;
        if changed {
            self.invalidate();
        }
    }

    /// Takes the windows beside the main one their close buttons asked to close.
    pub(super) fn take_closed(&self) -> Vec<Aside> {
        std::mem::take(&mut *self.shared.closed.borrow_mut())
    }
}

/// Emits every window for what `shared` holds: the main window's root and, while
/// what they show is open, the settings window and the folder picker's.
fn root(shared: &Shared) {
    main_view(shared);
    for aside in Aside::ALL {
        if !shared.model.read(|screen| aside.is_open(screen)) {
            continue;
        }
        let handles = Rc::clone(&shared.handles);
        let closed = Rc::clone(&shared.closed);
        let shared = shared.clone();
        rustle::key(slot(aside), || {
            rustle::window(
                aside.options(),
                move || closed.borrow_mut().push(aside),
                move || {
                    handles.borrow_mut()[slot(aside)] = Some(rustle::current_window());
                    aside_view(&shared, aside);
                },
            );
        });
    }
}

/// Emits what the main window shows for what `shared` holds.
pub(super) fn main_view(shared: &Shared) {
    let tokens = shared.tokens.borrow().clone();
    let size = shared.sizes.borrow()[0];
    let cursor = shared.cursor.get();
    shared.under_handler(None, || {
        shared
            .model
            .read(|screen| scene::main_root(screen, size.0, size.1, tokens, cursor));
    });
}

/// Emits what the window of `aside` shows for what `shared` holds.
pub(super) fn aside_view(shared: &Shared, aside: Aside) {
    let tokens = shared.tokens.borrow().clone();
    let (width, height) = aside_size(shared, aside);
    shared.under_handler(Some(aside), || {
        shared.model.read(|screen| {
            scene::aside_root(screen, report_key(Some(aside)), width, height, tokens);
        });
    });
}

/// How large the window of `aside` is, in logical pixels: as the desktop
/// last said, or as it opens.
pub(super) fn aside_size(shared: &Shared, aside: Aside) -> (f32, f32) {
    match shared.sizes.borrow()[slot(aside) + 1] {
        (0.0, 0.0) => aside.opening_size(),
        size => size,
    }
}

/// Lets the platform's input methods compose text into `window`, telling
/// them the caret's area as it moves.
pub(super) fn enable_ime(window: &dyn Window) {
    let data = ImeRequestData::default().with_cursor_area(
        LogicalPosition::new(0.0, 0.0).into(),
        LogicalSize::new(0.0, 0.0).into(),
    );
    let capabilities = ImeCapabilities::new().with_cursor_area();
    if let Some(request) = ImeEnableRequest::new(capabilities, data) {
        let _ = window.request_ime_update(ImeRequest::Enable(request));
    }
}

/// The shadow the main window casts around its frame while it floats.
fn window_shadow() -> WindowShadow {
    WindowShadow {
        margin: pm_components::SHADOW_MARGIN,
        radius: pm_components::FRAME_RADIUS,
        layers: pm_components::WINDOW_SHADOW
            .iter()
            .map(|(x, y, blur, spread, alpha)| {
                ShadowLayer::new(
                    *x,
                    *y,
                    blur * pm_components::kit::layout::SHADOW_SOFTNESS,
                    *spread,
                    *alpha,
                )
            })
            .collect(),
    }
}

/// Whether the event loop runs on Wayland, where the desktop shows a window's shadow beside it.
fn on_wayland(event_loop: &dyn ActiveEventLoop) -> bool {
    use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

    event_loop
        .owned_display_handle()
        .display_handle()
        .is_ok_and(|handle| matches!(handle.as_raw(), RawDisplayHandle::Wayland(_)))
}

impl App {
    /// The options the main window opens with, its frame `size` logical pixels across.
    fn main_options(size: (f32, f32)) -> WindowOptions {
        let (width, height) = window_frame::with_band(size);
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

    /// Tells the views about the frame they are about to be composed or painted for.
    pub(super) fn set_frame(&self) {
        frame::set(Frame {
            focused: self.focus.current(),
            clock_ms: self.started.elapsed().as_secs_f32() * 1000.0,
            held_bar: self.bar_drag.map(|(id, _)| id),
            selection: self.text_selection,
        });
    }

    /// The report of the main window, or of `aside`'s, moved into the
    /// coordinates of its frame, inside the band that holds its shadow.
    pub(super) fn report_of(&self, aside: Option<Aside>) -> Report {
        let band = if aside.is_none() {
            pm_components::shadow_margin()
        } else {
            0.0
        };
        report::last_of(report_key(aside)).shifted(-band, -band)
    }

    /// Composes again when anything asked for it since the last time, and
    /// runs what follows a composed frame.
    pub(super) fn flush(&mut self) {
        if std::mem::take(&mut self.recompose)
            || self.dirty.replace(false)
            || self.shell.take_deferred()
        {
            let started = self.timings.start();
            self.set_frame();
            self.shell.invalidate();
            self.shell.wake();
            let painted = self.timings.start();
            self.timings.frame(started, painted, true);
        }
        self.after_compose();
    }

    /// Opens the main window on rustle once the platform can make surfaces.
    fn open(&mut self, event_loop: &dyn ActiveEventLoop) {
        pm_components::set_shadow_outside(on_wayland(event_loop));
        rustle_wgpu::set_legible_text(false);
        rustle_wgpu::set_gamma_blending(true);
        rustle_wgpu::set_frame_latency(frame_latency());
        rustle::set_pixel_rounding(rustle::PixelRounding::Logical);
        rustle::set_close_handler(|| false);
        let size = Self::opening_size(event_loop);
        let shared = self.shell.shared.clone();
        self.shell.set_size(None, window_frame::with_band(size));
        self.set_frame();
        let Some(proxy) = self.shell.proxy.clone() else {
            fail_to_start("no event loop to open a window in");
        };
        let mut host = Host::new(move || root(&shared), Self::main_options(size), proxy);
        host.can_create_surfaces(event_loop);
        if let Some(error) = host.error() {
            fail_to_start(&format!("could not open a window: {error}"));
        }
        let Some(window) = host.window().cloned() else {
            fail_to_start("could not open a window");
        };
        crate::startup::mark("window-created");
        *self.shell.host.borrow_mut() = Some(host);
        self.system_appearance = appearance(window.theme());
        enable_ime(&*window);
        let port: Rc<dyn Port> = Rc::new(NativePort::new(window, WindowHandle::MAIN));
        self.shell.set_size(None, port.logical_size());
        self.window = Some(port);
        self.apply_appearance();
        let (all, mono) = pm_components::kit::font_families();
        self.model
            .update(|screen| *screen = std::mem::take(screen).with_fonts(all, mono));
        self.recompose = true;
        self.flush();
        crate::startup::mark("renderer-ready");
    }

    /// What the app does after the interface composed and laid out a frame:
    /// what the old frame did after painting, from what this one painted.
    pub(super) fn after_compose(&mut self) {
        self.sync_floating();
        self.fit_terminal();
        let reaching = self.reaching() || self.reaching_terminal_link();
        let edge = self.frame_edge();
        let browser_cursor = self.browser_cursor();
        let Some(window) = self.window.clone() else {
            return;
        };
        self.output = self.report_of(self.on_aside);
        let model = self.model.clone();
        let scroll_to = model.read(|state| {
            self.order_focus(None);
            if std::mem::take(&mut self.focus_terminal) {
                self.focus.focus(pm_pages::TERMINAL_BODY);
            }
            if std::mem::take(&mut self.focus_browser_address) {
                self.focus.focus(pm_components::browser::BROWSER_ADDRESS);
            }
            if std::mem::take(&mut self.focus_filter) {
                self.focus.focus(pm_components::listing::LISTING_FILTER);
            }
            let drawn = |id| self.output.drawn(id);
            Self::give_back_focus(
                &mut self.refocus,
                &mut self.focus,
                self.work.resuming(),
                drawn,
            );
            let starting = state.is_starting();
            if starting && !self.autofocused {
                self.focus.focus(pm_pages::PROMPT);
            }
            self.autofocused = starting;
            let naming = state.is_naming();
            if naming && !self.naming {
                self.focus.focus(pm_components::tree_actions::ENTRY_FIELD);
            }
            self.naming = naming;
            let mut scroll_to = None;
            let band = pm_components::shadow_margin();
            if let Some(id) = self.focus.current()
                && let Some(field) = state.field(id)
            {
                let caret = field.to_shown(field.caret());
                if field.is_multiline()
                    && field.follows_caret()
                    && let Some(scroller) = self.output.scroller(id)
                {
                    scroll_to = Some((id, scroller.offset));
                }
                if !field.is_multiline()
                    && let Some(held) = self.output.field(id)
                {
                    let caret_x = held.caret_at(caret).0 - held.origin.x;
                    let shown = held.view.origin.x - held.origin.x;
                    let want = field.scroll_for(
                        caret_x - shown + field.scroll(),
                        held.layout.size().width,
                        held.view.size.width,
                    );
                    if (want - field.scroll()).abs() > 0.5 {
                        scroll_to = Some((id, want));
                    }
                }
                if let Some(rect) = self.output.caret_rect(id, caret) {
                    let area = (rect.origin.x, rect.origin.y, 1.0, rect.size.height);
                    if self.ime_area != Some(area) {
                        self.ime_area = Some(area);
                        window.set_ime_area((area.0 + band, area.1 + band, area.2, area.3));
                    }
                }
            }
            scroll_to
        });
        if let Some((id, want)) = scroll_to {
            self.model
                .update(|state| *state = std::mem::take(state).edited(id, Edit::ScrollTo(want)));
        }
        if let Some(width) = model.read(|state| state.diff_resized(&self.output.scrollers)) {
            self.model
                .update(|state| *state = std::mem::take(state).measured_diff(width));
        }
        if pm_components::chat::history_pending() {
            self.model.update(|_| {});
        }
        let cursor = match self.sash_drag {
            Some((.., pm_components::sash::Grows::Up)) => Some(CursorIcon::RowResize),
            Some(_) => Some(CursorIcon::ColResize),
            None if reaching => Some(CursorIcon::Pointer),
            None => edge.map(window_frame::edge_icon).or(browser_cursor),
        };
        if self.on_aside.is_none() {
            self.shell.set_cursor(cursor);
        }
        crate::startup::scene_painted();
        if let Some(at) = frame::take_wake() {
            let now = self.started.elapsed().as_secs_f32() * 1000.0;
            self.frame_due =
                Some(Instant::now() + Duration::from_secs_f32((at - now).max(0.0) / 1000.0));
        }
        if frame::take_compose() {
            self.recompose = true;
        }
        crate::startup::frame_presented();
        self.place_completion();
        self.place_signature();
        self.note_code_view();
        self.note_conversation(true);
    }

    /// Which window beside the main one `id` is, if it is one.
    fn aside_of_window(&self, id: WindowId) -> Option<Aside> {
        Aside::ALL.into_iter().find(|aside| {
            self.shell
                .handle_of(*aside)
                .and_then(|handle| handle.native())
                .is_some_and(|window| window.id() == id)
        })
    }

    /// Whether `id` names the main window.
    fn is_main_window(&self, id: WindowId) -> bool {
        self.window
            .as_ref()
            .is_some_and(|window| window.native().is_some_and(|native| native.id() == id))
    }

    /// Carries out `input` of the main window, or of `aside`'s, once the
    /// interface has seen it: the app's own logic against the window it
    /// happened in, the files the interface took, and a frame composed again
    /// for what changed.
    pub(super) fn after_interface(&mut self, aside: Option<Aside>, input: Input) {
        let focus = self.focus.current();
        match aside {
            Some(aside) => self.aside_input(aside, input),
            None => {
                if matches!(input, Input::Focused(true))
                    && self.focus.current().is_some_and(super::asides::is_aside)
                {
                    self.focus.clear();
                }
                self.handle_input(input);
            }
        }
        self.claimed = Claims::default();
        if self.focus.current() != focus {
            self.recompose = true;
        }
        self.flush();
    }

    /// Carries out `input` of the window standing as the main one: the pointer, the keyboard and the window itself.
    pub(super) fn handle_input(&mut self, input: Input) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let band = if self.on_aside.is_none() {
            pm_components::shadow_margin()
        } else {
            0.0
        };
        match input {
            Input::Close => self.closing = self.close_window(),
            Input::Focused(_) => {}
            Input::Theme(appearance) => {
                self.system_appearance = appearance;
                self.apply_appearance();
            }
            Input::Resized(logical) => {
                self.shell.set_size(self.on_aside, logical);
                if self.on_aside.is_none() && self.keeps_to_disk {
                    Self::remember_size(&*window);
                }
            }
            Input::Moved(position) => {
                let received = self.timings.start();
                let at = (position.0 - band, position.1 - band);
                self.pointer(PointerEvent::Moved(at.0, at.1));
                self.settle_drop();
                self.browser_moved(at);
                if self.on_aside.is_none() {
                    self.timings.pointer(received);
                }
            }
            Input::Left => self.pointer(PointerEvent::Left),
            Input::Wheel(across, down) => {
                if self.browser_wheel(across, down) {
                    return;
                }
                self.scroll(-down);
            }
            Input::Button(button, pressed, position) => {
                let at = (position.0 - band, position.1 - band);
                if self.pointer.position != Some(at) {
                    self.pointer(PointerEvent::Moved(at.0, at.1));
                }
                self.refocus = None;
                if button == MouseButton::Left && pressed && self.frame_press(&*window) {
                    return;
                }
                let button = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Right => Button::Right,
                    MouseButton::Back | MouseButton::Forward => {
                        if pressed
                            && !(self.on_aside.is_none()
                                && self.browser_history(button == MouseButton::Back))
                        {
                            self.step_through_history(button == MouseButton::Back);
                        }
                        return;
                    }
                    _ => return,
                };
                self.browser_button(button, pressed);
                self.pointer(if pressed {
                    PointerEvent::Pressed(button)
                } else {
                    PointerEvent::Released(button)
                });
            }
            Input::Preedit(composing) => {
                if self.on_aside.is_none() {
                    self.timings.keyboard();
                }
                self.compose(composing);
            }
            Input::Commit(committed) => {
                if self.on_aside.is_none() {
                    self.timings.keyboard();
                }
                if self.commit_to_terminal(&committed) || self.commit_to_browser(&committed) {
                } else if let Some(id) = self.focus.current()
                    && Screen::is_field(id)
                {
                    self.edit_field(id, Edit::Paste(committed));
                }
            }
            Input::Modifiers(modifiers) => {
                self.modifiers = modifiers;
                self.editor_modifiers_changed();
                self.reach_terminal_link();
            }
            Input::Key(event) => {
                self.refocus = None;
                if event.pressed && self.on_aside.is_none() {
                    self.timings.keyboard();
                }
                self.key(event);
            }
        }
    }

    /// What the app does each time the event loop comes to rest: its waiting
    /// work, the windows beside the main one opened and closed as the state
    /// says, and a frame composed for what changed. Says how the loop is to
    /// wait and whether the app is to quit.
    pub(super) fn come_to_rest(&mut self) -> (ControlFlow, bool) {
        let rest = self.rest();
        self.tell_field_left();
        for aside in self.shell.take_closed() {
            self.model
                .update(|state| *state = aside.closed(std::mem::take(state)));
        }
        self.sync_asides();
        self.flush();
        rest
    }

    /// When the frame some drawing asked for is due, composing it once that time has come.
    pub(super) fn frame_deadline(&mut self) -> Option<Instant> {
        let due = self.frame_due?;
        if due > Instant::now() {
            return Some(due);
        }
        self.frame_due = None;
        self.recompose = true;
        None
    }

    /// The scrolling box the wheel turned over, as its view said, and how far it can scroll.
    pub(super) fn wheel_action(&self) -> Option<(FocusId, f32)> {
        self.claimed.wheel
    }

    /// The tokens for the theme and appearance the window is drawn in.
    pub(super) fn tokens(&self) -> Tokens {
        Tokens::new(&self.family, self.appearance)
    }
}

/// An event of a window, as the app's logic takes it once the interface has
/// seen it: positions and sizes in logical pixels of the window's surface.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Input {
    /// The desktop asked the window to close.
    Close,
    /// The window took the keyboard, or lost it.
    Focused(bool),
    /// The desktop changed between light and dark.
    Theme(Appearance),
    /// The window's surface is now this wide and tall.
    Resized((f32, f32)),
    /// The pointer moved to this place.
    Moved((f32, f32)),
    /// The pointer left the window.
    Left,
    /// The wheel or a trackpad scrolled across and down by this much.
    Wheel(f32, f32),
    /// A button went down, or up when not `pressed`, at this place.
    Button(MouseButton, bool, (f32, f32)),
    /// The input method is composing this, or ended composing when it is empty.
    Preedit(String),
    /// The input method produced this text.
    Commit(String),
    /// These modifier keys are now held.
    Modifiers(Modifiers),
    /// A key went down or up.
    Key(KeyEvent),
}

/// The app's input for `event` of the window `port`, with `modifiers` held,
/// or nothing for an event the app does not act on.
fn input_of(event: &WindowEvent, port: &dyn Port, modifiers: Modifiers) -> Option<Input> {
    let scale = port.scale();
    let logical = |x: f64, y: f64| (x as f32 / scale, y as f32 / scale);
    Some(match event {
        WindowEvent::CloseRequested => Input::Close,
        WindowEvent::Focused(focused) => Input::Focused(*focused),
        WindowEvent::ThemeChanged(theme) => Input::Theme(appearance(Some(*theme))),
        WindowEvent::SurfaceResized(size) => {
            let size = size.to_logical::<f32>(f64::from(scale));
            Input::Resized((size.width, size.height))
        }
        WindowEvent::ScaleFactorChanged { .. } => Input::Resized(port.logical_size()),
        WindowEvent::PointerMoved { position, .. } => Input::Moved(logical(position.x, position.y)),
        WindowEvent::PointerLeft { .. } => Input::Left,
        WindowEvent::MouseWheel { delta, .. } => {
            let (across, down) = match delta {
                MouseScrollDelta::LineDelta(across, down) => {
                    (across * WHEEL_LINE, down * WHEEL_LINE)
                }
                MouseScrollDelta::PixelDelta(moved) => logical(moved.x, moved.y),
                _ => (0.0, 0.0),
            };
            Input::Wheel(across, down)
        }
        WindowEvent::PointerButton {
            state,
            button,
            position,
            ..
        } => Input::Button(
            button.clone().mouse_button()?,
            *state == ElementState::Pressed,
            logical(position.x, position.y),
        ),
        WindowEvent::Ime(Ime::Preedit(composing, _)) => Input::Preedit(composing.clone()),
        WindowEvent::Ime(Ime::Disabled) => Input::Preedit(String::new()),
        WindowEvent::Ime(Ime::Commit(committed)) => Input::Commit(committed.clone()),
        WindowEvent::ModifiersChanged(changed) => {
            Input::Modifiers(keys::modifiers(changed.state()))
        }
        WindowEvent::KeyboardInput { event, .. } => Input::Key(keys::key_event(event, modifiers)?),
        _ => return None,
    })
}

/// The app as the event loop drives it, shared with the handlers its
/// interface runs.
pub struct Running(Rc<RefCell<App>>);

impl Running {
    /// The event loop's handler for `app`.
    pub fn new(app: App) -> Self {
        let app = Rc::new(RefCell::new(app));
        app.borrow().shell.shared.attach(&app);
        Self(app)
    }

    /// The interface of every window, held apart from the app so the
    /// handlers an event runs can reach the app.
    fn host(&self) -> Rc<RefCell<Option<Host>>> {
        Rc::clone(&self.0.borrow().shell.host)
    }
}

impl ApplicationHandler for Running {
    /// Opens the main window on rustle once the platform is ready.
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let host = self.host();
        if let Some(host) = host.borrow_mut().as_mut() {
            host.can_create_surfaces(event_loop);
            return;
        }
        self.0.borrow_mut().open(event_loop);
    }

    /// Lets the interface run what woke the loop, then the app's work.
    fn proxy_wake_up(&mut self, _: &dyn ActiveEventLoop) {
        self.0.borrow().set_frame();
        let host = self.host();
        if let Some(host) = host.borrow_mut().as_mut() {
            host.wake();
        }
        self.0.borrow_mut().after_compose();
    }

    /// Hands each event of a window to the interface, then to the app's
    /// own logic, against the window it happened in.
    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let (aside, main) = {
            let app = self.0.borrow();
            app.set_frame();
            (app.aside_of_window(id), app.is_main_window(id))
        };
        let closing = matches!(event, WindowEvent::CloseRequested);
        let host = self.host();
        if !closing && let Some(host) = host.borrow_mut().as_mut() {
            host.window_event(event_loop, id, event.clone());
        }
        let mut app = self.0.borrow_mut();
        let port = match aside {
            Some(aside) => app.shell.port_of(aside),
            None => app.window.clone(),
        };
        let input = port.and_then(|port| input_of(&event, &*port, app.modifiers));
        if (aside.is_some() || main)
            && let Some(input) = input
        {
            app.after_interface(aside, input);
        } else if std::mem::take(&mut app.claimed).acted || app.shell.take_deferred() {
            app.recompose = true;
            app.flush();
        }
    }

    /// Does the app's waiting work and decides when the loop wakes next.
    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        let host = self.host();
        if let Some(host) = host.borrow_mut().as_mut() {
            host.about_to_wait(event_loop);
        }
        let (flow, exit) = self.0.borrow_mut().come_to_rest();
        event_loop.set_control_flow(flow);
        if exit {
            event_loop.exit();
        }
    }
}

impl Drop for Running {
    /// Captures the final draft and waits for its ordered background write before the app leaves.
    fn drop(&mut self) {
        let mut app = self.0.borrow_mut();
        app.persist_view();
        app.view_writer.flush();
        app.browser_shutdown();
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
