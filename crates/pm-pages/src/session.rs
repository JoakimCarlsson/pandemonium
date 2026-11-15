//! The session view: the tabs of the conversation, the diff and open files,
//! split into two panes that take tabs dragged onto them, the session header, the pull request
//! and continue-chat bars, the conversation, the cards waiting on the reader
//! and the composer.

use rustle::prelude::*;

use pm_components::{
    Activity, ContinueChat, IconButton, Label, LabelVariant, PullRequestBar, PullRequestVariant,
    Size, SpendStrip, extended_colors,
};
use pm_shell::{
    ActivityKind, Pane, Project, PullRequestInfo, SessionTab, SpendSummary, TabAction, Workspace,
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
    let scheme = color_scheme();
    let files = &workspace.open_files;
    row_with(Modifier::new().fill_max_width().weight(1.0), || {
        for (pane, tab) in [(Pane::Left, files.tab), (Pane::Right, right)] {
            let focused = files.focus == pane;
            if pane == Pane::Right {
                spacer(
                    Modifier::new()
                        .width(1.0)
                        .fill_max_height()
                        .background(scheme.outline_variant),
                );
            }
            key(pane == Pane::Right, || {
                column_with(
                    Modifier::new()
                        .fill_max_height()
                        .weight(1.0)
                        .drop_target(dropped_on(actions, pane)),
                    || {
                        pane_bar(workspace, pane, tab, focused, actions);
                        pane_content(workspace, tab, actions);
                    },
                );
            });
        }
    });
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
        continue_bar(&workspace.projects, actions);
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
    let extended = extended_colors();
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(30.0)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(12.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        || {
            spacer(
                Modifier::new()
                    .size(7.0, 7.0)
                    .border(1.0, extended.subtle, 3.5),
            );
            Label::new(workspace.title.clone())
                .modifier(Modifier::new().weight(1.0))
                .show();
            if !workspace.branch.is_empty() {
                header_chip(phosphor::GIT_BRANCH, &workspace.branch);
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
            icon(glyph, 11.0, scheme.on_surface_variant);
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

/// The bar offering to continue the open chat in one of `projects`.
fn continue_bar(projects: &[Project], actions: &Actions) {
    let mut offered = Vec::new();
    flatten(projects, Vec::new(), &mut offered);
    let names = offered.iter().map(|(name, _)| name.clone()).collect();
    let carry = actions.continue_chat.clone();
    ContinueChat::new(names)
        .on_pick(move |index| {
            if let Some((_, at)) = offered.get(index) {
                carry(at.clone());
            }
        })
        .show();
}

/// Puts every project among `projects` under `prefix`, nested ones after
/// their parent, into `offered` with its index path.
fn flatten(projects: &[Project], prefix: Vec<usize>, offered: &mut Vec<(String, Vec<usize>)>) {
    for (index, project) in projects.iter().enumerate() {
        let mut at = prefix.clone();
        at.push(index);
        offered.push((project.name.clone(), at.clone()));
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

/// The strip saying what the conversation cost.
fn spend_strip(spend: &SpendSummary) {
    let dollars = |cost: Option<f64>| match cost {
        Some(cost) => format!("${cost:.2}"),
        None => "unpriced".to_owned(),
    };
    let total = match (spend.cost, spend.partial) {
        (Some(cost), true) => format!("at least ${cost:.2}"),
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

/// `count` shortened with `k` or `M`, such as `412k`.
fn thousands(count: u64) -> String {
    match count {
        0..1_000 => count.to_string(),
        1_000..1_000_000 => format!("{}k", count / 1_000),
        _ => format!("{:.1}M", count as f64 / 1_000_000.0),
    }
}
