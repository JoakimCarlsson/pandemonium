//! Turning what the harness reports into the plain data the pages show.

use pm_harness::{
    Inquiry, Kind, Permission, PlanStep, Progress, Spent, Standing, Tool, ToolState, TrustRequest,
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

/// The trust card `request` shows.
pub fn trust(request: &TrustRequest) -> TrustAsk {
    TrustAsk {
        ticket: request.ticket,
        root: request.root.display().to_string(),
        reason: match request.standing {
            Standing::Unknown => "This project has not been trusted yet.",
            Standing::Changed => "What this project brings changed since it was trusted.",
            Standing::Declined => "Trusting this project was declined before.",
        }
        .to_owned(),
        skipped: request
            .skipped
            .iter()
            .map(|skipped| (format!("{:?}", skipped.kind), skipped.name.clone()))
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
