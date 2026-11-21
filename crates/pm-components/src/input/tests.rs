//! Long inputs keep keyboard navigation and composition, and show at most their
//! most lines however long their text, at any font scale.

use crate::areas::ChatEvent;
use crate::target::Target;
use std::cell::RefCell;
use std::rc::Rc;

use rustle::{Embedded, Key, KeyEvent, Modifiers, PixelRounding};

use super::*;
use crate::focus::{self, FocusRequesters};

/// The heights a multiline input editing `field` was laid out at, in a
/// window 640 by 240 logical pixels, with the keyboard on it.
struct Shown {
    /// The interface.
    ui: Embedded,
    /// The heights, latest last.
    heights: Rc<RefCell<Vec<f32>>>,
}

/// Lays out a multiline input editing `field` in a small window with keyboard focus.
fn show(field: &TextFieldState) -> Shown {
    let id = Target::Chat(ChatEvent::Choice(999));
    let heights: Rc<RefCell<Vec<f32>>> = Rc::default();
    let (field, told) = (field.clone(), Rc::clone(&heights));
    let requesters = FocusRequesters::new();
    requesters.request_focus(id);
    let mut ui = Embedded::without_renderer(
        move || {
            let told = Rc::clone(&told);
            focus::provide(requesters.clone(), || {
                Input::new(&field, id)
                    .variant(Variant::Multiline)
                    .modifier(
                        Modifier::new()
                            .width(600.0)
                            .on_size_changed(move |size| told.borrow_mut().push(size.height)),
                    )
                    .build();
            });
        },
        || {},
    );
    ui.set_pixel_rounding(PixelRounding::Logical);
    ui.resize((640, 240), 1.0);
    ui.update();
    ui.update();
    Shown { ui, heights }
}

impl Shown {
    /// The height the input was last laid out at.
    fn height(&self) -> f32 {
        self.heights.borrow().last().copied().unwrap_or_default()
    }

    /// Presses `key` with `modifiers` held.
    fn press(&mut self, key: Key, modifiers: Modifiers) {
        self.ui.key_event(KeyEvent::press(key, modifiers));
        self.ui.update();
    }
}

/// Thousands of lines show as at most the input's most lines, and the caret
/// still moves through them by line, to line ends, and composes in place.
#[test]
fn long_text_keeps_keyboard_navigation_and_composition() {
    let text = "Unicode café 🦀 in an editable paragraph.\n".repeat(1500);
    let field = TextFieldState::with_selection(&text, rustle::TextRange::collapsed_at(0));
    let mut shown = show(&field);
    assert!(shown.ui.has_focus());
    let line = typography().line_height;
    assert!(shown.height() <= line * MAX_LINES as f32 + PADDING + 1.0);
    shown.press(Key::ArrowDown, Modifiers::NONE);
    let second = "Unicode café 🦀 in an editable paragraph.\n"
        .chars()
        .count();
    assert_eq!(field.selection(), rustle::TextRange::collapsed_at(second));
    shown.press(Key::End, Modifiers::NONE);
    assert_eq!(
        field.selection(),
        rustle::TextRange::collapsed_at(second * 2 - 1)
    );
    shown.ui.preedit("字", None);
    shown.ui.update();
    assert_eq!(field.text(), text);
    shown.ui.commit("字");
    shown.ui.update();
    assert!(field.text().contains('字'));
    shown.press(Key::Character("a".into()), Modifiers::CTRL);
    assert_eq!(field.selected_text(), field.text());
}

/// Density and text scaling keep the input as tall as its most lines at its own font.
#[test]
fn long_text_follows_font_scale() {
    let field = TextFieldState::new("large text with Unicode 🦀\n".repeat(100));
    for density in [0.8, 1.0] {
        for text_scale in [1.0, 1.5] {
            crate::theme::set_scales(density, text_scale);
            let shown = show(&field);
            let line = typography().line_height;
            assert!(shown.height() <= line * MAX_LINES as f32 + PADDING + 1.0);
            assert!(shown.height() >= line);
        }
    }
    crate::theme::set_scales(1.0, 1.0);
}
