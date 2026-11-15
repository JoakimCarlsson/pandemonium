//! The session view: the tabs of the conversation, the diff and open files,
//! split into two panes that take tabs dragged onto them, the session header, the pull request
//! and continue-chat bars, the conversation, the cards waiting on the reader
//! and the composer.

use rustle::prelude::*;

use pm_components::{
    Activity, ConfirmDialog, ContinueChat, IconButton, Label, LabelVariant, PullRequestBar,
    PullRequestVariant, Sash, Size, SpendStrip, extended_colors,
};
use pm_shell::{
    ActivityKind, PANE_SHARE_MIN, Pane, Project, PullRequestInfo, SessionStatus, SessionTab, SpendSummary,
    TabAction, Unsaved, Workspace,
};

use crate::actions::Actions;
use crate::browser_pane::browser_pane;
use crate::changes_page::{ChangesPage, changes_page};
use crate::conversation::conversation;
use crate::diff_page::diff_page;
use crate::file_view::open_file_view;
use crate::prompt_box::prompt_box;
use crate::session_asks::waiting_cards;
use crate::session_extras::{excerpts_tab, update_tab};
use crate::session_tabs::{session_tabs, tab_key};

/// Width of the conversation column.
pub(crate) const READING_WIDTH: f32 = 680.0;

/// Emits the session view for `workspace`: its tabs over the conversation,
/// the diff of the file the Git panel chose, or an open file.
pub fn session_view(workspace: &Workspace, actions: &Actions) {
    let scheme = color_scheme();
    let files = workspace.open_files.clone();
    let tab_keys = actions.file_tab.clone();
    stack_with(Modifier::new().fill_max_size(), || {
        column_with(
            Modifier::new()
                .fill_max_size()
                .background(scheme.background)
                .on_key_event(move |event| match tab_key(event, &files) {
                    Some(action) => {
                        tab_keys(action);
                        true
                    }
                    None => false,
                }),
            || {
                session_tabs(workspace, actions);
                match workspace.open_files.right_tab() {
                    Some(right) => split_panes(workspace, right, actions),
                    None => row_with(Modifier::new().fill_max_width().weight(1.0), || {
                        column_with(
                            Modifier::new()
                                .fill_max_height()
                                .weight(1.0)
                                .drop_target(dropped_on(actions, Pane::Left)),
                            || pane_content(workspace, workspace.open_files.tab, actions),
                        );
                        spacer(
                            Modifier::new()
                                .width(SIDE_DROP)
                                .fill_max_height()
                                .drop_target(dropped_on(actions, Pane::Right)),
                        );
                    }),
                }
            },
        );
        if workspace.open_files.asking.is_some() {
            unsaved_dialog(workspace, actions);
        }
    });
}

/// Emits the question asked before closing files with unsaved changes:
/// save them, throw the changes away, or keep the files open.
fn unsaved_dialog(workspace: &Workspace, actions: &Actions) {
    let files = &workspace.open_files;
    let names: Vec<String> = files
        .asking
        .map(|close| files.unsaved_closing(close))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|index| files.files.get(index))
        .map(|file| {
            file.path
                .rsplit('/')
                .next()
                .unwrap_or(&file.path)
                .to_owned()
        })
        .collect();
    let message = match names.as_slice() {
        [one] => format!("{one} has changes that are not saved. Save them before closing?"),
        many => format!(
            "{} files have changes that are not saved: {}. Save them before closing?",
            many.len(),
            many.join(", ")
        ),
    };
    let (save, discard, cancel) = (
        actions.file_tab.clone(),
        actions.file_tab.clone(),
        actions.file_tab.clone(),
    );
    ConfirmDialog::new("Unsaved changes", message, "Save")
        .alternative("Don't save", move || {
            discard(TabAction::Unsaved(Unsaved::Discard))
        })
        .on_confirm(move || save(TabAction::Unsaved(Unsaved::Save)))
        .on_cancel(move || cancel(TabAction::Unsaved(Unsaved::Cancel)))
        .show();
}

/// How wide the strip at the right edge of an unsplit view is, onto which a
/// tab is dropped to show it to the side.
const SIDE_DROP: f32 = 6.0;

/// What a tab dropped on `pane` does: shows the tab there.
fn dropped_on(actions: &Actions, pane: Pane) -> impl Fn(&dyn std::any::Any) + 'static {
    let move_to = actions.file_tab.clone();
    move |payload| {
        if let Some(tab) = payload.downcast_ref::<SessionTab>() {
            move_to(TabAction::MoveTo(*tab, pane));
        }
    }
}

/// Emits the view split in two panes side by side, the left showing the
/// strip's tab and the right `right`, each under a bar naming it that makes
/// it the pane tabs chosen show in, and each taking tabs dropped on it.
fn split_panes(workspace: &Workspace, right: SessionTab, actions: &Actions) {
    let files = &workspace.open_files;
    let width = remember(|| 0.0_f32);
    let share = files.left_share();
    let measured = width.clone();
    row_with(
        Modifier::new()
            .fill_max_width()
            .weight(1.0)
            .on_size_changed(move |size| measured.set(size.width)),
        || {
            for (pane, tab) in [(Pane::Left, files.tab), (Pane::Right, right)] {
                let focused = files.focus == pane;
                if pane == Pane::Right {
                    let resize = actions.file_tab.clone();
                    let across = width.get();
                    Sash::new(share * across)
                        .size(Size::Sm)
                        .range(across * PANE_SHARE_MIN, across * (1.0 - PANE_SHARE_MIN))
                        .on_resize(move |left| {
                            if across > 0.0 {
                                resize(TabAction::Resize((left / across * 1000.0).round() as u32));
                            }
                        })
                        .show();
                }
                let weight = match pane {
                    Pane::Left => share,
                    Pane::Right => 1.0 - share,
                };
                key(pane == Pane::Right, || {
                    column_with(
                        Modifier::new()
                            .fill_max_height()
                            .weight(weight)
                            .drop_target(dropped_on(actions, pane)),
                        || {
                            pane_bar(workspace, pane, tab, focused, actions);
                            pane_content(workspace, tab, actions);
                        },
                    );
                });
            }
        },
    );
}

/// Emits the bar over `pane`, which shows `tab`: its name, marked when the
/// pane is `focused`, a press making it the focused pane, and a button
/// closing the right pane.
fn pane_bar(workspace: &Workspace, pane: Pane, tab: SessionTab, focused: bool, actions: &Actions) {
    let scheme = color_scheme();
    let name = tab_name(workspace, tab);
    let focus = actions.file_tab.clone();
    let close = actions.file_tab.clone();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(24.0)
            .background(if focused {
                scheme.secondary_container
            } else {
                scheme.surface_container
            })
            .border_bottom_hairline(if focused {
                scheme.primary
            } else {
                scheme.outline_variant
            })
            .clickable_plain(move || focus(TabAction::Focus(pane)))
            .padding_xy(10.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(6.0)),
        Alignment::Center,
        move || {
            Label::new(name.clone())
                .variant(if focused {
                    LabelVariant::Body
                } else {
                    LabelVariant::Muted
                })
                .size(Size::Xs)
                .modifier(Modifier::new().weight(1.0))
                .show();
            if pane == Pane::Right {
                let close = close.clone();
                IconButton::new(phosphor::X)
                    .size(Size::Xs)
                    .on_click(move || close(TabAction::Unsplit))
                    .show();
            }
        },
    );
}

/// What the tab `tab` of `workspace` is called.
fn tab_name(workspace: &Workspace, tab: SessionTab) -> String {
    match tab {
        SessionTab::Chat if workspace.agent.is_empty() => "Chat".to_owned(),
        SessionTab::Chat => workspace.agent.clone(),
        SessionTab::Diff => "Diff".to_owned(),
        SessionTab::File(index) => workspace
            .open_files
            .files
            .get(index)
            .map_or_else(String::new, |file| file.path.clone()),
        SessionTab::Browser => "Browser".to_owned(),
        SessionTab::Excerpts => "Excerpts".to_owned(),
        SessionTab::Update => "Update".to_owned(),
    }
}

/// Emits what the tab `tab` of `workspace` shows.
fn pane_content(workspace: &Workspace, tab: SessionTab, actions: &Actions) {
    match tab {
        SessionTab::Chat => chat(workspace, actions),
        SessionTab::Diff => diff_tab(workspace, actions),
        SessionTab::File(index) => {
            if !open_file_view(workspace, index, actions) {
                chat(workspace, actions);
            }
        }
        SessionTab::Browser => match &workspace.browser {
            Some(browser) => browser_pane(browser, actions),
            None => chat(workspace, actions),
        },
        SessionTab::Excerpts => excerpts_tab(workspace, actions),
        SessionTab::Update => match &workspace.update {
            Some(offer) => update_tab(offer, actions),
            None => chat(workspace, actions),
        },
    }
}

/// Emits the Diff tab: a commit's or a branch's files when they were asked
/// for, the file the Git panel chose, or else every uncommitted change.
fn diff_tab(workspace: &Workspace, actions: &Actions) {
    let git = &workspace.git;
    if let Some(revision) = &git.revision {
        let message = if let Some(trouble) = &revision.trouble {
            Some(trouble.clone())
        } else if !revision.loaded {
            Some("Loading the changes…".to_owned())
        } else if revision.changes.is_empty() {
            Some("Nothing changed.".to_owned())
        } else {
            None
        };
        changes_page(
            ChangesPage {
                title: revision.title.clone(),
                files: &revision.changes,
                working: false,
                message,
            },
            actions,
        );
        return;
    }
    match &git.diff {
        Some(path) => diff_page(workspace, path, actions),
        None if git.changes.is_empty() => diff_placeholder(),
        None => changes_page(
            ChangesPage {
                title: "Uncommitted changes".to_owned(),
                files: &git.listed().into_iter().cloned().collect::<Vec<_>>(),
                working: true,
                message: None,
            },
            actions,
        ),
    }
}

/// Emits the conversation tab: the session header, the pull request and
/// continue-chat bars, the conversation and what waits below it.
fn chat(workspace: &Workspace, actions: &Actions) {
    session_header(workspace);
    if let Some(request) = &workspace.extras.pull_request {
        pull_request_bar(request, actions);
    }
    if workspace.extras.chat {
        continue_bar(workspace, actions);
    }
    conversation(&workspace.conversation, actions);
    below_conversation(workspace, actions);
}

/// Emits what the Diff tab shows while the session has no changes.
fn diff_placeholder() {
    column_arranged(
        Modifier::new().fill_max_width().weight(1.0),
        Arrangement::Center,
        Alignment::Center,
        || {
            Label::new("The session has no changes.")
                .variant(LabelVariant::Muted)
                .size(Size::Sm)
                .show()
        },
    );
}

/// The header naming the session, its branch, its agent and its account.
fn session_header(workspace: &Workspace) {
    let scheme = color_scheme();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(30.0)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(12.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        || {
            let status = workspace
                .open
                .as_ref()
                .and_then(|at| pm_shell::session_at(&workspace.projects, at))
                .map(|session| session.status)
                .unwrap_or_default();
            status_dot(status);
            Label::new(workspace.title.clone())
                .modifier(Modifier::new().weight(1.0))
                .show();
            if !workspace.branch.is_empty() {
                header_chip(phosphor::GIT_BRANCH, &workspace.branch);
            }
            if !workspace.model.is_empty() {
                header_chip(phosphor::SPARKLE, &workspace.model);
            }
            if !workspace.agent.is_empty() {
                Label::new(workspace.agent.clone())
                    .variant(LabelVariant::Subtle)
                    .size(Size::Xs)
                    .show();
            }
            if !workspace.account.is_empty() {
                header_chip(phosphor::USER_CIRCLE, &workspace.account);
            }
        },
    );
}

/// Emits the mark of the header saying how the open session stands as `status` says: turning while its agent works, filled in the accent when something is unread, in the danger tone when it failed, in the warning tone when it waits on the reader, broken when its server is out of reach, and an outline when it is idle.
fn status_dot(status: SessionStatus) {
    let scheme = color_scheme();
    let extended = extended_colors();
    let fill = match status {
        SessionStatus::Working => {
            circular_progress_indicator_indeterminate(9.0);
            return;
        }
        SessionStatus::OutOfReach => {
            icon(phosphor::LINK_BREAK, 12.0, scheme.on_surface_variant);
            return;
        }
        SessionStatus::Unread => scheme.primary,
        SessionStatus::Error => scheme.error,
        SessionStatus::NeedsInput => extended.warning,
        SessionStatus::Idle => {
            spacer(
                Modifier::new()
                    .size(7.0, 7.0)
                    .border(1.0, extended.subtle, 3.5),
            );
            return;
        }
    };
    spacer(Modifier::new().size(7.0, 7.0).background_rounded(fill, 3.5));
}

/// Emits a small rounded chip of the header showing `glyph` and `text`.
fn header_chip(glyph: Icon, text: &str) {
    let scheme = color_scheme();
    row_arranged(
        Modifier::new()
            .background_rounded(scheme.surface_container_high, 4.0)
            .padding_xy(6.0, 1.0),
        Arrangement::SpacedBy(pm_components::spacing(4.0)),
        Alignment::Center,
        || {
            icon(glyph, 12.0, scheme.on_surface_variant);
            Label::new(text.to_owned())
                .variant(LabelVariant::Muted)
                .size(Size::Xs)
                .show();
        },
    );
}

/// The bar of the branch's pull request.
fn pull_request_bar(request: &PullRequestInfo, actions: &Actions) {
    let variant = match (request.number, request.state.as_str(), request.draft) {
        (0, _, _) => PullRequestVariant::None,
        (_, "merged", _) => PullRequestVariant::Merged,
        (_, "closed", _) => PullRequestVariant::Closed,
        (_, _, true) => PullRequestVariant::Draft,
        _ => PullRequestVariant::Open,
    };
    let (passed, failed, pending) = request.checks;
    let act = actions.pull_request.clone();
    let open = actions.open_address.clone();
    let url = request.url.clone();
    PullRequestBar::new(request.number, request.title.clone())
        .variant(variant)
        .noun(request.noun.clone())
        .supported(request.supported)
        .ahead(request.ahead)
        .busy(request.busy.clone())
        .error(request.error.clone())
        .review(request.review.clone())
        .checks(passed, failed, pending)
        .on_action(move |action| {
            if action == "open" {
                open(url.clone());
            } else {
                act(action);
            }
        })
        .show();
}

/// The bar offering to continue the open chat of `workspace` in one of the projects on the chat's own machine, from a branch of the one picked; switched off while the agent works or prompts wait for it.
fn continue_bar(workspace: &Workspace, actions: &Actions) {
    let mut offered = Vec::new();
    flatten(&workspace.projects, Vec::new(), &mut offered);
    offered.retain(|(_, _, machine)| machine.as_deref() == workspace.machine.as_deref());
    let names = offered.iter().map(|(name, _, _)| name.clone()).collect();
    let chosen = workspace
        .continuing
        .project
        .as_ref()
        .and_then(|picked| offered.iter().position(|(_, at, _)| at == picked));
    let composer = &workspace.extras.composer;
    let busy = composer.working || !composer.queued.is_empty();
    let pick = actions.continue_project.clone();
    let carry = actions.continue_chat.clone();
    ContinueChat::new(names)
        .chosen(chosen, workspace.continuing.branches.clone())
        .disabled(busy)
        .on_pick(move |index| {
            if let Some((_, at, _)) = offered.get(index) {
                pick(at.clone());
            }
        })
        .on_branch(move |base| carry(base))
        .show();
}

/// Puts every project among `projects` under `prefix`, nested ones after
/// their parent, into `offered` with its index path and the machine it lives on.
fn flatten(
    projects: &[Project],
    prefix: Vec<usize>,
    offered: &mut Vec<(String, Vec<usize>, Option<String>)>,
) {
    for (index, project) in projects.iter().enumerate() {
        let mut at = prefix.clone();
        at.push(index);
        offered.push((
            project.name.clone(),
            at.clone(),
            project.machine().map(str::to_owned),
        ));
        flatten(&project.children, at, offered);
    }
}

/// Everything under the conversation: the cards waiting on the reader, the
/// activity chips, the composer and the spend strip.
fn below_conversation(workspace: &Workspace, actions: &Actions) {
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        || {
            column_arranged(
                Modifier::new().width(READING_WIDTH),
                Arrangement::SpacedBy(pm_components::spacing(8.0)),
                Alignment::Start,
                || {
                    status_line(workspace);
                    waiting_cards(&workspace.extras, actions);
                    let mut chips = Activity::new();
                    for activity in &workspace.extras.activity {
                        let glyph = match activity.kind {
                            ActivityKind::Background => phosphor::TERMINAL_WINDOW,
                            ActivityKind::Subagent => phosphor::ROBOT,
                        };
                        chips = chips.item(glyph, activity.label.clone());
                    }
                    chips.show();
                },
            );
            prompt_box(workspace, actions, READING_WIDTH);
            row_arranged(
                Modifier::new()
                    .width(READING_WIDTH)
                    .padding_each(0.0, 0.0, 0.0, 6.0),
                Arrangement::End,
                Alignment::Center,
                || {
                    if let Some(spend) = &workspace.extras.spend {
                        spend_strip(spend);
                    }
                },
            );
        },
    );
}

/// What the open session's agent is doing while nothing waits on the reader: that its server is out of reach, that the prompt is being sent, or that it is thinking; nothing while it is idle or asks something.
fn status_line(workspace: &Workspace) {
    let extras = &workspace.extras;
    let out_of_reach = workspace
        .open
        .as_ref()
        .and_then(|at| pm_shell::session_at(&workspace.projects, at))
        .is_some_and(|session| session.status == SessionStatus::OutOfReach);
    let asking = extras.approval.is_some()
        || extras.inquiry.is_some()
        || extras.trust.is_some()
        || extras.signed_out.is_some()
        || extras.container.is_some();
    let label = if out_of_reach {
        "The server is out of reach. Waiting for the connection to come back…"
    } else if asking {
        return;
    } else if workspace.sending {
        "Sending prompt…"
    } else if extras.composer.working {
        "Thinking…"
    } else {
        return;
    };
    let scheme = color_scheme();
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        move || {
            if out_of_reach {
                icon(phosphor::LINK_BREAK, 14.0, scheme.on_surface_variant);
            } else {
                circular_progress_indicator_indeterminate(12.0);
            }
            Label::new(label)
                .variant(LabelVariant::Muted)
                .size(Size::Xs)
                .show();
        },
    );
}

/// The strip saying what the conversation cost.
fn spend_strip(spend: &SpendSummary) {
    let dollars = |cost: Option<f64>| match cost {
        Some(cost) => cents(cost),
        None => "unpriced".to_owned(),
    };
    let total = match (spend.cost, spend.partial) {
        (Some(cost), true) => format!("at least {}", cents(cost)),
        (cost, _) => dollars(cost),
    };
    let mut strip = SpendStrip::new(total).detail(format!(
        "{} tokens · {} requests",
        thousands(spend.tokens),
        spend.requests
    ));
    for (model, cost, tokens) in &spend.models {
        strip = strip.model(
            model.clone(),
            dollars(*cost),
            format!("{} tokens", thousands(*tokens)),
        );
    }
    strip.show();
}

/// `cost` in dollars to the cent, such as `$1.20`, or `<$0.01` for a cost above nothing that rounds to no cent.
fn cents(cost: f64) -> String {
    if cost > 0.0 && cost < 0.005 {
        "<$0.01".to_owned()
    } else {
        format!("${cost:.2}")
    }
}

/// `count` shortened with `k` or `M`, such as `412k`.
fn thousands(count: u64) -> String {
    match count {
        0..1_000 => count.to_string(),
        1_000..1_000_000 => format!("{}k", count / 1_000),
        _ => format!("{:.1}M", count as f64 / 1_000_000.0),
    }
}
