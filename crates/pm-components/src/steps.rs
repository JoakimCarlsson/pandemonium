//! What an agent did on the way to a reply: the steps it took, its plan and its questions.

use crate::events::reduce;
use crate::theme;
use pm_harness::{
    Diff, Kind as ToolKind, Merge, Permission, PlanStep, Progress, ToolState, Verdict, Worktree,
};
use rustle::phosphor;
use rustle::{Alignment, column_arranged, row_arranged};
use rustle::{Arrangement, CursorIcon, FlowOptions, Modifier};

use crate::Button;
use crate::Tile;
use crate::button::Variant;
use crate::diff::diff_line;
use crate::diff_syntax::DiffColours;
use crate::events::Interactive;
use crate::file_icon::{file_icon, file_language};
use crate::focus::FocusId;
use crate::icons::spinner;
use crate::parts::{clipped, glyph, line, wrap};
use crate::sample::{DiffLine, Kind, Step, ToolUse};
use crate::size::Size;
use crate::state::{CHOICE_BASE, STEP_BASE, SUBAGENT_STOP_BASE};
use crate::theme::{Radius, Role, sp};
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

/// Emits one line of the steps: a mark for how it went (`mark` emits it), what was done and to what, and what came of it (`detail` emits it, or nothing).
#[track_caller]
fn step_line(
    mark: impl FnOnce(),
    verb: &str,
    target: Option<&str>,
    detail: impl FnOnce(),
    id: Option<FocusId>,
    open: bool,
) {
    let mut own = Modifier::new()
        .fill_max_width()
        .height_in(theme::sp(8), f32::INFINITY);
    if let Some(id) = id {
        own = own.cursor(CursorIcon::Pointer).control(id, reduce(id));
    }
    row_arranged(
        own,
        Arrangement::SpacedBy(sp(2.5)),
        Alignment::Center,
        || {
            row_arranged(
                Modifier::new().size(theme::sp(4), theme::sp(4)),
                Arrangement::Start,
                Alignment::Center,
                mark,
            );
            line(verb, 13, 1.4, Role::Muted);
            if let Some(target) = target {
                clipped_mono(target, verb == "Ran");
            } else {
                rustle::spacer(Modifier::new().weight(1.0));
            }
            detail();
            if id.is_some() {
                glyph(
                    if open {
                        phosphor::CARET_DOWN
                    } else {
                        phosphor::CARET_RIGHT
                    },
                    3.5,
                    Role::Subtle,
                );
            }
        },
    );
}

/// Emits a box that takes the room left in its row and cuts a line of code short at its edge, on a chip when `chip` is set.
#[track_caller]
fn clipped_mono(content: &str, chip: bool) {
    let code = theme::mono_style(12, 1.4)
        .color(theme::color(Role::Text))
        .max_lines(1);
    row_arranged(
        Modifier::new().weight(1.0),
        Arrangement::Start,
        Alignment::Start,
        || {
            let holder = Modifier::new().clip();
            let holder = if chip {
                Modifier::new()
                    .background_rounded(theme::color(Role::Surface), Radius::Md.px())
                    .clip()
                    .padding_xy(theme::sp(1.5), 0.0)
            } else {
                holder
            };
            row_arranged(holder, Arrangement::Start, Alignment::Start, || {
                rustle::basic_text(
                    Modifier::new(),
                    content,
                    code,
                    rustle::TextOverflow::Ellipsis,
                )
            });
        },
    );
}

/// Emits the mark of how a tool got on: a turning ring, or the glyph that is its kind.
#[track_caller]
fn tool_mark(tool: &ToolUse) {
    match tool.state {
        ToolState::Running => spinner(3.5, Role::Muted),
        ToolState::Failed => glyph(phosphor::X, 3.5, Role::Danger),
        ToolState::Done => match tool.kind {
            ToolKind::Read | ToolKind::Edit => {
                let path = tool
                    .target
                    .as_deref()
                    .or(tool.diff.as_ref().map(|diff| diff.path.as_str()))
                    .unwrap_or_default();
                file_icon(path, 4.0);
            }
            ToolKind::Search => glyph(phosphor::MAGNIFYING_GLASS, 4.0, Role::Subtle),
            ToolKind::Execute => glyph(phosphor::TERMINAL, 4.0, Role::Subtle),
            ToolKind::Fetch | ToolKind::Other => glyph(phosphor::CHECK, 4.0, Role::Subtle),
        },
    }
}

/// The printed output of `tool` worth showing under its line, when it has some.
fn tool_output(tool: &ToolUse) -> Option<&str> {
    tool.output.as_deref().filter(|output| !output.is_empty())
}

/// Whether `tool` has a detail to fold out under its line: what an edit changed, or what it printed.
fn has_detail(tool: &ToolUse) -> bool {
    tool.diff.is_some() || tool_output(tool).is_some()
}

/// Emits the detail under a tool's line, when it has some.
#[track_caller]
fn tool_detail(tool: &ToolUse) {
    if let Some(diff) = &tool.diff {
        let lines = changed_lines(diff);
        let colours = DiffColours::of(&lines, file_language(&diff.path).as_deref());
        CodeBox::new(|| {
            for (at, line) in lines.iter().enumerate() {
                diff_line(line.kind, &line.text, colours.line(at));
            }
        })
        .modifier(Modifier::new().fill_max_width())
        .max_height(DETAIL_HEIGHT)
        .build();
        return;
    }
    let Some(output) = tool_output(tool) else {
        return;
    };
    CodeBox::new(move || {
        for printed in output
            .lines()
            .filter(|printed| !printed.starts_with("```"))
            .take(40)
        {
            row_arranged(
                Modifier::new().padding_xy(theme::sp(3.5), 0.0),
                Arrangement::Start,
                Alignment::Start,
                || {
                    rustle::basic_text(
                        Modifier::new(),
                        printed.replace('\t', "    "),
                        theme::mono_style(12, 1.6).color(theme::color(Role::Muted)),
                        rustle::TextOverflow::Ellipsis,
                    );
                },
            );
        }
    })
    .modifier(Modifier::new().fill_max_width())
    .max_height(DETAIL_HEIGHT)
    .build();
}

/// Emits one tool of the steps.
#[track_caller]
fn tool_step(tool: &ToolUse, root: &str, id: FocusId, open: bool) {
    tool_line(tool, root, id, open, true);
}

/// Emits one tool of the steps, folding out its detail only when `foldable`.
#[track_caller]
fn tool_line(tool: &ToolUse, root: &str, id: FocusId, open: bool, foldable: bool) {
    let target = tool.target.as_deref().map(|target| relative(target, root));
    let (verb, shown) = match tool.kind {
        ToolKind::Read => ("Read", target),
        ToolKind::Search => ("Searched for", target),
        ToolKind::Edit => (
            "Edited",
            target.or(tool.diff.as_ref().map(|diff| relative(&diff.path, root))),
        ),
        ToolKind::Execute => ("Ran", target),
        ToolKind::Fetch | ToolKind::Other => (tool.title.as_str(), None),
    };
    let detail = || match tool.kind {
        ToolKind::Edit => {
            if let Some(diff) = &tool.diff {
                let changes = changed_lines(diff);
                let count = |wanted| changes.iter().filter(|line| line.kind == wanted).count();
                Tally::new(count(Kind::Add), count(Kind::Remove))
                    .size(11)
                    .build();
            }
        }
        ToolKind::Execute => match (tool.seconds, tool.state) {
            (Some(seconds), state) => {
                let (word, tone) = if state == ToolState::Failed {
                    ("failed", Role::Danger)
                } else {
                    ("passed", Role::Success)
                };
                line(format!("{word} · {seconds}s"), 11, 1.4, tone);
            }
            (None, ToolState::Running) => line(
                format!("running · {}", elapsed(tool.started.elapsed().as_secs())),
                11,
                1.4,
                Role::Muted,
            ),
            (None, _) => {}
        },
        _ => {}
    };
    let expandable = foldable && has_detail(tool);
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::Start,
        Alignment::Start,
        || {
            step_line(
                || tool_mark(tool),
                verb,
                shown,
                detail,
                expandable.then_some(id),
                open,
            );
            if open {
                tool_detail(tool);
            }
        },
    );
}

/// How long something has gone on, in words: `42s` or `3m 05s`.
pub fn elapsed(seconds: u64) -> String {
    if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

/// Emits the mark of how a subagent is getting on: a turning ring, a dot once it is done, or a cross.
#[track_caller]
fn agent_mark(state: ToolState) {
    row_arranged(
        Modifier::new().size(theme::sp(4), theme::sp(4)),
        Arrangement::Start,
        Alignment::Center,
        || match state {
            ToolState::Running => spinner(3.5, Role::Muted),
            ToolState::Done => rustle::spacer(
                Modifier::new()
                    .size(theme::sp(1.5), theme::sp(1.5))
                    .background_rounded(theme::color(Role::Subtle), Radius::Full.px()),
            ),
            ToolState::Failed => glyph(phosphor::X, 3.5, Role::Danger),
        },
    );
}

/// Emits the chip of a subagent working in a git worktree of its own: that it does, while it works, and what became of its changes once it ended.
#[track_caller]
fn worktree_chip(worktree: &Worktree) {
    let (words, role) = match &worktree.merge {
        None => ("worktree".to_owned(), Role::Muted),
        Some(Merge::Applied { files }) if files.len() == 1 => {
            ("applied 1 file".to_owned(), Role::Success)
        }
        Some(Merge::Applied { files }) => (format!("applied {} files", files.len()), Role::Success),
        Some(Merge::Unchanged) => ("no changes".to_owned(), Role::Subtle),
        Some(Merge::Conflicted { .. }) => ("conflict".to_owned(), Role::Danger),
        Some(Merge::Held { .. }) => ("not applied".to_owned(), Role::Warning),
    };
    row_arranged(
        Modifier::new()
            .background_rounded(theme::color(Role::SurfaceHover), Radius::Md.px())
            .padding_xy(theme::sp(1.5), 0.0),
        Arrangement::SpacedBy(sp(1)),
        Alignment::Center,
        || {
            glyph(phosphor::GIT_FORK, 3.0, role);
            rustle::basic_text(
                Modifier::new(),
                words,
                theme::text_style(12, 1.4).color(theme::color(role)),
                rustle::TextOverflow::Ellipsis,
            );
        },
    );
}

/// A small quiet label on a subagent's row naming the kind of subagent, set like its other labels.
#[track_caller]
fn quiet_chip(words: String) {
    quiet_text(words);
}

/// Emits a small quiet label on a subagent's row, such as the category and model it runs as.
#[track_caller]
fn quiet_label(words: String) {
    quiet_text(words);
}

/// Emits `words` small and muted on a chip.
#[track_caller]
fn quiet_text(words: String) {
    let kind = theme::text_style(12, 1.4).color(theme::color(Role::Muted));
    row_arranged(
        Modifier::new()
            .background_rounded(theme::color(Role::SurfaceHover), Radius::Md.px())
            .padding_xy(theme::sp(1.5), 0.0),
        Arrangement::Start,
        Alignment::Start,
        || rustle::basic_text(Modifier::new(), words, kind, rustle::TextOverflow::Ellipsis),
    );
}

/// Emits what a subagent has cost so far, `micros` millionths of a US dollar, as a muted amount cut short when the row runs out of room.
#[track_caller]
fn cost_label(micros: u64) {
    rustle::basic_text(
        Modifier::new().clip(),
        crate::spend::dollars(micros),
        theme::text_style(12, 1.4)
            .color(theme::color(Role::Subtle))
            .max_lines(1),
        rustle::TextOverflow::Ellipsis,
    );
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

/// Emits the card of subagents: how many there are and how many are at work, then a row for each that folds out the tools it used, with a button that stops it while it is at work and can be stopped.
///
/// `scroll` bounds the rows to 240 logical pixels and scrolls them independently as its
/// first state says, handing the wheel to its second, the area around, at either end.
/// `at` are the positions in `all`, the steps of the reply at `message`, of the tools that start them.
#[track_caller]
pub fn subagents(
    all: &[Step],
    at: &[usize],
    message: usize,
    opened: &dyn Fn(usize) -> bool,
    root: &str,
    scroll: Option<(&rustle::ScrollState, &rustle::ScrollState)>,
) {
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
    let rows = move |modifier: Modifier| {
        column_arranged(
            modifier.fill_max_width(),
            Arrangement::Start,
            Alignment::Start,
            || {
                for (position, tool) in agents {
                    rustle::key(position, || {
                        agent_row(all, message, opened, root, position, tool)
                    });
                }
            },
        );
    };
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .background_rounded(theme::color(Role::Surface), Radius::Lg.px())
            .border(1.0, theme::color(Role::Border), Radius::Lg.px())
            .padding(1.0)
            .clip(),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height(theme::sp(10))
                    .draw_behind(theme::edge(
                        theme::Side::Bottom,
                        1.0,
                        theme::color(Role::Border),
                    ))
                    .padding_each(0.0, 0.0, 0.0, 1.0)
                    .padding_xy(theme::sp(3.5), 0.0),
                Arrangement::SpacedBy(sp(2.5)),
                Alignment::Center,
                || {
                    glyph(phosphor::GIT_FORK, 4.0, Role::Muted);
                    line(title, 13, 1.4, Role::Text);
                },
            );
            match scroll {
                Some((own, parent)) => rows(
                    handing_on(
                        Modifier::new().fill_max_width().height_in(0.0, sp(60)),
                        own,
                        parent,
                    )
                    .vertical_scroll(own),
                ),
                None => rows(Modifier::new()),
            }
        },
    );
}

/// `modifier` handing the wheel to `parent` while `own`, which the area scrolls, is at
/// the end the wheel turns towards, so the area around goes on scrolling.
fn handing_on(
    modifier: Modifier,
    own: &rustle::ScrollState,
    parent: &rustle::ScrollState,
) -> Modifier {
    let (own, parent) = (own.clone(), parent.clone());
    modifier.pointer_input(move |event| {
        if event.phase != rustle::PointerInputPhase::Scroll {
            return false;
        }
        let pixels = -event.scroll_delta.y;
        let room = own.max_offset();
        let offset = own.offset().clamp(0.0, room);
        let at_end = (pixels > 0.0 && offset >= room) || (pixels < 0.0 && offset <= 0.0);
        if at_end {
            parent.scroll_drawn_by(pixels);
        }
        at_end
    })
}

/// Emits the row of the subagent started by `tool`, at `position` of `all`, the steps of the reply at `message`, folded out to the tools it used while `opened` says so.
#[track_caller]
fn agent_row(
    all: &[Step],
    message: usize,
    opened: &dyn Fn(usize) -> bool,
    root: &str,
    position: usize,
    tool: &ToolUse,
) {
    let used = beneath(all, &tool.id);
    let open = opened(position);
    let id = FocusId(STEP_BASE + (message * 1000 + position) as u64);
    let described = tool.target.as_deref().unwrap_or(&tool.title);
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::Start,
        Alignment::Start,
        || {
            Tile::new(id)
                .height(10.0)
                .gap(2.5)
                .radius(Radius::Xs)
                .inner(Modifier::new().padding_xy(theme::sp(3.5), 0.0))
                .modifier(Modifier::new().fill_max_width())
                .build_with(|| {
                    agent_mark(tool.state);
                    if tool.agent.as_deref() != Some("agent") {
                        quiet_chip(tool.agent.clone().unwrap_or_default());
                    }
                    if let Some(worktree) = tool.worktree.as_deref() {
                        worktree_chip(worktree);
                    }
                    clipped(described, 13, Role::Text);
                    if let Some(runs_as) = tool.runs_as.clone() {
                        quiet_label(runs_as);
                    }
                    if let Some(cost) = tool.cost {
                        cost_label(cost);
                    }
                    line(
                        match used.len() {
                            1 => "1 tool".to_owned(),
                            count => format!("{count} tools"),
                        },
                        12,
                        1.4,
                        Role::Subtle,
                    );
                    if tool.stoppable().is_some() {
                        Button::icon(
                            phosphor::STOP,
                            FocusId(SUBAGENT_STOP_BASE + (message * 1000 + position) as u64),
                        )
                        .size(Size::Xs)
                        .tooltip("Stop this subagent")
                        .build();
                    }
                    glyph(
                        if open {
                            phosphor::CARET_DOWN
                        } else {
                            phosphor::CARET_RIGHT
                        },
                        3.5,
                        Role::Subtle,
                    );
                });
            if open && !used.is_empty() {
                column_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .padding_each(theme::sp(10), 0.0, 0.0, 0.0)
                        .padding_each(0.0, 0.0, theme::sp(3.5), 0.0)
                        .padding_each(0.0, 0.0, 0.0, theme::sp(2)),
                    Arrangement::SpacedBy(sp(1)),
                    Alignment::Start,
                    || {
                        for tool in used {
                            tool_line(tool, root, FocusId(0), false, false);
                        }
                    },
                );
            }
        },
    );
}

/// Emits the mark in front of a plan step: a filled check when done, a spinner while under way, an empty ring when not begun.
#[track_caller]
fn plan_mark(progress: Progress) {
    match progress {
        Progress::Done => row_arranged(
            Modifier::new()
                .size(theme::sp(4), theme::sp(4))
                .background_rounded(theme::color(Role::Success), Radius::Full.px()),
            Arrangement::Center,
            Alignment::Center,
            || glyph(phosphor::CHECK, 3.0, Role::OnSuccess),
        ),
        Progress::Doing => spinner(4.0, Role::Muted),
        Progress::Pending => rustle::spacer(
            Modifier::new()
                .size(theme::sp(4), theme::sp(4))
                .border(1.0, theme::color(Role::Subtle), Radius::Full.px())
                .padding(1.0),
        ),
    }
}

/// Emits the agent's plan as a card: a header saying how far it has got over a checklist that strikes through what is done.
#[track_caller]
pub fn plan(items: &[PlanStep]) {
    let done = items
        .iter()
        .filter(|item| item.progress == Progress::Done)
        .count();
    let finished = done == items.len();
    column_arranged(
        Modifier::new()
            .fill_max_width()
            .background_rounded(theme::color(Role::Surface), Radius::Xl.px())
            .border(1.0, theme::color(Role::Border), Radius::Xl.px())
            .padding(1.0)
            .clip(),
        Arrangement::Start,
        Alignment::Start,
        || {
            row_arranged(
                Modifier::new()
                    .fill_max_width()
                    .height(theme::sp(12))
                    .draw_behind(theme::edge(
                        theme::Side::Bottom,
                        1.0,
                        theme::color(Role::Border),
                    ))
                    .padding_each(0.0, 0.0, 0.0, 1.0)
                    .padding_xy(theme::sp(4), 0.0),
                Arrangement::SpacedBy(sp(3)),
                Alignment::Center,
                || {
                    glyph(
                        phosphor::CHECK_CIRCLE,
                        4.5,
                        if finished { Role::Success } else { Role::Muted },
                    );
                    line(
                        format!("Plan · {done} of {} done", items.len()),
                        14,
                        1.4,
                        Role::Text,
                    );
                },
            );
            column_arranged(
                Modifier::new()
                    .fill_max_width()
                    .padding_xy(theme::sp(4), 0.0)
                    .padding_xy(0.0, theme::sp(3.5)),
                Arrangement::SpacedBy(sp(3)),
                Alignment::Start,
                || {
                    for item in items {
                        let (tone, struck) = match item.progress {
                            Progress::Done => (Role::Subtle, true),
                            Progress::Doing => (Role::Text, false),
                            Progress::Pending => (Role::Muted, false),
                        };
                        let kind = theme::text_style(13, 1.4)
                            .color(theme::color(tone))
                            .max_lines(1);
                        let kind = if struck { kind.strikethrough() } else { kind };
                        row_arranged(
                            Modifier::new().fill_max_width(),
                            Arrangement::SpacedBy(sp(3)),
                            Alignment::Center,
                            || {
                                row_arranged(
                                    Modifier::new().size(theme::sp(4), theme::sp(4)),
                                    Arrangement::Start,
                                    Alignment::Center,
                                    || plan_mark(item.progress),
                                );
                                rustle::basic_text(
                                    Modifier::new().weight(1.0).clip(),
                                    &item.text,
                                    kind,
                                    rustle::TextOverflow::Ellipsis,
                                );
                            },
                        );
                    }
                },
            );
        },
    );
}

/// Emits the steps of the reply at `message`, held in `all`, from position `range.start` to `range.end`: one line each, the plan as a card and the subagents together in one.
#[track_caller]
pub fn steps(
    all: &[Step],
    range: std::ops::Range<usize>,
    message: usize,
    opened: impl Fn(usize) -> bool,
    root: &str,
    width: f32,
) {
    column_arranged(
        Modifier::new()
            .draw_behind(theme::edge(
                theme::Side::Left,
                1.0,
                theme::color(Role::Border),
            ))
            .padding_each(1.0, 0.0, 0.0, 0.0)
            .padding_each(theme::sp(4), 0.0, 0.0, 0.0),
        Arrangement::SpacedBy(sp(1)),
        Alignment::Start,
        || {
            let mut agents: Vec<usize> = Vec::new();
            let flush = |agents: &mut Vec<usize>| {
                if let Some(&first) = agents.first() {
                    rustle::key(("agents", first), || {
                        subagents(all, agents, message, &opened, root, None)
                    });
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
                        flush(&mut agents);
                        rustle::key(position, || tool_step(tool, root, id, open));
                    }
                    Step::Plan(items) => {
                        flush(&mut agents);
                        rustle::key(position, || plan(items));
                    }
                    Step::Thought(thought) => {
                        flush(&mut agents);
                        let words = match thought.seconds {
                            Some(seconds) => format!("Thought for {seconds}s"),
                            None => "Thinking".to_owned(),
                        };
                        rustle::key(position, || {
                            column_arranged(
                                Modifier::new().fill_max_width(),
                                Arrangement::Start,
                                Alignment::Start,
                                || {
                                    step_line(
                                        || glyph(phosphor::SPARKLE, 3.5, Role::Subtle),
                                        &words,
                                        None,
                                        || {},
                                        Some(id),
                                        open,
                                    );
                                    if open {
                                        column_arranged(
                                            Modifier::new()
                                                .padding_xy(0.0, theme::sp(1))
                                                .padding_each(theme::sp(4), 0.0, 0.0, 0.0),
                                            Arrangement::Start,
                                            Alignment::Start,
                                            || {
                                                for piece in wrap(&thought.text, 13, width - 40.0) {
                                                    line(piece, 13, 1.6, Role::Subtle);
                                                }
                                            },
                                        );
                                    }
                                },
                            );
                        });
                    }
                }
            }
            flush(&mut agents);
        },
    );
}

/// What the card around a command and the command's own padding take from the card's width, in logical pixels.
const COMMAND_INSET: f32 = 64.0;

/// What the card's padding and border take from the card's width, in logical pixels.
const CARD_INSET: f32 = 34.0;

/// Emits the card that asks whether the agent may go ahead, `width` logical pixels wide: what it wants to do, its command broken to fit, and a button for each answer.
#[track_caller]
pub fn question(ask: &Permission, width: f32) {
    let title = theme::text_style(13, 1.4).color(theme::color(Role::Text));
    column_arranged(
        Modifier::new()
            .background_rounded(theme::color(Role::Surface), Radius::Xl.px())
            .border(1.0, theme::color(Role::Border), Radius::Xl.px())
            .padding(1.0)
            .padding_xy(theme::sp(4), 0.0)
            .padding_xy(0.0, theme::sp(3)),
        Arrangement::SpacedBy(sp(2.5)),
        Alignment::Start,
        || {
            line("Needs your approval", 12, 1.4, Role::Muted);
            column_arranged(
                Modifier::new(),
                Arrangement::Start,
                Alignment::Start,
                || {
                    for piece in rustle::wrap_at_spaces(&ask.title, &title, width - CARD_INSET) {
                        rustle::basic_text(
                            Modifier::new(),
                            &ask.title[piece.start..piece.end],
                            title,
                            rustle::TextOverflow::Ellipsis,
                        );
                    }
                },
            );
            if let Some(command) = &ask.command {
                let kind = theme::mono_style(12, 1.6).color(theme::color(Role::Text));
                let shown = format!("$ {command}").replace('\t', "    ");
                let room = width - COMMAND_INSET;
                column_arranged(
                    Modifier::new()
                        .fill_max_width()
                        .background_rounded(theme::color(Role::Background), Radius::Lg.px())
                        .padding_xy(theme::sp(3), 0.0)
                        .padding_xy(0.0, theme::sp(2)),
                    Arrangement::Start,
                    Alignment::Start,
                    || {
                        for piece in rustle::wrap_at_spaces(&shown, &kind, room) {
                            rustle::basic_text(
                                Modifier::new(),
                                &shown[piece.start..piece.end],
                                kind,
                                rustle::TextOverflow::Ellipsis,
                            );
                        }
                    },
                );
            }
            answers(ask, width - CARD_INSET);
        },
    );
}

/// Emits a button for each answer on offer, the refusal first and the boldest last, gathered at the end of the line and wrapping when they do not fit one line.
#[track_caller]
fn answers(ask: &Permission, width: f32) {
    let order = |verdict: &Verdict| match verdict {
        Verdict::Reject => 0,
        Verdict::Allow => 1,
        Verdict::Always => 2,
    };
    let mut choices: Vec<(usize, &pm_harness::Choice)> = ask.choices.iter().enumerate().collect();
    choices.sort_by_key(|(_, choice)| order(&choice.verdict));
    rustle::flow_row_with(
        Modifier::new().fill_max_width(),
        FlowOptions::new(sp(2), sp(2))
            .arrangement(Arrangement::End)
            .item_alignment(rustle::Alignment::Center),
        || {
            for (index, choice) in choices {
                Button::new(&choice.name, FocusId(CHOICE_BASE + index as u64))
                    .variant(match choice.verdict {
                        Verdict::Reject => Variant::Ghost,
                        Verdict::Allow => Variant::Outline,
                        Verdict::Always => Variant::Default,
                    })
                    .size(Size::Sm)
                    .wrapped(width)
                    .build();
            }
        },
    );
}
