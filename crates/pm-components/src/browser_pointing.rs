//! The pointer over the window of a web page or of its tools, handed on to the page: the
//! moves over it and its leaving, the buttons pressed on it and let go wherever the pointer
//! is, the wheel turned over it and the mouse's back and forward buttons, each told to the
//! app as a [`BrowserPointer`], with how the pointer looks as the page asks.

use crate::browser::Pointer;
use crate::events::{self, OnPointer};
use crate::pointing::{BrowserPointer, BrowserPointing};
use crate::target::Target;
use std::cell::Cell;
use std::rc::Rc;

use rustle::{CursorIcon, Modifier, PointerButton, PointerEvent, PointerInputPhase, PointerPhase};

/// How the pointer looks over a page that asks for `pointer`.
pub fn cursor_of(pointer: Pointer) -> CursorIcon {
    match pointer {
        Pointer::Arrow => CursorIcon::Default,
        Pointer::Hand => CursorIcon::Pointer,
        Pointer::Text => CursorIcon::Text,
        Pointer::Cross => CursorIcon::Crosshair,
        Pointer::Wait => CursorIcon::Wait,
        Pointer::ResizeHorizontal => CursorIcon::ColResize,
        Pointer::ResizeVertical => CursorIcon::RowResize,
    }
}

/// What the window `surface` of a page keeps of the pointer between its events.
#[derive(Default)]
struct Held {
    /// Whether the primary button went down on the window and is still held.
    pressed: Cell<bool>,
    /// How many presses in a row the last press on the window made.
    clicks: Cell<u32>,
}

/// The pointer over the window `surface` of a page, handed on to the page while `open`,
/// which it is not while a question covers the window; `covered` says whether the pointer
/// is over what is drawn over the page, which takes the pointer from it.
pub struct PagePointer {
    /// The window of the page or of its tools.
    surface: Target,
    /// Whether the page takes the pointer.
    open: bool,
    /// Whether the pointer is over what is drawn over the page.
    covered: Rc<Cell<bool>>,
    /// What the window keeps of the pointer.
    held: Rc<Held>,
}

impl PagePointer {
    /// The pointer over the window `surface`, handed on while `open` and while not `covered`.
    #[track_caller]
    pub fn new(surface: Target, open: bool, covered: Rc<Cell<bool>>) -> Self {
        let held = rustle::remember(|| Rc::new(Held::default())).get();
        Self {
            surface,
            open,
            covered,
            held,
        }
    }

    /// Tells the app the pointer did `pointing` at `at` of the window.
    fn tell(
        operations: &Rc<dyn events::Operations>,
        surface: Target,
        at: (f32, f32),
        pointing: BrowserPointing,
    ) {
        operations.browser_pointer(BrowserPointer {
            surface,
            at,
            pointing,
        });
    }

    /// What the window does with its own presses, for its `on_pointer`: the primary button
    /// going down and the secondary one going down and up, with how many presses in a row
    /// the primary one makes.
    pub fn on_pointer(&self) -> OnPointer {
        let operations = events::operations();
        let (surface, open, held) = (self.surface, self.open, Rc::clone(&self.held));
        Rc::new(move |event: &PointerEvent| {
            if !open {
                return;
            }
            let at = (event.position.x, event.position.y);
            let pointing = match (event.button, event.phase) {
                (PointerButton::Primary, PointerPhase::Press) => {
                    held.pressed.set(true);
                    held.clicks.set(event.click_count);
                    BrowserPointing::Button {
                        secondary: false,
                        down: true,
                        clicks: event.click_count,
                    }
                }
                (PointerButton::Secondary, PointerPhase::Press | PointerPhase::Release) => {
                    BrowserPointing::Button {
                        secondary: true,
                        down: event.phase == PointerPhase::Press,
                        clicks: 1,
                    }
                }
                _ => return,
            };
            Self::tell(&operations, surface, at, pointing);
        })
    }

    /// The modifier of the window: the pointer moving over it or leaving it, moving and
    /// letting go of the primary button pressed on it wherever the pointer is, the wheel
    /// turned over it, which goes no further, and, for the page's own window, the mouse's
    /// back and forward buttons.
    pub fn modifier(&self) -> Modifier {
        let operations = events::operations();
        let (surface, open) = (self.surface, self.open);
        let (moved, input, history) = (
            Rc::clone(&operations),
            Rc::clone(&operations),
            Rc::clone(&operations),
        );
        let (hovering, holding) = (Rc::clone(&self.held), Rc::clone(&self.held));
        let covered = Rc::clone(&self.covered);
        let modifier = Modifier::new()
            .on_pointer_move(move |local| {
                if !open || hovering.pressed.get() {
                    return;
                }
                match local.filter(|_| !covered.get()) {
                    Some(local) => {
                        Self::tell(&moved, surface, (local.x, local.y), BrowserPointing::Moved);
                    }
                    None => Self::tell(&moved, surface, (0.0, 0.0), BrowserPointing::Left),
                }
            })
            .pointer_input(move |event| {
                if !open {
                    return false;
                }
                let at = (event.position.x, event.position.y);
                let pointing = match event.phase {
                    PointerInputPhase::Move if holding.pressed.get() => BrowserPointing::Moved,
                    PointerInputPhase::Up | PointerInputPhase::Cancel if holding.pressed.get() => {
                        holding.pressed.set(false);
                        BrowserPointing::Button {
                            secondary: false,
                            down: false,
                            clicks: holding.clicks.get(),
                        }
                    }
                    PointerInputPhase::Scroll => {
                        BrowserPointing::Wheel(event.scroll_delta.x, event.scroll_delta.y)
                    }
                    _ => return false,
                };
                let wheel = event.phase == PointerInputPhase::Scroll;
                Self::tell(&input, surface, at, pointing);
                wheel
            });
        if surface != Target::Browser(crate::areas::BrowserEvent::BrowserBody) {
            return modifier;
        }
        let forward = Rc::clone(&history);
        modifier
            .on_pointer_button(PointerButton::Back, move |event| {
                if event.phase == PointerPhase::Press {
                    let at = (event.position.x, event.position.y);
                    Self::tell(&history, surface, at, BrowserPointing::History(true));
                }
            })
            .on_pointer_button(PointerButton::Forward, move |event| {
                if event.phase == PointerPhase::Press {
                    let at = (event.position.x, event.position.y);
                    Self::tell(&forward, surface, at, BrowserPointing::History(false));
                }
            })
    }
}
