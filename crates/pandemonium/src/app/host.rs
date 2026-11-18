//! The windows on rustle: the event loop's handler, which hands every event
//! to the interface and then to the app's own logic, the root composable
//! the main window and the windows beside it show, and what the app does
//! after the interface composed a frame.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pm_components::kit::frame::{self, Frame};
use pm_components::kit::report::{self, Report};
use pm_components::kit::theme::Tokens;
use pm_components::kit::{Action, Button, PointerEvent, State, bus};
use pm_field::Edit;
use pm_pages::Screen;
use rustle::{CursorIcon, Host, ShadowLayer, WindowHandle, WindowOptions, WindowShadow};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::{
    ImeCapabilities, ImeEnableRequest, ImeRequest, ImeRequestData, Window, WindowId,
};

use super::asides::Aside;
use super::{App, WHEEL_LINE, appearance, fail_to_start, frame as window_frame};
use crate::{keys, scene};

/// What the root composable reads that the app changes outside rustle's own state.
#[derive(Clone)]
pub(super) struct Shared {
    /// The screen the app holds.
    model: State<Screen>,
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
}

/// The rustle side of the app: the host that owns the windows, and what its root reads.
pub(super) struct Shell {
    /// The interface and its windows, once the platform could make surfaces.
    pub(super) host: Option<Host>,
    /// The event loop's proxy, which wakes it for the host and for the app's work.
    proxy: EventLoopProxy,
    /// What the root reads.
    pub(super) shared: Shared,
}

/// The index of `aside` among the windows beside the main one.
fn slot(aside: Aside) -> usize {
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
    /// A shell for `model`, its event loop woken through `proxy`, with no window yet.
    pub(super) fn new(model: State<Screen>, proxy: EventLoopProxy) -> Self {
        Self {
            host: None,
            proxy,
            shared: Shared {
                model,
                sizes: Rc::new(RefCell::new([(0.0, 0.0); 3])),
                tokens: Rc::new(RefCell::new(Tokens::default())),
                cursor: Rc::new(Cell::new(None)),
                handles: Rc::new(RefCell::new([None, None])),
                closed: Rc::new(RefCell::new(Vec::new())),
            },
        }
    }

    /// Composes everything again at the next wake-up.
    pub(super) fn invalidate(&mut self) {
        if let Some(host) = self.host.as_mut() {
            host.invalidate();
        }
    }

    /// The handle of `aside`'s window, while it is composed.
    pub(super) fn handle_of(&self, aside: Aside) -> Option<WindowHandle> {
        self.shared.handles.borrow()[slot(aside)]
    }

    /// The native window of `aside`, while it is open.
    pub(super) fn window_of(&self, aside: Aside) -> Option<Arc<dyn Window>> {
        self.handle_of(aside)?.native()
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
    let tokens = shared.tokens.borrow().clone();
    let sizes = *shared.sizes.borrow();
    let cursor = shared.cursor.get();
    shared.model.read(|screen| {
        scene::main_root(screen, sizes[0].0, sizes[0].1, tokens.clone(), cursor);
        for aside in Aside::ALL {
            if !aside.is_open(screen) {
                continue;
            }
            let handles = Rc::clone(&shared.handles);
            let closed = Rc::clone(&shared.closed);
            let tokens = tokens.clone();
            let (width, height) = match sizes[slot(aside) + 1] {
                (0.0, 0.0) => aside.opening_size(),
                size => size,
            };
            rustle::key(slot(aside), || {
                rustle::window(
                    aside.options(),
                    move || closed.borrow_mut().push(aside),
                    || {
                        handles.borrow_mut()[slot(aside)] = Some(rustle::current_window());
                        scene::aside_root(screen, report_key(Some(aside)), width, height, tokens);
                    },
                );
            });
        }
    });
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
            .map(|(x, y, blur, spread, alpha)| ShadowLayer::new(*x, *y, *blur, *spread, *alpha))
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
        if std::mem::take(&mut self.recompose) || self.dirty.replace(false) {
            let started = self.timings.start();
            self.set_frame();
            self.shell.invalidate();
            if let Some(host) = self.shell.host.as_mut() {
                host.wake();
            }
            let painted = self.timings.start();
            self.timings.frame(started, painted, true);
        }
        self.after_compose();
    }

    /// Opens the main window on rustle once the platform can make surfaces.
    fn open(&mut self, event_loop: &dyn ActiveEventLoop) {
        pm_components::set_shadow_outside(on_wayland(event_loop));
        rustle_wgpu::set_legible_text(false);
        rustle::set_pixel_rounding(rustle::PixelRounding::Logical);
        rustle::set_close_handler(|| false);
        let size = Self::opening_size(event_loop);
        let shared = self.shell.shared.clone();
        self.shell.set_size(None, window_frame::with_band(size));
        self.set_frame();
        let mut host = Host::new(
            move || root(&shared),
            Self::main_options(size),
            self.shell.proxy.clone(),
        );
        host.can_create_surfaces(event_loop);
        if let Some(error) = host.error() {
            fail_to_start(&format!("could not open a window: {error}"));
        }
        let Some(window) = host.window().cloned() else {
            fail_to_start("could not open a window");
        };
        crate::startup::mark("window-created");
        self.shell.host = Some(host);
        self.system_appearance = appearance(window.theme());
        enable_ime(&*window);
        let scale = window.scale_factor();
        let surface = window.surface_size().to_logical::<f32>(scale);
        self.window = Some(window);
        self.shell.set_size(None, (surface.width, surface.height));
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
            let held_by_aside = self
                .focus
                .current()
                .filter(|id| super::asides::is_aside(*id));
            let mut order = self.output.focus_order();
            order.extend(held_by_aside);
            self.focus.set_order(order);
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
                        let data = ImeRequestData::default().with_cursor_area(
                            LogicalPosition::new(area.0 + band, area.1 + band).into(),
                            LogicalSize::new(area.2, area.3).into(),
                        );
                        let _ = window.request_ime_update(ImeRequest::Update(data));
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
                .window_of(*aside)
                .is_some_and(|window| window.id() == id)
        })
    }

    /// Handles an event of a window of the app: the interface sees it first,
    /// then the app's own logic, against the window it happened in.
    fn route(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.set_frame();
        let focus = self.focus.current();
        let aside = self.aside_of_window(id);
        let main = self.window.as_ref().is_some_and(|window| window.id() == id);
        let closing = matches!(event, WindowEvent::CloseRequested);
        if !closing && let Some(host) = self.shell.host.as_mut() {
            host.window_event(event_loop, id, event.clone());
        }
        self.actions = bus::drain();
        if let Some(aside) = aside {
            self.aside_event(aside, event_loop, event);
        } else if main {
            if matches!(event, WindowEvent::Focused(true))
                && self.focus.current().is_some_and(super::asides::is_aside)
            {
                self.focus.clear();
            }
            self.handle_window_event(event_loop, event);
        }
        for dropped in std::mem::take(&mut self.actions) {
            if let Action::Dropped { path, .. } = dropped
                && !self.outside_offered()
            {
                self.file_dropped(path);
            }
        }
        if self.focus.current() != focus {
            self.recompose = true;
        }
        self.flush();
    }

    /// Routes an event of the window standing as the main one to the pointer and the keyboard.
    pub(super) fn handle_window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let scale = window.scale_factor() as f32;
        let band = if self.on_aside.is_none() {
            pm_components::shadow_margin()
        } else {
            0.0
        };
        match event {
            WindowEvent::CloseRequested => {
                if self.close_window() {
                    event_loop.exit();
                }
            }
            WindowEvent::ThemeChanged(theme) => {
                self.system_appearance = appearance(Some(theme));
                self.apply_appearance();
            }
            WindowEvent::SurfaceResized(size) => {
                let logical = size.to_logical::<f32>(f64::from(scale));
                self.shell
                    .set_size(self.on_aside, (logical.width, logical.height));
                if self.on_aside.is_none() {
                    Self::remember_size(&*window);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let logical = window.surface_size().to_logical::<f32>(f64::from(scale));
                self.shell
                    .set_size(self.on_aside, (logical.width, logical.height));
            }
            WindowEvent::PointerMoved { position, .. } => {
                let received = self.timings.start();
                let at = (
                    position.x as f32 / scale - band,
                    position.y as f32 / scale - band,
                );
                self.pointer(PointerEvent::Moved(at.0, at.1));
                self.settle_drop();
                self.browser_moved(at);
                if self.on_aside.is_none() {
                    self.timings.pointer(received);
                }
            }
            WindowEvent::PointerLeft { .. } => self.pointer(PointerEvent::Left),
            WindowEvent::MouseWheel { delta, .. } => {
                let (across, down) = match delta {
                    MouseScrollDelta::LineDelta(across, down) => {
                        (across * WHEEL_LINE, down * WHEEL_LINE)
                    }
                    MouseScrollDelta::PixelDelta(moved) => {
                        (moved.x as f32 / scale, moved.y as f32 / scale)
                    }
                    _ => (0.0, 0.0),
                };
                if self.browser_wheel(across, down) {
                    return;
                }
                self.scroll(-down);
            }
            WindowEvent::PointerButton {
                state,
                button,
                position,
                ..
            } => {
                let Some(button) = button.mouse_button() else {
                    return;
                };
                let at = (
                    position.x as f32 / scale - band,
                    position.y as f32 / scale - band,
                );
                if self.pointer.position != Some(at) {
                    self.pointer(PointerEvent::Moved(at.0, at.1));
                }
                self.refocus = None;
                if button == MouseButton::Left
                    && state == ElementState::Pressed
                    && self.frame_press(&*window)
                {
                    return;
                }
                let button = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Right => Button::Right,
                    MouseButton::Back | MouseButton::Forward => {
                        if state == ElementState::Pressed
                            && !(self.on_aside.is_none()
                                && self.browser_history(button == MouseButton::Back))
                        {
                            self.step_through_history(button == MouseButton::Back);
                        }
                        return;
                    }
                    _ => return,
                };
                self.browser_button(button, state == ElementState::Pressed);
                self.pointer(match state {
                    ElementState::Pressed => PointerEvent::Pressed(button),
                    ElementState::Released => PointerEvent::Released(button),
                });
            }
            WindowEvent::Ime(Ime::Preedit(composing, _)) => {
                if self.on_aside.is_none() {
                    self.timings.keyboard();
                }
                self.compose(composing);
            }
            WindowEvent::Ime(Ime::Disabled) => self.compose(String::new()),
            WindowEvent::Ime(Ime::Commit(committed)) => {
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
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = keys::modifiers(modifiers.state());
                self.editor_modifiers_changed();
                self.reach_terminal_link();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.refocus = None;
                if let Some(event) = keys::key_event(&event, self.modifiers) {
                    if event.pressed && self.on_aside.is_none() {
                        self.timings.keyboard();
                    }
                    self.key(event);
                }
            }
            _ => {}
        }
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

    /// The scroll action the wheel just made, if the interface turned it into one:
    /// the box the pointer is over and how far it can scroll.
    pub(super) fn wheel_action(&self) -> Option<(pm_components::kit::FocusId, f32)> {
        self.actions.iter().find_map(|action| match action {
            Action::Scroll { id, room, .. } => Some((*id, *room)),
            _ => None,
        })
    }

    /// The control the interface says the primary button went down on, if it said.
    pub(super) fn pressed_action(&self) -> Option<pm_components::kit::FocusId> {
        self.actions.iter().find_map(|action| match action {
            Action::Down { id, .. } => Some(*id),
            _ => None,
        })
    }

    /// The control the interface says was pressed and let go, if it said.
    pub(super) fn released_action(&self) -> Option<pm_components::kit::FocusId> {
        self.actions.iter().find_map(|action| match action {
            Action::Press(id) => Some(*id),
            _ => None,
        })
    }

    /// The tokens for the theme and appearance the window is drawn in.
    pub(super) fn tokens(&self) -> Tokens {
        Tokens::new(&self.family, self.appearance)
    }
}

/// The app as the event loop drives it.
pub struct Running(pub App);

impl ApplicationHandler for Running {
    /// Opens the main window on rustle once the platform is ready.
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let app = &mut self.0;
        match app.shell.host.as_mut() {
            Some(host) => host.can_create_surfaces(event_loop),
            None => app.open(event_loop),
        }
    }

    /// Lets the interface run what woke the loop, then the app's work.
    fn proxy_wake_up(&mut self, _: &dyn ActiveEventLoop) {
        let app = &mut self.0;
        app.set_frame();
        if let Some(host) = app.shell.host.as_mut() {
            host.wake();
        }
        app.after_compose();
    }

    /// Hands each event of a window to the interface, then to the app.
    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.0.route(event_loop, id, event);
    }

    /// Does the app's waiting work and decides when the loop wakes next.
    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        let app = &mut self.0;
        if let Some(host) = app.shell.host.as_mut() {
            host.about_to_wait(event_loop);
        }
        app.idle(event_loop);
        for aside in app.shell.take_closed() {
            app.model
                .update(|state| *state = aside.closed(std::mem::take(state)));
        }
        app.sync_asides();
        app.flush();
    }
}

impl Drop for Running {
    /// Captures the final draft and waits for its ordered background write before the app leaves.
    fn drop(&mut self) {
        self.0.persist_view();
        self.0.view_writer.flush();
        self.0.browser_shutdown();
    }
}
