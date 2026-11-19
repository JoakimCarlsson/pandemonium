//! Controls run the lambdas they were given: a click, Enter or Space while the control has
//! the keyboard, and a secondary press.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use rustle::{
    Embedded, Key, KeyEvent, Modifier, Modifiers, Offset, PointerButton, Rect, ResizeEdge,
};

use super::{Change, Interactive, Operations, activates, provide};
use crate::browser_menu::BrowserCommand;
use crate::focus::FocusId;
use crate::git_menus::MenuKind;
use crate::terminal_menu::TerminalCommand;
use crate::tree_actions::TreeTarget;

/// The control the tests press.
const CONTROL: FocusId = FocusId(41);

/// Operations that let every click count and keep what the views asked of them.
#[derive(Default)]
struct Recording {
    /// The controls the primary button went down on, in order.
    pressed: RefCell<Vec<FocusId>>,
    /// How many times the menu of the conversation's text was asked for.
    message_menus: Cell<usize>,
}

impl Operations for Recording {
    /// Does nothing.
    fn update(&self, _: Change) {}

    /// Keeps the control pressed.
    fn pressed(&self, id: FocusId, _: Rect) {
        self.pressed.borrow_mut().push(id);
    }

    /// Lets every click count.
    fn click_lands(&self, _: FocusId, _: Rect) -> bool {
        true
    }

    /// Takes nothing.
    fn key(&self, _: &KeyEvent) -> bool {
        false
    }

    /// Does nothing.
    fn dropped(&self, _: PathBuf) {}

    /// Does nothing.
    fn hovered(&self, _: FocusId, _: bool) {}

    /// Does nothing.
    fn sash_dragged(&self, _: FocusId, _: (f32, f32)) {}

    /// Does nothing.
    fn sash_released(&self) {}

    /// Does nothing.
    fn minimap_dragged(&self, _: f32, _: f32) {}

    /// Does nothing.
    fn dismiss_requested(&self) {}

    /// Carries nothing.
    fn carries(&self, _: FocusId) -> bool {
        false
    }

    /// Does nothing.
    fn carry_started(&self, _: FocusId, _: (f32, f32)) {}

    /// Does nothing.
    fn carry_moved(&self, _: (f32, f32)) {}

    /// Does nothing.
    fn carry_ended(&self, _: Modifiers) {}

    /// Does nothing.
    fn comment_dragged(&self, _: FocusId) {}

    /// Does nothing.
    fn comment_drag_ended(&self) {}

    /// Does nothing.
    fn edge_pressed(&self, _: ResizeEdge) {}

    /// Does nothing.
    fn minimize_window(&self) {}

    /// Does nothing.
    fn toggle_maximized(&self) {}

    /// Does nothing.
    fn close_window(&self) {}

    /// Does nothing.
    fn open_command_palette(&self) {}

    /// Does nothing.
    fn show_code_actions(&self) {}

    /// Does nothing.
    fn navigate(&self, _: isize) {}

    /// Does nothing.
    fn take_listing(&self, _: Option<usize>) {}

    /// Does nothing.
    fn open_color_picker(&self, _: usize) {}

    /// Does nothing.
    fn field_menu_picked(&self, _: FocusId) {}

    /// Does nothing.
    fn message_menu_picked(&self, _: FocusId) {}

    /// Does nothing.
    fn open_browser_capture_menu(&self) {}

    /// Does nothing.
    fn browser_menu_command(&self, _: BrowserCommand) {}

    /// Does nothing.
    fn terminal_command(&self, _: TerminalCommand) {}

    /// Does nothing.
    fn attach(&self) {}

    /// Does nothing.
    fn open_settings_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_project_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_git_menu(&self, _: MenuKind, _: Option<FocusId>) {}

    /// Does nothing.
    fn open_pane_menu(&self) {}

    /// Does nothing.
    fn open_session_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_change_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_folder_change_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_tab_menu(&self, _: usize) {}

    /// Does nothing.
    fn open_tree_menu(&self, _: TreeTarget) {}

    /// Does nothing.
    fn open_field_menu(&self, _: FocusId) {}

    /// Does nothing.
    fn open_terminal_menu(&self) {}

    /// Does nothing.
    fn open_browser_menu(&self) {}

    /// Counts the menu asked for.
    fn open_message_menu(&self) {
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
/// otherwise, under `operations`, with the app's focus kept on `focused`.
fn laid_out(
    operations: &Rc<Recording>,
    asked: &Rc<Asked>,
    menu: bool,
    focused: Option<FocusId>,
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
                size.control_menu(CONTROL, on_click, move || {
                    menus.menus.set(menus.menus.get() + 1)
                })
            } else {
                size.control_with(CONTROL, on_click)
            };
            rustle::spacer(modifier);
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

/// Enter and the space bar going down activate a control; their release and other keys do not.
#[test]
fn enter_and_space_activate() {
    assert!(activates(&down(Key::Enter, Modifiers::NONE)));
    assert!(activates(&down(
        Key::Character(" ".into()),
        Modifiers::NONE
    )));
    assert!(!activates(&KeyEvent {
        pressed: false,
        ..KeyEvent::press(Key::Enter, Modifiers::NONE)
    }));
    assert!(!activates(&down(
        Key::Character("a".into()),
        Modifiers::NONE
    )));
    assert!(!activates(&down(Key::Escape, Modifiers::NONE)));
}

/// A click runs the control's lambda with the keys held, after the press is reported.
#[test]
fn a_click_runs_the_lambda() {
    let operations = Rc::new(Recording::default());
    let asked = Rc::new(Asked::default());
    let mut ui = laid_out(&operations, &asked, false, None);
    press(&mut ui, PointerButton::Primary);
    assert_eq!(*operations.pressed.borrow(), vec![CONTROL]);
    assert_eq!(asked.clicks.borrow().len(), 1);
}

/// Enter and Space run the lambda of the control that has the keyboard, and of no other.
#[test]
fn keys_run_the_lambda_of_the_focused_control() {
    let operations = Rc::new(Recording::default());
    let asked = Rc::new(Asked::default());
    let mut ui = laid_out(&operations, &asked, false, None);
    ui.key_event(down(Key::Enter, Modifiers::NONE));
    ui.update();
    assert!(asked.clicks.borrow().is_empty());
    let mut ui = laid_out(&operations, &asked, false, Some(CONTROL));
    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    ui.key_event(down(Key::Enter, shift));
    ui.key_event(down(Key::Character(" ".into()), Modifiers::NONE));
    ui.update();
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
