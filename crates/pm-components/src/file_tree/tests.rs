//! Regression coverage for the lazy list of the project's files: what it composes, where its
//! rows land, the field a name is typed into, and Tab walking rows not composed.

use std::cell::Cell;
use std::rc::Rc;

use rustle::{Embedded, Key, KeyEvent, Modifiers};

use super::*;
use crate::focus::FocusKeys;
use crate::tagged::{Tagged, interface, tagged_in};
use crate::theme::set_scales;

/// Makes a large flat tree whose file indices also follow display order.
fn sample(count: usize) -> WorkspaceState {
    let mut state = WorkspaceState::sample();
    state.files = (0..count)
        .map(|index| format!("file-{index:05}.rs"))
        .collect();
    state.tree_folders.clear();
    state.open_tree.clear();
    state
}

/// Lays out and recomposes `ui` a few times, as frames do, so the list learns its heights.
fn settle(ui: &mut Embedded) {
    for _ in 0..6 {
        ui.invalidate();
        ui.update();
    }
}

/// Lays out the list of `state`'s files, `height` logical pixels tall, and returns its
/// controls by tag.
fn paint(state: &WorkspaceState, height: f32) -> Vec<Tagged> {
    let held = state.clone();
    let mut ui = interface(380.0, height, None, move || {
        rustle::column_with(Modifier::new().fill_max_size(), || list(&held))
    });
    settle(&mut ui);
    tagged_in(&ui, 0.0, 0.0)
}

/// The rows of `laid` that can be seen, with the top of each.
fn rows_seen(laid: &[Tagged]) -> Vec<(FocusId, f32)> {
    laid.iter()
        .filter(|node| node.id.0 >= TREE_FILE_BASE && node.visible.size.height > 0.0)
        .map(|node| (node.id, node.bounds.origin.y))
        .collect()
}

/// The list composes only the rows near its view, each a row's height under the one before,
/// wherever it is scrolled to, at every density.
#[test]
fn the_list_composes_only_rows_in_view_at_their_places() {
    for scale in [1.0, 0.8] {
        set_scales(scale, 1.0);
        let height = 24.0 * scale;
        for first in [0, 5, 100, 150] {
            let state = sample(200);
            state.files_list.scroll_to_item_offset(first, 0.0);
            let laid = paint(&state, 240.0);
            let composed = laid
                .iter()
                .filter(|node| node.id.0 >= TREE_FILE_BASE)
                .count();
            assert!(
                composed <= (240.0 / height).ceil() as usize + 4,
                "only rows near the view are composed: {composed}, scale={scale}, first={first}"
            );
            let seen = rows_seen(&laid);
            let top = seen.first().expect("a row is seen");
            assert_eq!(top.0, FocusId(TREE_FILE_BASE + first as u64));
            for pair in seen.windows(2) {
                assert_eq!(pair[1].0.0, pair[0].0.0 + 1, "rows follow in order");
                assert!(
                    (pair[1].1 - pair[0].1 - height).abs() < 1.0,
                    "rows are a row's height apart: {pair:?}, scale={scale}"
                );
            }
        }
    }
    set_scales(1.0, 1.0);
}

/// The space under the rows fills what a short list leaves of the view, and keeps its least
/// height under a long one.
#[test]
fn the_space_under_the_rows_fills_the_view() {
    let laid = paint(&sample(3), 400.0);
    let space = laid
        .iter()
        .find(|node| node.id == TREE_SPACE)
        .expect("the space is laid out");
    let bottom = space.bounds.origin.y + space.bounds.size.height;
    assert!(
        (bottom - (400.0 - sp(1))).abs() < 1.0,
        "the space reaches the bottom of the view: {bottom}"
    );
    let state = sample(200);
    state.files_list.scroll_to_end();
    let laid = paint(&state, 240.0);
    let space = laid
        .iter()
        .find(|node| node.id == TREE_SPACE)
        .expect("the space is laid out at the end");
    assert!((space.bounds.size.height - sp(SPACE_LEAST)).abs() < 1.0);
}

/// Expanding folders and changing the server's file list does not leave stale rows or indices.
#[test]
fn folder_expansion_and_file_changes() {
    let mut state = sample(0);
    state.files = vec!["src/b.rs".to_owned(), "src/a.rs".to_owned()];
    state.tree_folders = vec!["empty".to_owned()];
    assert_eq!(
        shown_rows(&state.files, &state.tree_folders, &state.open_tree).len(),
        2
    );
    state.open_tree.insert("src".to_owned());
    let shown = shown_rows(&state.files, &state.tree_folders, &state.open_tree);
    assert_eq!(
        shown
            .iter()
            .map(|row| row.path.as_str())
            .collect::<Vec<_>>(),
        ["empty", "src", "src/a.rs", "src/b.rs"]
    );
    state.files.remove(0);
    assert_eq!(
        shown_rows(&state.files, &state.tree_folders, &state.open_tree)
            .last()
            .unwrap()
            .path,
        "src/a.rs"
    );
}

/// A reproducible CPU measure of composing, laying out and painting a large expanded tree.
#[test]
#[ignore = "manual CPU benchmark: cargo test -p pm-components benchmark_large_tree -- --ignored --nocapture"]
fn benchmark_large_tree() {
    let state = sample(10_000);
    state.files_list.scroll_to_item_offset(5_000, 0.0);
    std::hint::black_box(paint(&state, 600.0));
    let started = std::time::Instant::now();
    for _ in 0..20 {
        std::hint::black_box(paint(&state, 600.0));
    }
    eprintln!(
        "tree build+layout+paint: {:.2} ms/frame",
        started.elapsed().as_secs_f64() * 50.0
    );
}

/// The field a new name is typed into, with what is wrong with the name under it, is taller
/// than a row, and the rows after it start under it.
#[test]
fn naming_keeps_variable_height_rows() {
    let mut state = sample(200);
    state.git.naming = Some((
        Naming::New {
            directory: String::new(),
            folder: false,
        },
        rustle::TextFieldState::new("file-00000.rs"),
    ));
    assert!(state.naming_problem().is_some());
    let laid = paint(&state, 240.0);
    let field = laid
        .iter()
        .find(|node| node.id == ENTRY_FIELD)
        .expect("the field is laid out");
    let first = rows_seen(&laid);
    let first = first.first().expect("a row is seen");
    assert_eq!(first.0, FocusId(TREE_FILE_BASE));
    assert!(
        first.1 > field.bounds.origin.y + field.bounds.size.height + 10.0,
        "the problem line sits between the field and the first row: {first:?}, {:?}",
        field.bounds
    );
}

/// Tab and Shift+Tab walk every row of a long tree in tree order, those not composed
/// included: a row past the ones in view is scrolled into view and takes focus, and the row
/// holding focus stays composed while it is scrolled away.
#[test]
fn tab_walks_every_row_scrolling_hidden_ones_in() {
    let state = sample(60);
    let rows: Vec<FocusId> = (0..60)
        .map(|index| FocusId(TREE_FILE_BASE + index))
        .collect();
    let held: Rc<Cell<Option<FocusId>>> = Rc::new(Cell::new(rows.first().copied()));
    let (shown, keeps, asks) = (Rc::clone(&held), Rc::clone(&held), Rc::clone(&held));
    let tree = state.clone();
    let mut ui = Embedded::without_renderer(
        move || {
            let keeps = Rc::clone(&keeps);
            let asks = Rc::clone(&asks);
            let keys = FocusKeys {
                held: shown.get(),
                on_moved: Rc::new(move |id, focused| {
                    if focused {
                        keeps.set(Some(id));
                    } else if keeps.get() == Some(id) {
                        keeps.set(None);
                    }
                }),
                request: Rc::new(move |id| asks.set(Some(id))),
            };
            crate::focus::provide(keys, || {
                rustle::column_with(Modifier::new().fill_max_size(), || list(&tree))
            });
        },
        || {},
    );
    ui.resize((380, 240), 1.0);
    settle(&mut ui);
    let mut walked = vec![held.get().expect("the first row holds focus")];
    for _ in 1..rows.len() {
        ui.key_event(KeyEvent::press(Key::Tab, Modifiers::NONE));
        settle(&mut ui);
        walked.push(held.get().expect("a row holds focus"));
    }
    assert_eq!(walked, rows);
    assert!(
        state.files_list.scroll().offset() > 0.0,
        "the last rows were scrolled in"
    );
    state.files_list.scroll_to_item_offset(0, 0.0);
    settle(&mut ui);
    assert_eq!(
        held.get(),
        rows.last().copied(),
        "the focused row stays composed"
    );
    let mut back = vec![held.get().expect("the last row holds focus")];
    for _ in 1..rows.len() {
        ui.key_event(KeyEvent::press(Key::Tab, Modifiers::SHIFT));
        settle(&mut ui);
        back.push(held.get().expect("a row holds focus"));
    }
    back.reverse();
    assert_eq!(back, rows);
}
