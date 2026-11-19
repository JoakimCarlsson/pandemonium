//! Regression coverage for virtual tree geometry, focus order and variable-height naming.

use crate::events::reduce;
use crate::theme;
use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::events::Interactive;
use crate::focus::FocusKeys;
use crate::tagged::{Laid, Tagged, lay_out};
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

/// Emits every row as a reference for the virtual list, and returns how many pieces that was.
fn full_rows(state: &WorkspaceState) -> usize {
    let mut rows = Rows {
        visible: 0..usize::MAX,
        index: 0,
        height: 24.0 * step_scale(),
        skipped: Vec::new(),
        emitted: 0,
        walk: Rc::default(),
    };
    rows_of(
        state,
        &marks(state),
        &state.tree_picked(),
        &tree(&state.files, &state.tree_folders),
        0,
        &mut rows,
    );
    rows.emitted
}

/// Emits the review panel's padded scroller around the rows `content` emits, with its blank area below the files.
fn list(state: &WorkspaceState, content: impl FnOnce()) {
    rustle::layout(
        Modifier::new()
            .fill_max_size()
            .vertical_scroll(&state.tree_scroll)
            .fill_max_width()
            .padding_xy(0.0, theme::sp(1)),
        crate::review::FilledColumn {
            least: crate::theme::sp(crate::review::FILLER_LEAST),
        },
        || {
            content();
            rustle::spacer(Modifier::new().fill_max_width().control(
                crate::tree_actions::TREE_SPACE,
                reduce(crate::tree_actions::TREE_SPACE),
            ));
        },
    );
}

/// Lays out just the tree, drawn virtually when `virtualized` and in full otherwise, to inspect its scroll range, visible geometry and keyboard order; returns what was laid out and how many pieces the rows were.
fn paint(state: &WorkspaceState, virtualized: bool, viewport: f32) -> (Laid, usize) {
    let held = state.clone();
    let emitted = Rc::new(Cell::new(0));
    let counted = emitted.clone();
    let report = lay_out(380.0, viewport + 8.0 * step_scale(), None, move || {
        list(&held, || {
            counted.set(if virtualized {
                emit_rows(&held, viewport)
            } else {
                full_rows(&held)
            });
        });
    });
    (report, emitted.get())
}

/// The controls of `laid` that can be seen, by their place.
fn seen(laid: &Laid) -> Vec<(FocusId, rustle::Rect)> {
    laid.nodes
        .iter()
        .filter(|node| node.visible.size.width > 0.0 && node.visible.size.height > 0.0)
        .map(|node: &Tagged| (node.id, node.visible))
        .collect()
}

/// Every row of `state`'s tree in tree order, as the tree drawn in full lays them out.
fn every_row(state: &WorkspaceState) -> Vec<FocusId> {
    let (full, _) = paint(state, false, 240.0);
    full.ids()
        .into_iter()
        .filter(|id| *id != crate::tree_actions::TREE_SPACE)
        .collect()
}

/// Virtual rows draw exactly the visible geometry of the full tree at both scroll limits,
/// and only the rows near the viewport.
#[test]
fn virtual_tree_preserves_geometry() {
    for scale in [1.0, 0.8] {
        set_scales(scale, 1.0);
        let state = sample(200);
        for offset in [0.0, 113.0, 2400.0, 100_000.0] {
            state.tree_scroll.jump_to(offset);
            let (virtualized, emitted) = paint(&state, true, 240.0);
            let virtual_length = state.tree_scroll.content_length().round();
            assert!(
                emitted <= (240.0 / (24.0 * scale)).ceil() as usize + 7,
                "only viewport rows should be drawn: {emitted}, scale={scale}, offset={offset}, naming={:?}",
                state.git.naming
            );
            let (full, _) = paint(&state, false, 240.0);
            assert_eq!(
                seen(&virtualized),
                seen(&full),
                "offset={offset}, scale={scale}"
            );
            assert_eq!(virtual_length, state.tree_scroll.content_length().round());
        }
    }
    set_scales(1.0, 1.0);
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

/// A reproducible CPU comparison of constructing, laying out and painting a large expanded tree.
#[test]
#[ignore = "manual CPU benchmark: cargo test -p pm-components benchmark_large_tree -- --ignored --nocapture"]
fn benchmark_large_tree() {
    let state = sample(10_000);
    state.tree_scroll.jump_to(120_000.0);
    for virtualized in [false, true] {
        std::hint::black_box(paint(&state, virtualized, 600.0));
        let started = std::time::Instant::now();
        for _ in 0..20 {
            std::hint::black_box(paint(&state, virtualized, 600.0));
        }
        eprintln!(
            "tree build+layout+paint virtualized={virtualized}: {:.2} ms/frame",
            started.elapsed().as_secs_f64() * 50.0
        );
    }
}

/// Inline validation messages use the complete list so variable heights cannot leave gaps.
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
    assert_eq!(paint(&state, true, 240.0).1, 202);
}

/// Tab and Shift+Tab walk every row of a long tree in tree order, those not drawn included:
/// a row past the ones drawn is scrolled into view and takes focus, and the row holding
/// focus stays drawn while it is scrolled away.
#[test]
fn tab_walks_every_row_scrolling_hidden_ones_in() {
    use rustle::{Embedded, Key, KeyEvent, Modifiers};

    let state = sample(60);
    state.tree_scroll.jump_to(0.0);
    let rows = every_row(&state);
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
                list(&tree, || {
                    emit_rows(&tree, 240.0);
                });
            });
        },
        || {},
    );
    ui.resize((380, (240.0 + 8.0 * step_scale()) as u32), 1.0);
    let settle = |ui: &mut Embedded| {
        for _ in 0..3 {
            ui.invalidate();
            ui.update();
        }
    };
    settle(&mut ui);
    let mut walked = vec![held.get().expect("the first row holds focus")];
    for _ in 1..rows.len() {
        ui.key_event(KeyEvent::press(Key::Tab, Modifiers::NONE));
        settle(&mut ui);
        walked.push(held.get().expect("a row holds focus"));
    }
    assert_eq!(walked, rows);
    assert!(
        state.tree_scroll.offset() > 0.0,
        "the last rows were scrolled in"
    );
    state.tree_scroll.jump_to(0.0);
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
