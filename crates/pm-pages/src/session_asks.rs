//! The cards above the composer: the agent's plan, and what the agent waits
//! on the reader for: approving a plan or permission, answering its
//! questions or trusting the project.

use pm_components::{
    ApproveBand, ApproveBandVariant, InquiryCard, InquiryPrompt, PlanPanel, PlanStepState,
    TrustAnswer, TrustCard,
};
use pm_shell::{Approval, Extras, InquiryForm, InquiryReply, PlanProgress, TrustAsk, TrustVerdict};

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
