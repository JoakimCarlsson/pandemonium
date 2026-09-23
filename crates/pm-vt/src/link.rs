//! Links on the screen: the ones a program marks, and the ones it only prints.
//!
//! A program that knows it is writing a link says so with OSC 8, and the
//! cells it writes while the link is open carry its [`LinkId`]. Most programs
//! only print the address, so a place that carries no link is read as text
//! and an address found around it is a link all the same.

use std::collections::HashMap;

use crate::grid::Grid;
use crate::selection::Place;

/// The schemes an address printed as plain text is recognised by.
const SCHEMES: [&str; 4] = ["https://", "http://", "ftp://", "mailto:"];

/// Characters that end an address printed as plain text.
const STOPS: &[char] = &['<', '>', '"', '`', '\'', '{', '}', '|', '\\', '^'];

/// Characters an address is not taken to end in, though it may hold them.
const TRAILING: &[char] = &['.', ',', ':', ';', '!', '?'];

/// One target a program has marked cells as linking to.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LinkId(u32);

/// Every target a program has marked, each kept once however often it is used.
#[derive(Default)]
pub struct Links {
    /// The targets, indexed by their id.
    targets: Vec<String>,
    /// The id each target was given.
    ids: HashMap<String, LinkId>,
}

impl Links {
    /// The id of `target`, giving it one if it has none yet.
    pub fn intern(&mut self, target: &str) -> LinkId {
        if let Some(id) = self.ids.get(target) {
            return *id;
        }
        let id = LinkId(self.targets.len() as u32);
        self.targets.push(target.to_owned());
        self.ids.insert(target.to_owned(), id);
        id
    }

    /// Where `id` goes.
    pub fn target(&self, id: LinkId) -> Option<&str> {
        self.targets.get(id.0 as usize).map(String::as_str)
    }
}

/// A link on the screen: where it goes and the cells it is written across.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Link {
    /// Where following it goes.
    pub target: String,
    /// Its first cell.
    pub start: Place,
    /// Its last cell.
    pub end: Place,
}

impl Link {
    /// Whether `place` is one of the cells it is written across.
    pub fn covers(&self, place: Place) -> bool {
        self.start <= place && place <= self.end
    }
}

/// The link written across `place` in `grid`, if one is.
pub fn link_at(grid: &Grid, links: &Links, place: Place) -> Option<Link> {
    let cell = grid.line(place.line)?.cell(place.col)?;
    match cell.link {
        Some(id) => marked(grid, id, links.target(id)?, place),
        None => printed(grid, place),
    }
}

/// The marked link `id` that `place` is part of, spread across its cells.
fn marked(grid: &Grid, id: LinkId, target: &str, place: Place) -> Option<Link> {
    let carries = |place: Place| {
        grid.line(place.line)
            .and_then(|line| line.cell(place.col))
            .is_some_and(|cell| cell.link == Some(id))
    };
    let mut start = place;
    while let Some(before) = grid.step_back(start).filter(|before| carries(*before)) {
        start = before;
    }
    let mut end = place;
    while let Some(after) = grid.step_forward(end).filter(|after| carries(*after)) {
        end = after;
    }
    Some(Link {
        target: target.to_owned(),
        start,
        end,
    })
}

/// The address printed across `place`, read from the whole of its wrapped line.
fn printed(grid: &Grid, place: Place) -> Option<Link> {
    let text = grid.logical_line(place.line);
    let chars: Vec<char> = text.iter().map(|(ch, _)| *ch).collect();
    let at = text.iter().position(|(_, cell)| *cell == place)?;

    let mut from = 0;
    while let Some((start, end)) = next_address(&chars, from) {
        if (start..end).contains(&at) {
            return Some(Link {
                target: chars[start..end].iter().collect(),
                start: text[start].1,
                end: text[end - 1].1,
            });
        }
        from = end;
    }
    None
}

/// The next address in `chars` at or after `from`, as the range it spans.
fn next_address(chars: &[char], from: usize) -> Option<(usize, usize)> {
    (from..chars.len()).find_map(|start| {
        let scheme = SCHEMES
            .iter()
            .find(|scheme| starts_with(&chars[start..], scheme))?;
        let starts_word = start == 0 || !chars[start - 1].is_alphanumeric();
        if !starts_word {
            return None;
        }
        let body = start + scheme.chars().count();
        let end = chars[body..]
            .iter()
            .position(|ch| ch.is_whitespace() || ch.is_control() || STOPS.contains(ch))
            .map_or(chars.len(), |length| body + length);
        let end = trimmed(chars, body, end);
        (end > body).then_some((start, end))
    })
}

/// Where the address in `chars[..end]` really ends, trailing punctuation aside.
///
/// A closing bracket is kept only when the address opened one: a link written
/// inside parentheses does not take the parenthesis that closes them.
fn trimmed(chars: &[char], body: usize, mut end: usize) -> usize {
    while end > body {
        let last = chars[end - 1];
        let unbalanced = match last {
            ')' => count(&chars[body..end], '(') < count(&chars[body..end], ')'),
            ']' => count(&chars[body..end], '[') < count(&chars[body..end], ']'),
            _ => false,
        };
        if TRAILING.contains(&last) || unbalanced {
            end -= 1;
        } else {
            break;
        }
    }
    end
}

/// How many times `ch` occurs in `chars`.
fn count(chars: &[char], ch: char) -> usize {
    chars.iter().filter(|each| **each == ch).count()
}

/// Whether `chars` begins with `prefix`.
fn starts_with(chars: &[char], prefix: &str) -> bool {
    let mut chars = chars.iter();
    prefix
        .chars()
        .all(|expected| chars.next() == Some(&expected))
}
