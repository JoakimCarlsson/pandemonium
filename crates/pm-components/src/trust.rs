//! The card above the prompt box that asks the reader to trust the project a session works in, listing what the agent goes without until they do.

use std::path::Path;

use pm_harness::{Input, Skipped, Standing, TrustRequest, Verdict};
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_col, flex_none, flex_wrap, font_mono, gap,
    items_center, justify_end, leading_ratio, min_w_px, px, py, rounded, text_color, text_px,
};
use pm_text::Shaper;
use pm_ui::{Node, node, text};

use crate::Button;
use crate::button::Variant;
use crate::parts::{glyph, line, paragraph};
use crate::size::Size;
use crate::state::{Outgoing, Page, WorkspaceState};

/// Trusts the project for good, for every session of its repository.
pub const TRUST_ALWAYS: FocusId = FocusId(7120);

/// Trusts the project for the conversation of this session only.
pub const TRUST_SESSION: FocusId = FocusId(7121);

/// Records that the project is not trusted.
pub const TRUST_DECLINE: FocusId = FocusId(7122);

/// What the card's padding and border take from its width, in logical pixels.
const CARD_INSET: f32 = 34.0;

/// What the padding of the list of skipped inputs takes from the card's inner width, in logical pixels.
const LIST_INSET: f32 = 24.0;

/// How many inputs of a kind other than hooks are listed before the rest are only counted; every hook is listed, since each runs a command.
const LISTED: usize = 3;

/// The kinds of input in the order the card lists them: hooks first, since they run commands.
const KINDS: [Input; 4] = [Input::Hook, Input::Instruction, Input::Skill, Input::Agent];

/// What a group of `count` inputs of `kind` is called on the card.
fn heading(kind: Input, count: usize) -> String {
    let (one, many) = match kind {
        Input::Hook => ("hook", "hooks"),
        Input::Instruction => ("instruction file", "instruction files"),
        Input::Skill => ("skill", "skills"),
        Input::Agent => ("agent", "agents"),
    };
    format!("{count} {}", if count == 1 { one } else { many })
}

/// What inputs of `kind` are called in a sentence about what the agent goes without.
fn plural(kind: Input) -> &'static str {
    match kind {
        Input::Hook => "hooks",
        Input::Instruction => "instructions",
        Input::Skill => "skills",
        Input::Agent => "agents",
    }
}

/// The words of `items` run together as a list: `a`, `a and b`, `a, b and c`.
fn listed(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The name the card gives the project rooted at `root`: the name of its folder, or the whole path when it has none.
fn project_name(root: &Path) -> String {
    root.file_name().map_or_else(
        || root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The file `path` as seen from the project's `root`, or whole when it lies outside it.
fn from_root(path: &Path, root: &Path) -> String {
    path.strip_prefix(root).map_or_else(
        |_| path.display().to_string(),
        |inner| inner.display().to_string(),
    )
}

/// The heading and the sentence under it that say where the trust of `request`'s project stands and what the agent goes without.
fn words(request: &TrustRequest) -> (String, String) {
    let name = project_name(&request.root);
    let kinds: Vec<&str> = KINDS
        .into_iter()
        .filter(|kind| request.skipped.iter().any(|skipped| skipped.kind == *kind))
        .map(plural)
        .collect();
    let missing = listed(&kinds);
    match request.standing {
        Standing::Changed => (
            format!("The hooks of {name} changed since you trusted it"),
            "Hooks were added or changed since trust was granted, so they are not run until you trust the project again. Its instructions, skills and agents are still used.".to_owned(),
        ),
        Standing::Declined => (
            format!("You chose not to trust {name}"),
            format!("This session is not using its {missing}. Trust the project to use them from the next turn."),
        ),
        Standing::Unknown => (
            format!("Trust {name}?"),
            if request.skipped.iter().any(|skipped| skipped.kind == Input::Hook) {
                format!("This session is not using its {missing} until you trust the project. Its hooks run commands on this machine, so only trust a project whose files you know.")
            } else {
                format!("This session is not using its {missing} until you trust the project.")
            },
        ),
    }
}

/// One skipped input of the project rooted at `root`, `width` logical pixels wide: a hook as its event, matcher and command in code type with the file it comes from below, anything else on one line as its name with the file beside it; the file is left out when `filed` says its group shows it already, or when it is the name itself.
fn item(skipped: &Skipped, root: &Path, filed: bool, width: f32, shaper: &mut Shaper) -> Node {
    let file = from_root(&skipped.path, root);
    let named = match skipped.kind {
        Input::Instruction => from_root(Path::new(&skipped.name), root),
        _ => skipped.name.clone(),
    };
    let beside = (!filed && file != named)
        .then(|| line(file.clone(), 11, 1.5, Role::Subtle).styles([font_mono()]));
    if skipped.kind != Input::Hook {
        return node()
            .styles([flex(), items_center(), gap(2), min_w_px(0)])
            .child(line(named, 12, 1.5, Role::Text).styles([flex_none()]))
            .children(beside);
    }
    let mono = [
        text_px(12),
        leading_ratio(1.5),
        font_mono(),
        text_color(Role::Text),
    ];
    node()
        .styles([flex(), flex_col(), min_w_px(0)])
        .children(
            pm_ui::wrap(shaper, &named, mono, width)
                .into_iter()
                .map(|piece| text(&named[piece.start..piece.end]).styles(mono)),
        )
        .children(beside)
}

/// The group of `request`'s skipped inputs of `kind`, `width` logical pixels wide, or nothing when it skipped none; hooks are marked in the warning colour, since they run commands. When more than one input of the group comes from the same file, as hooks usually do, the file is named once beside the group's heading.
fn group(request: &TrustRequest, kind: Input, width: f32, shaper: &mut Shaper) -> Option<Node> {
    let of_kind: Vec<&Skipped> = request
        .skipped
        .iter()
        .filter(|skipped| skipped.kind == kind)
        .collect();
    if of_kind.is_empty() {
        return None;
    }
    let shown = if kind == Input::Hook {
        of_kind.len()
    } else {
        of_kind.len().min(LISTED)
    };
    let title = heading(kind, of_kind.len());
    let filed = of_kind.len() > 1
        && of_kind
            .iter()
            .all(|skipped| skipped.path == of_kind[0].path);
    let file = filed.then(|| {
        line(
            from_root(&of_kind[0].path, &request.root),
            11,
            1.4,
            Role::Subtle,
        )
        .styles([font_mono()])
    });
    let header = if kind == Input::Hook {
        node()
            .styles([flex(), items_center(), gap(1.5), min_w_px(0)])
            .child(glyph(IconName::Terminal, 3.5, Role::Warning))
            .child(
                line(format!("{title} · run commands"), 12, 1.4, Role::Warning)
                    .styles([flex_none()]),
            )
            .children(file)
    } else {
        node()
            .styles([flex(), items_center(), gap(1.5), min_w_px(0)])
            .child(line(title, 12, 1.4, Role::Muted).styles([flex_none()]))
            .children(file)
    };
    let mut held = node()
        .styles([flex(), flex_col(), gap(0.5), min_w_px(0)])
        .child(header)
        .children(
            of_kind[..shown]
                .iter()
                .map(|skipped| item(skipped, &request.root, filed, width, shaper))
                .collect::<Vec<_>>(),
        );
    if of_kind.len() > shown {
        held = held.child(line(
            format!("and {} more", of_kind.len() - shown),
            12,
            1.4,
            Role::Subtle,
        ));
    }
    Some(held)
}

/// The buttons that answer the request, the refusal first and the boldest last, wrapping when they do not fit one line.
fn choices(width: f32, shaper: &mut Shaper) -> Node {
    node()
        .styles([flex(), flex_wrap(), items_center(), justify_end(), gap(2)])
        .children(
            [
                ("Don't trust", TRUST_DECLINE, Variant::Ghost),
                ("This session only", TRUST_SESSION, Variant::Outline),
                ("Trust", TRUST_ALWAYS, Variant::Default),
            ]
            .map(|(label, id, variant)| {
                Button::new(label, id)
                    .variant(variant)
                    .size(Size::Sm)
                    .wrapped(width, shaper)
                    .build()
            }),
        )
}

/// The card that asks the reader to trust the project of `request`, `width` logical pixels wide: which repository, what the agent goes without until it is trusted, grouped by kind, and a button for each answer.
pub fn card(request: &TrustRequest, width: f32, shaper: &mut Shaper) -> Node {
    let inner = width - CARD_INSET;
    let (title, said) = words(request);
    let title_styles = [text_px(13), leading_ratio(1.4), text_color(Role::Text)];
    let groups: Vec<Node> = KINDS
        .into_iter()
        .filter_map(|kind| group(request, kind, inner - LIST_INSET, shaper))
        .collect();
    node()
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
        .child(
            node()
                .styles([flex(), items_center(), gap(1.5), min_w_px(0)])
                .child(glyph(IconName::Lock, 3.5, Role::Muted))
                .child(line("Project trust", 12, 1.4, Role::Muted).styles([flex_none()]))
                .child(
                    line(request.root.display().to_string(), 11, 1.4, Role::Subtle)
                        .styles([font_mono()]),
                ),
        )
        .child(
            node().styles([flex(), flex_col()]).children(
                pm_ui::wrap(shaper, &title, title_styles, inner)
                    .into_iter()
                    .map(|piece| text(&title[piece.start..piece.end]).styles(title_styles)),
            ),
        )
        .child(paragraph(&said, 12, 1.5, Role::Muted, inner, shaper))
        .child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    gap(2.5),
                    px(3),
                    py(2.5),
                    rounded(Radius::Lg),
                    bg(Role::Background),
                    min_w_px(0),
                ])
                .children(groups),
        )
        .child(choices(inner, shaper))
}

impl WorkspaceState {
    /// Handles a press on the trust card's buttons, answering the open session's request and taking it off the card at once; false when `action` is not one of them.
    pub(crate) fn trust_pressed(&mut self, action: FocusId) -> bool {
        let verdict = match action {
            TRUST_ALWAYS => Verdict::Always,
            TRUST_SESSION => Verdict::Allow,
            TRUST_DECLINE => Verdict::Reject,
            _ => return false,
        };
        let Page::Session(open) = self.page else {
            return true;
        };
        if let Some(request) = self
            .sessions
            .get_mut(open)
            .and_then(|session| session.trust.take())
        {
            self.outbox.push(Outgoing::Trust {
                session: open,
                ticket: request.ticket,
                verdict,
            });
        }
        true
    }
}

#[cfg(test)]
mod tests;
