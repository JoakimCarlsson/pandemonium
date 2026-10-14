//! The prompt box where work is handed to an agent.

use pm_acp::Setting;
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, ShadowLayer, bg, border, border_color, border_t, cursor_pointer, flex, flex_1,
    flex_col, flex_none, flex_wrap, font_mono, gap, h, h_px, items_baseline, items_center,
    justify_between, leading_ratio, max_h, min_w_px, mt, overflow_hidden, p, pb, pr, pt, px, py,
    relative, rounded, shadow_layers, text_color, text_px, truncate, w_full, w_px,
};
use pm_text::Shaper;
use pm_ui::{Node, node, text};

use crate::parts::{line, spacer};
use crate::popover::{anchored, heading, row};
use crate::sample::Session;
use crate::state::{
    ACCOUNT, ACCOUNT_BASE, AGENT_TOGGLE, ATTACH, ATTACHED_BASE, BRANCH_BASE, EFFORT_BASE, LOCATION,
    Location, MENTION, MENU_BODY, MENU_SCROLL, PICK_BASE, PLACE_BASE, PROMPT, Picker, SEND,
    SETTING_BASE, SKILL_BUTTON, SLASH_BASE, SLASH_BUTTON, STOP, WorkspaceState,
};
use crate::{Button, Checkbox, Input, Stepper, button, checkbox, input};
use crate::{Chip, Popover, chip};

/// The shadow under the prompt box: a soft, wide one that lifts it off the page.
const COMPOSER_SHADOW: [ShadowLayer; 1] = [ShadowLayer::new(0.0, 8.0, 30.0, 0.0, 0.18, false)];

/// How many branches the menu of where work is done lists.
const BRANCH_ROWS: usize = 6;

/// How many commands the list behind a slash shows at once.
const SLASH_ROWS: usize = 8;

/// The height in logical pixels of one row of the list of files.
const MENTION_ROW_HEIGHT: f32 = 34.0;

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

/// The menu of one setting: its values in a list that scrolls, and the effort beneath the models.
fn setting_menu(state: &WorkspaceState, index: usize, rows: u32, above: f32) -> Option<Node> {
    let settings = state.settings_now();
    let setting = settings.get(index)?;
    let (title, hint) = match setting.category.as_str() {
        "model" => ("Select a model", None),
        "mode" => ("Modes", Some(("shift+tab", "to switch"))),
        _ => (setting.name.as_str(), None),
    };
    let mut menu = Popover::new().width(MENU_WIDTH).heading(title, hint).child(
        node()
            .styles([
                flex(),
                flex_col(),
                max_h(list_height(settings, above, rows) / 4.0),
                overflow_hidden(),
            ])
            .scroll_from_start(MENU_SCROLL, state.menu_scroll)
            .children(setting.picks.iter().enumerate().map(|(position, pick)| {
                row(
                    &pick.name,
                    &pick.description,
                    pick.value == setting.current,
                    FocusId(PICK_BASE + position as u64),
                )
            })),
    );
    if setting.category == "model"
        && let Some(effort) = settings
            .iter()
            .find(|held| held.category == "thought_level")
    {
        menu = menu.child(effort_row(effort));
    }
    if setting.category == "model"
        && let Some(agent) = settings.iter().find(|held| held.id == "agent")
    {
        menu = menu.child(agent_row(agent));
    }
    Some(menu.over_button(MENU_BODY))
}

/// The menu of where new work is done, and the branch a worktree is made from.
fn location_menu(state: &WorkspaceState) -> Node {
    let worktree = state.location == Location::Worktree;
    let chosen = state.base.as_ref().or(state.branches.first());
    let mut menu = Popover::new()
        .width(MENU_WIDTH)
        .heading("Run in", None)
        .row(
            "Worktree",
            "A new branch and worktree for this task",
            worktree,
            FocusId(PLACE_BASE),
        )
        .row(
            "Local",
            "Work directly in the main checkout",
            !worktree,
            FocusId(PLACE_BASE + 1),
        );
    if worktree {
        menu = menu
            .child(
                node()
                    .styles([border_t(), border_color(Role::Border), mt(1)])
                    .child(heading("Branch from", None)),
            )
            .children(state.branches.iter().take(BRANCH_ROWS).enumerate().map(
                |(position, branch)| {
                    row(
                        branch,
                        "",
                        chosen == Some(branch),
                        FocusId(BRANCH_BASE + position as u64),
                    )
                },
            ));
    }
    menu.over_button(MENU_BODY)
}

/// The menu of the logins of the chosen agent: its own, each one added, and adding another.
fn account_menu(state: &WorkspaceState) -> Node {
    let used = state.login_in_use();
    let logins = state.logins_here();
    let mut menu = Popover::new()
        .width(MENU_WIDTH)
        .heading("Account", None)
        .row(
            state.default_account().unwrap_or("Not signed in"),
            "",
            used.is_none(),
            FocusId(ACCOUNT_BASE),
        );
    for (position, (id, name)) in logins.iter().enumerate() {
        menu = menu.row(
            name,
            "",
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
    .over_button(MENU_BODY)
}

/// The line under the models that sets how hard the agent thinks: its name and a track with a stop for each level.
fn effort_row(effort: &Setting) -> Node {
    let track = Stepper::new(effort.picks.len(), EFFORT_BASE)
        .current(
            effort
                .picks
                .iter()
                .position(|pick| pick.value == effort.current),
        )
        .build();
    labelled(
        "Effort",
        effort.current_name().unwrap_or("Default"),
        track,
        true,
    )
}

/// The line under the models that switches the agent persona on and off.
fn agent_row(agent: &Setting) -> Node {
    let on = agent
        .picks
        .first()
        .is_some_and(|first| first.value != agent.current);
    labelled(
        "Agent",
        if on { "On" } else { "Default" },
        Checkbox::new(on, AGENT_TOGGLE)
            .variant(checkbox::Variant::Switch)
            .build(),
        false,
    )
}

/// A line of a menu's footer: a name with how it is set in brackets, and the control that sets it.
fn labelled(name: &str, value: &str, control: Node, rule: bool) -> Node {
    let mut row = node().styles([
        flex(),
        items_center(),
        justify_between(),
        h(9),
        px(2.5),
        flex_none(),
    ]);
    if rule {
        row = row.styles([border_t(), border_color(Role::Border)]);
    }
    row.child(
        node()
            .styles([flex(), items_center(), gap(1)])
            .child(line(name, 13, 1.4, Role::Text))
            .child(line(format!("({value})"), 13, 1.4, Role::Subtle)),
    )
    .child(control)
}

/// The list of files the word after an `@` being typed narrows.
fn mention_list(state: &WorkspaceState) -> Option<Node> {
    let matches = state.mention_matches();
    if matches.is_empty() {
        state.mention_query()?;
        return Some(
            Popover::new()
                .full_width()
                .heading("Files", None)
                .child(node().styles([px(2.5), py(2)]).child(line(
                    "No files found",
                    13,
                    1.4,
                    Role::Subtle,
                )))
                .above_box(),
        );
    }
    let chosen = state.slash.min(matches.len() - 1);
    Some(
        Popover::new()
            .full_width()
            .heading("Files", None)
            .child(
                node()
                    .styles([
                        flex(),
                        flex_col(),
                        max_h(MENTION_ROW_HEIGHT * SLASH_ROWS as f32 / 4.0),
                        overflow_hidden(),
                    ])
                    .scroll_from_start(MENU_SCROLL, state.menu_scroll)
                    .children(matches.iter().enumerate().map(|(position, path)| {
                        let mut styles = vec![
                            flex(),
                            items_baseline(),
                            px(2.5),
                            py(2),
                            rounded(Radius::Lg),
                            cursor_pointer(),
                            bg(Role::SurfaceHover).hover(),
                        ];
                        if position == chosen {
                            styles.push(bg(Role::SurfaceHover));
                        }
                        node()
                            .styles(styles)
                            .child(
                                node()
                                    .styles([flex_1(), min_w_px(0), overflow_hidden()])
                                    .child(
                                        line(path.as_str(), 13, 1.4, Role::Text)
                                            .styles([font_mono(), truncate()]),
                                    ),
                            )
                            .focusable(FocusId(SLASH_BASE + position as u64))
                    })),
            )
            .above_box(),
    )
}

/// The list of commands the agent offers, which the slash being typed narrows.
fn slash_list(state: &WorkspaceState) -> Option<Node> {
    let matches = state.slash_matches();
    if matches.is_empty() {
        return mention_list(state);
    }
    let chosen = state.slash.min(matches.len() - 1);
    Some(
        Popover::new()
            .full_width()
            .heading(
                if matches[0].0 == '$' {
                    "Skills"
                } else {
                    "Commands"
                },
                None,
            )
            .children(matches.iter().take(SLASH_ROWS).enumerate().map(
                |(position, (prefix, command))| {
                    let mut styles = vec![
                        flex(),
                        items_baseline(),
                        gap(3),
                        px(2.5),
                        py(2),
                        rounded(Radius::Lg),
                        cursor_pointer(),
                        bg(Role::SurfaceHover).hover(),
                    ];
                    if position == chosen {
                        styles.push(bg(Role::SurfaceHover));
                    }
                    node()
                        .styles(styles)
                        .child(text(format!("{prefix}{}", command.name)).styles([
                            text_px(13),
                            leading_ratio(1.4),
                            font_mono(),
                            text_color(Role::Text),
                        ]))
                        .child(
                            node()
                                .styles([flex_1(), min_w_px(0), overflow_hidden()])
                                .child(
                                    line(&command.description, 12, 1.4, Role::Subtle)
                                        .styles([truncate()]),
                                ),
                        )
                        .children((!state.has_skills() && *prefix == '/').then(|| {
                            line(
                                if state.is_skill(&command.name) {
                                    "skill"
                                } else {
                                    "command"
                                },
                                11,
                                1.4,
                                Role::Subtle,
                            )
                        }))
                        .focusable(FocusId(SLASH_BASE + position as u64))
                },
            ))
            .above_box(),
    )
}

/// How far across a thumbnail of an attached picture is, in logical pixels.
const THUMBNAIL: f32 = 28.0;

/// How far across the picture of a file in a sent message is, in logical pixels.
const SENT_PICTURE: f32 = 160.0;

/// The file at `path` as the reader sees it: its picture when it is one and is decoded, and its name.
pub fn attached_file(state: &WorkspaceState, path: &str, side: f32) -> Node {
    let name = path.rsplit('/').next().unwrap_or(path);
    let picture = state.previews.get(path).cloned();
    let mut view = node().styles([flex(), items_center(), gap(2)]);
    if let Some(picture) = picture {
        view = view.child(
            node()
                .styles([w_px(side), h_px(side), flex_none(), rounded(Radius::Md)])
                .picture(picture),
        );
    }
    view.child(line(name, 12, 1.4, Role::Muted))
}

/// The files attached to the prompt, each with a button that takes it off again.
fn attachment_row(state: &WorkspaceState) -> Option<Node> {
    if state.attachments.is_empty() {
        return None;
    }
    Some(
        node()
            .styles([flex(), flex_wrap(), gap(2), px(3), pt(3)])
            .children(state.attachments.iter().enumerate().map(|(index, path)| {
                node()
                    .styles([
                        flex(),
                        items_center(),
                        gap(1.5),
                        p(1),
                        pr(1),
                        rounded(Radius::Lg),
                        bg(Role::SurfaceHover),
                    ])
                    .child(attached_file(state, path, THUMBNAIL))
                    .child(
                        Button::icon(IconName::Close, FocusId(ATTACHED_BASE + index as u64))
                            .build(),
                    )
            })),
    )
}

/// The files sent with a message, each as large as the room beside the words allows.
pub fn sent_files(state: &WorkspaceState, files: &[String]) -> Node {
    node()
        .styles([flex(), flex_col(), gap(2)])
        .children(files.iter().map(|path| {
            match state.previews.get(path) {
                Some(picture) => node()
                    .styles([w_px(SENT_PICTURE), h_px(SENT_PICTURE), rounded(Radius::Lg)])
                    .picture(picture.clone()),
                None => attached_file(state, path, THUMBNAIL),
            }
        }))
}

/// The round button at the end of the strip: it sends the prompt, or stops the turn while there is one.
fn send_button(working: bool, empty: bool) -> Node {
    let (icon, id) = if working {
        (IconName::Stop, STOP)
    } else {
        (IconName::ArrowUp, SEND)
    };
    Button::icon(icon, id)
        .variant(button::Variant::Default)
        .size(button::Size::IconLg)
        .round()
        .disabled(empty && !working)
        .build()
}

/// A button in the prompt box's strip that names a setting's current value and opens its menu.
fn chip(label: &str, icon: Option<IconName>, id: FocusId, open: bool) -> Node {
    let chip = Chip::new(label, id)
        .variant(chip::Variant::Ghost)
        .selected(open);
    match icon {
        Some(icon) => chip.icon(icon),
        None => chip,
    }
    .build()
}

/// The prompt box for `state`: a few lines to write in, a strip of what
/// the agent is set to and where the work goes, and a send button that
/// stops the turn while there is one.
pub fn composer(
    state: &WorkspaceState,
    placeholder: &str,
    session: Option<&Session>,
    rows: u32,
    above: f32,
    text_width: f32,
    shaper: &mut Shaper,
) -> Node {
    let working = session.is_some_and(|session| session.status == crate::sample::Status::Working);
    let empty = state.prompt.text().trim().is_empty() && state.attachments.is_empty();
    let settings = state.settings_now();
    let skills = state.has_skills();
    let placeholder = if skills {
        format!("{placeholder} or $ for skills")
    } else {
        placeholder.to_owned()
    };
    let mut strip = node()
        .styles([flex(), items_center(), gap(1), px(2.5), pt(1), pb(2.5)])
        .child(Button::icon(IconName::Plus, ATTACH).build())
        .child(Button::icon(IconName::At, MENTION).build())
        .child(
            Button::new("/", SLASH_BUTTON)
                .size(button::Size::Icon)
                .variant(button::Variant::Ghost)
                .monospace()
                .build(),
        );
    if skills {
        strip = strip.child(
            Button::new("$", SKILL_BUTTON)
                .size(button::Size::Icon)
                .variant(button::Variant::Ghost)
                .monospace()
                .build(),
        );
    }
    for (position, index) in WorkspaceState::chips(settings).into_iter().enumerate() {
        let setting = &settings[index];
        let icon = (setting.category == "mode").then_some(IconName::Zap);
        let open = state.picker == Some(Picker::Setting(index));
        strip = strip.child(anchored(
            chip(
                setting
                    .current_name()
                    .unwrap_or(&setting.current)
                    .replace(" (recommended)", "")
                    .as_str(),
                icon,
                FocusId(SETTING_BASE + position as u64),
                open,
            ),
            open.then(|| setting_menu(state, index, rows, above))
                .flatten(),
        ));
    }
    if session.is_none() && state.is_repository() {
        let base = state
            .base
            .as_ref()
            .filter(|base| state.branches.first() != Some(base));
        let label = match (state.location, base) {
            (Location::Worktree, Some(base)) => format!("Worktree from {base}"),
            (Location::Worktree, None) => "Worktree".to_owned(),
            (Location::Local, _) => "Local".to_owned(),
        };
        let open = state.picker == Some(Picker::Location);
        strip = strip.child(anchored(
            chip(&label, Some(IconName::GitFork), LOCATION, open),
            open.then(|| location_menu(state)),
        ));
    }
    if state.on_this_computer()
        && pm_acp::AGENTS
            .get(state.agent)
            .is_some_and(|agent| agent.has_accounts())
    {
        let account = state.account().unwrap_or("Account");
        let open = state.picker == Some(Picker::Account);
        let label = account.to_owned();
        strip = strip.child(anchored(
            chip(&label, Some(IconName::CircleUser), ACCOUNT, open),
            open.then(|| account_menu(state)),
        ));
    }
    let strip = strip.child(spacer()).child(send_button(working, empty));
    node()
        .styles([
            relative(),
            flex(),
            flex_col(),
            w_full(),
            rounded(Radius::Xl),
            border(),
            border_color(Role::Border),
            border_color(Role::Accent).focus(),
            bg(Role::Surface),
            shadow_layers(&COMPOSER_SHADOW),
        ])
        .focused_with(PROMPT)
        .children(slash_list(state))
        .children(attachment_row(state))
        .child(
            Input::new(&state.prompt, PROMPT)
                .variant(input::Variant::Multiline)
                .placeholder(&placeholder)
                .rows(rows)
                .wrap(text_width, shaper)
                .build(),
        )
        .child(strip)
}
