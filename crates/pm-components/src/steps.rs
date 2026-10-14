//! What an agent did on the way to a reply: the steps it took, its plan and its questions.

use pm_acp::{Diff, Kind as ToolKind, Permission, PlanStep, Progress, ToolState, Verdict};
use pm_icon::IconName;
use pm_input::FocusId;
use pm_style::{
    Color, Radius, Role, animate_spin, bg, border, border_color, border_l, cursor_pointer, flex,
    flex_1, flex_col, flex_none, font_mono, gap, items_center, leading_ratio, min_h, min_w_px, my,
    overflow_hidden, pl, px, py, rounded, rounded_full, size, text_color, text_px, w_full,
};
use pm_ui::{Node, node, text};

use crate::Button;
use crate::button::{Size, Variant};
use crate::diff::diff_line;
use crate::parts::{clipped, glyph, line, wrap};
use crate::sample::{DiffLine, Kind, Step, ToolUse};
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

/// The icon and colour of a file of the kind `path` names, a plain muted file when it is of no known kind.
fn file_kind(path: &str) -> (IconName, Color) {
    let rgb = |red, green, blue| Color::rgb(red, green, blue);
    match path.rsplit('.').next().unwrap_or_default() {
        "rs" => (IconName::FileRs, rgb(0xe8, 0x95, 0x6a)),
        "ts" => (IconName::FileTs, rgb(0x5a, 0xa9, 0xe6)),
        "tsx" => (IconName::FileTsx, rgb(0x5a, 0xa9, 0xe6)),
        "js" => (IconName::FileJs, rgb(0xe6, 0xc8, 0x4f)),
        "jsx" => (IconName::FileJsx, rgb(0xe6, 0xc8, 0x4f)),
        "md" => (IconName::FileMd, rgb(0x7f, 0xb7, 0xe8)),
        "css" => (IconName::FileCss, rgb(0xb5, 0x8b, 0xe0)),
        "html" => (IconName::FileHtml, rgb(0xe8, 0x79, 0x5f)),
        "py" => (IconName::FilePy, rgb(0x6f, 0xb0, 0xe0)),
        "toml" | "yml" | "yaml" => (IconName::FileText, rgb(0xd9, 0x8b, 0x8b)),
        "json" => (IconName::FileText, rgb(0xe6, 0xc8, 0x4f)),
        "lock" => (IconName::FileText, Color::from(Role::Subtle)),
        _ => (IconName::File, Color::from(Role::Muted)),
    }
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
                let (icon, tone) = file_kind(path);
                glyph(icon, 4.0, tone)
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
        return Some(
            CodeBox::new(
                changed_lines(diff)
                    .iter()
                    .map(|line| diff_line(line.kind, &line.text)),
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
            tool.seconds.map(|seconds| {
                let (word, tone) = if tool.state == ToolState::Failed {
                    ("failed", Role::Danger)
                } else {
                    ("passed", Role::Success)
                };
                line(format!("{word} · {seconds}s"), 11, 1.4, tone).styles([flex_none()])
            }),
        ),
        ToolKind::Fetch | ToolKind::Other => (tool.title.as_str(), None, None),
    };
    let expandable = tool_detail(tool).is_some();
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

/// The agent's plan as a checklist with how far it has got.
pub fn plan(items: &[PlanStep]) -> Node {
    let done = items
        .iter()
        .filter(|item| item.progress == Progress::Done)
        .count();
    node()
        .styles([
            flex(),
            flex_col(),
            gap(1.5),
            px(3.5),
            py(2.5),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
        ])
        .child(line(
            format!("Plan · {done} of {} done", items.len()),
            12,
            1.4,
            Role::Muted,
        ))
        .children(items.iter().map(|item| {
            let (mark, tone) = match item.progress {
                Progress::Done => (
                    glyph(IconName::CheckCircle, 3.5, Role::Success),
                    Role::Subtle,
                ),
                Progress::Doing => (
                    glyph(IconName::Loading, 3.5, Role::Muted).styles([animate_spin()]),
                    Role::Text,
                ),
                Progress::Pending => (
                    node().styles([
                        size(3),
                        rounded_full(),
                        border(),
                        border_color(Role::Subtle),
                    ]),
                    Role::Muted,
                ),
            };
            node()
                .styles([flex(), items_center(), gap(2.5)])
                .child(
                    node()
                        .styles([flex(), items_center(), size(4), flex_none()])
                        .child(mark),
                )
                .child(clipped(&item.text, 13, tone))
        }))
}

/// The steps of the reply at `message` that start at position `first` of its steps: one line each, the plan as a card.
pub fn steps(
    steps: &[Step],
    first: usize,
    message: usize,
    opened: impl Fn(usize) -> bool,
    root: &str,
    width: f32,
) -> Node {
    node()
        .styles([
            flex(),
            flex_col(),
            gap(1),
            pl(4),
            border_l(),
            border_color(Role::Border),
        ])
        .children(steps.iter().enumerate().map(|(offset, step)| {
            let position = first + offset;
            let id = FocusId(STEP_BASE + (message * 1000 + position) as u64);
            let open = opened(position);
            match step {
                Step::Tool(tool) => tool_step(tool, root, id, open),
                Step::Plan(items) => plan(items),
                Step::Thought(thought) => {
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
                    held
                }
            }
        }))
}

/// The card that asks whether the agent may go ahead: what it wants to do and a button for each answer.
pub fn question(ask: &Permission) -> Node {
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
        card = card.child(
            node()
                .styles([
                    flex(),
                    px(3),
                    py(2),
                    rounded(Radius::Lg),
                    bg(Role::Background),
                ])
                .child(text(format!("$ {command}")).styles([
                    text_px(12),
                    leading_ratio(1.6),
                    font_mono(),
                    text_color(Role::Text),
                ])),
        );
    }
    let order = |verdict: &Verdict| match verdict {
        Verdict::Reject => 0,
        Verdict::Allow => 1,
        Verdict::Always => 2,
    };
    let mut choices: Vec<(usize, &pm_acp::Choice)> = ask.choices.iter().enumerate().collect();
    choices.sort_by_key(|(_, choice)| order(&choice.verdict));
    card.child(
        node()
            .styles([flex(), items_center(), pm_style::justify_end(), gap(2)])
            .children(choices.into_iter().map(|(index, choice)| {
                Button::new(&choice.name, FocusId(CHOICE_BASE + index as u64))
                    .variant(match choice.verdict {
                        Verdict::Reject => Variant::Ghost,
                        Verdict::Allow => Variant::Outline,
                        Verdict::Always => Variant::Default,
                    })
                    .size(Size::Sm)
                    .build()
            })),
    )
}
