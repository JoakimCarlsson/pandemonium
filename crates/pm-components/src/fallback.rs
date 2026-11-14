//! The small notice in a conversation that its turn moved to a backup model, because the model of a slot kept failing.

use pm_icon::IconName;
use pm_style::{Role, flex, flex_none, gap, items_center, min_w_px, px, py};
use pm_ui::{Node, node};

use crate::parts::{clipped, glyph};

/// What became of the model that failed, in a few words, as `reason`, the provider's first line, says: overloaded, rate limited, not to be had, or failing.
pub fn cause(reason: &str) -> &'static str {
    let reason = reason.to_lowercase();
    if reason.contains("overload") || reason.contains("529") {
        "was overloaded"
    } else if reason.contains("rate limit") || reason.contains("429") {
        "was rate limited"
    } else if reason.contains("not found")
        || reason.contains("unavailable")
        || reason.contains("404")
        || reason.contains("503")
    {
        "could not be reached"
    } else {
        "kept failing"
    }
}

/// What the notice says: the backup `to` asked now, and what became of `from`, as `reason` says.
pub fn fallback_words(from: &str, to: &str, reason: &str) -> String {
    format!("Switched to {to}: {from} {}", cause(reason))
}

/// The notice that the rest of the turn asks `to` because `from` failed for `reason`: one muted line with a bolt before it.
pub fn fallback_notice(from: &str, to: &str, reason: &str) -> Node {
    node()
        .styles([
            flex(),
            items_center(),
            gap(1.5),
            px(1),
            py(0.5),
            min_w_px(0),
        ])
        .child(glyph(IconName::Zap, 3.5, Role::Warning).styles([flex_none()]))
        .child(clipped(fallback_words(from, to, reason), 12, Role::Muted))
}

#[cfg(test)]
/// Tests for the words of the notice.
mod tests {
    use super::*;

    /// The notice names the backup and says what became of the model that failed.
    #[test]
    fn the_notice_says_what_happened() {
        assert_eq!(
            fallback_words(
                "claude-opus-4-5",
                "openai/gpt-5",
                "provider returned 529: overloaded"
            ),
            "Switched to openai/gpt-5: claude-opus-4-5 was overloaded"
        );
        assert_eq!(cause("HTTP 429 Too Many Requests"), "was rate limited");
        assert_eq!(cause("model not found"), "could not be reached");
        assert_eq!(cause("internal error"), "kept failing");
    }
}
