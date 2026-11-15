//! Turning what the harness reports into the plain data the pages show.

use pm_harness::{
    Inquiry, Kind, Permission, PlanStep, Progress, Spent, Standing, Tool, ToolState, TrustRequest,
    Verdict,
};

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
        step.lines = Some(line_counts(diff.old.as_deref().unwrap_or(""), &diff.new));
    }
}

/// Lines added and removed going from `old` to `new`, counted by lines
/// present in one and missing from the other.
fn line_counts(old: &str, new: &str) -> (usize, usize) {
    let before: std::collections::HashSet<&str> = old.lines().collect();
    let after: std::collections::HashSet<&str> = new.lines().collect();
    (
        new.lines().filter(|line| !before.contains(line)).count(),
        old.lines().filter(|line| !after.contains(line)).count(),
    )
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
