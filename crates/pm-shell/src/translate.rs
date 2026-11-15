//! Turning what the harness reports into the plain data the pages show.

use pm_harness::{
    Input, Inquiry, Kind, Permission, PlanStep, Progress, Spent, Standing, Tool, ToolState, TrustRequest,
    Verdict,
};
use pm_harness::{Merge, Worktree};

use crate::conversation::{
    Approval, ApprovalChoice, InquiryForm, InquiryQuestion, PlanItem, PlanProgress, SpendSummary,
    StepState, ToolKind, ToolStep, TrustAsk,
};

/// The step a first report of `tool` shows.
pub fn tool_step(tool: &Tool) -> ToolStep {
    let mut step = ToolStep {
        id: tool.id.clone(),
        ..ToolStep::default()
    };
    update_step(&mut step, tool);
    step
}

/// Updates `step` with what a later report of its tool says.
pub fn update_step(step: &mut ToolStep, tool: &Tool) {
    if let Some(title) = &tool.title {
        step.title = title.clone();
    }
    if let Some(kind) = tool.kind {
        step.kind = match kind {
            Kind::Read => ToolKind::Read,
            Kind::Edit => ToolKind::Edit,
            Kind::Search => ToolKind::Search,
            Kind::Execute => ToolKind::Execute,
            Kind::Fetch => ToolKind::Fetch,
            Kind::Other => ToolKind::Other,
        };
    }
    if let Some(state) = tool.state {
        step.state = match state {
            ToolState::Running => StepState::Running,
            ToolState::Done => StepState::Done,
            ToolState::Failed => StepState::Failed,
        };
    }
    if tool.target.is_some() {
        step.target = tool.target.clone();
    }
    if tool.output.is_some() {
        step.output = tool.output.clone();
    }
    if let Some(diff) = &tool.diff {
        let (lines, text) = unified(diff.old.as_deref().unwrap_or(""), &diff.new);
        step.lines = Some(lines);
        step.diff = Some(text);
    }
    if tool.agent.is_some() {
        step.agent = tool.agent.clone();
    }
    if let Some(worktree) = &tool.worktree {
        step.worktree = Some(worktree_chip(worktree));
    }
}

/// What the chip of a subagent's `worktree` says.
fn worktree_chip(worktree: &Worktree) -> String {
    let files = |count: usize| {
        if count == 1 {
            "1 file".to_owned()
        } else {
            format!("{count} files")
        }
    };
    match &worktree.merge {
        None => "In its own worktree".to_owned(),
        Some(Merge::Applied { files: changed }) => format!("Merged {}", files(changed.len())),
        Some(Merge::Unchanged) => "Changed nothing".to_owned(),
        Some(Merge::Conflicted { files: changed, .. }) => {
            format!("Conflicts in {}", files(changed.len()))
        }
        Some(Merge::Held { reason, .. }) => format!("Not merged: {reason}"),
    }
}

/// Lines added and removed going from `old` to `new`, and the changed lines as a unified diff.
fn unified(old: &str, new: &str) -> ((usize, usize), String) {
    let diff = similar::TextDiff::from_lines(old, new);
    let mut added = 0;
    let mut removed = 0;
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Insert => added += 1,
            similar::ChangeTag::Delete => removed += 1,
            similar::ChangeTag::Equal => {}
        }
    }
    let text = diff.unified_diff().context_radius(2).to_string();
    ((added, removed), text.trim_end().to_owned())
}

/// The plan `steps` describe.
pub fn plan(steps: &[PlanStep]) -> Vec<PlanItem> {
    steps
        .iter()
        .map(|step| PlanItem {
            text: step.text.clone(),
            progress: match step.progress {
                Progress::Pending => PlanProgress::Pending,
                Progress::Doing => PlanProgress::Doing,
                Progress::Done => PlanProgress::Done,
            },
        })
        .collect()
}

/// The approval `permission` asks for.
pub fn approval(permission: &Permission) -> Approval {
    Approval {
        ticket: permission.ticket,
        title: permission.title.clone(),
        command: permission.command.clone(),
        plan: permission.plan.as_deref().map(pm_markdown::parse),
        choices: permission
            .choices
            .iter()
            .map(|choice| ApprovalChoice {
                id: choice.id.clone(),
                name: choice.name.clone(),
                refuses: choice.verdict == Verdict::Reject,
            })
            .collect(),
    }
}

/// The form `inquiry` asks.
pub fn inquiry(inquiry: &Inquiry) -> InquiryForm {
    InquiryForm {
        ticket: inquiry.ticket,
        questions: inquiry
            .questions
            .iter()
            .map(|question| InquiryQuestion {
                header: question.header.clone(),
                text: question.text.clone(),
                multiple: question.multiple,
                options: question
                    .alternatives
                    .iter()
                    .map(|alternative| (alternative.label.clone(), alternative.description.clone()))
                    .collect(),
            })
            .collect(),
    }
}

/// The kinds of input in the order the trust card lists them: hooks first, since they run commands.
const TRUST_KINDS: [Input; 4] = [Input::Hook, Input::Instruction, Input::Skill, Input::Agent];

/// What one input of `kind` is called on the trust card, and what several are called in a sentence.
fn input_words(kind: Input) -> (&'static str, &'static str) {
    match kind {
        Input::Hook => ("Hook", "hooks"),
        Input::Instruction => ("Instructions", "instructions"),
        Input::Skill => ("Skill", "skills"),
        Input::Agent => ("Agent", "agents"),
    }
}

/// The words of `items` run together as a list: `a`, `a and b`, `a, b and c`.
fn run_together(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The trust card `request` shows: a heading and a sentence saying where the project's trust stands and what the agent goes without, warning that hooks run commands, then what was left out, hooks first.
pub fn trust(request: &TrustRequest) -> TrustAsk {
    let name = request.root.file_name().map_or_else(
        || request.root.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let kinds: Vec<&str> = TRUST_KINDS
        .into_iter()
        .filter(|kind| request.skipped.iter().any(|skipped| skipped.kind == *kind))
        .map(|kind| input_words(kind).1)
        .collect();
    let missing = run_together(&kinds);
    let hooks = request
        .skipped
        .iter()
        .any(|skipped| skipped.kind == Input::Hook);
    let (title, reason) = match request.standing {
        Standing::Changed => (
            format!("The hooks of {name} changed since you trusted it"),
            "Hooks were added or changed since trust was granted, so they are not run until you trust the project again. Its instructions, skills and agents are still used.".to_owned(),
        ),
        Standing::Declined => (
            format!("You chose not to trust {name}"),
            format!("This session is not using its {missing}. Trust the project to use them from the next turn."),
        ),
        Standing::Unknown if hooks => (
            format!("Trust {name}?"),
            format!("This session is not using its {missing} until you trust the project. Its hooks run commands on this machine, so only trust a project whose files you know."),
        ),
        Standing::Unknown => (
            format!("Trust {name}?"),
            format!("This session is not using its {missing} until you trust the project."),
        ),
    };
    let mut skipped: Vec<_> = request.skipped.iter().collect();
    skipped.sort_by_key(|item| TRUST_KINDS.iter().position(|kind| *kind == item.kind));
    TrustAsk {
        ticket: request.ticket,
        root: request.root.display().to_string(),
        title,
        reason,
        skipped: skipped
            .into_iter()
            .map(|item| (input_words(item.kind).0.to_owned(), item.name.clone()))
            .collect(),
    }
}

/// The spend strip `spent` shows.
pub fn spend(spent: &Spent) -> SpendSummary {
    SpendSummary {
        cost: spent.cost().map(pm_harness::dollars),
        partial: spent.partial(),
        tokens: spent.tokens(),
        requests: spent.requests(),
        models: spent
            .models
            .iter()
            .map(|model| {
                (
                    model.model.clone(),
                    model.cost.map(pm_harness::dollars),
                    model.tokens(),
                )
            })
            .collect(),
    }
}
