//! The prompt box where work is handed to an agent.

use std::cell::Cell;

use pm_harness::Setting;
use rustle::{CursorIcon, Modifier};

use crate::Popover;
use crate::choices::{LIST_ROW_HEIGHT, LIST_ROWS, PRESSABLE_LIST_ROWS};
use crate::kit::{
    Flex, FocusId, Hovered, IconName, Interactive, Looks, Placement, Radius, Role, Scroll, Side,
    Styled, Type, block, column, label, label_with, picture, row, scroll_area, sp, wrapping,
};
use crate::parts::{line, spacer};
use crate::plan::proposal;
use crate::popover::{anchored, anchored_end, heading};
use crate::sample::Session;
use crate::size::Size;
use crate::state::{
    ACCOUNT, ACCOUNT_BASE, AGENT_TOGGLE, ATTACH, ATTACHED_BASE, BRANCH_BASE, EFFORT_BASE,
    GOAL_BASE, GOAL_CHIP, LOCATION, Location, MENTION, MENU_BODY, MENU_FILTER, MENU_PICK_BASE,
    MENU_SCROLL, ORCHESTRATE, PLACE_BASE, PROMPT, Picker, SEND, SEND_NOW, SETTING_BASE, SLASH_BASE,
    SLASH_BUTTON, STOP, WorkspaceState,
};
use crate::usage;
use crate::{Button, Checkbox, Input, Stepper, button, checkbox, input};

/// The shadow under the prompt box: a soft, wide one that lifts it off the page, as `(x, y, blur, spread, alpha)`.
const COMPOSER_SHADOW: [(f32, f32, f32, f32, f32); 1] = [(0.0, 8.0, 30.0, 0.0, 0.18)];

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

/// The modifier of a section of a menu set off from what is above it by a rule: `border_t()` with `mt(1)`.
fn ruled() -> Modifier {
    Modifier::new()
        .pad_side(Side::Top, 1)
        .stroke_side(Side::Top, 1.0, Role::Border)
}

/// Emits the menu of the setting at `index`: its values in a list that scrolls, and the effort beneath the models; nothing when there is no such setting.
#[track_caller]
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
        .modifier(Modifier::new().target(MENU_BODY))
        .build_with(|| {
            if filters {
                block(Modifier::new().pad_x(1).pad_side(Side::Bottom, 1), || {
                    Input::search(&state.menu_filter, MENU_FILTER)
                        .size(Size::Sm)
                        .placeholder(match setting.category.as_str() {
                            "model" => "Search models",
                            _ => "Search",
                        })
                        .build();
                });
            }
            scroll_area(
                Modifier::new().max_h_px(sp(list_height(settings, above, rows) / 4.0)),
                Scroll::from_start(MENU_SCROLL, state.menu_scroll),
                || {
                    column(Modifier::new().fill_max_width(), Flex::new(), || {
                        for position in &shown {
                            let Some(pick) = setting.picks.get(*position) else {
                                continue;
                            };
                            crate::popover::row(
                                &pick.name,
                                &pick.description,
                                pick.value == setting.current,
                                FocusId(MENU_PICK_BASE + *position as u64),
                            );
                        }
                        if shown.is_empty() {
                            block(Modifier::new().pad_x(2).pad_y(2), || {
                                line("Nothing matches", 13, 1.4, Role::Muted);
                            });
                        }
                    });
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
                orchestrate_row(on);
            }
        });
}

/// Emits the row at the foot of the model menu that switches the conversation in and out of orchestrator mode, ticked while it is `on`.
#[track_caller]
fn orchestrate_row(on: bool) {
    block(ruled(), || {
        crate::popover::row(
            "Orchestrate",
            "Lead, and hand work to subagents by category",
            on,
            ORCHESTRATE,
        );
    });
}

/// Emits the menu of where new work is done, and the branch a worktree is made from.
#[track_caller]
fn location_menu(state: &WorkspaceState) {
    let worktree = state.location == Location::Worktree;
    let mut menu = Popover::new().width(MENU_WIDTH).heading("Run in", None);
    if state.is_repository() {
        menu = menu
            .row(
                "Worktree",
                "A worktree of its own on a detached head",
                worktree,
                FocusId(PLACE_BASE),
            )
            .row(
                "Local",
                "Work directly in the main checkout",
                !worktree,
                FocusId(PLACE_BASE + 1),
            );
    }
    menu.modifier(Modifier::new().target(MENU_BODY))
        .build_with(|| {
            if state.offers_container() {
                block(ruled(), || {
                    crate::popover::row(
                        "Dev container",
                        "Not a security boundary",
                        state.container,
                        FocusId(PLACE_BASE + 2),
                    );
                });
            }
            if worktree {
                block(ruled(), || heading("Branch from", None));
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
            FocusId(BRANCH_BASE + position as u64),
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
#[track_caller]
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
            FocusId(ACCOUNT_BASE),
        );
    for (position, (id, name)) in logins.iter().enumerate() {
        menu = menu.row(
            name,
            &spoken(state, Some(id)),
            used == Some(*id),
            FocusId(ACCOUNT_BASE + 1 + position as u64),
        );
    }
    menu.row(
        "Add account",
        "Sign in to another login",
        false,
        FocusId(ACCOUNT_BASE + 1 + logins.len() as u64),
    )
    .build_target(MENU_BODY);
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
#[track_caller]
fn goal_menu(goal: &pm_harness::Goal) {
    use pm_harness::GoalState;
    let mut menu = Popover::new().width(MENU_WIDTH).heading("Goal", None).row(
        &goal.objective,
        goal.note.as_deref().unwrap_or(""),
        false,
        MENU_BODY,
    );
    if goal.state == GoalState::Active {
        menu = menu.row(
            "Pause",
            "Finish this turn and start no more",
            false,
            FocusId(GOAL_BASE),
        );
    }
    if goal.state == GoalState::Paused {
        menu = menu.row(
            "Resume",
            "Go on working toward it",
            false,
            FocusId(GOAL_BASE + 1),
        );
    }
    menu.row("Clear", "Forget the goal", false, FocusId(GOAL_BASE + 2))
        .build_target(MENU_BODY);
}

/// Emits the line under the models that sets how hard the agent thinks: its name and a track with a stop for each level.
#[track_caller]
fn effort_row(effort: &Setting) {
    let current = effort
        .picks
        .iter()
        .position(|pick| pick.value == effort.current);
    let stops = effort.picks.len();
    labelled(
        "Effort",
        effort.current_name().unwrap_or("Default"),
        || Stepper::new(stops, EFFORT_BASE).current(current).build(),
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
            Checkbox::new(on, AGENT_TOGGLE)
                .variant(checkbox::Variant::Switch)
                .build();
        },
        false,
    );
}

/// Emits a line of a menu's footer: `name` with how it is set, `value`, in brackets, then what `control` emits to set it, under a rule when `rule`.
#[track_caller]
fn labelled(name: &str, value: &str, control: impl FnOnce(), rule: bool) {
    let mut own = Modifier::new().h(9);
    if rule {
        own = own.stroke_side(Side::Top, 1.0, Role::Border);
    }
    row(own.pad_x(2.5), Flex::new().center().between(), || {
        row(Modifier::new(), Flex::new().center().gap(1), || {
            line(name, 13, 1.4, Role::Text);
            line(format!("({value})"), 13, 1.4, Role::Subtle);
        });
        control();
    });
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
fn list_row_looks(chosen: bool) -> Looks {
    let looks = Looks::new().hover(Role::SurfaceHover).radius(Radius::Lg);
    if chosen {
        looks.idle(Role::SurfaceHover)
    } else {
        looks
    }
}

/// Emits the list of files the word after an `@` being typed narrows; nothing when no `@` word is being typed.
#[track_caller]
fn mention_list(state: &WorkspaceState) {
    let matches = state.mention_matches();
    if matches.is_empty() {
        if state.mention_query().is_none() {
            return;
        }
        Popover::new()
            .full_width()
            .heading("Files", None)
            .modifier(above_box())
            .build_with(|| {
                block(Modifier::new().pad_x(2.5).pad_y(2), || {
                    line("No files found", 13, 1.4, Role::Subtle);
                });
            });
        return;
    }
    let chosen = state.slash.min(matches.len() - 1);
    Popover::new()
        .full_width()
        .heading("Files", None)
        .modifier(above_box())
        .build_with(|| {
            scroll_area(
                Modifier::new().max_h_px(sp(LIST_ROW_HEIGHT * LIST_ROWS as f32 / 4.0)),
                Scroll::from_start(MENU_SCROLL, state.menu_scroll),
                || {
                    column(Modifier::new().fill_max_width(), Flex::new(), || {
                        for (position, path) in matches.iter().enumerate() {
                            let id = FocusId(SLASH_BASE + position as u64);
                            row(
                                Modifier::new()
                                    .looks_of(list_row_looks(position == chosen), id)
                                    .cursor(CursorIcon::Pointer)
                                    .target(id)
                                    .pad_x(2.5)
                                    .pad_y(2),
                                Flex::new().center(),
                                || {
                                    label_with(
                                        Modifier::new().grow(),
                                        path.as_str(),
                                        Type::new(13).ratio(1.4).tone(Role::Text).mono().truncate(),
                                    );
                                },
                            );
                        }
                    });
                },
            );
        });
}

/// Emits the list of commands the agent offers, which the slash being typed narrows, or else the list of files; nothing when neither is being typed.
#[track_caller]
fn slash_list(state: &WorkspaceState) {
    let matches = state.slash_matches();
    if matches.is_empty() {
        mention_list(state);
        return;
    }
    let chosen = state.slash.min(matches.len() - 1);
    Popover::new()
        .full_width()
        .heading("Commands", None)
        .modifier(above_box())
        .build_with(|| {
            scroll_area(
                Modifier::new().max_h_px(sp(LIST_ROW_HEIGHT * LIST_ROWS as f32 / 4.0)),
                Scroll::from_start(MENU_SCROLL, state.menu_scroll),
                || {
                    column(Modifier::new().fill_max_width(), Flex::new(), || {
                        for (position, command) in matches.iter().enumerate() {
                            let looks = list_row_looks(position == chosen);
                            let own = if position < PRESSABLE_LIST_ROWS {
                                let id = FocusId(SLASH_BASE + position as u64);
                                Modifier::new()
                                    .looks_of(looks, id)
                                    .cursor(CursorIcon::Pointer)
                                    .target(id)
                            } else {
                                Modifier::new().looks(looks).cursor(CursorIcon::Pointer)
                            };
                            row(own.pad_x(2.5).pad_y(2), Flex::new().center().gap(3), || {
                                label(
                                    format!("/{}", command.name),
                                    Type::new(13).ratio(1.4).mono().tone(Role::Text),
                                );
                                label_with(
                                    Modifier::new().grow(),
                                    &command.description,
                                    Type::new(12).ratio(1.4).tone(Role::Subtle).truncate(),
                                );
                            });
                        }
                    });
                },
            );
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
    row(Modifier::new(), Flex::new().center().gap(2), || {
        if let Some(shown) = shown {
            picture(
                Modifier::new().width(side).height(side).clip_to(Radius::Md),
                &shown,
            );
        }
        line(name, 12, 1.4, Role::Muted);
    });
}

/// Emits the files attached to the prompt, each with a button that takes it off again; nothing when none is attached.
#[track_caller]
fn attachment_row(state: &WorkspaceState) {
    if state.attachments.is_empty() {
        return;
    }
    wrapping(Modifier::new().pad_x(3).pad_side(Side::Top, 3), 2, || {
        for (index, path) in state.attachments.iter().enumerate() {
            rustle::key(index, || {
                row(
                    Modifier::new()
                        .fill_rounded(Role::SurfaceHover, Radius::Lg)
                        .pad(1),
                    Flex::new().center().gap(1.5),
                    || {
                        attached_file(state, path, THUMBNAIL);
                        Button::icon(IconName::Close, FocusId(ATTACHED_BASE + index as u64))
                            .build();
                    },
                );
            });
        }
    });
}

/// Emits the files sent with a message, each as large as the room beside the words allows.
#[track_caller]
pub fn sent_files(state: &WorkspaceState, files: &[String]) {
    column(Modifier::new(), Flex::new().gap(2), || {
        for path in files {
            match state.previews.get(path) {
                Some(shown) => picture(
                    Modifier::new()
                        .width(SENT_PICTURE)
                        .height(SENT_PICTURE)
                        .clip_to(Radius::Lg),
                    shown,
                ),
                None => attached_file(state, path, THUMBNAIL),
            }
        }
    });
}

/// Emits the round button at the end of the strip, answering to `id` with `icon`, switched off while `disabled`.
#[track_caller]
fn round_button(icon: IconName, id: FocusId, disabled: bool) {
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
#[track_caller]
fn send_buttons(working: bool, empty: bool) {
    match (working, empty) {
        (true, true) => round_button(IconName::Stop, STOP, false),
        (true, false) => {
            Button::new("Send now", SEND_NOW)
                .variant(button::Variant::Ghost)
                .size(Size::Sm)
                .with_icon(IconName::Zap)
                .build();
            round_button(IconName::ArrowUp, SEND, false);
        }
        (false, _) => round_button(IconName::ArrowUp, SEND, empty),
    }
}

/// Emits a button in the prompt box's strip that names a setting's current value, `label`, with `icon`, answering to `id`, and opens its menu; shown pressed while `open`.
#[track_caller]
fn chip(label: &str, icon: Option<IconName>, id: FocusId, open: bool) {
    let chip = Button::new(label, id)
        .variant(button::Variant::Ghost)
        .size(Size::Xs)
        .selected(open);
    match icon {
        Some(icon) => chip.with_icon(icon),
        None => chip,
    }
    .build();
}

/// Emits the card that asks the reader to sign in to the agent of `session` again, saying `reason`.
#[track_caller]
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
    column(
        Modifier::new()
            .fill_rounded(Role::Surface, Radius::Xl)
            .stroke(Role::Border, Radius::Xl)
            .pad(4),
        Flex::new().gap(3),
        || {
            label(words, Type::new(14.0).tone(Role::Text));
            label(reason, Type::new(12.0).tone(Role::Muted));
            Button::new("Sign in", crate::state::SESSION_SIGN_IN).build();
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
    row(
        Modifier::new()
            .pad_x(2.5)
            .pad_side(Side::Top, 1)
            .pad_side(Side::Bottom, 2.5),
        Flex::new().center().gap(1),
        || {
            Button::icon(IconName::Plus, ATTACH).build();
            Button::icon(IconName::At, MENTION).build();
            Button::new("/", SLASH_BUTTON)
                .size(Size::Sm)
                .variant(button::Variant::Ghost)
                .monospace()
                .build();
            for (position, index) in WorkspaceState::chips(settings).into_iter().enumerate() {
                let setting = &settings[index];
                let icon = (setting.category == "mode").then_some(IconName::Zap);
                let open = state.picker == Some(Picker::Setting(index));
                let words = setting
                    .current_name()
                    .unwrap_or(&setting.current)
                    .replace(" (recommended)", "");
                rustle::key(index, || {
                    anchored(
                        || chip(&words, icon, FocusId(SETTING_BASE + position as u64), open),
                        open.then_some(|| setting_menu(state, index, rows, above)),
                    );
                });
            }
            if state.on_this_computer() && state.agent_has_accounts(state.agent_now()) {
                let account = state.account().unwrap_or("Account");
                let open = state.picker == Some(Picker::Account);
                anchored(
                    || chip(account, Some(IconName::CircleUser), ACCOUNT, open),
                    open.then_some(|| account_menu(state)),
                );
            }
            if let Some(goal) = session.and_then(|session| session.goal.as_ref()) {
                let open = state.picker == Some(Picker::Goal);
                anchored_end(
                    || chip(&goal_label(goal), Some(IconName::Target), GOAL_CHIP, open),
                    open.then_some(|| goal_menu(goal)),
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
    row(
        Modifier::new().pad_x(2.5),
        Flex::new().center().gap(1),
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
            anchored(
                || chip(&words, Some(IconName::GitFork), LOCATION, open),
                open.then_some(|| location_menu(state)),
            );
        },
    );
}

/// Emits the prompt box for `state`: a few lines to write in, `placeholder`
/// while empty, at least `rows` lines tall and wrapped to `text_width`, a strip
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
    text_width: f32,
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
    column(
        Modifier::new().fill_max_width(),
        Flex::new().gap(1.5),
        || {
            if let Some(session) = session {
                crate::shared_checkout::notice(state, session);
            }
            place(state, session);
            crate::kit::anchored(
                Modifier::new().fill_max_width(),
                Placement::Above,
                2.0,
                || {
                    column(
                        Modifier::new()
                            .fill_max_width()
                            .on_size_changed(|size| {
                                PROMPT_BOX_WIDTH.with(|width| width.set(size.width))
                            })
                            .shadows(&COMPOSER_SHADOW, Radius::Xl)
                            .fill_rounded(Role::Surface, Radius::Xl)
                            .stroke_by_focus(PROMPT, Role::Border, Role::Accent, Radius::Xl)
                            .focus_of(PROMPT),
                        Flex::new(),
                        || {
                            if let Some(ask) = ask {
                                proposal(ask);
                            }
                            attachment_row(state);
                            crate::elements::chips(state);
                            crate::captures::chips(state);
                            Input::new(&state.prompt, PROMPT)
                                .variant(input::Variant::Multiline)
                                .placeholder(placeholder)
                                .rows(rows)
                                .wrap(text_width)
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
