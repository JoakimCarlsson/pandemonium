//! Display kind, overflow and visibility.

use pm_layout::{Display, DisplayRule, Overflow, Visibility};
use pm_primitives::Axis;

constants! {
    /// A block box.
    block = DisplayRule::Display(Display::Block);
    /// An inline box.
    inline = DisplayRule::Display(Display::Inline);
    /// An inline box laid out inside like a block.
    inline_block = DisplayRule::Display(Display::InlineBlock);
    /// A flex container.
    flex = DisplayRule::Display(Display::Flex);
    /// An inline flex container.
    inline_flex = DisplayRule::Display(Display::InlineFlex);
    /// A grid container.
    grid = DisplayRule::Display(Display::Grid);
    /// An inline grid container.
    inline_grid = DisplayRule::Display(Display::InlineGrid);
    /// No box; the children join the parent's layout.
    contents = DisplayRule::Display(Display::Contents);
    /// Not laid out and not drawn.
    hidden = DisplayRule::Display(Display::None);
    /// Overflow drawn outside the box.
    overflow_visible = DisplayRule::Overflow(Overflow::Visible);
    /// Overflow clipped.
    overflow_hidden = DisplayRule::Overflow(Overflow::Hidden);
    /// Overflow clipped exactly at the box.
    overflow_clip = DisplayRule::Overflow(Overflow::Clip);
    /// Overflow clipped, scrollbars always shown.
    overflow_scroll = DisplayRule::Overflow(Overflow::Scroll);
    /// Overflow clipped, scrollbars when needed.
    overflow_auto = DisplayRule::Overflow(Overflow::Auto);
    /// Horizontal overflow clipped.
    overflow_x_hidden = DisplayRule::OverflowAxis(Axis::Horizontal, Overflow::Hidden);
    /// Horizontal overflow scrollable when needed.
    overflow_x_auto = DisplayRule::OverflowAxis(Axis::Horizontal, Overflow::Auto);
    /// Horizontal overflow always scrollable.
    overflow_x_scroll = DisplayRule::OverflowAxis(Axis::Horizontal, Overflow::Scroll);
    /// Horizontal overflow visible.
    overflow_x_visible = DisplayRule::OverflowAxis(Axis::Horizontal, Overflow::Visible);
    /// Vertical overflow clipped.
    overflow_y_hidden = DisplayRule::OverflowAxis(Axis::Vertical, Overflow::Hidden);
    /// Vertical overflow scrollable when needed.
    overflow_y_auto = DisplayRule::OverflowAxis(Axis::Vertical, Overflow::Auto);
    /// Vertical overflow always scrollable.
    overflow_y_scroll = DisplayRule::OverflowAxis(Axis::Vertical, Overflow::Scroll);
    /// Vertical overflow visible.
    overflow_y_visible = DisplayRule::OverflowAxis(Axis::Vertical, Overflow::Visible);
    /// Drawn.
    visible = DisplayRule::Visibility(Visibility::Visible);
    /// Takes space but is not drawn.
    invisible = DisplayRule::Visibility(Visibility::Hidden);
    /// Hidden within a collapsing container.
    collapse = DisplayRule::Visibility(Visibility::Collapse);
}
