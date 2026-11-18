//! Regression coverage for virtual tree geometry, focus order and variable-height naming.

use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::kit::report::Report;
use crate::kit::snapshot::lay_out;
use crate::kit::theme::set_scales;
use crate::kit::{Scroll, scroll_area};

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
    scroll_area(
        Modifier::new().fill_max_size(),
        Scroll::from_start(crate::state::TREE_SCROLL, state.tree_scroll),
        || {
            rustle::flex_column(
                Modifier::new().fill_max_width().pad_y(1),
                rustle::Flex::new(),
                || {
                    content();
                    rustle::spacer(
                        Modifier::new()
                            .grow()
                            .min_h(32.0)
                            .target(crate::tree_actions::TREE_SPACE),
                    );
                },
            );
        },
    );
}

/// Lays out just the tree, drawn virtually when `virtualized` and in full otherwise, to inspect its scroll range, visible geometry and keyboard order; returns what was laid out and how many pieces the rows were.
fn paint(state: &WorkspaceState, virtualized: bool, viewport: f32) -> (Report, usize) {
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

/// The keyboard identities of what `report` laid out, the virtual ones included, in a fixed order.
fn identities(report: &Report) -> Vec<FocusId> {
    let mut ids = report.focus_order();
    ids.sort_by_key(|id| id.0);
    ids
}

/// The regions of `report` that can be seen, by their place.
fn seen(report: &Report) -> Vec<crate::kit::report::Region> {
    report
        .regions
        .iter()
        .filter(|region| region.rect.size.width > 0.0 && region.rect.size.height > 0.0)
        .copied()
        .collect()
}

/// Virtual rows preserve every keyboard identity and exactly the visible geometry at both scroll limits.
#[test]
fn virtual_tree_preserves_geometry_and_focus_order() {
    for scale in [1.0, 0.8] {
        set_scales(scale, 1.0);
        let mut state = sample(200);
        for offset in [0.0, 113.0, 2400.0, 100_000.0] {
            state.tree_scroll = offset;
            let (virtualized, emitted) = paint(&state, true, 240.0);
            assert!(
                emitted <= (240.0 / (24.0 * scale)).ceil() as usize + 7,
                "only viewport rows should be drawn: {emitted}, scale={scale}, offset={offset}, naming={:?}",
                state.git.naming
            );
            let (full, _) = paint(&state, false, 240.0);
            assert_eq!(identities(&virtualized), identities(&full));
            assert_eq!(
                seen(&virtualized),
                seen(&full),
                "offset={offset}, scale={scale}"
            );
            assert_eq!(virtualized.scrollers, full.scrollers);
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
    let mut state = sample(10_000);
    state.tree_scroll = 120_000.0;
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
        pm_field::Field::new("file-00000.rs"),
    ));
    assert!(state.naming_problem().is_some());
    assert_eq!(paint(&state, true, 240.0).1, 202);
}
