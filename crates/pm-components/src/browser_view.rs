//! The window of a web page: the newest picture the page painted, drawn to fill its area, which hands the page the pointer and, while it holds focus, the keyboard, and tells how big it is laid out.

use std::cell::Cell;
use std::rc::Rc;

use pm_browser::{Button, Cursor, Frame, Modifiers as Held, PageInput};
use rustle::prelude::*;
use rustle::{Key, KeyEvent, PointerButton, PointerEvent, PointerPhase};

use crate::browser_keys::{BrowserShortcut, browser_shortcut, held, page_key};
use crate::label::{Label, LabelVariant};
use crate::size::Size;
use rustle::color_scheme;

/// How a page's window is drawn while the page has painted nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrowserViewVariant {
    /// A page of the web, white until it paints, as a browser's is.
    #[default]
    Page,
    /// The developer tools, on the window's background until they paint.
    Tools,
}

/// What learns how big a page's window is laid out: its width and height in logical pixels, and the display scale.
type Resized = Rc<dyn Fn(u32, u32, f32)>;

/// The callbacks a page's window hands what the reader did to.
#[derive(Clone, Default)]
struct Callbacks {
    /// What receives the pointer and the keys.
    on_input: Option<Rc<dyn Fn(PageInput)>>,
    /// What learns how big the window is, in logical pixels, and at what display scale.
    on_resize: Option<Resized>,
    /// What learns when the window takes or loses the keyboard.
    on_focus: Option<Rc<dyn Fn(bool)>>,
    /// What runs the keys the browser takes for itself.
    on_shortcut: Option<Rc<dyn Fn(BrowserShortcut)>>,
    /// What opens the menu over the page, at a point of it.
    on_menu: Option<Rc<dyn Fn(f32, f32)>>,
}

impl Callbacks {
    /// Hands `input` to the page.
    fn input(&self, input: PageInput) {
        if let Some(on_input) = &self.on_input {
            on_input(input);
        }
    }
}

/// A page's window with a variant and a size.
#[derive(Clone)]
pub struct BrowserView {
    /// The newest picture of the page.
    frame: Option<Frame>,
    /// How the pointer looks over the page.
    cursor: Cursor,
    /// What it says while the page has painted nothing.
    placeholder: String,
    /// How it is drawn.
    variant: BrowserViewVariant,
    /// Its size, which sets the placeholder's.
    size: Size,
    /// The modifier it is laid out with.
    modifier: Modifier,
    /// What it tells of what the reader does.
    callbacks: Callbacks,
}

impl BrowserView {
    /// The window of a page whose newest picture is `frame`.
    pub fn new(frame: Option<Frame>) -> Self {
        Self {
            frame,
            cursor: Cursor::Arrow,
            placeholder: String::new(),
            variant: BrowserViewVariant::Page,
            size: Size::Sm,
            modifier: Modifier::new(),
            callbacks: Callbacks::default(),
        }
    }

    /// The window with the pointer drawn as the page asks, `cursor`.
    pub fn cursor(mut self, cursor: Cursor) -> Self {
        self.cursor = cursor;
        self
    }

    /// The window saying `placeholder` while the page has painted nothing.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// The window in `variant`.
    pub fn variant(mut self, variant: BrowserViewVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The window at `size`.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// The window laid out with `modifier`.
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The window handing the pointer and the keys to `on_input`.
    pub fn on_input(mut self, on_input: impl Fn(PageInput) + 'static) -> Self {
        self.callbacks.on_input = Some(Rc::new(on_input));
        self
    }

    /// The window telling `on_resize` how many logical pixels wide and tall it is, and the display scale.
    pub fn on_resize(mut self, on_resize: impl Fn(u32, u32, f32) + 'static) -> Self {
        self.callbacks.on_resize = Some(Rc::new(on_resize));
        self
    }

    /// The window telling `on_focus` when it takes or loses the keyboard.
    pub fn on_focus(mut self, on_focus: impl Fn(bool) + 'static) -> Self {
        self.callbacks.on_focus = Some(Rc::new(on_focus));
        self
    }

    /// The window handing the keys the browser takes for itself to `on_shortcut`.
    pub fn on_shortcut(mut self, on_shortcut: impl Fn(BrowserShortcut) + 'static) -> Self {
        self.callbacks.on_shortcut = Some(Rc::new(on_shortcut));
        self
    }

    /// The window asking `on_menu` to open the menu over the page at a point of it, on a secondary click.
    pub fn on_menu(mut self, on_menu: impl Fn(f32, f32) + 'static) -> Self {
        self.callbacks.on_menu = Some(Rc::new(on_menu));
        self
    }

    /// Emits the window.
    #[track_caller]
    pub fn show(self) {
        let scheme = color_scheme();
        let focus = remember_focus();
        let focused = focus.is_focused();
        let told = remember(|| false);
        if told.get() != focused {
            told.set(focused);
            if let Some(on_focus) = self.callbacks.on_focus.clone() {
                side_effect(move || on_focus(focused));
            }
        }
        let picture = remember(|| None::<(u64, u32, u32, ImageData)>);
        let shown = self.frame.as_ref().and_then(|frame| {
            let held = picture.get().filter(|(generation, width, height, _)| {
                (*generation, *width, *height) == (frame.generation, frame.width, frame.height)
            });
            match held {
                Some((_, _, _, image)) => Some(image),
                None => {
                    let image =
                        ImageData::from_rgba(frame.width, frame.height, frame.rgba.to_vec())?;
                    picture.set(Some((
                        frame.generation,
                        frame.width,
                        frame.height,
                        image.clone(),
                    )));
                    Some(image)
                }
            }
        });
        let buttons = remember(|| Rc::new(Cell::new(0_u32))).get();
        let scale = rustle::current_window().scale_factor();
        let callbacks = self.callbacks.clone();
        let background = match self.variant {
            BrowserViewVariant::Page => Color::rgb(255, 255, 255),
            BrowserViewVariant::Tools => scheme.background,
        };
        let on_resize = callbacks.on_resize.clone();
        let moving = (callbacks.clone(), buttons.clone());
        let primary = (callbacks.clone(), buttons.clone(), focus.clone());
        let middle = (callbacks.clone(), buttons.clone());
        let menu = callbacks.on_menu.clone();
        let wheel = callbacks.clone();
        let keys = callbacks.clone();
        let modifier = self
            .modifier
            .background(background)
            .clip()
            .on_size_changed(move |size| {
                if let Some(on_resize) = &on_resize {
                    on_resize(size.width.round() as u32, size.height.round() as u32, scale);
                }
            })
            .on_pointer_move(move |at| {
                let (callbacks, buttons) = &moving;
                let (x, y, left) = at.map_or((0, 0, true), |at| (at.x as i32, at.y as i32, false));
                callbacks.input(PageInput::Moved {
                    x,
                    y,
                    modifiers: Held(buttons.get()),
                    left,
                });
            })
            .on_pointer_button(PointerButton::Primary, move |event| {
                let (callbacks, buttons, focus) = &primary;
                if event.phase == PointerPhase::Press {
                    focus.request_focus();
                }
                button_event(callbacks, buttons, event, Button::Left, Held::LEFT_BUTTON);
            })
            .on_pointer_button(PointerButton::Middle, move |event| {
                let (callbacks, buttons) = &middle;
                button_event(
                    callbacks,
                    buttons,
                    event,
                    Button::Middle,
                    Held::MIDDLE_BUTTON,
                );
            })
            .on_secondary_click(move |event| {
                if let Some(menu) = &menu {
                    menu(event.position.x, event.position.y);
                }
            })
            .pointer_input(move |input| {
                if input.phase != PointerInputPhase::Scroll {
                    return false;
                }
                wheel.input(PageInput::Scrolled {
                    x: input.position.x as i32,
                    y: input.position.y as i32,
                    dx: input.scroll_delta.x as i32,
                    dy: input.scroll_delta.y as i32,
                    modifiers: held(input.modifiers, 0),
                });
                true
            })
            .focusable(&focus, move |event| key_event(&keys, event))
            .cursor(cursor_icon(self.cursor));
        let placeholder = self.placeholder;
        let size = self.size;
        stack_with(modifier, move || match shown {
            Some(image) => rustle::image(
                Modifier::new().fill_max_size(),
                &image,
                ContentScale::Stretch,
            ),
            None if !placeholder.is_empty() => {
                column_arranged(
                    Modifier::new().fill_max_size().padding(24.0),
                    Arrangement::Center,
                    Alignment::Center,
                    || {
                        Label::new(placeholder)
                            .variant(LabelVariant::Muted)
                            .size(size)
                            .show();
                    },
                );
            }
            None => {}
        });
    }
}

/// Hands the page a press, drag or release of `button`, keeping `buttons` in step with what is held.
fn button_event(
    callbacks: &Callbacks,
    buttons: &Cell<u32>,
    event: &PointerEvent,
    button: Button,
    bit: u32,
) {
    let (x, y) = (event.position.x as i32, event.position.y as i32);
    match event.phase {
        PointerPhase::Press => {
            buttons.set(buttons.get() | bit);
            callbacks.input(PageInput::Button {
                x,
                y,
                button,
                pressed: true,
                clicks: event.click_count as i32,
                modifiers: held(event.modifiers, buttons.get()),
            });
        }
        PointerPhase::Drag => callbacks.input(PageInput::Moved {
            x,
            y,
            modifiers: held(event.modifiers, buttons.get()),
            left: false,
        }),
        PointerPhase::Release => {
            buttons.set(buttons.get() & !bit);
            callbacks.input(PageInput::Button {
                x,
                y,
                button,
                pressed: false,
                clicks: event.click_count.max(1) as i32,
                modifiers: held(event.modifiers, buttons.get()),
            });
        }
        PointerPhase::Click => {}
    }
}

/// Hands `event` to the browser's own keys first, then to the page; returns whether either took it.
fn key_event(callbacks: &Callbacks, event: &KeyEvent) -> bool {
    if let Some(shortcut) = browser_shortcut(event) {
        if let Some(on_shortcut) = &callbacks.on_shortcut {
            on_shortcut(shortcut);
        }
        return true;
    }
    if event.key == Key::Other {
        if let Some(text) = event
            .text
            .clone()
            .filter(|text| event.pressed && !text.is_empty())
        {
            callbacks.input(PageInput::Text(text));
        }
        return true;
    }
    match page_key(event) {
        Some(key) => {
            callbacks.input(PageInput::Key(key));
            true
        }
        None => false,
    }
}

/// How the pointer looks in the window when the page asks for `cursor`.
fn cursor_icon(cursor: Cursor) -> CursorIcon {
    match cursor {
        Cursor::Arrow | Cursor::Help => CursorIcon::Default,
        Cursor::Hand => CursorIcon::Pointer,
        Cursor::Text => CursorIcon::Text,
        Cursor::Cross => CursorIcon::Crosshair,
        Cursor::Wait => CursorIcon::Wait,
        Cursor::ResizeHorizontal => CursorIcon::EwResize,
        Cursor::ResizeVertical => CursorIcon::NsResize,
    }
}
