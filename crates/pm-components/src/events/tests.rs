//! Controls run the lambdas they were given: a click, Enter or Space while the control has
//! the keyboard, and a secondary press.

use crate::areas::WindowEvent;
use crate::target::Target;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rustle::{Embedded, Key, KeyEvent, Modifier, Modifiers, Offset, PointerButton};

use super::{Change, Interactive, Operations, provide};
use crate::pointing::Press;

/// Operations that let every click count and keep what the views asked of them.
#[derive(Default)]
struct Recording {
    /// The controls the primary button went down on, in order.
    pressed: RefCell<Vec<Target>>,
    /// How many times the menu of the conversation's text was asked for.
    message_menus: Cell<usize>,
}

impl Operations for Recording {
    /// Does nothing.
    fn update(&self, _: Change) {}

    /// Keeps the control pressed.
    fn pressed(&self, press: Press) {
        self.pressed.borrow_mut().push(press.id);
    }

    /// Lets every click count.
    fn click_lands(&self, _: Press) -> bool {
        true
    }

    /// Counts the menu asked for.
    fn open_message_menu(&self, _: Option<usize>, _: (f32, f32)) {
        self.message_menus.set(self.message_menus.get() + 1);
    }
}

/// What a control laid out by [`laid_out`] was asked to do: its clicks with the modifier
/// keys held, and its secondary presses when it has a menu of its own.
#[derive(Default)]
struct Asked {
    /// The modifier keys held for each click, in order.
    clicks: RefCell<Vec<Modifiers>>,
    /// How many times its menu was asked for.
    menus: Cell<usize>,
}

/// A window 200 by 100 logical pixels holding one control 100 by 40 at its top left, made
/// with [`Interactive::control_menu`] when `menu` and [`Interactive::control_with`]
/// otherwise, in the first message of a conversation, under `operations`, with rustle's
/// focus given to `focused`.
fn laid_out(
    operations: &Rc<Recording>,
    asked: &Rc<Asked>,
    menu: bool,
    focused: Option<Target>,
) -> Embedded {
    let operations: Rc<dyn Operations> = Rc::clone(operations) as Rc<dyn Operations>;
    let asked = Rc::clone(asked);
    crate::tagged::interface(200.0, 100.0, focused, move || {
        let asked = Rc::clone(&asked);
        provide(Rc::clone(&operations), move || {
            let clicked = Rc::clone(&asked);
            let on_click = move |held| clicked.clicks.borrow_mut().push(held);
            let size = Modifier::new().size(100.0, 40.0);
            let modifier = if menu {
                let menus = Rc::clone(&asked);
                size.control_menu(Target::Window(WindowEvent::Control), on_click, move |_| {
                    menus.menus.set(menus.menus.get() + 1)
                })
            } else {
                size.control_with(Target::Window(WindowEvent::Control), on_click)
            };
            rustle::column_arranged(
                Modifier::new().fill_max_size().message_area(0),
                rustle::Arrangement::Start,
                rustle::Alignment::Start,
                move || rustle::spacer(modifier),
            );
        });
    })
}

/// A key event of `key` going down, with `modifiers` held.
fn down(key: Key, modifiers: Modifiers) -> KeyEvent {
    KeyEvent {
        pressed: true,
        ..KeyEvent::press(key, modifiers)
    }
}

/// Presses and lets go of `button` at the middle of the control.
fn press(ui: &mut Embedded, button: PointerButton) {
    ui.pointer_moved(Offset::new(50.0, 20.0));
    ui.pointer_button(button, true);
    ui.pointer_button(button, false);
    ui.update();
}

/// Sends `key` going down, then coming up, with the modifier keys `modifiers` held.
fn stroke(ui: &mut Embedded, key: Key, modifiers: Modifiers) {
    ui.set_modifiers(modifiers);
    ui.key_event(down(key.clone(), modifiers));
    ui.key_event(KeyEvent {
        pressed: false,
        ..KeyEvent::press(key, modifiers)
    });
    ui.set_modifiers(Modifiers::NONE);
    ui.update();
}

/// Enter and the space bar activate the focused control as they come up; going down alone,
/// coming up alone and other keys do not.
#[test]
fn enter_and_space_activate() {
    let operations = Rc::new(Recording::default());
    let asked = Rc::new(Asked::default());
    let mut ui = laid_out(
        &operations,
        &asked,
        false,
        Some(Target::Window(WindowEvent::Control)),
    );
    ui.key_event(down(Key::Enter, Modifiers::NONE));
    ui.update();
    assert!(asked.clicks.borrow().is_empty(), "the key went down only");
    ui.key_event(KeyEvent {
        pressed: false,
        ..KeyEvent::press(Key::Enter, Modifiers::NONE)
    });
    ui.update();
    assert_eq!(asked.clicks.borrow().len(), 1);
    ui.key_event(KeyEvent {
        pressed: false,
        ..KeyEvent::press(Key::Enter, Modifiers::NONE)
    });
    stroke(&mut ui, Key::Character("a".into()), Modifiers::NONE);
    stroke(&mut ui, Key::Escape, Modifiers::NONE);
    assert_eq!(asked.clicks.borrow().len(), 1);
    let mut ui = laid_out(
        &operations,
        &asked,
        false,
        Some(Target::Window(WindowEvent::Control)),
    );
    stroke(&mut ui, Key::Character(" ".into()), Modifiers::NONE);
    assert_eq!(asked.clicks.borrow().len(), 2);
}

/// A click runs the control's lambda with the keys held, after the press is reported.
#[test]
fn a_click_runs_the_lambda() {
    let operations = Rc::new(Recording::default());
    let asked = Rc::new(Asked::default());
    let mut ui = laid_out(&operations, &asked, false, None);
    press(&mut ui, PointerButton::Primary);
    assert_eq!(
        *operations.pressed.borrow(),
        vec![Target::Window(WindowEvent::Control)]
    );
    assert_eq!(asked.clicks.borrow().len(), 1);
}

/// Enter and Space run the lambda of the control that has the keyboard, and of no other.
#[test]
fn keys_run_the_lambda_of_the_focused_control() {
    let operations = Rc::new(Recording::default());
    let asked = Rc::new(Asked::default());
    let mut ui = laid_out(&operations, &asked, false, None);
    stroke(&mut ui, Key::Enter, Modifiers::NONE);
    assert!(asked.clicks.borrow().is_empty());
    let mut ui = laid_out(
        &operations,
        &asked,
        false,
        Some(Target::Window(WindowEvent::Control)),
    );
    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    stroke(&mut ui, Key::Enter, shift);
    stroke(&mut ui, Key::Character(" ".into()), Modifiers::NONE);
    assert_eq!(*asked.clicks.borrow(), vec![shift, Modifiers::NONE]);
}

/// A secondary press opens the control's own menu when it has one, and the menu of the
/// conversation's text otherwise.
#[test]
fn a_secondary_press_opens_the_right_menu() {
    let operations = Rc::new(Recording::default());
    let asked = Rc::new(Asked::default());
    let mut ui = laid_out(&operations, &asked, true, None);
    press(&mut ui, PointerButton::Secondary);
    assert_eq!((asked.menus.get(), operations.message_menus.get()), (1, 0));
    let mut ui = laid_out(&operations, &asked, false, None);
    press(&mut ui, PointerButton::Secondary);
    assert_eq!((asked.menus.get(), operations.message_menus.get()), (1, 1));
    assert!(asked.clicks.borrow().is_empty());
}
