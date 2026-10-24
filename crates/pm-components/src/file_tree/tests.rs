//! Regression coverage for virtual tree geometry, focus order and variable-height naming.

use super::*;
use pm_renderer::DrawList;
use pm_text::Shaper;
use pm_theme::default_family;
use pm_ui::{Input as FrameInput, Output, draw};

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

/// Builds every row as a reference for the virtual list.
fn full_rows(state: &WorkspaceState) -> Vec<Node> {
    let mut rows = Rows {
        visible: 0..usize::MAX,
        index: 0,
        height: 24.0 * pm_style::step_scale(),
        skipped: Vec::new(),
        nodes: Vec::new(),
    };
    rows_of(
        state,
        &marks(state),
        &state.tree_picked(),
        &tree(&state.files, &state.tree_folders),
        0,
        &mut rows,
    );
    rows.nodes
}

/// Reproduces the review panel's padded scroller and its blank area below the files.
fn list(state: &WorkspaceState, rows: Vec<Node>) -> Node {
    node()
        .styles([flex(), flex_col(), overflow_hidden(), pm_style::py(1)])
        .scroll_from_start(crate::state::TREE_SCROLL, state.tree_scroll)
        .children(rows)
        .child(
            node()
                .styles([flex_1(), pm_style::min_h(32.0)])
                .focusable(crate::tree_actions::TREE_SPACE),
        )
}

/// Draws just the tree to inspect its scroll range, visible geometry and keyboard order.
fn paint(state: &WorkspaceState, nodes: Vec<Node>, viewport: f32, shaper: &mut Shaper) -> Output {
    draw(
        &list(state, nodes),
        380.0,
        viewport + 8.0 * pm_style::step_scale(),
        &FrameInput::default(),
        &default_family().dark,
        shaper,
        &mut DrawList::new(),
    )
}

/// Virtual rows preserve every keyboard identity and exactly the visible geometry at both scroll limits.
#[test]
fn virtual_tree_preserves_geometry_and_focus_order() {
    let mut shaper = Shaper::new();
    for scale in [1.0, 0.8] {
        pm_style::set_scales(scale, 1.0);
        let mut state = sample(200);
        for offset in [0.0, 113.0, 2400.0, 100_000.0] {
            state.tree_scroll = offset;
            let nodes = rows(&state, 240.0);
            assert!(
                nodes.len() <= (240.0 / (24.0 * scale)).ceil() as usize + 7,
                "only viewport rows should have nodes: {}, scale={scale}, offset={offset}, naming={:?}",
                nodes.len(),
                state.git.naming
            );
            let virtualized = paint(&state, nodes, 240.0, &mut shaper);
            let full = paint(&state, full_rows(&state), 240.0, &mut shaper);
            assert_eq!(
                virtualized
                    .regions
                    .iter()
                    .map(|region| region.id)
                    .collect::<Vec<_>>(),
                full.regions
                    .iter()
                    .map(|region| region.id)
                    .collect::<Vec<_>>()
            );
            let seen = |output: &Output| {
                output
                    .regions
                    .iter()
                    .filter(|region| region.rect.width > 0.0 && region.rect.height > 0.0)
                    .copied()
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                seen(&virtualized),
                seen(&full),
                "offset={offset}, scale={scale}"
            );
            assert_eq!(virtualized.scrollers, full.scrollers);
        }
    }
    pm_style::set_scales(1.0, 1.0);
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
    let mut shaper = Shaper::new();
    for virtualized in [false, true] {
        let warm = if virtualized {
            rows(&state, 600.0)
        } else {
            full_rows(&state)
        };
        std::hint::black_box(paint(&state, warm, 600.0, &mut shaper));
        let started = std::time::Instant::now();
        for _ in 0..20 {
            let nodes = if virtualized {
                rows(&state, 600.0)
            } else {
                full_rows(&state)
            };
            std::hint::black_box(paint(&state, nodes, 600.0, &mut shaper));
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
        pm_text::Field::new("file-00000.rs"),
    ));
    assert!(state.naming_problem().is_some());
    assert_eq!(rows(&state, 240.0).len(), 202);
}
