//! The prompt box where work is handed to an agent.

use crate::areas::ChatEvent;
use crate::areas::ComposerEvent;
use crate::parts;
use crate::theme::Frame;
use crate::theme::ShadowLayers;
use crate::theme::{FocusOutline, Indicated};
use rustle::TextOverflow;

use crate::events::{Event, send};
use crate::target::Target;
use crate::theme;
use rustle::{Alignment, Arrangement, FlowOptions, flow_row_with, row_arranged};
use rustle::{Icon, phosphor};
use std::cell::Cell;

use pm_harness::Setting;
use rustle::{CursorIcon, Modifier};

use crate::Popover;
use crate::choices::{LIST_ROW_HEIGHT, LIST_ROWS, PRESSABLE_LIST_ROWS};
use crate::events::Interactive;
use crate::parts::{line, spacer};
use crate::plan::proposal;
use crate::popover::{anchored, anchored_end, heading};
use crate::popups::Placement;
use crate::sample::Session;
use crate::size::Size;
use crate::state::{Location, Picker, WorkspaceState};
use crate::theme::{Radius, Role, sp};
use crate::usage;
use crate::{Button, Checkbox, Input, Stepper, button, checkbox, input};

/// The shadow under the prompt box: a soft, wide one that lifts it off the page, as `(x, y, blur, spread, alpha)`.
const COMPOSER_SHADOW: [rustle::Shadow; 1] = [crate::theme::box_shadow(0.0, 8.0, 30.0, 0.0, 0.18)];

/// The prompt box, which takes the keyboard once a control that leads to typing in it was pressed.
const PROMPT: Target = Target::Composer(ComposerEvent::Prompt);

/// How many branches the menu of where work is done lists.
const BRANCH_ROWS: usize = 8;

/// How tall a line of the prompt is, in logical pixels.
const LINE: f32 = 22.0;

/// The room the prompt's lines have above and below them, in logical pixels.
const PADDING: f32 = 18.0;

/// How far a menu stays clear of the prompt box, in logical pixels.
const GAP: f32 = 10.0;

/// How tall one value of a menu is, in logical pixels.
const ROW_HEIGHT: f32 = crate::choices::ROW_HEIGHT;

/// How many values of a menu show at once at most.
const MAX_ROWS: f32 = 6.0;

/// How tall the heading of a menu is, in logical pixels.
const HEADING: f32 = 32.0;

/// How tall a line in the footer of a menu is, in logical pixels.
const FOOTER_ROW: f32 = 36.0;

/// The room a menu keeps around what it holds, in logical pixels.
const MENU_PADDING: f32 = 12.0;

/// How wide a menu over the prompt box is, in logical pixels.
const MENU_WIDTH: f32 = 360.0;

thread_local! {
    /// How wide the prompt box was laid out last, in logical pixels, so the lists hanging above it can be as wide; zero before it was laid out.
    static PROMPT_BOX_WIDTH: Cell<f32> = const { Cell::new(0.0) };
}

/// How tall the list of values in a menu is, in logical pixels, when `above` pixels are free over the prompt box:
/// as many whole values as fit beside the heading and the footer, between two and six.
fn list_height(settings: &[Setting], above: f32, rows: u32) -> f32 {
    let footer = settings
        .iter()
        .filter(|held| held.category == "thought_level" || held.id == "agent")
        .count() as f32
        * FOOTER_ROW;
    let free = above + rows as f32 * LINE + PADDING - GAP - HEADING - footer - MENU_PADDING;
    let shown = (free / ROW_HEIGHT).floor().clamp(2.0, MAX_ROWS);
    shown * ROW_HEIGHT
}

/// Emits `content` in a section of a menu across its parent, set off from
/// what is above it by a rule: `border_t()` with `mt(1)`.
#[track_caller]
pub(crate) fn ruled(content: impl FnOnce()) {
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .padding_each(0.0, theme::sp(1), 0.0, 0.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            rustle::horizontal_divider(Modifier::new(), 1.0, theme::color(Role::Border));
            content();
        },
    );
}

/// The modifier of the body of a menu over the prompt box: a press inside it keeps it open.
pub(crate) fn menu_body() -> Modifier {
    Modifier::new().control(
        Target::Composer(ComposerEvent::MenuBody),
        send(Event::Composer(ComposerEvent::MenuBody)),
    )
}

/// Emits the menu of the setting at `index`: its values in a list that scrolls, and the effort beneath the models; nothing when there is no such setting.
fn setting_menu(state: &WorkspaceState, index: usize, rows: u32, above: f32) {
    let settings = state.settings_now();
    let Some(setting) = settings.get(index) else {
        return;
    };
    let (title, hint) = match setting.category.as_str() {
        "model" => ("Select a model", None),
        "mode" => ("Modes", Some(("shift+tab", "to switch"))),
        _ => (setting.name.as_str(), None),
    };
    let filters = state.menu_filters();
    let shown: Vec<usize> = if filters {
        state.menu_matches()
    } else {
        (0..setting.picks.len()).collect()
    };
    Popover::new()
        .width(MENU_WIDTH)
        .heading(title, hint)
        .modifier(menu_body())
        .build_with(|| {
            if filters {
                rustle::column_arranged(
                    Modifier::new().fill_max_width().padding_each(
                        theme::sp(1),
                        0.0,
                        theme::sp(1),
                        theme::sp(1),
                    ),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        crate::focus::request_focus_on(
                            Target::Composer(ComposerEvent::MenuFilter),
                            &index,
                        );
                        Input::search(
                            &state.menu_filter,
                            Target::Composer(ComposerEvent::MenuFilter),
                        )
                        .modifier(Modifier::new().fill_max_width())
                        .size(Size::Sm)
                        .placeholder(match setting.category.as_str() {
                            "model" => "Search models",
                            _ => "Search",
                        })
                        .submits(Target::Composer(ComposerEvent::MenuFilterPick))
                        .build();
                    },
                );
            }
            let height = sp(list_height(settings, above, rows) / 4.0);
            rustle::column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height_in(0.0, height)
                    .vertical_scroll(&state.menu_scroll)
                    .fill_max_width(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    for position in &shown {
                        let Some(pick) = setting.picks.get(*position) else {
                            continue;
                        };
                        crate::popover::row(
                            &pick.name,
                            &pick.description,
                            pick.value == setting.current,
                            Target::Composer(ComposerEvent::MenuPick(*position)),
                        );
                    }
                    if shown.is_empty() {
                        parts::note(2, 2, "Nothing matches", 13, Role::Muted);
                    }
                },
            );
            if setting.category == "model"
                && let Some(effort) = settings
                    .iter()
                    .find(|held| held.category == "thought_level")
            {
                effort_row(effort);
            }
            if setting.category == "model"
                && let Some(agent) = settings.iter().find(|held| held.id == "agent")
            {
                agent_row(agent);
            }
            if setting.category == "model"
                && let Some(on) = state.orchestrating()
            {
                ruled_row(
                    "Orchestrate",
                    "Lead, and hand work to subagents by category",
                    on,
                    Target::Composer(ComposerEvent::Orchestrate),
                );
            }
        });
}

/// Emits a row of a menu set off from what is above it by a rule: `name` over `said`, ticked while `on`, answering to `id`.
#[track_caller]
fn ruled_row(name: &str, said: &str, on: bool, id: Target) {
    ruled(|| crate::popover::row(name, said, on, id));
}

/// Emits the menu of where new work is done, and the branch a worktree is made from.
fn location_menu(state: &WorkspaceState) {
    let worktree = state.location == Location::Worktree;
    let mut menu = Popover::new().width(MENU_WIDTH).heading("Run in", None);
    if state.is_repository() {
        menu = menu
            .row(
                "Worktree",
                "A worktree of its own on a detached head",
                worktree,
                Target::Composer(ComposerEvent::Place(0)),
            )
            .row(
                "Local",
                "Work directly in the main checkout",
                !worktree,
                Target::Composer(ComposerEvent::Place(1)),
            );
    }
    menu.modifier(menu_body()).build_with(|| {
        if state.offers_container() {
            ruled_row(
                "Dev container",
                "Not a security boundary",
                state.container,
                Target::Composer(ComposerEvent::DevContainer),
            );
        }
        if worktree {
            ruled(|| heading("Branch from", None));
            branch_rows(&state.branches, state.base.as_ref());
        }
    });
}

/// Emits the rows choosing the branch a worktree is made from, out of `branches`: `picked` ticked, or the first while none was picked.
#[track_caller]
pub(crate) fn branch_rows(branches: &[String], picked: Option<&String>) {
    let chosen = picked.or(branches.first());
    for (position, branch) in branches.iter().take(BRANCH_ROWS).enumerate() {
        crate::popover::row(
            branch,
            "",
            chosen == Some(branch),
            Target::Composer(ComposerEvent::Branch(position)),
        );
    }
}

/// What the menu of accounts says under an account: how much of its allowance is used, when the provider has said.
fn spoken(state: &WorkspaceState, login: Option<&str>) -> String {
    state
        .allowance_of(state.agent_now(), login)
        .and_then(crate::usage::summary)
        .unwrap_or_default()
}

/// Emits the menu of the logins of the chosen agent: its own, each one added, and adding another.
fn account_menu(state: &WorkspaceState) {
    let used = state.login_in_use();
    let logins = state.logins_here();
    let mut menu = Popover::new()
        .width(MENU_WIDTH)
        .heading("Account", None)
        .row(
            state.default_account().unwrap_or("Not signed in"),
            &spoken(state, None),
            used.is_none(),
            Target::Composer(ComposerEvent::AccountRow(0)),
        );
    for (position, (id, name)) in logins.iter().enumerate() {
        menu = menu.row(
            name,
            &spoken(state, Some(id)),
            used == Some(*id),
            Target::Composer(ComposerEvent::AccountRow(1 + position)),
        );
    }
    menu.row(
        "Add account",
        "Sign in to another login",
        false,
        Target::Composer(ComposerEvent::AccountRow(1 + logins.len())),
    )
    .build_target(Target::Composer(ComposerEvent::MenuBody));
}

/// What the goal chip says of `goal`: how it stands, and how much of its budget is used when it has one.
fn goal_label(goal: &pm_harness::Goal) -> String {
    use pm_harness::GoalState;
    let state = match goal.state {
        GoalState::Active => "Goal",
        GoalState::Paused => "Goal paused",
        GoalState::BudgetLimited => "Goal out of budget",
        GoalState::Complete => "Goal done",
        GoalState::Cleared => "No goal",
    };
    match goal.budget {
        Some(budget) => format!(
            "{state} · {}k/{}k",
            goal.used.div_ceil(1000),
            budget.div_ceil(1000)
        ),
        None => state.to_owned(),
    }
}

/// Emits the menu of the goal chip: the objective and why the goal stands as it does, then pausing it while it is active, resuming it while it is paused, and clearing it.
fn goal_menu(goal: &pm_harness::Goal) {
    use pm_harness::GoalState;
    let mut menu = Popover::new().width(MENU_WIDTH).heading("Goal", None).row(
        &goal.objective,
        goal.note.as_deref().unwrap_or(""),
        false,
        Target::Composer(ComposerEvent::MenuBody),
    );
    let offered = [
        (
            GoalState::Active,
            "Pause",
            "Finish this turn and start no more",
        ),
        (GoalState::Paused, "Resume", "Go on working toward it"),
        (goal.state, "Clear", "Forget the goal"),
    ];
    for (row, (when, name, said)) in offered.into_iter().enumerate() {
        if goal.state == when {
            menu = menu.row(
                name,
                said,
                false,
                Target::Composer(ComposerEvent::GoalRow(row)),
            );
        }
    }
    menu.build_target(Target::Composer(ComposerEvent::MenuBody));
}

/// Emits the line under the models that sets how hard the agent thinks: its name and a track with a stop for each level.
fn effort_row(effort: &Setting) {
    let current = effort
        .picks
        .iter()
        .position(|pick| pick.value == effort.current);
    let stops = effort.picks.len();
    labelled(
        "Effort",
        effort.current_name().unwrap_or("Default"),
        || {
            Stepper::new(stops, Target::Composer(ComposerEvent::Effort(0)))
                .current(current)
                .build()
        },
        true,
    );
}

/// Emits the line under the models that switches the agent persona on and off.
#[track_caller]
fn agent_row(agent: &Setting) {
    let on = agent
        .picks
        .first()
        .is_some_and(|first| first.value != agent.current);
    labelled(
        "Agent",
        if on { "On" } else { "Default" },
        || {
            Checkbox::new(on, Target::Composer(ComposerEvent::AgentToggle))
                .variant(checkbox::Variant::Switch)
                .build();
        },
        false,
    );
}

/// The lambda that offers a paste in the prompt to the app first, which attaches the files
/// or the picture on the clipboard and says whether there were any, so the prompt pastes
/// only text.
fn attaches_pasted() -> impl Fn() -> bool + 'static {
    let operations = crate::events::operations();
    move || operations.paste_attachments()
}

/// Emits a line of a menu's footer: `name` with how it is set, `value`, in brackets, then what `control` emits to set it, under a rule when `rule`.
#[track_caller]
fn labelled(name: &str, value: &str, control: impl FnOnce(), rule: bool) {
    rustle::column_arranged(
        Modifier::new().fill_max_width().height(theme::sp(9)),
        Arrangement::Start,
        Alignment::Start,
        || {
            if rule {
                rustle::horizontal_divider(Modifier::new(), 1.0, theme::color(Role::Border));
            }
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .weight(1.0)
                    .padding_xy(theme::sp(2.5), 0.0),
                Arrangement::SpaceBetween,
                Alignment::Center,
                || {
                    rustle::row_arranged(
                        Modifier::new(),
                        Arrangement::SpacedBy(theme::sp(1)),
                        Alignment::Center,
                        || {
                            line(name, 13, 1.4, Role::Text);
                            line(format!("({value})"), 13, 1.4, Role::Subtle);
                        },
                    );
                    control();
                },
            );
        },
    );
}

/// Whether a list hangs above the prompt box: commands the slash being typed narrows, or files the word after an `@` narrows.
fn lists_shown(state: &WorkspaceState) -> bool {
    !state.slash_matches().is_empty()
        || !state.mention_matches().is_empty()
        || state.mention_query().is_some()
}

/// The modifier of a list hanging above the prompt box: as wide as the box was laid out last.
fn above_box() -> Modifier {
    let width = PROMPT_BOX_WIDTH.with(Cell::get);
    if width > 0.0 {
        Modifier::new().width(width)
    } else {
        Modifier::new().fill_max_width()
    }
}

/// The look of a row of a list above the prompt box: filled on hover, and filled while it is the `chosen` one.
fn list_row_looks(chosen: bool) -> theme::InteractionTones {
    let looks = theme::InteractionTones::new()
        .hover(Role::SurfaceHover)
        .radius(Radius::Lg);
    if chosen {
        looks.idle(Role::SurfaceHover)
    } else {
        looks
    }
}

/// Emits a list hanging above the prompt box, headed `title`, holding what `content` emits.
#[track_caller]
fn above_list(title: &str, content: impl FnOnce()) {
    Popover::new()
        .full_width()
        .heading(title, None)
        .modifier(above_box())
        .build_with(content);
}

/// Emits the rows `content` emits in a column of a list above the prompt box that scrolls as
/// `state` holds once it is taller than the list's rows.
#[track_caller]
fn scrolled_rows(state: &WorkspaceState, content: impl FnOnce()) {
    let height = sp(LIST_ROW_HEIGHT * LIST_ROWS as f32 / 4.0);
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .height_in(0.0, height)
            .vertical_scroll(&state.menu_scroll)
            .fill_max_width(),
        Arrangement::Start,
        Alignment::Start,
        content,
    );
}

/// The modifier of the row at `position` of a list above the prompt box, filled while it is
/// `chosen`, and pressed as a slash row when `pressable`, padded around its text.
fn list_row(position: usize, chosen: bool, pressable: bool) -> Modifier {
    let looks = list_row_looks(chosen);
    let own = if pressable {
        let id = Target::Composer(ComposerEvent::Slash(position));
        Modifier::new()
            .indicated(looks, theme::Focused::Control(crate::focus::is_focused(id)))
            .cursor(CursorIcon::Pointer)
            .control(id, {
                let pressed = send(Event::pressing(id));
                let prompt =
                    (position < PRESSABLE_LIST_ROWS).then(|| crate::focus::focus_of(PROMPT));
                move || {
                    pressed();
                    if let Some(prompt) = &prompt {
                        prompt.request_focus();
                    }
                }
            })
    } else {
        Modifier::new()
            .indicated(looks, theme::Focused::Rustle)
            .cursor(CursorIcon::Pointer)
    };
    Modifier::new()
        .fill_max_width()
        .chain(own)
        .padding_xy(theme::sp(2.5), theme::sp(2))
}

/// Emits the list of files the word after an `@` being typed narrows; nothing when no `@` word is being typed.
fn mention_list(state: &WorkspaceState) {
    let matches = state.mention_matches();
    if matches.is_empty() {
        if state.mention_query().is_some() {
            above_list("Files", || {
                parts::note(2.5, 2, "No files found", 13, Role::Subtle)
            });
        }
        return;
    }
    let chosen = state.slash.min(matches.len() - 1);
    above_list("Files", || {
        scrolled_rows(state, || {
            for (position, path) in matches.iter().enumerate() {
                rustle::row_arranged(
                    list_row(position, position == chosen, true),
                    Arrangement::Start,
                    Alignment::Center,
                    || {
                        rustle::basic_text(
                            Modifier::new().weight(1.0),
                            path.as_str(),
                            theme::code(13, Role::Text),
                            TextOverflow::Ellipsis,
                        );
                    },
                );
            }
        });
    });
}

/// Emits the list of commands the agent offers, which the slash being typed narrows, or else the list of files; nothing when neither is being typed.
fn slash_list(state: &WorkspaceState) {
    let matches = state.slash_matches();
    if matches.is_empty() {
        mention_list(state);
        return;
    }
    let chosen = state.slash.min(matches.len() - 1);
    above_list("Commands", || {
        scrolled_rows(state, || {
            for (position, command) in matches.iter().enumerate() {
                let own = list_row(position, position == chosen, position < PRESSABLE_LIST_ROWS);
                rustle::row_arranged(
                    own,
                    Arrangement::SpacedBy(theme::sp(3)),
                    Alignment::Center,
                    || {
                        rustle::basic_text(
                            Modifier::new(),
                            format!("/{}", command.name),
                            theme::mono_style(13, 1.4).color(theme::color(Role::Text)),
                            TextOverflow::Ellipsis,
                        );
                        rustle::basic_text(
                            Modifier::new().weight(1.0),
                            &command.description,
                            theme::line(12, Role::Subtle),
                            TextOverflow::Ellipsis,
                        );
                    },
                );
            }
        });
    });
}

/// How far across a thumbnail of an attached picture is, in logical pixels.
const THUMBNAIL: f32 = 28.0;

/// How far across the picture of a file in a sent message is, in logical pixels.
const SENT_PICTURE: f32 = 160.0;

/// Emits the file at `path` as the reader sees it: its picture, `side` pixels across, when it is one and is decoded, and its name.
#[track_caller]
pub fn attached_file(state: &WorkspaceState, path: &str, side: f32) {
    let name = path.rsplit('/').next().unwrap_or(path);
    let shown = state.previews.get(path).cloned();
    rustle::row_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Center,
        || {
            if let Some(shown) = shown {
                rustle::image(
                    Modifier::new().width(side).height(side).clip(),
                    &shown,
                    rustle::ContentScale::Crop,
                );
            }
            line(name, 12, 1.4, Role::Muted);
        },
    );
}

/// Emits the files attached to the prompt, each with a button that takes it off again; nothing when none is attached.
fn attachment_row(state: &WorkspaceState) {
    if state.attachments.is_empty() {
        return;
    }
    flow_row_with(
        Modifier::new().fill_max_width().padding_each(
            theme::sp(3),
            theme::sp(3),
            theme::sp(3),
            0.0,
        ),
        FlowOptions::new(sp(2), sp(2)),
        || {
            for (index, path) in state.attachments.iter().enumerate() {
                rustle::key(index, || {
                    rustle::row_arranged(
                        Modifier::new()
                            .background_rounded(theme::color(Role::SurfaceHover), Radius::Lg.px())
                            .padding(theme::sp(1)),
                        Arrangement::SpacedBy(theme::sp(1.5)),
                        Alignment::Center,
                        || {
                            attached_file(state, path, THUMBNAIL);
                            Button::icon(
                                phosphor::X,
                                Target::Composer(ComposerEvent::Attached(index)),
                            )
                            .build();
                        },
                    );
                });
            }
        },
    );
}

/// Emits the files sent with a message, each as large as the room beside the words allows.
#[track_caller]
pub fn sent_files(state: &WorkspaceState, files: &[String]) {
    rustle::column_arranged(
        Modifier::new(),
        Arrangement::SpacedBy(theme::sp(2)),
        Alignment::Start,
        || {
            for path in files {
                match state.previews.get(path) {
                    Some(shown) => rustle::image(
                        Modifier::new()
                            .width(SENT_PICTURE)
                            .height(SENT_PICTURE)
                            .clip(),
                        shown,
                        rustle::ContentScale::Crop,
                    ),
                    None => attached_file(state, path, THUMBNAIL),
                }
            }
        },
    );
}

/// Emits the round button at the end of the strip, answering to `id` with `icon`, switched off while `disabled`.
#[track_caller]
fn round_button(icon: Icon, id: Target, disabled: bool) {
    Button::icon(icon, id)
        .variant(button::Variant::Default)
        .size(Size::Md)
        .round()
        .disabled(disabled)
        .build();
}

/// Emits the buttons at the end of the strip: one that sends the prompt, or,
/// while the agent is on a turn (`working`), one that stops it when nothing is
/// written (`empty`) and otherwise one that queues the prompt behind the turn
/// and one that sends it in place of the turn.
fn send_buttons(working: bool, empty: bool) {
    if working && empty {
        round_button(phosphor::STOP, Target::Composer(ComposerEvent::Stop), false);
        return;
    }
    if working {
        button::small_button(
            "Send now",
            Target::Composer(ComposerEvent::SendNow),
            button::Variant::Ghost,
        )
        .with_icon(phosphor::LIGHTNING)
        .build();
    }
    round_button(
        phosphor::ARROW_UP,
        Target::Composer(ComposerEvent::Send),
        !working && empty,
    );
}

/// Emits a button in the prompt box's strip that names a setting's current value, `label`, with `icon`, answering to `id`, and opens `menu` above it, against its end when `end`; shown pressed while `open`.
#[track_caller]
fn chip_menu(
    label: &str,
    icon: impl Into<Option<Icon>>,
    id: Target,
    open: bool,
    end: bool,
    menu: impl FnOnce(),
) {
    let chip = Button::new(label, id)
        .variant(button::Variant::Ghost)
        .size(Size::Xs)
        .selected(open);
    let chip = match icon.into() {
        Some(icon) => chip.with_icon(icon),
        None => chip,
    };
    if end {
        anchored_end(|| chip.build(), open.then_some(menu));
    } else {
        anchored(|| chip.build(), open.then_some(menu));
    }
}

/// Emits the card that asks the reader to sign in to the agent of `session` again, saying `reason`.
fn sign_in_again(state: &WorkspaceState, session: &Session, reason: &str, account: Option<&str>) {
    let agent = state.agent_look(session.agent).name;
    let account = account.or_else(|| {
        session
            .login
            .as_deref()
            .map(|id| state.account_as(session.agent, Some(id)).unwrap_or(id))
    });
    let words = match account {
        Some(account) => format!("Sign in to {agent} again ({account})"),
        None => format!("Sign in to {agent} again"),
    };
    rustle::column_arranged(
        Modifier::new()
            .fill_max_width()
            .panel(Radius::Xl)
            .padding(theme::sp(4)),
        Arrangement::SpacedBy(theme::sp(3)),
        Alignment::Start,
        || {
            rustle::basic_text(
                Modifier::new(),
                words,
                theme::text_style(14.0, 1.5).color(theme::color(Role::Text)),
                TextOverflow::Ellipsis,
            );
            rustle::basic_text(
                Modifier::new(),
                reason,
                theme::text_style(12.0, 1.5).color(theme::color(Role::Muted)),
                TextOverflow::Ellipsis,
            );
            Button::new("Sign in", Target::Chat(ChatEvent::SessionSignIn))
                .modifier(Modifier::new().fill_max_width())
                .build();
        },
    );
}

/// Emits the strip under the prompt: attaching, mentioning, commands, the chips of the settings, the account and the goal, the usage rings and the send buttons.
#[track_caller]
fn strip(state: &WorkspaceState, session: Option<&Session>, rows: u32, above: f32) {
    let working = session.is_some_and(|session| session.status == crate::sample::Status::Working);
    let empty = state.prompt.text().trim().is_empty()
        && state.attachments.is_empty()
        && state.elements.is_empty()
        && state.captures.is_empty();
    let settings = state.settings_now();
    rustle::row_arranged(
        Modifier::new().fill_max_width().padding_each(
            theme::sp(2.5),
            theme::sp(1),
            theme::sp(2.5),
            theme::sp(2.5),
        ),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            let operations = crate::events::operations();
            Button::icon(phosphor::PLUS, Target::Composer(ComposerEvent::Attach))
                .on_click(move || operations.attach())
                .build();
            let (mention, slash) = (
                Target::Composer(ComposerEvent::Mention),
                Target::Composer(ComposerEvent::SlashButton),
            );
            Button::icon(phosphor::AT, mention)
                .on_click(crate::focus::pressing_into(mention, PROMPT))
                .build();
            button::small_button("/", slash, button::Variant::Ghost)
                .monospace()
                .on_click(crate::focus::pressing_into(slash, PROMPT))
                .build();
            for (position, index) in WorkspaceState::chips(settings).into_iter().enumerate() {
                let setting = &settings[index];
                let icon = (setting.category == "mode").then_some(phosphor::LIGHTNING);
                let open = state.picker == Some(Picker::Setting(index));
                let words = setting
                    .current_name()
                    .unwrap_or(&setting.current)
                    .replace(" (recommended)", "");
                rustle::key(index, || {
                    chip_menu(
                        &words,
                        icon,
                        Target::Composer(ComposerEvent::SettingChip(position)),
                        open,
                        false,
                        || setting_menu(state, index, rows, above),
                    );
                });
            }
            if state.on_this_computer() && state.agent_has_accounts(state.agent_now()) {
                chip_menu(
                    state.account().unwrap_or("Account"),
                    phosphor::USER_CIRCLE,
                    Target::Composer(ComposerEvent::Account),
                    state.picker == Some(Picker::Account),
                    false,
                    || account_menu(state),
                );
            }
            if let Some(goal) = session.and_then(|session| session.goal.as_ref()) {
                chip_menu(
                    &goal_label(goal),
                    phosphor::TARGET,
                    Target::Composer(ComposerEvent::GoalChip),
                    state.picker == Some(Picker::Goal),
                    true,
                    || goal_menu(goal),
                );
            }
            spacer();
            match session {
                Some(session) => usage::rings_of(
                    session.context.as_ref(),
                    session
                        .limits
                        .as_ref()
                        .or_else(|| state.allowance_of(session.agent, session.login.as_deref())),
                ),
                None => usage::rings_of(
                    None,
                    state.allowance_of(state.agent, state.login_for_new_work(state.agent)),
                ),
            };
            send_buttons(working, empty);
        },
    );
}

/// Emits the line above the prompt box that says where new work goes, or the chip that continues the chat `session` in a project; nothing when neither applies.
#[track_caller]
fn place(state: &WorkspaceState, session: Option<&Session>) {
    let chat = session.filter(|chat| crate::continue_chat::shows_chip(state, chat));
    let location = session.is_none() && (state.is_repository() || state.offers_container());
    if chat.is_none() && !location {
        return;
    }
    rustle::row_arranged(
        Modifier::new().padding_xy(theme::sp(2.5), 0.0),
        Arrangement::SpacedBy(theme::sp(1)),
        Alignment::Center,
        || {
            if let Some(chat) = chat {
                crate::continue_chat::chip(state, chat);
                return;
            }
            let base = state
                .base
                .as_ref()
                .filter(|base| state.branches.first() != Some(base));
            let mut words = match (state.location, base) {
                (Location::Worktree, Some(base)) => format!("Worktree from {base}"),
                (Location::Worktree, None) => "Worktree".to_owned(),
                (Location::Local, _) => "Local".to_owned(),
            };
            if state.container_for_new_work() {
                words.push_str(" in a dev container");
            }
            let open = state.picker == Some(Picker::Location);
            chip_menu(
                &words,
                phosphor::GIT_FORK,
                Target::Composer(ComposerEvent::Location),
                open,
                false,
                || location_menu(state),
            );
        },
    );
}

/// Emits the prompt box for `state`: a few lines to write in, `placeholder`
/// while empty, at least `rows` lines tall and wrapped at its width, a strip
/// of what the agent is set to and where the work goes, and a send button that
/// stops the turn while there is one; above it, a notice while the agents
/// of other sessions are working in the same checkout as `session`, and the
/// lists of commands and files, whose menus use the `above` pixels free over it.
/// While `session`'s agent is signed out it is a card asking to sign in again,
/// and while a plan waits for a decision the card of that plan stands in for it.
#[track_caller]
pub fn composer(
    state: &WorkspaceState,
    placeholder: &str,
    session: Option<&Session>,
    rows: u32,
    above: f32,
) {
    if let Some(session) = session
        && let Some((reason, account)) = &session.signed_out
    {
        sign_in_again(state, session, reason, account.as_deref());
        return;
    }
    let ask = session.and_then(|session| session.ask.as_ref());
    if let Some(ask) = ask
        && ask.plan.is_some()
    {
        proposal(ask);
        return;
    }
    rustle::column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(theme::sp(1.5)),
        Alignment::Start,
        || {
            if let Some(session) = session {
                crate::shared_checkout::notice(state, session);
            }
            place(state, session);
            crate::popups::anchored(
                Modifier::new().fill_max_width(),
                Placement::Above,
                2.0,
                crate::popups::Dismissal::Never,
                || {
                    rustle::column_arranged(
                        Modifier::new()
                            .fill_max_width()
                            .on_size_changed(|size| {
                                PROMPT_BOX_WIDTH.with(|width| width.set(size.width))
                            })
                            .shadow_layers(&COMPOSER_SHADOW, Radius::Xl.px())
                            .background_rounded(theme::color(Role::Surface), Radius::Xl.px())
                            .focus_outline(
                                crate::focus::is_focused(Target::Composer(ComposerEvent::Prompt)),
                                theme::color(Role::Border),
                                theme::color(Role::Accent),
                                Radius::Xl,
                            )
                            .padding(1.0),
                        Arrangement::Start,
                        Alignment::Start,
                        || {
                            if let Some(ask) = ask {
                                proposal(ask);
                            }
                            attachment_row(state);
                            crate::elements::chips(state);
                            crate::captures::chips(state);
                            Input::new(&state.prompt, Target::Composer(ComposerEvent::Prompt))
                                .scroll(&state.prompt_scroll)
                                .modifier(Modifier::new().fill_max_width())
                                .variant(input::Variant::Multiline)
                                .placeholder(placeholder)
                                .rows(rows)
                                .submits(Target::Composer(ComposerEvent::Send))
                                .hurries(Target::Composer(ComposerEvent::SendNow))
                                .on_paste(attaches_pasted())
                                .build();
                            strip(state, session, rows, above);
                        },
                    );
                },
                lists_shown(state).then_some(|| slash_list(state)),
            );
        },
    );
}
