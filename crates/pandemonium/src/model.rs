//! The screen the app holds, shared by every window's root, and that the app's logic and its
//! work change.
//!
//! Changes are counted by two rustle states: a generation of the screen, which every change
//! moves on but the words an agent streams into a conversation, and a generation of those.
//! Every read of the screen follows the first, so the roots compose again for any change but a
//! streamed word, and the block of the main window's root that follows the screen runs the
//! app's reaction to it as a side effect. The title bar and the sidebar are keyed on that
//! generation, so they are composed again only when it moved. The regions of the screen that
//! can show a conversation follow both generations themselves ([`pm_pages::observed`]). The
//! sizes of panels are held apart from the screen as observable state, so dragging a handle
//! writes no screen and leaves every region whose room did not change as it was.

use std::cell::RefCell;
use std::rc::Rc;

use pm_pages::observed::ScreenState;
use pm_pages::{Screen, WorkspaceState};

/// The screen as a rustle state. Every clone is a handle to the same screen.
///
/// The screen itself is held outside rustle's state: what composes follows two generations instead, one
/// that every change moves on but the words an agent streams into a conversation, and one
/// those move on, so a streamed token composes again only what shows a conversation.
#[derive(Clone)]
pub struct Model {
    /// The screen, held outside rustle's state so reading it records nothing: the
    /// generations say when it changed.
    state: Rc<RefCell<Screen>>,
    /// The generation of every change of the screen but the words streamed into conversations,
    /// which every read of the screen follows.
    screen: rustle::State<u64>,
    /// The generation of the words streamed into conversations, which only the regions that
    /// can show one follow.
    conversation: rustle::State<u64>,
    /// The generation of what streamed words and reading them change of the sidebar and the
    /// title bar, which only those follow.
    chrome: rustle::State<u64>,
    /// Handles to the generations that the views are given, so the regions of the screen
    /// follow them themselves.
    observed: ScreenState,
}

impl Model {
    /// A model holding `screen`.
    pub fn new(screen: Screen) -> Self {
        let state = Rc::new(RefCell::new(screen));
        let generation = rustle::mutable_state(0u64);
        let conversation = rustle::mutable_state(0u64);
        let chrome = rustle::mutable_state(0u64);
        Self {
            observed: ScreenState::new(generation.clone(), conversation.clone(), chrome.clone()),
            state,
            screen: generation,
            conversation,
            chrome,
        }
    }

    /// Reads the screen through `read` and returns what it returns; what composes while it
    /// reads composes again for every change of the screen but streamed words.
    pub fn read<R>(&self, read: impl FnOnce(&Screen) -> R) -> R {
        self.screen.with(|_| ());
        read(&self.state.borrow())
    }

    /// Changes the screen in place through `change`, which composes again what read it.
    pub fn update(&self, change: impl FnOnce(&mut Screen)) {
        change(&mut self.state.borrow_mut());
        self.screen.update(|generation| *generation += 1);
    }

    /// Changes the screen in place through `change`, a change that only the conversation it
    /// streams words into shows, which composes again only the regions that show one.
    pub fn update_conversation(&self, change: impl FnOnce(&mut Screen)) {
        change(&mut self.state.borrow_mut());
        self.conversation.update(|generation| *generation += 1);
    }

    /// Replaces the workspace the screen shows with what `change` makes of it,
    /// as [`Model::update`] does.
    pub fn change(&self, change: impl FnOnce(WorkspaceState) -> WorkspaceState) {
        self.update(|screen| *screen = std::mem::take(screen).in_workspace(change));
    }

    /// Changes the screen in place through `change`, a change that only the conversations and
    /// the chrome show, such as which updates were read: it composes again the regions that
    /// show a conversation and, when the sidebar or the title bar shows it, those.
    pub fn update_read(&self, change: impl FnOnce(&mut Screen)) {
        let before = self.read(chrome);
        self.update_conversation(change);
        let after = self.read(chrome);
        if before.is_none() || after.is_none() {
            self.update(|_| {});
        } else if after != before {
            self.chrome.update(|generation| *generation += 1);
        }
    }

    /// Takes in what the agent of the session at `session` did, as `event`: words it streams
    /// move on only the generation of the conversations, and the chrome's as well when they
    /// change the sidebar or the title bar; anything else moves on the screen's.
    pub fn hear(&self, session: usize, event: pm_harness::Event) {
        if matches!(
            event,
            pm_harness::Event::Said(_) | pm_harness::Event::Thought(_)
        ) {
            self.update_read(|screen| screen.hear_in_place(session, event));
        } else {
            self.update(|screen| screen.hear_in_place(session, event));
        }
    }

    /// The generations of the screen and of its conversations as they are now, which move on
    /// with every change of either, read without composing anything again for them.
    pub fn generations(&self) -> (u64, u64) {
        rustle::untracked(|| {
            (
                self.screen.with(|generation| *generation),
                self.conversation.with(|generation| *generation),
            )
        })
    }

    /// The handle to the screen's generations the views are given.
    pub fn observed(&self) -> ScreenState {
        self.observed.clone()
    }
}

/// What the window shows of `screen` outside its conversations that the words an agent
/// streams could change: the sidebar and the title bar; nothing while the history page,
/// which shows the conversations' words, is open.
fn chrome(
    screen: &Screen,
) -> Option<(
    pm_components::sidebar::SidebarState,
    pm_components::title_bar::TitleBar,
)> {
    match screen {
        Screen::Workspace(state) if state.page != pm_components::state::Page::History => Some((
            pm_components::sidebar::SidebarState::of(state),
            pm_components::title_bar::TitleBar::of(state),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A change through any handle is seen through every other.
    #[test]
    fn a_change_reaches_every_handle() {
        let model = Model::new(Screen::default());
        let other = model.clone();
        other.change(|state| state.with_fonts(vec!["Inter".to_owned()], Vec::new()));
        assert!(model.read(|screen| *screen != Screen::default()));
    }

    /// A block that reads the screen runs again for a change of the screen, and not for a
    /// panel resized through the size it holds apart from the screen.
    #[test]
    fn only_a_held_resize_leaves_the_screen() {
        use pm_components::areas::WindowEvent;
        use pm_components::target::Target;
        use std::cell::Cell;
        use std::rc::Rc;
        let model = Model::new(Screen::default());
        let runs = Rc::new(Cell::new(0));
        let (reading, counted) = (model.clone(), Rc::clone(&runs));
        let mut ui = rustle::Embedded::without_renderer(
            move || {
                let (reading, counted) = (reading.clone(), Rc::clone(&counted));
                rustle::skippable((), move || {
                    reading.read(|_| ());
                    counted.set(counted.get() + 1);
                });
            },
            || {},
        );
        ui.resize((100, 100), 1.0);
        ui.update();
        let before = runs.get();
        let sash = Target::Window(WindowEvent::SidebarSash);
        assert!(model.read(|screen| screen.hold_sash_size(sash, 300.0, (1280.0, 800.0))));
        ui.update();
        assert_eq!(runs.get(), before);
        assert_eq!(model.read(|screen| screen.sizes.sidebar_width.get()), 300.0);
        model.update(|_| {});
        ui.update();
        assert_eq!(runs.get(), before + 1);
    }

    /// A change of a conversation alone leaves a block that read the screen as it was, while
    /// it moves on the generation the regions showing a conversation follow.
    #[test]
    fn a_conversation_change_leaves_the_screen_readers() {
        use std::cell::Cell;
        use std::rc::Rc;
        let model = Model::new(Screen::default());
        let runs = Rc::new(Cell::new(0));
        let (reading, counted) = (model.clone(), Rc::clone(&runs));
        let mut ui = rustle::Embedded::without_renderer(
            move || {
                let (reading, counted) = (reading.clone(), Rc::clone(&counted));
                rustle::skippable((), move || {
                    reading.read(|_| ());
                    counted.set(counted.get() + 1);
                });
            },
            || {},
        );
        ui.resize((100, 100), 1.0);
        ui.update();
        let before = runs.get();
        let conversation = model.conversation.with(|generation| *generation);
        model.update_conversation(|_| {});
        ui.update();
        assert_eq!(runs.get(), before);
        assert_eq!(
            model.conversation.with(|generation| *generation),
            conversation + 1
        );
    }

    /// Words an agent streams move on the conversations' generation, and the chrome's only
    /// when the sidebar or the title bar shows them, and leave a block that read the screen as
    /// it was.
    #[test]
    fn streamed_words_leave_the_screen_readers() {
        use std::cell::Cell;
        use std::rc::Rc;
        let model = Model::new(Screen::Workspace(crate::snapshot::heavy_workspace()));
        let runs = Rc::new(Cell::new(0));
        let (reading, counted) = (model.clone(), Rc::clone(&runs));
        let mut ui = rustle::Embedded::without_renderer(
            move || {
                let (reading, counted) = (reading.clone(), Rc::clone(&counted));
                rustle::skippable((), move || {
                    reading.read(|_| ());
                    counted.set(counted.get() + 1);
                });
            },
            || {},
        );
        ui.resize((100, 100), 1.0);
        ui.update();
        let before = runs.get();
        let (screen, conversation) = model.generations();
        let chrome = model.chrome.with(|generation| *generation);
        let chrome_of = super::chrome;
        model.hear(0, pm_harness::Event::Said("More words.".to_owned()));
        ui.update();
        assert_eq!(runs.get(), before);
        assert_eq!(model.generations(), (screen, conversation + 1));
        let sidebar_changed = model.chrome.with(|generation| *generation) != chrome;
        let shown = model.read(chrome_of);
        model.hear(0, pm_harness::Event::Said("Yet more words.".to_owned()));
        assert_eq!(
            model.chrome.with(|generation| *generation) != chrome + u64::from(sidebar_changed),
            model.read(chrome_of) != shown
        );
    }
}
