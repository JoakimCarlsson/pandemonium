//! The card in a session's conversation that asks the reader to review and trust the hooks its Codex holds back.

use pm_acp::Hook;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, border_t, flex, flex_1, flex_col, flex_wrap, font_mono,
    gap, items_center, items_start, leading_ratio, min_w_px, pt, px, py, rounded, text_color,
    text_px,
};
use pm_ui::{Node, node, text};

use crate::Button;
use crate::button::Variant;
use crate::hooks_prompt::{
    HOOKS_CONTINUE, HOOKS_REVIEW, HOOKS_TRUST_ALL, HOOKS_TRUST_BASE, HOOKS_TRUST_ROWS, HooksPrompt,
};
use crate::parts::{line, wrap};
use crate::sample::Session;
use crate::size::Size;

/// The card about the hooks `session`'s Codex holds back, `width` logical pixels across, when it shows.
pub fn hooks_card(session: &Session, width: f32) -> Option<Node> {
    if session.held_hooks.is_empty() || session.hooks_prompt == HooksPrompt::Hidden {
        return None;
    }
    let hooks = &session.held_hooks;
    let count = match hooks.len() {
        1 => "1 hook is new or changed.".to_owned(),
        count => format!("{count} hooks are new or changed."),
    };
    let mut card = node()
        .styles([
            flex(),
            flex_col(),
            gap(2.5),
            px(4),
            py(3),
            rounded(Radius::Xl),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
        ])
        .child(line("Hooks need review", 12, 1.4, Role::Muted))
        .child(line(count, 13, 1.4, Role::Text))
        .child(line(
            "Hooks can run outside the sandbox after you trust them.",
            12,
            1.4,
            Role::Muted,
        ));
    if session.hooks_prompt == HooksPrompt::Reviewing {
        card = card.children(
            hooks
                .iter()
                .take(HOOKS_TRUST_ROWS)
                .enumerate()
                .map(|(index, hook)| hook_row(hook, index, width - 160.0)),
        );
    }
    let buttons = [
        ("Continue without trusting", HOOKS_CONTINUE, Variant::Ghost),
        ("Review hooks", HOOKS_REVIEW, Variant::Outline),
        ("Trust all", HOOKS_TRUST_ALL, Variant::Default),
    ]
    .into_iter()
    .filter(|(_, id, _)| *id != HOOKS_REVIEW || session.hooks_prompt == HooksPrompt::Asking);
    Some(
        card.child(
            node()
                .styles([
                    flex(),
                    flex_wrap(),
                    items_center(),
                    pm_style::justify_end(),
                    gap(2),
                ])
                .children(buttons.map(|(label, id, variant)| {
                    Button::new(label, id)
                        .variant(variant)
                        .size(Size::Sm)
                        .build()
                })),
        ),
    )
}

/// One hook of the review, `index` among them, its command wrapped to `width` logical pixels: the event and matcher it runs on, the command, the file it comes from and a button that trusts it.
fn hook_row(hook: &Hook, index: usize, width: f32) -> Node {
    let state = if hook.changed { "changed" } else { "new" };
    let on = match &hook.matcher {
        Some(matcher) if !matcher.is_empty() => format!("{} · {matcher} · {state}", hook.event),
        _ => format!("{} · {state}", hook.event),
    };
    node()
        .styles([
            flex(),
            items_start(),
            gap(3),
            pt(2.5),
            border_t(),
            border_color(Role::Border),
        ])
        .child(
            node()
                .styles([flex(), flex_col(), flex_1(), min_w_px(0), gap(1)])
                .child(line(on, 12, 1.4, Role::Text))
                .children(wrap(&hook.command, 12, width).into_iter().map(|piece| {
                    text(piece).styles([
                        text_px(12),
                        leading_ratio(1.5),
                        font_mono(),
                        text_color(Role::Text),
                    ])
                }))
                .child(line(&hook.source, 11, 1.4, Role::Muted)),
        )
        .child(
            Button::new("Trust", FocusId(HOOKS_TRUST_BASE + index as u64))
                .variant(Variant::Outline)
                .size(Size::Sm)
                .build(),
        )
}
