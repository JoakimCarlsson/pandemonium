//! The command line: what `:` runs.
//!
//! A command is a range and a name: `:%s/a/b/g`, `:'<,'>d`, `:.,+3>`,
//! `:g/TODO/d`. The range is read the way vim reads it — numbers, `.`, `$`,
//! `%`, marks, searches, each with offsets — and a name may be cut short to
//! any prefix vim accepts for it.

use pm_text::{Position, Selection};

use crate::action::Command;
use crate::engine::cursors::heads;
use crate::engine::{Effect, Stage, Vim};
use crate::key::{Key, Keystroke};
use crate::mode::Mode;
use crate::operator::Span;
use crate::register::Filling;
use crate::search::{LastSearch, Pattern, replacement};
use crate::text::first_non_blank;

/// The commands by name, each with the shortest prefix that names it.
const NAMES: [(&str, &str); 36] = [
    ("w", "write"),
    ("wa", "wall"),
    ("wq", "wq"),
    ("wqa", "wqall"),
    ("xa", "xall"),
    ("x", "xit"),
    ("q", "quit"),
    ("qa", "qall"),
    ("clo", "close"),
    ("sp", "split"),
    ("vs", "vsplit"),
    ("new", "new"),
    ("vne", "vnew"),
    ("tabn", "tabnext"),
    ("tabp", "tabprevious"),
    ("tabN", "tabNext"),
    ("bn", "bnext"),
    ("bp", "bprevious"),
    ("bN", "bNext"),
    ("bd", "bdelete"),
    ("noh", "nohlsearch"),
    ("d", "delete"),
    ("y", "yank"),
    ("pu", "put"),
    ("m", "move"),
    ("t", "t"),
    ("co", "copy"),
    ("j", "join"),
    ("s", "substitute"),
    ("g", "global"),
    ("v", "vglobal"),
    ("norm", "normal"),
    ("sor", "sort"),
    ("u", "undo"),
    ("red", "redo"),
    ("le", "left"),
];

/// Where a command line stands while it is read.
struct Reader<'a> {
    /// What is left of it.
    rest: &'a str,
}

impl Reader<'_> {
    /// Skips blanks.
    fn blanks(&mut self) {
        self.rest = self.rest.trim_start();
    }

    /// The next character, without reading it.
    fn peek(&self) -> Option<char> {
        self.rest.chars().next()
    }

    /// Reads the next character.
    fn take(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.rest = &self.rest[ch.len_utf8()..];
        Some(ch)
    }

    /// Reads a number, if one comes next.
    fn number(&mut self) -> Option<usize> {
        let digits = self.rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let (number, rest) = self.rest.split_at(digits);
        self.rest = rest;
        number.parse().ok()
    }

    /// Reads up to `end`, unescaped, taking the `end` too.
    fn until(&mut self, end: char) -> String {
        let mut text = String::new();
        let mut escaped = false;
        while let Some(ch) = self.take() {
            if !escaped && ch == end {
                break;
            }
            escaped = !escaped && ch == '\\';
            text.push(ch);
        }
        text
    }
}

impl Vim {
    /// Runs the command line `line`.
    pub(crate) fn ex(&mut self, stage: &mut Stage, line: &str) {
        let from_visual = stage.state.mode.is_visual();
        if from_visual {
            self.leave_visual(stage);
        }
        stage.state.mode = Mode::Normal;
        stage.state.regions.clear();
        self.run_ex(stage, line, None);
    }

    /// Runs one command, with `default` as its range when it names none.
    fn run_ex(&mut self, stage: &mut Stage, line: &str, default: Option<(usize, usize)>) {
        let mut reader = Reader {
            rest: line.trim_start_matches([' ', ':']),
        };
        let Some(range) = self.range(stage, &mut reader) else {
            return;
        };
        reader.blanks();
        let name = reader
            .rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphabetic())
            .collect::<String>();
        let name = match (name.as_str(), reader.rest.chars().next()) {
            ("", Some(ch @ ('&' | '>' | '<' | '!' | '='))) => ch.to_string(),
            _ => name,
        };
        reader.rest = &reader.rest[name.len()..];
        let bang = reader.peek() == Some('!');
        if bang {
            reader.take();
        }
        let head = stage.buffer.selection().head.line;
        let lines = range.or(default).unwrap_or((head, head));
        let command = full_name(&name);
        if command.is_none() && name.is_empty() {
            if let Some((_, last)) = range {
                self.go_to_line(stage, last);
            }
            return;
        }
        let command = command.unwrap_or(&name);
        let argument = reader.rest.trim().to_owned();
        let effect = |command: Command| Effect::Command(command);
        match command {
            "write" => stage.effects.push(effect(Command::Save)),
            "wall" => stage.effects.push(effect(Command::SaveAll)),
            "wq" | "xit" => stage
                .effects
                .extend([effect(Command::Save), effect(Command::Close)]),
            "wqall" | "xall" => stage
                .effects
                .extend([effect(Command::SaveAll), effect(Command::Close)]),
            "quit" | "qall" | "close" | "bdelete" => stage.effects.push(effect(Command::Close)),
            "split" | "new" => stage.effects.push(effect(Command::SplitDown)),
            "vsplit" | "vnew" => stage.effects.push(effect(Command::SplitRight)),
            "tabnext" | "bnext" => stage.effects.push(effect(Command::NextTab)),
            "tabprevious" | "tabNext" | "bprevious" | "bNext" => {
                stage.effects.push(effect(Command::PreviousTab))
            }
            "nohlsearch" => self.highlight = false,
            "undo" => {
                stage.buffer.undo();
            }
            "redo" => {
                stage.buffer.redo();
            }
            "delete" | "yank" => {
                let (register, count) = register_and_count(&argument);
                let lines = counted(lines, count, stage.buffer.line_count());
                let span = Span::lines(lines.0, lines.1);
                let text = span.text(stage.buffer);
                match command {
                    "delete" => {
                        self.keep(stage, register, text, Filling::Delete);
                        let range = span.removal(stage.buffer);
                        stage.buffer.grouped(|buffer| buffer.replace(range, ""));
                        self.go_to_line(stage, lines.0);
                    }
                    _ => self.keep(stage, register, text, Filling::Yank),
                }
            }
            "put" => {
                let (register, _) = register_and_count(&argument);
                if let Some(held) = self.registers.read(register, &mut *stage.clipboard) {
                    let text = match held.text.ends_with('\n') {
                        true => held.text,
                        false => format!("{}\n", held.text),
                    };
                    let line = if bang { lines.1 } else { lines.1 + 1 };
                    insert_lines(stage, line, &text);
                    self.go_to_line(stage, line);
                }
            }
            "move" | "copy" | "t" => {
                let mut target = Reader { rest: &argument };
                let Some(Some((_, to))) = self.range(stage, &mut target) else {
                    return;
                };
                let Some(to) = to_line(&argument, to) else {
                    return;
                };
                let text = Span::lines(lines.0, lines.1).text(stage.buffer);
                let count = lines.1 - lines.0 + 1;
                stage.buffer.grouped(|buffer| match command {
                    "move" => {
                        if (lines.0..=lines.1).contains(&to) && to != usize::MAX {
                            return;
                        }
                        let removal = Span::lines(lines.0, lines.1).removal(buffer);
                        let into = match to {
                            usize::MAX => 0,
                            to if to > lines.1 => to + 1 - count,
                            to => to + 1,
                        };
                        buffer.replace(removal, "");
                        insert_text(buffer, into, &text);
                    }
                    _ => insert_text(buffer, to.wrapping_add(1), &text),
                });
                let landed = match (command, to) {
                    ("move", usize::MAX) => count - 1,
                    ("move", to) if to > lines.1 => to,
                    (_, usize::MAX) => count - 1,
                    (_, to) => to + count,
                };
                self.go_to_line(
                    stage,
                    landed.min(stage.buffer.line_count().saturating_sub(1)),
                );
            }
            "join" => {
                let (first, last) = match lines.0 == lines.1 {
                    true => (lines.0, lines.0 + 1),
                    false => lines,
                };
                if last >= stage.buffer.line_count() {
                    return;
                }
                stage.buffer.grouped(|buffer| {
                    for _ in first..last {
                        let end = Position::new(first, buffer.line_len(first));
                        match bang {
                            true => buffer.replace(end..Position::new(first + 1, 0), ""),
                            false => {
                                buffer.set_selection(Selection::at(end));
                                buffer.join_lines();
                            }
                        }
                    }
                });
                self.go_to_line(stage, first);
            }
            ">" | "<" => {
                let times = 1 + argument
                    .chars()
                    .filter(|ch| ch.to_string() == command)
                    .count();
                crate::operator::shift(
                    stage.buffer,
                    Span::lines(lines.0, lines.1),
                    command == ">",
                    times,
                );
                self.go_to_line(stage, lines.1);
            }
            "=" => crate::format::reindent(stage.buffer, lines.0, lines.1),
            "substitute" | "&" => self.substitute(stage, lines, &argument, command == "&"),
            "global" | "vglobal" => self.global(
                stage,
                lines,
                &argument,
                bang || command == "vglobal",
                range.is_none(),
            ),
            "normal" => self.normal_lines(stage, lines, reader.rest.trim_start()),
            "sort" => sort(stage, lines, bang, &argument),
            "left" => {
                let edits = (lines.0..=lines.1)
                    .map(|line| {
                        (
                            Position::new(line, 0)
                                ..Position::new(line, first_non_blank(stage.buffer, line)),
                            String::new(),
                        )
                    })
                    .collect();
                stage.buffer.apply_edits(edits);
            }
            _ => {}
        }
    }

    /// Reads a range: `%`, or one or two addresses apart by `,` or `;`.
    ///
    /// Answers `None` inside when the line names no range, and `None`
    /// outside when it names one that cannot be read.
    fn range(&mut self, stage: &Stage, reader: &mut Reader) -> Option<Option<(usize, usize)>> {
        reader.blanks();
        if reader.peek() == Some('%') {
            reader.take();
            return Some(Some((0, stage.buffer.line_count().saturating_sub(1))));
        }
        let Some(first) = self.address(stage, reader)? else {
            return Some(None);
        };
        reader.blanks();
        match reader.peek() {
            Some(',' | ';') => {
                reader.take();
                let last = self.address(stage, reader)?.unwrap_or(first);
                Some(Some((first.min(last), first.max(last))))
            }
            _ => Some(Some((first, first))),
        }
    }

    /// Reads one address and its offsets, as a line counted from zero.
    fn address(&mut self, stage: &Stage, reader: &mut Reader) -> Option<Option<usize>> {
        reader.blanks();
        let head = stage.buffer.selection().head.line;
        let last = stage.buffer.line_count().saturating_sub(1);
        let base = match reader.peek() {
            Some('.') => {
                reader.take();
                Some(head)
            }
            Some('$') => {
                reader.take();
                Some(last)
            }
            Some(ch) if ch.is_ascii_digit() => Some(reader.number()?.saturating_sub(1)),
            Some('\'') => {
                reader.take();
                let name = reader.take()?;
                Some(stage.state.marks.get(&name)?.line)
            }
            Some(separator @ ('/' | '?')) => {
                reader.take();
                let pattern = Pattern::typed(&reader.until(separator));
                let from = stage.buffer.char_of(Position::new(head, 0));
                let found = pattern.find(stage.buffer, from, separator == '/')?;
                Some(stage.buffer.position_of(found).line)
            }
            Some('+' | '-') => Some(head),
            _ => None,
        };
        let Some(mut line) = base else {
            return Some(None);
        };
        loop {
            reader.blanks();
            match reader.peek() {
                Some('+') => {
                    reader.take();
                    line = line.saturating_add(reader.number().unwrap_or(1));
                }
                Some('-') => {
                    reader.take();
                    line = line.saturating_sub(reader.number().unwrap_or(1));
                }
                _ => break,
            }
        }
        Some(Some(line.min(last)))
    }

    /// Puts the cursor on the first non-blank of `line`.
    fn go_to_line(&mut self, stage: &mut Stage, line: usize) {
        let line = line.min(stage.buffer.line_count().saturating_sub(1));
        let column = first_non_blank(stage.buffer, line);
        let from = stage.buffer.selection().head;
        stage.state.marks.insert('\'', from);
        stage
            .buffer
            .set_selection(Selection::at(Position::new(line, column)));
    }

    /// `:s/pattern/replacement/flags` on `lines`, or the last one again for
    /// `:&`, `&&` keeping its flags.
    fn substitute(
        &mut self,
        stage: &mut Stage,
        lines: (usize, usize),
        argument: &str,
        again: bool,
    ) {
        let written = match again {
            true => {
                let Some(last) = self.last_substitute.clone() else {
                    return;
                };
                match argument.starts_with('&') {
                    true => last,
                    false => {
                        let cut = last.rfind(last.chars().next().unwrap_or('/')).unwrap_or(0);
                        format!("{}{}", &last[..=cut], argument)
                    }
                }
            }
            false => argument.to_owned(),
        };
        let mut reader = Reader { rest: &written };
        let Some(separator) = reader
            .take()
            .filter(|ch| !ch.is_alphanumeric() && !ch.is_whitespace())
        else {
            return;
        };
        let source = reader.until(separator);
        let with = reader.until(separator);
        let flags = reader.rest.trim().to_owned();
        let source = match source.is_empty() {
            true => match &self.last_search {
                Some(search) => search.pattern.source.clone(),
                None => return,
            },
            false => source,
        };
        self.last_substitute = Some(written.clone());
        let pattern = Pattern::typed(&source);
        self.last_search = Some(LastSearch {
            pattern: pattern.clone(),
            forward: true,
        });
        let Some(regex) = pattern_with_flags(&pattern, &flags) else {
            return;
        };
        let every = flags.contains('g');
        let with = replacement(&with);
        let mut edits = Vec::new();
        for line in lines.0..=lines.1.min(stage.buffer.line_count().saturating_sub(1)) {
            let text = stage.buffer.line_text(line);
            let replaced = match every {
                true => regex.replace_all(&text, with.as_str()).into_owned(),
                false => regex.replace(&text, with.as_str()).into_owned(),
            };
            if replaced != text {
                edits.push((
                    Position::new(line, 0)..Position::new(line, stage.buffer.line_len(line)),
                    replaced,
                ));
            }
        }
        let Some(last) = edits.last().map(|(range, _)| range.start.line) else {
            return;
        };
        let lines_before = stage.buffer.line_count();
        stage.buffer.apply_edits(edits);
        let grown = stage.buffer.line_count() as isize - lines_before as isize;
        self.go_to_line(stage, last.saturating_add_signed(grown));
    }

    /// `:g/pattern/command`, or `:v` for the lines that do not match: runs
    /// the command on each such line of `lines`, the whole file when no
    /// range was given.
    fn global(
        &mut self,
        stage: &mut Stage,
        lines: (usize, usize),
        argument: &str,
        inverted: bool,
        whole: bool,
    ) {
        let mut reader = Reader { rest: argument };
        let Some(separator) = reader
            .take()
            .filter(|ch| !ch.is_alphanumeric() && !ch.is_whitespace())
        else {
            return;
        };
        let source = reader.until(separator);
        let command = match reader.rest.trim() {
            "" => "p".to_owned(),
            command => command.to_owned(),
        };
        let pattern = Pattern::typed(&source);
        let Some(regex) = pattern.regex() else {
            return;
        };
        let lines = match whole {
            true => (0, stage.buffer.line_count().saturating_sub(1)),
            false => lines,
        };
        let mut marked = (lines.0..=lines.1)
            .filter(|line| regex.is_match(&stage.buffer.line_text(*line)) != inverted)
            .collect::<Vec<_>>();
        let depth = stage.buffer.undo_depth();
        let mut index = 0;
        while index < marked.len() {
            let line = marked[index];
            if line >= stage.buffer.line_count() {
                break;
            }
            let before = stage.buffer.line_count();
            stage
                .buffer
                .set_selection(Selection::at(Position::new(line, 0)));
            self.run_ex(stage, &command, Some((line, line)));
            let delta = stage.buffer.line_count() as isize - before as isize;
            for later in marked.iter_mut().skip(index + 1) {
                *later = later.saturating_add_signed(delta);
            }
            index += 1;
        }
        stage.buffer.squash_since(depth);
        self.last_search = Some(LastSearch {
            pattern,
            forward: true,
        });
    }

    /// `:normal keys`: plays `keys` in normal mode at the start of each of
    /// `lines`, as though they were typed there.
    fn normal_lines(&mut self, stage: &mut Stage, lines: (usize, usize), keys: &str) {
        let keys = keys
            .chars()
            .map(|ch| Keystroke::plain(Key::Char(ch)))
            .collect::<Vec<_>>();
        let depth = stage.buffer.undo_depth();
        let count = lines.1 - lines.0 + 1;
        let total = stage.buffer.line_count();
        for offset in 0..count {
            let shift = stage.buffer.line_count() as isize - total as isize;
            let line = (lines.0 + offset).saturating_add_signed(shift.min(0));
            if line >= stage.buffer.line_count() {
                break;
            }
            stage
                .buffer
                .set_selection(Selection::at(Position::new(line, 0)));
            stage.state.mode = Mode::Normal;
            self.playing += 1;
            for key in &keys {
                self.feed(stage, *key);
            }
            if stage.state.mode != Mode::Normal {
                self.feed(stage, Keystroke::plain(Key::Escape));
            }
            self.playing -= 1;
            stage.state.pending = Default::default();
        }
        stage.buffer.squash_since(depth);
        let _ = heads(stage.buffer);
    }
}

/// The full name a command's written name stands for, if it names one.
fn full_name(written: &str) -> Option<&'static str> {
    if written.is_empty() {
        return None;
    }
    if let Some(symbol) = ["&", ">", "<", "=", "t"]
        .into_iter()
        .find(|symbol| *symbol == written)
    {
        return Some(symbol);
    }
    NAMES
        .iter()
        .find(|(short, full)| {
            written.len() >= short.len() && full.starts_with(written) && written.starts_with(short)
        })
        .map(|(_, full)| *full)
}

/// The register and the count written after `:d` or `:y`.
fn register_and_count(argument: &str) -> (Option<char>, Option<usize>) {
    let mut chars = argument.trim().chars().peekable();
    let register = chars.peek().copied().filter(|ch| !ch.is_ascii_digit());
    if register.is_some() {
        chars.next();
    }
    let count = chars.collect::<String>().trim().parse().ok();
    (register, count)
}

/// `lines` narrowed to `count` lines from their last, as `:d 3` counts.
fn counted(lines: (usize, usize), count: Option<usize>, total: usize) -> (usize, usize) {
    match count {
        Some(count) if count > 0 => (lines.1, (lines.1 + count - 1).min(total.saturating_sub(1))),
        _ => lines,
    }
}

/// The line `:m` and `:t` put lines after, `usize::MAX` meaning above the
/// first line, as `0` writes it.
fn to_line(argument: &str, parsed: usize) -> Option<usize> {
    match argument.trim() {
        "0" => Some(usize::MAX),
        _ => Some(parsed),
    }
}

/// Puts `text`, whole lines, in at line `line` of the buffer.
fn insert_lines(stage: &mut Stage, line: usize, text: &str) {
    stage
        .buffer
        .grouped(|buffer| insert_text(buffer, line, text));
}

/// Puts `text`, whole lines, in above `line`, or below the last line when
/// `line` is past it.
fn insert_text(buffer: &mut pm_text::Buffer, line: usize, text: &str) {
    let count = buffer.line_count();
    match line < count {
        true => {
            let at = Position::new(line, 0);
            buffer.replace(at..at, text);
        }
        false => {
            let last = count.saturating_sub(1);
            let at = Position::new(last, buffer.line_len(last));
            buffer.replace(
                at..at,
                &format!("\n{}", text.strip_suffix('\n').unwrap_or(text)),
            );
        }
    }
}

/// `pattern` as a regular expression, with `:s`'s `i` and `I` flags.
fn pattern_with_flags(pattern: &Pattern, flags: &str) -> Option<regex::Regex> {
    let source = match (flags.contains('i'), flags.contains('I')) {
        (true, _) => format!("\\c{}", pattern.source),
        (_, true) => format!("\\C{}", pattern.source),
        _ => pattern.source.clone(),
    };
    Pattern::typed(&source).regex()
}

/// `:sort` on `lines`: reversed with `!`, ignoring case with `i`, by number
/// with `n`, keeping one of each with `u`.
fn sort(stage: &mut Stage, lines: (usize, usize), reverse: bool, flags: &str) {
    let (first, last) = match lines.0 == lines.1 {
        true => (0, stage.buffer.line_count().saturating_sub(1)),
        false => lines,
    };
    let mut texts = (first..=last)
        .map(|line| stage.buffer.line_text(line))
        .collect::<Vec<_>>();
    let number = |text: &str| -> i64 {
        let digits = text
            .chars()
            .skip_while(|ch| !ch.is_ascii_digit() && *ch != '-')
            .take_while(|ch| ch.is_ascii_digit() || *ch == '-')
            .collect::<String>();
        digits.parse().unwrap_or(i64::MIN)
    };
    match (flags.contains('n'), flags.contains('i')) {
        (true, _) => texts.sort_by_key(|text| number(text)),
        (false, true) => texts.sort_by_key(|text| text.to_lowercase()),
        (false, false) => texts.sort(),
    }
    if flags.contains('u') {
        texts.dedup_by(|left, right| match flags.contains('i') {
            true => left.eq_ignore_ascii_case(right),
            false => left == right,
        });
    }
    if reverse {
        texts.reverse();
    }
    let range = Position::new(first, 0)..Position::new(last, stage.buffer.line_len(last));
    let sorted = texts.join("\n");
    stage
        .buffer
        .grouped(|buffer| buffer.replace(range, &sorted));
    stage
        .buffer
        .set_selection(Selection::at(Position::new(first, 0)));
}
