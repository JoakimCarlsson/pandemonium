//! What the browser pane shows for the reader's safety: the bar with a page's question about the reader's machine, the note about the last download, and the page that stands in for a site whose certificate is not trusted.

use std::rc::Rc;
use std::time::Instant;

use rustle::prelude::*;

use pm_components::{Button, ButtonVariant, IconButton, Label, LabelVariant, Size};
use pm_shell::{BrowserAction, Certificate, Download, Question, Safety, Saving};

/// How tall the bar with a question or a note is, in logical pixels.
const BAR_HEIGHT: f32 = 40.0;

/// How wide the card of the certificate page is at most, in logical pixels.
const CARD_WIDTH: f32 = 560.0;

/// What a callback of the pane does with an action of the browser.
type Act = Rc<dyn Fn(BrowserAction)>;

/// A callback that hands `action` to `act` each time it is called.
fn send(act: &Act, action: BrowserAction) -> impl Fn() + 'static {
    let act = act.clone();
    move || act(action.clone())
}

/// The bar laid over the top of the page with the first `question` and `waiting` more after it; Allow is inert until the question has shown long enough.
pub fn question_bar(question: &Question, waiting: usize, safety: &Safety, act: &Act) {
    let scheme = color_scheme();
    let ready = safety.allow_ready(Instant::now());
    let mut words = question.words();
    if waiting > 0 {
        words.push_str(&format!(" (+{waiting} more)"));
    }
    let id = question.id;
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(BAR_HEIGHT)
            .background(scheme.surface_container)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(12.0, 0.0)
            .clickable(|| {}),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        || {
            Label::new(words)
                .size(Size::Sm)
                .modifier(Modifier::new().weight(1.0))
                .show();
            Button::new("Block")
                .variant(ButtonVariant::Secondary)
                .size(Size::Sm)
                .on_click(send(act, BrowserAction::Permission { id, allow: false }))
                .show();
            Button::new("Allow")
                .variant(ButtonVariant::Primary)
                .size(Size::Sm)
                .disabled(!ready)
                .on_click(send(act, BrowserAction::Permission { id, allow: true }))
                .show();
        },
    );
}

/// The bar that says where the last download was saved, or that it could not be.
pub fn download_bar(download: &Download, act: &Act) {
    let scheme = color_scheme();
    let failed = download.saving == Saving::Failed;
    row_arranged(
        Modifier::new()
            .fill_max_width()
            .height(BAR_HEIGHT)
            .background(scheme.surface_container)
            .border_bottom_hairline(scheme.outline_variant)
            .padding_xy(12.0, 0.0),
        Arrangement::SpacedBy(pm_components::spacing(8.0)),
        Alignment::Center,
        || {
            let label = Label::new(download.words())
                .size(Size::Sm)
                .modifier(Modifier::new().weight(1.0));
            if failed {
                label.colour(scheme.error).show();
            } else {
                label.show();
            }
            IconButton::new(phosphor::X)
                .size(Size::Xs)
                .on_click(send(act, BrowserAction::DismissDownload))
                .show();
        },
    );
}

/// A row of the certificate's details: `label`, and what it says in `value`.
fn detail(label: &str, value: &str, code: bool) {
    let value = if value.is_empty() { "unknown" } else { value };
    row_arranged(
        Modifier::new().fill_max_width(),
        Arrangement::SpacedBy(pm_components::spacing(12.0)),
        Alignment::Start,
        || {
            Label::new(label)
                .variant(LabelVariant::Muted)
                .size(Size::Xs)
                .modifier(Modifier::new().width(96.0))
                .show();
            Label::new(value)
                .variant(if code {
                    LabelVariant::Code
                } else {
                    LabelVariant::Body
                })
                .size(Size::Xs)
                .wrap()
                .modifier(Modifier::new().weight(1.0))
                .show();
        },
    );
}

/// The page that stands in for a site whose certificate is not trusted: what is wrong and the buttons that go back or, deliberately, go on.
pub fn certificate_page(certificate: &Certificate, act: &Act) {
    let scheme = color_scheme();
    column_arranged(
        Modifier::new()
            .fill_max_size()
            .background(scheme.background)
            .padding(24.0),
        Arrangement::Center,
        Alignment::Center,
        || {
            column_arranged(
                Modifier::new()
                    .width_in(0.0, CARD_WIDTH)
                    .background(scheme.surface_container)
                    .border(1.0, scheme.outline_variant, 12.0)
                    .padding(20.0),
                Arrangement::SpacedBy(pm_components::spacing(12.0)),
                Alignment::Start,
                || {
                    Label::new("Not secure: certificate not trusted")
                        .variant(LabelVariant::Heading)
                        .colour(scheme.error)
                        .size(Size::Md)
                        .show();
                    Label::new(format!(
                        "Pandemonium could not verify that {} is who it says it is. Anything you send to it could be read or changed on the way.",
                        certificate.host()
                    ))
                    .size(Size::Sm)
                    .wrap()
                    .show();
                    Label::new(format!("{} ({})", certificate.reason(), certificate.error))
                        .variant(LabelVariant::Muted)
                        .size(Size::Xs)
                        .wrap()
                        .show();
                    detail("Site", &certificate.url, false);
                    detail("Issued to", &certificate.subject, false);
                    detail("Issued by", &certificate.issuer, false);
                    detail("Expires", &certificate.expires, false);
                    detail("SHA-256", &certificate.fingerprint, true);
                    row_arranged(
                        Modifier::new(),
                        Arrangement::SpacedBy(pm_components::spacing(8.0)),
                        Alignment::Center,
                        || {
                            Button::new("Go back")
                                .variant(ButtonVariant::Primary)
                                .size(Size::Sm)
                                .on_click(send(act, BrowserAction::CertificateBack))
                                .show();
                            let (label, variant) = if certificate.confirming {
                                ("Yes, proceed anyway (unsafe)", ButtonVariant::Danger)
                            } else {
                                ("Proceed anyway (unsafe)", ButtonVariant::Secondary)
                            };
                            Button::new(label)
                                .variant(variant)
                                .size(Size::Sm)
                                .on_click(send(act, BrowserAction::CertificateProceed))
                                .show();
                        },
                    );
                    Label::new(if certificate.confirming {
                        "Press again to load the site. This is remembered for this site until the browser closes."
                    } else {
                        "Going on is not recommended."
                    })
                    .variant(LabelVariant::Muted)
                    .size(Size::Xs)
                    .wrap()
                    .show();
                },
            );
        },
    );
}
