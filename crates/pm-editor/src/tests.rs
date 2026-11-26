//! Tests that the text, its index of lines, the change told for each edit, undo and the rows lines wrap into stay as working everything out afresh would give.

use std::ops::Range;

use pm_buffer::{Change, Lines};

use crate::{Document, Edit, Motion};

/// A sequence of numbers that looks random and is the same every run.
struct Dice(u64);

impl Dice {
    /// A number below `below`, which must not be nought.
    fn roll(&mut self, below: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % below as u64) as usize
    }
}

/// The nearest character boundary of `text` at or before `at`.
fn boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// An edit chosen by `dice` for a document holding `text`.
fn some_edit(dice: &mut Dice, text: &str) -> Edit {
    let pieces = ["a", "é", "\n", "🦀x", "  ", "\n\nfn", "", "b\nc"];
    let at = boundary(text, dice.roll(text.len() + 1));
    let to = boundary(text, at + dice.roll(8));
    match dice.roll(11) {
        0 => Edit::Insert('q'),
        1 => Edit::Insert('\n'),
        2 => Edit::Backspace,
        3 => Edit::Delete,
        4 => Edit::Paste(pieces[dice.roll(pieces.len())].to_owned()),
        5 => Edit::Replace(at..to, pieces[dice.roll(pieces.len())].to_owned(), (at, at)),
        6 => Edit::Undo,
        7 => Edit::Redo,
        8 => Edit::SelectAll,
        9 => Edit::Select(at, to),
        _ => Edit::Move(Motion::LineDown, false),
    }
}

/// Every edit keeps the index of the lines as a fresh one, tells the change comparing the texts gives, and keeps undo, redo and whether the file is dirty right.
#[test]
fn edits_follow_the_text_step_by_step() {
    let original = "fn main() {\n    let x = 1;\n}\n\nsome é text\n";
    let mut document = Document::new(original, false, false);
    let mut dice = Dice(0x9e37_79b9_7f4a_7c15);
    let mut seen = vec![original.to_owned()];
    document.synced = true;
    for _ in 0..3000 {
        let before = document.field.text().to_owned();
        let before_lines = Lines::new(&before);
        let edit = some_edit(&mut dice, &before);
        let edited = document.edit(edit);
        let after = document.field.text().to_owned();
        assert_eq!(document.line_index(), &Lines::new(&after));
        let between = Change::between(&before, &after);
        assert_eq!(edited.is_some(), between.is_some());
        if let (Some(edited), Some(change)) = (&edited, &between) {
            let told = Document::text_change(change, &before, &before_lines);
            assert_eq!(edited.change.as_ref().map(|(_, said)| said), Some(&told));
        }
        assert_eq!(document.is_dirty(), after != original);
        seen.push(after);
    }
    while document.field.can_undo() {
        document.edit(Edit::Undo);
        assert_eq!(document.line_index(), &Lines::new(document.field.text()));
    }
    assert!(seen.iter().any(|text| text == document.field.text()));
}

/// Undo takes back a change at every caret in one step, and redo makes it again.
#[test]
fn undo_takes_back_changes_at_every_caret() {
    let mut document = Document::new("one\ntwo\nthree", false, false);
    document.edit(Edit::Select(0, 0));
    document.edit(Edit::AddSelection(4, 4));
    document.edit(Edit::AddSelection(8, 8));
    document.edit(Edit::Paste("> ".to_owned()));
    assert_eq!(document.field.text(), "> one\n> two\n> three");
    document.edit(Edit::Undo);
    assert_eq!(document.field.text(), "one\ntwo\nthree");
    assert!(!document.is_dirty());
    document.edit(Edit::Redo);
    assert_eq!(document.field.text(), "> one\n> two\n> three");
    let depth = document.field.undo_depth();
    document.edit(Edit::Paste("x".to_owned()));
    document.edit(Edit::Paste("y".to_owned()));
    document.field.squash_undo_since(depth);
    document.edit(Edit::Undo);
    assert_eq!(document.field.text(), "> one\n> two\n> three");
}

/// The rows the lines that show take, worked out line by line as the index stands for them.
fn naive_rows(document: &Document, columns: Option<usize>) -> Vec<(usize, usize)> {
    document
        .shown_lines(0, document.lines())
        .into_iter()
        .map(|index| (index, document.line_rows(index, columns)))
        .collect()
}

/// The rows the index tells match those worked out line by line, with folds and as the text is edited.
#[test]
fn wrapped_rows_match_the_lines_counted_one_by_one() {
    let mut text = String::new();
    for line in 0..60 {
        text.push_str(&"word ".repeat(line % 13));
        text.push_str(if line % 5 == 0 { "{\n" } else { "\n" });
    }
    let mut document = Document::new(&text, false, false);
    let mut dice = Dice(42);
    for round in 0..200 {
        let before = document.field.text().to_owned();
        document.edit(some_edit(&mut dice, &before));
        if round % 25 == 0 {
            let caret = document.line(dice.roll(document.lines())).start;
            document.edit(Edit::Place(caret, false));
            document.fold_at_caret();
        }
        for columns in [None, Some(7), Some(20)] {
            let rows = naive_rows(&document, columns);
            let total: usize = match columns {
                Some(_) => rows.iter().map(|(_, rows)| rows).sum(),
                None => document.rows(),
            };
            assert_eq!(document.wrapped_rows(columns), total);
            let mut row = 0;
            for (index, taken) in &rows {
                assert_eq!(document.wrapped_row_of_line(*index, columns), row);
                let wanted = if columns.is_some() { *taken } else { 1 };
                let (lines, start) = document.wrapped_lines(row + wanted - 1, 1, columns);
                assert_eq!((lines.first().copied(), start), (Some(*index), row));
                row += wanted;
            }
            assert_eq!(
                document.wrapped_lines(row + 3, 2, columns),
                (Vec::new(), row)
            );
            let range: Range<usize> = 0..rows.len().min(5);
            assert_eq!(
                document.shown_lines(range.start, range.len()),
                rows[range]
                    .iter()
                    .map(|(index, _)| *index)
                    .collect::<Vec<_>>()
            );
        }
    }
}
