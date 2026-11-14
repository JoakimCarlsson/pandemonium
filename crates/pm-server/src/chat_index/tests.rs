//! Tests of how the server indexes its sessions' conversations and ranks what a search finds.

use super::*;

/// The prompt event of a turn saying `text`.
fn prompted(text: &str) -> Event {
    Event::Prompted {
        text: text.to_owned(),
        files: Vec::new(),
        checkpoint: false,
        turn: 0,
        saved: None,
    }
}

/// A log of the prompt `prompt` answered by `reply` in pieces, as an agent streams it.
fn exchange(prompt: &str, reply: &[&str]) -> Vec<(u64, Event)> {
    std::iter::once(prompted(prompt))
        .chain(reply.iter().map(|piece| Event::Said((*piece).to_owned())))
        .enumerate()
        .map(|(seq, event)| (seq as u64 + 1, event))
        .collect()
}

/// The sessions numbered by `hits`, in order.
fn numbers(hits: &[ChatHit]) -> Vec<u64> {
    hits.iter().map(|hit| hit.session).collect()
}

/// A reply streamed in pieces is indexed as one message, so words split across pieces are found.
#[test]
fn a_streamed_reply_is_one_message() {
    let log = exchange("hello", &["The cache is ", "invali", "dated on save"]);
    assert_eq!(
        messages(&log),
        vec![
            (ChatField::Prompt, "hello".to_owned()),
            (
                ChatField::Reply,
                "The cache is invalidated on save".to_owned()
            ),
        ]
    );
    let documents = [Document::new(1, "Untitled", 0, &log)];
    let hits = search(&documents, "invalidated", 10);
    assert_eq!(numbers(&hits), vec![1]);
    assert_eq!(hits[0].field, ChatField::Reply);
}

/// Every word must be found, in any order and ignoring case, but in one text.
#[test]
fn every_word_must_be_found_in_one_text() {
    let documents = [
        Document::new(1, "Fix login", 0, &exchange("the Token expires", &["ok"])),
        Document::new(2, "Token", 0, &exchange("expires soon", &["ok"])),
    ];
    assert_eq!(numbers(&search(&documents, "expires token", 10)), vec![1]);
    assert!(search(&documents, "   ", 10).is_empty());
    assert!(search(&documents, "absent", 10).is_empty());
}

/// A title beats a prompt, which beats a reply, and a phrase found whole beats its words found apart.
#[test]
fn titles_prompts_and_phrases_rank_first() {
    let documents = [
        Document::new(1, "Chat", 9, &exchange("hi", &["the parser crashes"])),
        Document::new(2, "Chat", 8, &exchange("why does the parser crash", &[])),
        Document::new(3, "Parser crash", 1, &exchange("hi", &[])),
        Document::new(4, "Crash in the parser", 2, &exchange("hi", &[])),
    ];
    assert_eq!(
        numbers(&search(&documents, "parser crash", 10)),
        vec![3, 4, 2, 1]
    );
}

/// Words found where a word starts beat words found inside others, and a newer session wins a tie.
#[test]
fn word_starts_and_recency_settle_ties() {
    let documents = [
        Document::new(1, "Reformat", 5, &[]),
        Document::new(2, "Format code", 1, &[]),
        Document::new(3, "Format docs", 7, &[]),
    ];
    assert_eq!(numbers(&search(&documents, "format", 10)), vec![3, 2, 1]);
}

/// Each session is listed once, by its best match, and no more than the limit are answered.
#[test]
fn sessions_are_listed_once_within_the_limit() {
    let log = exchange("deploy the app", &["deploying now"]);
    let documents: Vec<Document> = (0..5)
        .map(|number| Document::new(number, "Deploy", number, &log))
        .collect();
    let hits = search(&documents, "deploy", 3);
    assert_eq!(numbers(&hits), vec![4, 3, 2]);
    assert!(hits.iter().all(|hit| hit.field == ChatField::Title));
}

/// A preview shows the text around the match on one line, marked where it was cut.
#[test]
fn a_preview_is_one_line_around_the_match() {
    let long = format!(
        "{}needle\nin the haystack{}",
        "a ".repeat(60),
        " b".repeat(80)
    );
    let documents = [Document::new(1, "Chat", 0, &exchange(&long, &[]))];
    let hits = search(&documents, "needle", 10);
    let preview = &hits[0].preview;
    assert!(preview.starts_with('…'));
    assert!(preview.ends_with('…'));
    assert!(preview.contains("needle in the haystack"));
    assert!(!preview.contains('\n'));
    assert!(preview.chars().count() <= PREVIEW_LENGTH + 2);
}

/// A match after characters whose lower case is longer still previews the right stretch.
#[test]
fn a_preview_survives_case_that_changes_length() {
    let documents = [Document::new(
        1,
        "Chat",
        0,
        &exchange("İİİİ the needle", &[]),
    )];
    let hits = search(&documents, "needle", 10);
    assert!(hits[0].preview.contains("the needle"));
}
