//! Scrolling: the wheel over a box and over a box inside another, holding
//! and dragging a scrollbar, and the conversation keeping to its end.

use pm_components::adder::{Found, Kind};
use pm_components::areas::{ComposerEvent, DialogEvent, EditorEvent, SidebarEvent};
use pm_components::state::WorkspaceState;
use pm_components::target::Target;
use pm_pages::Screen;
use rustle::Key;

use crate::app::asides::Aside;
use crate::app::headless::Rig;
use crate::snapshot::Shot;

/// How wide the strip along a scrolling area's right edge is where the pointer takes hold of its bar.
const GRIP: f32 = 14.0;

/// How far the conversation is scrolled back from its end, and how far back it can go.
fn back(rig: &Rig) -> (f32, f32) {
    rig.read(|state| {
        let room = state.chat_list.scroll().max_offset();
        (state.chat_list.scroll().offset().min(room), room)
    })
}

/// Where the conversation is drawn.
fn conversation(rig: &Rig) -> rustle::Rect {
    rig.area(Screen::conversation_area)
        .expect("the conversation is drawn")
}

/// The middle of the conversation.
fn middle(rig: &Rig) -> (f32, f32) {
    let rect = conversation(rig);
    (
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    )
}

/// How far the lines of the prompt are scrolled.
fn prompt_offset(rig: &Rig) -> f32 {
    rig.read(|state| state.prompt_scroll.offset())
}

/// Whether the conversation shows its oldest entry and is scrolled exactly as far back as it
/// can go, the room it has measured once that entry is laid out.
fn at_start(rig: &Rig) -> bool {
    rig.read(|state| {
        let info = state.chat_list.layout_info();
        let oldest = info.total_items_count.checked_sub(1);
        let scroll = state.chat_list.scroll();
        scroll.offset() == scroll.max_offset()
            && info
                .visible_items_info
                .iter()
                .any(|item| Some(item.index) == oldest)
    })
}

/// The wheel over the conversation scrolls it, and not past either end. The room to scroll
/// back is estimated until the oldest entries are measured, as a lazy list's is, so the start
/// is checked against the room measured once it is reached.
#[test]
fn the_wheel_scrolls_the_conversation() {
    let mut rig = Rig::shot(Shot::Workspace);
    let (offset, room) = back(&rig);
    assert!(room > 100.0);
    assert_eq!(offset, 0.0);
    let at = middle(&rig);
    rig.move_to(at);
    rig.wheel(-100.0);
    assert_eq!(back(&rig).0, 100.0);
    rig.wheel(-10_000.0);
    assert!(at_start(&rig));
    let start = back(&rig).0;
    rig.wheel(-100.0);
    assert_eq!(back(&rig).0, start);
    rig.wheel(10_000.0);
    assert_eq!(back(&rig).0, 0.0);
}

/// A scroll of the conversation by the wheel reaches the screen through its observed scroll
/// state, with no event of the app's own, once the scroll rests: the open session keeps how
/// far back it is scrolled.
#[test]
fn the_screen_follows_the_scrolled_conversation() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = middle(&rig);
    rig.move_to(at);
    rig.wheel(-100.0);
    rig.wait(200);
    let kept = rig.state(|state| {
        state
            .open_index()
            .map(|index| state.sessions[index].chat_back)
    });
    assert_eq!(kept, Some(100.0));
}

/// The wheel over the long prompt scrolls the prompt, not the conversation around it.
#[test]
fn the_wheel_scrolls_the_innermost_box() {
    let mut rig = Rig::shot(Shot::ComposerLong);
    let chat = back(&rig).0;
    let prompt = prompt_offset(&rig);
    let at = rig.center(Target::Composer(ComposerEvent::Prompt));
    rig.move_to(at);
    rig.wheel(60.0);
    assert!(prompt_offset(&rig) > prompt);
    assert_eq!(back(&rig).0, chat);
}

/// Holding the conversation's scrollbar and dragging it up scrolls towards the start,
/// and selects nothing on the way.
#[test]
fn dragging_the_scrollbar_scrolls() {
    let mut rig = Rig::shot(Shot::Workspace);
    let rect = conversation(&rig);
    let x = rect.origin.x + rect.size.width - GRIP / 2.0;
    let bottom = rect.origin.y + rect.size.height - 20.0;
    rig.drag((x, bottom), (x, bottom - 120.0), 6);
    let (offset, room) = back(&rig);
    assert!(offset > 50.0, "{offset} of {room}");
    assert_eq!(rig.app.borrow().text_selection(), None);
}

/// At its end the conversation keeps to its end when it has less room to show it.
#[test]
fn the_conversation_keeps_to_its_end() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.resize((1280.0, 680.0));
    let (offset, room) = back(&rig);
    assert!(room > 0.0);
    assert_eq!(offset, 0.0);
}

/// Scrolled up from its end, the conversation keeps how far it is from the end
/// when it has less room.
#[test]
fn the_conversation_keeps_its_place_from_the_end() {
    let mut rig = Rig::shot(Shot::Workspace);
    let at = middle(&rig);
    rig.move_to(at);
    rig.wheel(-100.0);
    rig.resize((1280.0, 680.0));
    assert_eq!(back(&rig).0, 100.0);
}

/// How far the sidebar's list is scrolled, and how far it can go.
fn sidebar(rig: &Rig) -> (f32, f32) {
    rig.state(|state| {
        let scroll = &state.sidebar_scroll;
        (scroll.offset(), scroll.max_offset())
    })
}

/// Whether item `index` of `list` lies wholly inside its viewport as of its latest layout,
/// give or take the gap of a few pixels a list leaves after each of its items.
fn wholly_shown(list: &rustle::LazyListState, index: usize) -> bool {
    let info = list.layout_info();
    info.visible_items_info.iter().any(|item| {
        item.index == index
            && item.offset >= info.viewport_start_offset
            && item.offset + item.size <= info.viewport_end_offset + 3.0
    })
}

/// The wheel over the sidebar's list scrolls it, as far as the list reaches and no further.
#[test]
fn the_wheel_scrolls_the_sidebar() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.resize((1280.0, 330.0));
    let (offset, room) = sidebar(&rig);
    assert_eq!(offset, 0.0);
    assert!(room > 40.0, "{room}");
    let at = rig.center(Target::Sidebar(SidebarEvent::Project(0)));
    rig.move_to(at);
    rig.wheel(30.0);
    assert_eq!(sidebar(&rig).0, 30.0);
    rig.wheel(10_000.0);
    let (offset, room) = sidebar(&rig);
    assert_eq!(offset, room);
    rig.wheel(-10_000.0);
    assert_eq!(sidebar(&rig).0, 0.0);
}

/// The wheel over the subagents' list scrolls it first, and what it has no room for
/// scrolls the conversation around it.
#[test]
fn the_subagents_hand_the_wheel_on() {
    let mut rig = Rig::shot(Shot::AgentsScrolled);
    let start = rig.state(|state| {
        let scroll = &state.subagents_scroll;
        scroll.offset().min(scroll.max_offset())
    });
    assert!(start > 40.0, "{start}");
    let at = rig.center(Target::Composer(ComposerEvent::Prompt));
    rig.move_to((at.0, at.1 - 160.0));
    rig.wheel(10.0);
    assert_eq!(rig.state(|state| state.subagents_scroll.offset()), start);
    rig.wheel(-30.0);
    assert_eq!(
        rig.state(|state| state.subagents_scroll.offset()),
        start - 30.0
    );
    assert_eq!(back(&rig).0, 0.0);
    rig.wheel(-10_000.0);
    assert_eq!(rig.state(|state| state.subagents_scroll.offset()), 0.0);
    assert!(back(&rig).0 > 0.0);
}

/// `count` accounts to pick from in the panel for adding a project.
fn many_owners(count: usize) -> Vec<Found> {
    (0..count)
        .map(|index| Found {
            name: format!("account-{index}"),
            detail: String::new(),
            value: format!("account-{index}"),
            kind: Kind::Owner(false),
        })
        .collect()
}

/// Moving the highlight down the long list of the panel for adding a project keeps the
/// highlighted row in view, and moving it back up does too.
#[test]
fn the_adder_keeps_its_highlight_in_view() {
    let state = WorkspaceState::sample()
        .handle(pm_components::events::Event::Sidebar(
            SidebarEvent::AddProject,
        ))
        .handle(pm_components::events::Event::Dialog(
            DialogEvent::AdderClone,
        ))
        .with_owners(Ok(many_owners(30)));
    let mut rig = Rig::new(Screen::Workspace(state), crate::app::headless::SIZE);
    let shown = |rig: &Rig| {
        rig.state(|state| {
            let open = state.adder.as_ref().expect("the panel is open");
            wholly_shown(&open.list, open.selected)
        })
    };
    for _ in 0..20 {
        rig.key(Key::ArrowDown);
        assert!(shown(&rig));
    }
    assert!(rig.state(|state| state.adder.as_ref().unwrap().list.scroll().offset()) > 0.0);
    for _ in 0..20 {
        rig.key(Key::ArrowUp);
        assert!(shown(&rig));
    }
}

/// Moving the highlight down the long list of the folder picker keeps the highlighted
/// folder in view.
#[test]
fn the_folder_picker_keeps_its_highlight_in_view() {
    let mut rig = Rig::shot(Shot::Workspace);
    rig.click_on(Target::Sidebar(SidebarEvent::AddProject));
    rig.click_on(Target::Dialog(DialogEvent::AdderBrowse));
    rig.app.borrow().model.update(|screen| {
        let Screen::Workspace(state) = std::mem::take(screen);
        let requested = state.folder_picker.as_ref().unwrap().requested.clone();
        let folders = (0..40).map(|index| format!("folder-{index:02}")).collect();
        *screen =
            Screen::Workspace(state.with_folders(&requested, Ok(("/home/me".to_owned(), folders))));
    });
    rig.settle();
    let shown = |rig: &Rig| {
        rig.state(|state| {
            let open = state.folder_picker.as_ref().expect("the picker is open");
            wholly_shown(&open.list, open.selected)
        })
    };
    for step in 0..30 {
        rig.key_in(Some(Aside::Picker), Key::ArrowDown);
        assert!(shown(&rig), "step {step}");
    }
    assert!(rig.state(|state| state.folder_picker.as_ref().unwrap().list.scroll().offset()) > 0.0);
}

/// Moving the pick down the long list of the quick input keeps the picked row inside it.
#[test]
fn the_quick_input_keeps_its_pick_in_view() {
    let mut rig = Rig::shot(Shot::Commands);
    rig.key(Key::Backspace);
    rig.key(Key::Backspace);
    let count = rig.state(|state| {
        state
            .listing
            .as_ref()
            .map_or(0, |listing| listing.items.rows().len())
    });
    assert!(
        count > pm_components::quick_input::QUICK_SHOWN + 4,
        "{count}"
    );
    let body = rig
        .region(Target::Editor(EditorEvent::ListingBody))
        .expect("the quick input is drawn");
    for picked in 1..count.min(40) {
        rig.key(Key::ArrowDown);
        let row = rig
            .region(Target::Editor(EditorEvent::ListingRow(picked)))
            .expect("the picked row is drawn");
        assert!(row.size.height > 20.0, "row {picked} is cut: {row:?}");
        assert!(
            row.origin.y >= body.origin.y
                && row.origin.y + row.size.height <= body.origin.y + body.size.height + 0.5,
            "{picked}: {row:?} in {body:?}"
        );
    }
}

/// Whether the language server is to be asked for the names of the lines in view now.
fn names_wanted(rig: &Rig) -> bool {
    rig.read(|screen| screen.names_in_view_wanted())
}

/// The names of the lines in view of a long file are asked for as the file shows and again
/// after a scroll brings other lines into view, by the root's effect over the lines in view
/// and the file's version rather than after every frame.
#[test]
fn the_names_in_view_are_asked_for_as_they_come_into_view() {
    let mut rig = Rig::shot(Shot::Code);
    rig.app.borrow().model.update(|screen| {
        screen.editor.semantic_highlighting = true;
        screen.editor.semantic_range_lines = 1;
        screen.editor.semantic_range_margin = 0;
        for viewer in &mut screen.git.viewers {
            if let pm_components::open_files::Contents::Text(document) = &mut viewer.contents {
                document.synced = true;
                document.semantic_tokens_supported = true;
                document.semantic_ranges_supported = true;
            }
        }
    });
    assert!(names_wanted(&rig), "the shot's file wants no names");
    rig.settle();
    assert!(!names_wanted(&rig), "the names in view were not asked for");
    let area = rig.read(Screen::code_area).expect("the file is shown");
    rig.move_to((area.origin.x + 100.0, area.origin.y + 100.0));
    let top = |rig: &Rig| {
        rig.read(|screen| {
            screen
                .shown_viewer()
                .and_then(|viewer| viewer.scroll.first_visible_item())
        })
    };
    let before = top(&rig);
    rig.wheel(600.0);
    assert_ne!(top(&rig), before, "the file did not scroll");
    assert!(
        !names_wanted(&rig),
        "the lines scrolled into view were not asked for"
    );
}
