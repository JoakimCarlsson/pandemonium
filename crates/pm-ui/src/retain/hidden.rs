//! Compact selection and focus metadata for retained subtrees outside the viewport.

use std::sync::Arc;

use pm_input::FocusId;
use pm_renderer::Rect;
use pm_style::layout::{Display, Placed};

use crate::Node;

/// One offscreen metadata entry, replayed without walking styled nodes.
#[derive(Clone, Debug)]
pub(crate) enum Hidden {
    /// A keyboard target with a zero-area rectangle.
    Focus(FocusId, Rect),
    /// A selectable group with a zero-area rectangle.
    Selectable(FocusId, Rect),
    /// Selectable text with its unscrolled geometry and optional owner override.
    Text(Option<FocusId>, Arc<str>, Rect),
}

/// Captures metadata for static text; editable fields keep the ordinary navigation traversal.
pub(super) fn collect(root: &Node, placed: &Placed) -> Option<Vec<Hidden>> {
    let mut hidden = Vec::new();
    append(root, placed, (0.0, 0.0), None, &mut hidden)?;
    Some(hidden)
}

/// Records nodes in the same tree order and coordinate space as offscreen painting.
fn append(
    node: &Node,
    laid: &Placed,
    origin: (f32, f32),
    owner: Option<FocusId>,
    hidden: &mut Vec<Hidden>,
) -> Option<()> {
    if node.style.base().layout.display.display == Display::None {
        return Some(());
    }
    if node.field_line.is_some() || node.caret.is_some() || node.field_layout.is_some() {
        return None;
    }
    let rect = Rect::new(laid.x + origin.0, laid.y + origin.1, 0.0, 0.0);
    hidden.extend(node.virtual_focus.iter().map(|id| Hidden::Focus(*id, rect)));
    let owner = node.selectable.or(owner);
    if let Some(id) = node.selectable {
        hidden.push(Hidden::Selectable(id, rect));
    }
    if let Some(text) = &node.text {
        hidden.push(Hidden::Text(
            owner,
            text.clone(),
            Rect::new(rect.x, rect.y, laid.width, laid.height),
        ));
    }
    if let Some(id) = node.focus {
        hidden.push(Hidden::Focus(id, rect));
    }
    for (child, placed) in node.children.iter().zip(&laid.children) {
        append(child, placed, origin, owner, hidden)?;
    }
    if let Some(retained) = &node.retained {
        append(
            &retained.root,
            &retained.placed,
            (rect.x, rect.y),
            owner,
            hidden,
        )?;
    }
    Some(())
}

/// Selection correctness is compared to a full traversal of the same nested subtree.
#[cfg(test)]
mod tests {
    use pm_renderer::DrawList;
    use pm_style::{flex, flex_col, flex_none, h_px, overflow_hidden};
    use pm_text::Shaper;

    use super::*;
    use crate::{Input, Output, draw, node, retained, text};

    /// Draws a retained subtree that lies entirely below a clipped viewport.
    fn paint(content: Node, shaper: &mut Shaper) -> Output {
        let root = node()
            .styles([flex(), flex_col(), overflow_hidden()])
            .selectable(FocusId(7))
            .child(node().styles([h_px(200), flex_none()]))
            .child(content);
        draw(
            &root,
            200.0,
            100.0,
            &Input::default(),
            &pm_theme::default_family().dark,
            shaper,
            &mut DrawList::new(),
        )
    }

    /// Flattened offscreen metadata preserves nested owners, byte contents, focus order and copying.
    #[test]
    fn hidden_metadata_preserves_copy_and_focus() {
        let mut shaper = Shaper::new();
        let content = node()
            .styles([flex(), flex_col(), flex_none()])
            .child(text("first café 🦀").focusable(FocusId(1)))
            .child(node().selectable(FocusId(8)).child(text("nested owner")))
            .child(text("last row").focusable(FocusId(2)));
        let cached = retained(0x9867, 200.0, &mut shaper, |_| content.clone());
        let ordinary = paint(content, &mut shaper);
        let summarized = paint(cached, &mut shaper);
        assert_eq!(summarized.regions, ordinary.regions);
        assert_eq!(summarized.selectables, ordinary.selectables);
        let pieces = |output: &Output| {
            output
                .pieces
                .iter()
                .map(|piece| (piece.id, piece.index, piece.content.clone(), piece.laid))
                .collect::<Vec<_>>()
        };
        assert_eq!(pieces(&summarized), pieces(&ordinary));
        for id in [FocusId(7), FocusId(8)] {
            let selection = ordinary.select_all(id).unwrap();
            assert_eq!(
                summarized.selected_text(&selection),
                ordinary.selected_text(&selection)
            );
        }
    }
}
