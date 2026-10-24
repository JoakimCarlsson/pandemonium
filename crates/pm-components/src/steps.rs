//! What an agent did on the way to a reply: the steps it took, its plan and its questions.

use pm_acp::{Diff, Kind as ToolKind, Permission, PlanStep, Progress, ToolState, Verdict};
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Radius, Role, animate_spin, bg, border, border_b, border_color, border_l, cursor_pointer, flex,
    flex_1, flex_col, flex_none, flex_wrap, font_mono, gap, h, items_center, justify_center,
    leading_ratio, line_through, max_h, min_h, min_w_px, my, overflow_hidden, pb, pl, pr, px, py,
    rounded, rounded_full, size, text_color, text_px, truncate, w_full,
};
use pm_text::Shaper;
use pm_ui::{Node, node, text};

use crate::Button;
use crate::Tile;
use crate::button::Variant;
use crate::diff::diff_line;
use crate::diff_syntax::DiffColours;
use crate::file_icon::{file_icon, file_language};
use crate::parts::{clipped, glyph, line, wrap};
use crate::sample::{DiffLine, Kind, Step, ToolUse};
use crate::size::Size;
use crate::state::{CHOICE_BASE, STEP_BASE};
use crate::{CodeBox, Tally};

/// How many unchanged lines an edit shows around what changed.
const CONTEXT: usize = 2;

/// How tall the detail of a step is at most, in logical pixels.
const DETAIL_HEIGHT: f32 = 280.0;

/// `path` without the folder `root` it is under.
fn relative<'a>(path: &'a str, root: &str) -> &'a str {
    path.strip_prefix(root)
        .map(|rest| rest.trim_start_matches('/'))
        .filter(|rest| !rest.is_empty())
        .unwrap_or(path)
}

/// The lines an edit comes to, with a little of what stayed the same around them.
fn changed_lines(diff: &Diff) -> Vec<DiffLine> {
    let old: Vec<&str> = diff.old.as_deref().unwrap_or_default().lines().collect();
    let new: Vec<&str> = diff.new.lines().collect();
    let head = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let line = |kind, text: &str| DiffLine {
        kind,
        text: text.to_owned(),
    };
    let mut lines = Vec::new();
    for context in &old[head.saturating_sub(CONTEXT)..head] {
        lines.push(line(Kind::Context, context));
    }
    for removed in &old[head..old.len() - tail] {
        lines.push(line(Kind::Remove, removed));
    }
    for added in &new[head..new.len() - tail] {
        lines.push(line(Kind::Add, added));
    }
    for context in old[old.len() - tail..].iter().take(CONTEXT) {
        lines.push(line(Kind::Context, context));
    }
    lines
}

/// One line of the steps: a mark for how it went, what was done and to what, and what came of it.
fn step_line(
    mark: Node,
    verb: &str,
    target: Option<&str>,
    detail: Option<Node>,
    id: Option<FocusId>,
    open: bool,
) -> Node {
    let mut row = node()
        .styles([
            flex(),
            items_center(),
            gap(2.5),
            w_full(),
            min_h(8),
            min_w_px(0),
        ])
        .child(
            node()
                .styles([flex(), items_center(), size(4), flex_none()])
                .child(mark),
        )
        .child(line(verb, 13, 1.4, Role::Muted).styles([flex_none()]));
    if let Some(target) = target {
        row = row.child(clipped_mono(target, verb == "Ran"));
    } else {
        row = row.child(node().styles([flex_1()]));
    }
    row = row.children(detail);
    if let Some(id) = id {
        row = row
            .child(glyph(
                if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                },
                3.5,
                Role::Subtle,
            ))
            .styles([cursor_pointer()])
            .focusable(id);
    }
    row
}

/// A box that takes the room left in its row and cuts a line of code short at its edge, on a chip when `chip` is set.
fn clipped_mono(content: &str, chip: bool) -> Node {
    let code = text(content.to_owned()).styles([
        text_px(12),
        leading_ratio(1.4),
        font_mono(),
        text_color(Role::Text),
        pm_style::truncate(),
    ]);
    let holder = node().styles([flex(), min_w_px(0), overflow_hidden()]);
    node().styles([flex(), flex_1(), min_w_px(0)]).child(
        match chip {
            true => holder.styles([px(1.5), rounded(Radius::Md), bg(Role::Surface)]),
            false => holder,
        }
        .child(code),
    )
}

/// The mark of how a tool got on: a turning ring, or the glyph that is its kind.
fn tool_mark(tool: &ToolUse) -> Node {
    match tool.state {
        ToolState::Running => glyph(IconName::Loading, 3.5, Role::Muted).styles([animate_spin()]),
        ToolState::Failed => glyph(IconName::Close, 3.5, Role::Danger),
        ToolState::Done => match tool.kind {
            ToolKind::Read | ToolKind::Edit => {
                let path = tool
                    .target
                    .as_deref()
                    .or(tool.diff.as_ref().map(|diff| diff.path.as_str()))
                    .unwrap_or_default();
                file_icon(path, 4.0)
            }
            ToolKind::Search => glyph(IconName::Search, 4.0, Role::Subtle),
            ToolKind::Execute => glyph(IconName::Terminal, 4.0, Role::Subtle),
            ToolKind::Fetch | ToolKind::Other => glyph(IconName::Check, 4.0, Role::Subtle),
        },
    }
}

/// The detail under a tool's line, when it has some and it is open.
fn tool_detail(tool: &ToolUse) -> Option<Node> {
    if let Some(diff) = &tool.diff {
        let lines = changed_lines(diff);
        let colours = DiffColours::of(&lines, file_language(&diff.path).as_deref());
        return Some(
            CodeBox::new(
                lines
                    .iter()
                    .enumerate()
                    .map(|(at, line)| diff_line(line.kind, &line.text, colours.line(at))),
            )
            .max_height(DETAIL_HEIGHT)
            .build(),
        );
    }
    let output = tool.output.as_deref().filter(|output| !output.is_empty())?;
    Some(
        CodeBox::new(
            output
                .lines()
                .filter(|printed| !printed.starts_with("```"))
                .take(40)
                .map(|printed| {
                    node().styles([flex(), px(3.5)]).child(
                        text(printed.replace('\t', "    ")).styles([
                            text_px(12),
                            leading_ratio(1.6),
                            font_mono(),
                            text_color(Role::Muted),
                        ]),
                    )
                }),
        )
        .max_height(DETAIL_HEIGHT)
        .build(),
    )
}

/// One tool of the steps.
fn tool_step(tool: &ToolUse, root: &str, id: FocusId, open: bool) -> Node {
    tool_line(tool, root, id, open, true)
}

/// One tool of the steps, folding out its detail only when `foldable`.
fn tool_line(tool: &ToolUse, root: &str, id: FocusId, open: bool, foldable: bool) -> Node {
    let target = tool.target.as_deref().map(|target| relative(target, root));
    let (verb, shown, detail) = match tool.kind {
        ToolKind::Read => ("Read", target, None),
        ToolKind::Search => ("Searched for", target, None),
        ToolKind::Edit => (
            "Edited",
            target.or(tool.diff.as_ref().map(|diff| relative(&diff.path, root))),
            tool.diff.as_ref().map(|diff| {
                let changes = changed_lines(diff);
                let count = |wanted| changes.iter().filter(|line| line.kind == wanted).count();
                Tally::new(count(Kind::Add), count(Kind::Remove))
                    .size(11)
                    .build()
            }),
        ),
        ToolKind::Execute => (
            "Ran",
            target,
            match (tool.seconds, tool.state) {
                (Some(seconds), state) => {
                    let (word, tone) = if state == ToolState::Failed {
                        ("failed", Role::Danger)
                    } else {
                        ("passed", Role::Success)
                    };
                    Some(line(format!("{word} · {seconds}s"), 11, 1.4, tone).styles([flex_none()]))
                }
                (None, ToolState::Running) => Some(
                    line(
                        format!("running · {}", elapsed(tool.started.elapsed().as_secs())),
                        11,
                        1.4,
                        Role::Muted,
                    )
                    .styles([flex_none()]),
                ),
                (None, _) => None,
            },
        ),
        ToolKind::Fetch | ToolKind::Other => (tool.title.as_str(), None, None),
    };
    let expandable = foldable && tool_detail(tool).is_some();
    let mut step = node()
        .styles([flex(), flex_col(), w_full()])
        .child(step_line(
            tool_mark(tool),
            verb,
            shown,
            detail,
            expandable.then_some(id),
            open,
        ));
    if open && let Some(detail) = tool_detail(tool) {
        step = step.child(detail);
    }
    step
}

/// How long something has gone on, in words: `42s` or `3m 05s`.
pub fn elapsed(seconds: u64) -> String {
    if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

/// The mark of how a subagent is getting on: a turning ring, a dot once it is done, or a cross.
fn agent_mark(state: ToolState) -> Node {
    let slot = node().styles([flex(), items_center(), size(4), flex_none()]);
    slot.child(match state {
        ToolState::Running => glyph(IconName::Loading, 3.5, Role::Muted).styles([animate_spin()]),
        ToolState::Done => node().styles([size(1.5), rounded_full(), bg(Role::Subtle)]),
        ToolState::Failed => glyph(IconName::Close, 3.5, Role::Danger),
    })
}

/// The steps of `all` that are the work of the subagent `parent`.
fn beneath<'a>(all: &'a [Step], parent: &str) -> Vec<&'a ToolUse> {
    all.iter()
        .filter_map(|step| match step {
            Step::Tool(tool) if tool.parent.as_deref() == Some(parent) => Some(tool),
            _ => None,
        })
        .collect()
}

/// Whether the step at `position` of `all` is a tool of a subagent that the steps hold, which its card shows.
pub fn is_inside_card(all: &[Step], position: usize) -> bool {
    let Step::Tool(tool) = &all[position] else {
        return false;
    };
    tool.parent.as_deref().is_some_and(|parent| {
        all.iter()
            .any(|step| matches!(step, Step::Tool(held) if held.id == parent && held.is_subagent()))
    })
}

/// The card of subagents: how many there are and how many are at work, then a row for each that folds out the tools it used.
///
/// `scroll` bounds the rows to 240 logical pixels and scrolls them independently.
/// `at` are the positions in `all`, the steps of the reply at `message`, of the tools that start them.
pub fn subagents(
    all: &[Step],
    at: &[usize],
    message: usize,
    opened: &dyn Fn(usize) -> bool,
    root: &str,
    scroll: Option<(FocusId, f32)>,
) -> Node {
    let agents: Vec<(usize, &ToolUse)> = at
        .iter()
        .filter_map(|position| match &all[*position] {
            Step::Tool(tool) => Some((*position, tool)),
            _ => None,
        })
        .collect();
    let working = agents
        .iter()
        .filter(|(_, tool)| tool.is_working_agent())
        .count();
    let title = match (agents.len(), working) {
        (1, 0) => "1 subagent".to_owned(),
        (count, 0) => format!("{count} subagents"),
        (1, _) => "1 subagent · running".to_owned(),
        (count, working) => format!("{count} subagents · {working} running"),
    };
    let header = node()
        .styles([
            flex(),
            items_center(),
            gap(2.5),
            px(3.5),
            h(10),
            border_b(),
            border_color(Role::Border),
        ])
        .child(glyph(IconName::GitFork, 4.0, Role::Muted))
        .child(line(title, 13, 1.4, Role::Text));
    let rows = agents.into_iter().map(|(position, tool)| {
        let used = beneath(all, &tool.id);
        let open = opened(position);
        let id = FocusId(STEP_BASE + (message * 1000 + position) as u64);
        let described = tool.target.as_deref().unwrap_or(&tool.title);
        let mut row = node().styles([flex(), flex_col(), w_full()]).child(
            Tile::new(id)
                .height(10.0)
                .gap(2.5)
                .radius(Radius::Xs)
                .build()
                .styles([px(3.5), min_w_px(0)])
                .child(agent_mark(tool.state))
                .children((tool.agent.as_deref() != Some("agent")).then(|| {
                    node()
                        .styles([
                            flex(),
                            flex_none(),
                            px(1.5),
                            rounded(Radius::Md),
                            bg(Role::SurfaceHover),
                        ])
                        .child(text(tool.agent.clone().unwrap_or_default()).styles([
                            text_px(12),
                            leading_ratio(1.4),
                            font_mono(),
                            text_color(Role::Muted),
                        ]))
                }))
                .child(clipped(described, 13, Role::Text))
                .child(
                    line(
                        match used.len() {
                            1 => "1 tool".to_owned(),
                            count => format!("{count} tools"),
                        },
                        12,
                        1.4,
                        Role::Subtle,
                    )
                    .styles([flex_none()]),
                )
                .child(glyph(
                    if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    },
                    3.5,
                    Role::Subtle,
                )),
        );
        if open && !used.is_empty() {
            row = row.child(
                node()
                    .styles([flex(), flex_col(), gap(1), pl(10), pr(3.5), pb(2)])
                    .children(
                        used.into_iter()
                            .map(|tool| tool_line(tool, root, FocusId(0), false, false)),
                    ),
            );
        }
        row
    });
    let mut body = node().styles([flex(), flex_col()]).children(rows);
    if let Some((id, offset)) = scroll {
        body = body
            .styles([max_h(60), overflow_hidden()])
            .scroll_from_start(id, offset);
    }
    node()
        .styles([
            flex(),
            flex_col(),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
            overflow_hidden(),
        ])
        .child(header.styles([flex_none()]))
        .child(body)
}

/// The mark in front of a plan step: a filled check when done, a spinner while under way, an empty ring when not begun.
fn plan_mark(progress: Progress) -> Node {
    match progress {
        Progress::Done => node()
            .styles([
                flex(),
                items_center(),
                justify_center(),
                size(4),
                rounded_full(),
                bg(Role::Success),
            ])
            .child(glyph(IconName::Check, 3.0, Role::OnSuccess)),
        Progress::Doing => glyph(IconName::Loading, 4.0, Role::Muted).styles([animate_spin()]),
        Progress::Pending => node().styles([
            size(4),
            rounded_full(),
            border(),
            border_color(Role::Subtle),
        ]),
    }
}

/// The agent's plan as a card: a header saying how far it has got over a checklist that strikes through what is done.
pub fn plan(items: &[PlanStep]) -> Node {
    let done = items
        .iter()
        .filter(|item| item.progress == Progress::Done)
        .count();
    let finished = done == items.len();
    let header = node()
        .styles([
            flex(),
            items_center(),
            gap(3),
            px(4),
            h(12),
            border_b(),
            border_color(Role::Border),
        ])
        .child(glyph(
            IconName::CheckCircle,
            4.5,
            if finished { Role::Success } else { Role::Muted },
        ))
        .child(line(
            format!("Plan · {done} of {} done", items.len()),
            14,
            1.4,
            Role::Text,
        ));
    let rows = node()
        .styles([flex(), flex_col(), gap(3), px(4), py(3.5)])
        .children(items.iter().map(|item| {
            let (tone, struck) = match item.progress {
                Progress::Done => (Role::Subtle, true),
                Progress::Doing => (Role::Text, false),
                Progress::Pending => (Role::Muted, false),
            };
            let label = line(&item.text, 13, 1.4, tone).styles([truncate()]);
            node()
                .styles([flex(), items_center(), gap(3)])
                .child(
                    node()
                        .styles([flex(), items_center(), size(4), flex_none()])
                        .child(plan_mark(item.progress)),
                )
                .child(
                    node()
                        .styles([flex_1(), min_w_px(0), overflow_hidden()])
                        .child(if struck {
                            label.styles([line_through()])
                        } else {
                            label
                        }),
                )
        }));
    node()
        .styles([
            flex(),
            flex_col(),
            rounded(Radius::Xl),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
            overflow_hidden(),
        ])
        .child(header)
        .child(rows)
}

/// The steps of the reply at `message`, held in `all`, from position `range.start` to `range.end`: one line each, the plan as a card and the subagents together in one.
pub fn steps(
    all: &[Step],
    range: std::ops::Range<usize>,
    message: usize,
    opened: impl Fn(usize) -> bool,
    root: &str,
    width: f32,
) -> Node {
    let mut shown: Vec<Node> = Vec::new();
    let mut agents: Vec<usize> = Vec::new();
    let flush = |shown: &mut Vec<Node>, agents: &mut Vec<usize>| {
        if !agents.is_empty() {
            shown.push(subagents(all, agents, message, &opened, root, None));
            agents.clear();
        }
    };
    for position in range {
        if is_inside_card(all, position) {
            continue;
        }
        let id = FocusId(STEP_BASE + (message * 1000 + position) as u64);
        let open = opened(position);
        match &all[position] {
            Step::Tool(tool) if tool.is_subagent() => agents.push(position),
            Step::Tool(tool) => {
                flush(&mut shown, &mut agents);
                shown.push(tool_step(tool, root, id, open));
            }
            Step::Plan(items) => {
                flush(&mut shown, &mut agents);
                shown.push(plan(items));
            }
            Step::Thought(thought) => {
                flush(&mut shown, &mut agents);
                let label = match thought.seconds {
                    Some(seconds) => format!("Thought for {seconds}s"),
                    None => "Thinking".to_owned(),
                };
                let mut held = node()
                    .styles([flex(), flex_col(), w_full()])
                    .child(step_line(
                        glyph(IconName::Sparkles, 3.5, Role::Subtle),
                        &label,
                        None,
                        None,
                        Some(id),
                        open,
                    ));
                if open {
                    held = held.child(
                        node().styles([flex(), flex_col(), my(1), pl(4)]).children(
                            wrap(&thought.text, 13, width - 40.0)
                                .into_iter()
                                .map(|piece| line(piece, 13, 1.6, Role::Subtle)),
                        ),
                    );
                }
                shown.push(held);
            }
        }
    }
    flush(&mut shown, &mut agents);
    node()
        .styles([
            flex(),
            flex_col(),
            gap(1),
            pl(4),
            border_l(),
            border_color(Role::Border),
        ])
        .children(shown)
}

/// What the card around a command and the command's own padding take from the card's width, in logical pixels.
const COMMAND_INSET: f32 = 64.0;

/// The card that asks whether the agent may go ahead, `width` logical pixels wide: what it wants to do, its command broken to fit, and a button for each answer.
pub fn question(ask: &Permission, width: f32, shaper: &mut Shaper) -> Node {
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
        .child(line("Needs your approval", 12, 1.4, Role::Muted))
        .child(line(&ask.title, 13, 1.4, Role::Text));
    if let Some(command) = &ask.command {
        let styles = [
            text_px(12),
            leading_ratio(1.6),
            font_mono(),
            text_color(Role::Text),
        ];
        let shown = format!("$ {command}").replace('\t', "    ");
        let room = width - COMMAND_INSET;
        card = card.child(
            node()
                .styles([
                    flex(),
                    flex_col(),
                    px(3),
                    py(2),
                    rounded(Radius::Lg),
                    bg(Role::Background),
                ])
                .children(
                    pm_ui::wrap(shaper, &shown, styles, room)
                        .into_iter()
                        .map(|piece| text(&shown[piece.start..piece.end]).styles(styles)),
                ),
        );
    }
    card.child(answers(ask))
}

/// A button for each answer on offer, the refusal first and the boldest last, wrapping when they do not fit one line.
fn answers(ask: &Permission) -> Node {
    let order = |verdict: &Verdict| match verdict {
        Verdict::Reject => 0,
        Verdict::Allow => 1,
        Verdict::Always => 2,
    };
    let mut choices: Vec<(usize, &pm_acp::Choice)> = ask.choices.iter().enumerate().collect();
    choices.sort_by_key(|(_, choice)| order(&choice.verdict));
    node()
        .styles([
            flex(),
            flex_wrap(),
            items_center(),
            pm_style::justify_end(),
            gap(2),
        ])
        .children(choices.into_iter().map(|(index, choice)| {
            Button::new(&choice.name, FocusId(CHOICE_BASE + index as u64))
                .variant(match choice.verdict {
                    Verdict::Reject => Variant::Ghost,
                    Verdict::Allow => Variant::Outline,
                    Verdict::Always => Variant::Default,
                })
                .size(Size::Sm)
                .build()
        }))
}
