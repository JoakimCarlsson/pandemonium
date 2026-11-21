//! The card above the prompt box that asks the reader to trust the project a session works in, listing what the agent goes without until they do.

use crate::areas::ChatEvent;
use crate::button;
use crate::target::Target;
use crate::theme;
use crate::theme::Frame;
use rustle::phosphor;
use rustle::{Alignment, Arrangement, TextOverflow};

use std::path::Path;

use crate::button::Variant;
use crate::parts::{glyph, line, paragraph};
use crate::state::{Outgoing, Page, WorkspaceState};
use crate::theme::{Radius, Role};
use pm_harness::{Input, Skipped, Standing, TrustRequest, Verdict};
use rustle::Modifier;

/// What the card's padding and border take from its width, in logical pixels.
const CARD_INSET: f32 = 34.0;

/// What the padding of the list of skipped inputs takes from the card's inner width, in logical pixels.
const LIST_INSET: f32 = 24.0;

/// How many inputs of a kind other than hooks are listed before the rest are only counted; every hook is listed, since each runs a command.
const LISTED: usize = 3;

/// The kinds of input in the order the card lists them: hooks first, since they run commands.
const KINDS: [Input; 4] = [Input::Hook, Input::Instruction, Input::Skill, Input::Agent];

/// What one input of `kind` is called on the card, what many are, and what they are called in a sentence about what the agent goes without.
fn names(kind: Input) -> (&'static str, &'static str, &'static str) {
    match kind {
        Input::Hook => ("hook", "hooks", "hooks"),
        Input::Instruction => ("instruction file", "instruction files", "instructions"),
        Input::Skill => ("skill", "skills", "skills"),
        Input::Agent => ("agent", "agents", "agents"),
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
        .map(|kind| names(kind).2)
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

/// Emits one skipped input of the project rooted at `root`, `width` logical pixels wide: a hook as its event, matcher and command in code type with the file it comes from below, anything else on one line as its name with the file beside it; the file is left out when `filed` says its group shows it already, or when it is the name itself.
#[track_caller]
fn item(skipped: &Skipped, root: &Path, filed: bool, width: f32) {
    let file = from_root(&skipped.path, root);
    let named = match skipped.kind {
        Input::Instruction => from_root(Path::new(&skipped.name), root),
        _ => skipped.name.clone(),
    };
    let beside = (!filed && file != named).then_some(file);
    let file_type = theme::mono_style(11, 1.5)
        .color(theme::color(Role::Subtle))
        .max_lines(1);
    if skipped.kind != Input::Hook {
        rustle::row_arranged(
            Modifier::new().fill_max_width(),
            Arrangement::SpacedBy(theme::sp(2)),
            Alignment::Center,
            move || {
                line(named, 12, 1.5, Role::Text);
                if let Some(file) = beside {
                    rustle::basic_text(
                        Modifier::new().weight(1.0),
                        file,
                        file_type,
                        TextOverflow::Ellipsis,
                    );
                }
            },
        );
        return;
    }
    let mono = theme::mono_style(12, 1.5).color(theme::color(Role::Text));
    rustle::column_arranged(
        Modifier::new(),
        Arrangement::Start,
        Alignment::Start,
        move || {
            for piece in rustle::wrap_at_spaces(&named, &mono, width) {
                rustle::basic_text(
                    Modifier::new(),
                    &named[piece.start..piece.end],
                    mono,
                    TextOverflow::Ellipsis,
                );
            }
            if let Some(file) = beside {
                rustle::basic_text(Modifier::new(), file, file_type, TextOverflow::Ellipsis);
            }
        },
    );
}

/// Emits the group of `request`'s skipped inputs of `kind`, `width` logical pixels wide, or nothing when it skipped none; hooks are marked in the warning colour, since they run commands. When more than one input of the group comes from the same file, as hooks usually do, the file is named once beside the group's heading.
#[track_caller]
fn group(request: &TrustRequest, kind: Input, width: f32) {
    let of_kind: Vec<&Skipped> = request
        .skipped
        .iter()
        .filter(|skipped| skipped.kind == kind)
        .collect();
    if of_kind.is_empty() {
        return;
    }
    let shown = if kind == Input::Hook {
        of_kind.len()
    } else {
        of_kind.len().min(LISTED)
    };
    let (one, many, _) = names(kind);
    let title = format!(
        "{} {}",
        of_kind.len(),
        if of_kind.len() == 1 { one } else { many }
    );
    let filed = of_kind.len() > 1
        && of_kind
            .iter()
            .all(|skipped| skipped.path == of_kind[0].path);
    let file = filed.then(|| from_root(&of_kind[0].path, &request.root));
    rustle::column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(0.5)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(1.5)),
                Alignment::Center,
                || {
                    if kind == Input::Hook {
                        glyph(phosphor::TERMINAL, 3.5, Role::Warning);
                        line(format!("{title} · run commands"), 12, 1.4, Role::Warning);
                    } else {
                        line(title, 12, 1.4, Role::Muted);
                    }
                    if let Some(file) = file {
                        rustle::basic_text(
                            Modifier::new().weight(1.0),
                            file,
                            theme::code(11, Role::Subtle),
                            TextOverflow::Ellipsis,
                        );
                    }
                },
            );
            for skipped in &of_kind[..shown] {
                item(skipped, &request.root, filed, width);
            }
            if of_kind.len() > shown {
                let more = format!("and {} more", of_kind.len() - shown);
                line(more, 12, 1.4, Role::Subtle);
            }
        },
    );
}

/// Emits the buttons that answer the request, the refusal first and the boldest last, at the end of the line and wrapping when they do not fit one line.
#[track_caller]
fn choices(width: f32) {
    rustle::flow_row_with(
        Modifier::new().fill_max_width(),
        rustle::FlowOptions::new(theme::sp(2), theme::sp(2))
            .arrangement(Arrangement::End)
            .item_alignment(Alignment::Center),
        || {
            for (label, id, variant) in [
                ("Don't trust", ChatEvent::TrustDecline, Variant::Ghost),
                (
                    "This session only",
                    ChatEvent::TrustSession,
                    Variant::Outline,
                ),
                ("Trust", ChatEvent::TrustAlways, Variant::Default),
            ] {
                button::small_button(label, Target::Chat(id), variant)
                    .wrapped(width)
                    .build();
            }
        },
    );
}

/// Emits the card that asks the reader to trust the project of `request`, `width` logical pixels wide: which repository, what the agent goes without until it is trusted, grouped by kind, and a button for each answer.
#[track_caller]
pub fn card(request: &TrustRequest, width: f32) {
    let inner = width - CARD_INSET;
    let (title, said) = words(request);
    let own = Modifier::new()
        .panel(Radius::Xl)
        .padding_xy(theme::sp(4), theme::sp(3));
    rustle::column_arranged(
        own,
        Arrangement::SpacedBy(theme::sp(2.5)),
        Alignment::Start,
        || {
            rustle::row_arranged(
                Modifier::new().fill_max_width(),
                Arrangement::SpacedBy(theme::sp(1.5)),
                Alignment::Center,
                || {
                    glyph(phosphor::LOCK_SIMPLE, 3.5, Role::Muted);
                    rustle::basic_text(
                        Modifier::new(),
                        "Project trust",
                        theme::line(12, Role::Muted),
                        TextOverflow::Ellipsis,
                    );
                    rustle::basic_text(
                        Modifier::new().weight(1.0),
                        request.root.display().to_string(),
                        theme::code(11, Role::Subtle),
                        TextOverflow::Ellipsis,
                    );
                },
            );
            paragraph(&title, 13, 1.4, Role::Text, inner);
            paragraph(&said, 12, 1.5, Role::Muted, inner);
            rustle::column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .background_rounded(theme::color(Role::Background), Radius::Lg.px())
                    .padding_xy(theme::sp(3), theme::sp(2.5)),
                Arrangement::SpacedBy(theme::sp(2.5)),
                Alignment::Start,
                || {
                    for kind in KINDS {
                        group(request, kind, inner - LIST_INSET);
                    }
                },
            );
            choices(inner);
        },
    );
}

impl WorkspaceState {
    /// Handles a press on the trust card's buttons, answering the open session's request and taking it off the card at once; false when `action` is not one of them.
    pub(crate) fn trust_pressed(&mut self, action: Target) -> bool {
        let verdict = match action {
            Target::Chat(ChatEvent::TrustAlways) => Verdict::Always,
            Target::Chat(ChatEvent::TrustSession) => Verdict::Allow,
            Target::Chat(ChatEvent::TrustDecline) => Verdict::Reject,
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
