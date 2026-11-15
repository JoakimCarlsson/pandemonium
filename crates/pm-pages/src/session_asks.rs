//! The cards above the composer: the agent's plan, and what the agent waits
//! on the reader for: approving a plan or permission, answering its
//! questions, trusting the project or signing in, and the bars saying the
//! dev container is being prepared and how the goal stands.

use rustle::prelude::*;

use pm_components::{
    Button, ButtonVariant, Chip, Label, LabelVariant, Notice, NoticeVariant, Size, ApproveBand, ApproveBandVariant, InquiryCard, InquiryPrompt, PlanPanel, PlanStepState,
    TrustAnswer, TrustCard,
};
use pm_shell::{ConversationCommand, Approval, Extras, InquiryForm, InquiryReply, PlanProgress, TrustAsk, TrustVerdict};

use crate::actions::Actions;

/// Emits the plan and whatever the agent waits on, as `extras` holds them.
pub fn waiting_cards(extras: &Extras, actions: &Actions) {
    let mut plan = PlanPanel::new();
    for item in &extras.plan {
        let state = match item.progress {
            PlanProgress::Pending => PlanStepState::Pending,
            PlanProgress::Doing => PlanStepState::Doing,
            PlanProgress::Done => PlanStepState::Done,
        };
        plan = plan.step(item.text.clone(), state);
    }
    plan.show();
    if let Some(approval) = &extras.approval {
        approve_band(approval, actions);
    }
    if let Some(inquiry) = &extras.inquiry {
        inquiry_card(inquiry, actions);
    }
    if let Some(trust) = &extras.trust {
        trust_card(trust, actions);
    }
    sign_in_bars(extras, actions);
    if let Some(step) = &extras.container {
        Label::new(format!("Preparing the dev container: {step}"))
            .variant(LabelVariant::Muted)
            .size(Size::Xs)
            .show();
    }
    if let Some(goal) = &extras.composer.goal {
        Chip::new(format!("Goal · {} · {}", goal.state, goal.objective))
            .icon(phosphor::FLAG)
            .size(Size::Xs)
            .selected(goal.active)
            .show();
        Label::new(goal.detail.clone())
            .variant(LabelVariant::Subtle)
            .size(Size::Xs)
            .show();
    }
}

/// The bars asking the reader to sign in again as the session's account, or to a tool server.
fn sign_in_bars(extras: &Extras, actions: &Actions) {
    if let Some(reason) = &extras.signed_out {
        let run = actions.conversation.clone();
        sign_in_bar("The session's account is signed out", reason, "Sign in again", move || {
            run(ConversationCommand::SignIn)
        });
    }
    if let Some(page) = &extras.tool_sign_in {
        let open = actions.open_address.clone();
        let address = page.clone();
        sign_in_bar("A tool server asks you to sign in", page, "Open sign-in page", move || {
            open(address.clone())
        });
    }
}

/// One bar saying `title` and `detail`, with a button reading `label` that runs `on_click`.
fn sign_in_bar(title: &str, detail: &str, label: &str, on_click: impl Fn() + 'static) {
    column_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(pm_components::spacing(6.0)),
        Alignment::Start,
        || {
            Notice::new(title)
                .detail(detail)
                .icon(phosphor::SIGN_IN)
                .variant(NoticeVariant::Warning)
                .size(Size::Sm)
                .show();
            Button::new(label)
                .variant(ButtonVariant::Primary)
                .size(Size::Sm)
                .on_click(on_click)
                .show();
        },
    );
}

/// The band asking to approve `approval`.
fn approve_band(approval: &Approval, actions: &Actions) {
    let mut band = ApproveBand::new(approval.title.clone())
        .variant(if approval.plan.is_some() {
            ApproveBandVariant::Plan
        } else {
            ApproveBandVariant::Permission
        })
        .command(approval.command.clone())
        .plan(approval.plan.clone());
    for choice in &approval.choices {
        let choose = actions.choose.clone();
        let (ticket, id) = (approval.ticket, choice.id.clone());
        band = band.choice(choice.name.clone(), choice.refuses, move || {
            choose(ticket, id.clone())
        });
    }
    band.show();
}

/// The card asking the questions of `inquiry`.
fn inquiry_card(inquiry: &InquiryForm, actions: &Actions) {
    let questions = inquiry
        .questions
        .iter()
        .map(|question| InquiryPrompt {
            header: question.header.clone(),
            text: question.text.clone(),
            multiple: question.multiple,
            options: question.options.clone(),
        })
        .collect();
    let ticket = inquiry.ticket;
    let send = actions.answer_inquiry.clone();
    let skip = actions.answer_inquiry.clone();
    InquiryCard::new(questions)
        .on_submit(move |answers| {
            let replies = answers
                .into_iter()
                .map(|(picks, other)| InquiryReply { picks, other })
                .collect();
            send(ticket, Some(replies));
        })
        .on_skip(move || skip(ticket, None))
        .show();
}

/// The card asking to trust the project `trust` names.
fn trust_card(trust: &TrustAsk, actions: &Actions) {
    let mut card = TrustCard::new(trust.root.clone()).reason(trust.reason.clone());
    for (kind, name) in &trust.skipped {
        card = card.skipped(kind.clone(), name.clone());
    }
    let answer = actions.trust.clone();
    let ticket = trust.ticket;
    card.on_answer(move |verdict| {
        answer(
            ticket,
            match verdict {
                TrustAnswer::Once => TrustVerdict::Once,
                TrustAnswer::Always => TrustVerdict::Always,
                TrustAnswer::Reject => TrustVerdict::Reject,
            },
        )
    })
    .show();
}
