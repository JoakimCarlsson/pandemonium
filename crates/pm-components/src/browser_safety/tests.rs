//! Tests of what the reader's presses do to the questions, the certificate page and the note about a download.

use super::{
    Answer, BROWSER_ALLOW, BROWSER_BLOCK, BROWSER_CERT_BACK, BROWSER_CERT_PROCEED, BROWSER_DISMISS,
    Certificate, Download, Question, Safety, Saving,
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
    let mut safety = Safety::default();
    safety.ask(asked(1, "https://a.example"));
    safety.ask(asked(1, "https://a.example"));
    safety.ask(asked(2, "https://b.example"));
    assert_eq!(safety.questions.len(), 2);
    assert!(safety.press(BROWSER_BLOCK));
    assert!(safety.press(BROWSER_ALLOW));
    assert!(safety.press(BROWSER_ALLOW));
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

/// A question the helper cancelled is no longer asked, and no answer is sent for it.
#[test]
fn a_cancelled_question_is_not_asked() {
    let mut safety = Safety::default();
    safety.ask(asked(7, "https://a.example"));
    safety.cancel(7);
    assert!(safety.questions.is_empty());
    assert!(safety.press(BROWSER_ALLOW));
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
    assert_eq!(safety.bars(), 2.0 * super::BAR);
    assert!(safety.press(BROWSER_DISMISS));
    assert!(safety.download.is_none());
    assert!(!safety.press(pm_input::FocusId(1)));
}
