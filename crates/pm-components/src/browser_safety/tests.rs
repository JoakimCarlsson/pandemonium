//! Tests of what the reader's presses do to the questions, the certificate page and the note about a download.

use std::time::{Duration, Instant};

use super::{
    ALLOW_DELAY, Answer, BROWSER_CERT_BACK, BROWSER_CERT_PROCEED, BROWSER_DISMISS,
    BROWSER_QUESTION_BAR, Certificate, Download, Question, Safety, Saving, allow_id, block_id,
};

/// A question about the camera of `origin`, numbered `id`.
fn asked(id: u64, origin: &str) -> Question {
    Question {
        id,
        origin: origin.to_owned(),
        permission: "camera".to_owned(),
    }
}

/// Allow and Block answer the oldest question, one at a time, and a question asked twice is held once.
#[test]
fn questions_are_answered_in_the_order_they_came() {
    let start = Instant::now();
    let mut safety = Safety::default();
    safety.ask_at(asked(1, "https://a.example"), start);
    safety.ask_at(asked(1, "https://a.example"), start);
    safety.ask_at(asked(2, "https://b.example"), start);
    assert_eq!(safety.questions.len(), 2);
    let later = start + ALLOW_DELAY;
    assert!(safety.press_at(block_id(1), later));
    assert!(safety.press_at(allow_id(2), later + ALLOW_DELAY));
    assert!(safety.questions.is_empty());
    assert_eq!(
        safety.answers,
        vec![
            Answer::Permission {
                id: 1,
                allow: false
            },
            Answer::Permission { id: 2, allow: true },
        ]
    );
}

/// Allow does nothing until half a second after the question first showed, and Block works at once.
#[test]
fn allow_waits_and_block_does_not() {
    let start = Instant::now();
    let mut safety = Safety::default();
    safety.ask_at(asked(1, "https://a.example"), start);
    assert!(!safety.allow_ready(start));
    assert!(!safety.allow_ready(start + ALLOW_DELAY - Duration::from_millis(1)));
    assert!(safety.press_at(allow_id(1), start + Duration::from_millis(100)));
    assert!(safety.answers.is_empty());
    assert_eq!(safety.questions.len(), 1);
    assert!(safety.allow_ready(start + ALLOW_DELAY));
    assert!(safety.press_at(allow_id(1), start + ALLOW_DELAY));
    assert_eq!(
        safety.answers,
        vec![Answer::Permission { id: 1, allow: true }]
    );

    let mut safety = Safety::default();
    safety.ask_at(asked(2, "https://a.example"), start);
    assert!(safety.press_at(block_id(2), start));
    assert_eq!(
        safety.answers,
        vec![Answer::Permission {
            id: 2,
            allow: false
        }]
    );
}

/// A question that takes the place of the one answered, or of one that went away, starts the delay over, so a press meant for the old one cannot allow the new one.
#[test]
fn the_delay_starts_over_when_the_question_changes() {
    let start = Instant::now();
    let mut safety = Safety::default();
    safety.ask_at(asked(1, "https://a.example"), start);
    safety.ask_at(asked(2, "https://b.example"), start);
    let ready = start + Duration::from_secs(5);
    assert!(safety.press_at(block_id(1), ready));
    assert!(!safety.allow_ready(ready));
    assert!(!safety.allow_ready(ready + ALLOW_DELAY - Duration::from_millis(1)));
    assert!(safety.allow_ready(ready + ALLOW_DELAY));

    let mut safety = Safety::default();
    safety.ask_at(asked(1, "https://a.example"), start);
    safety.ask_at(asked(2, "https://b.example"), start);
    safety.cancel_at(1, ready);
    assert!(!safety.allow_ready(ready));
    assert!(safety.allow_ready(ready + ALLOW_DELAY));

    let mut safety = Safety::default();
    safety.ask_at(asked(1, "https://a.example"), start);
    safety.cancel_at(1, ready);
    safety.ask_at(asked(1, "https://a.example"), ready);
    assert!(!safety.allow_ready(ready));
}

/// A press that began on one question does not answer another that took its place before the press ended.
#[test]
fn a_press_answers_only_the_question_it_was_for() {
    let start = Instant::now();
    let ready = start + Duration::from_secs(5);
    let mut safety = Safety::default();
    safety.ask_at(asked(1, "https://a.example"), start);
    let pressing = allow_id(1);
    safety.cancel_at(1, ready);
    safety.ask_at(asked(2, "https://b.example"), ready);
    let much_later = ready + Duration::from_secs(5);
    assert!(safety.press_at(pressing, much_later));
    assert!(safety.answers.is_empty());
    assert_eq!(safety.questions.len(), 1);
    assert!(safety.press_at(block_id(1), much_later));
    assert!(safety.answers.is_empty());
    assert!(safety.press_at(allow_id(2), much_later));
    assert_eq!(
        safety.answers,
        vec![Answer::Permission { id: 2, allow: true }]
    );
}

/// A press on the bar itself, or with no question asked, answers nothing, and the question does not move the page.
#[test]
fn the_bar_takes_presses_and_leaves_the_page_where_it_is() {
    let start = Instant::now();
    let mut safety = Safety::default();
    assert!(safety.press_at(BROWSER_QUESTION_BAR, start));
    assert!(safety.question_overlay(400.0, start).is_none());
    safety.ask_at(asked(3, "https://a.example"), start);
    assert!(safety.press_at(BROWSER_QUESTION_BAR, start + ALLOW_DELAY));
    assert!(safety.press_at(allow_id(3), start + ALLOW_DELAY * 4));
    assert!(safety.answers.len() == 1);
    safety.ask_at(asked(4, "https://a.example"), start);
    assert_eq!(safety.bars(), 0.0);
    assert!(safety.bar_nodes().is_empty());
    assert!(safety.question_overlay(400.0, start).is_some());
}

/// A question the helper cancelled is no longer asked, and no answer is sent for it.
#[test]
fn a_cancelled_question_is_not_asked() {
    let mut safety = Safety::default();
    safety.ask(asked(7, "https://a.example"));
    safety.cancel(7);
    assert!(safety.questions.is_empty());
    assert!(safety.press(allow_id(7)));
    assert!(safety.answers.is_empty());
}

/// Proceeding past a certificate that is not trusted takes two presses, and going back takes one and never proceeds.
#[test]
fn proceeding_past_a_certificate_is_deliberate() {
    let mut safety = Safety {
        certificate: Some(Certificate::default()),
        ..Safety::default()
    };
    assert!(safety.press(BROWSER_CERT_PROCEED));
    assert!(safety.answers.is_empty());
    assert!(
        safety
            .certificate
            .as_ref()
            .is_some_and(|held| held.confirming)
    );
    assert!(safety.press(BROWSER_CERT_PROCEED));
    assert_eq!(safety.answers, vec![Answer::Certificate { proceed: true }]);
    assert!(safety.certificate.is_none());

    let mut safety = Safety {
        certificate: Some(Certificate::default()),
        ..Safety::default()
    };
    assert!(safety.press(BROWSER_CERT_PROCEED));
    assert!(safety.press(BROWSER_CERT_BACK));
    assert_eq!(safety.answers, vec![Answer::Certificate { proceed: false }]);
}

/// The note about a download is taken away by its button, and the bars above the page take room only while there is something in them.
#[test]
fn the_download_note_can_be_dismissed() {
    let mut safety = Safety::default();
    assert_eq!(safety.bars(), 0.0);
    safety.download = Some(Download {
        name: "a.pdf".to_owned(),
        saving: Saving::Finished,
    });
    safety.ask(asked(1, "https://a.example"));
    assert_eq!(safety.bars(), super::BAR);
    assert!(safety.press(BROWSER_DISMISS));
    assert!(safety.download.is_none());
    assert!(!safety.press(pm_input::FocusId(1)));
}
