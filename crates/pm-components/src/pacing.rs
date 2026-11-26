//! The pace a reply being streamed is shown at: what has arrived is revealed a whole word at a
//! time at a steady rate, faster when it falls behind, and the words just revealed fade in.

use std::collections::VecDeque;
use std::time::Duration;

/// How many bytes of text are revealed each second while little is waiting.
const STEADY_RATE: f32 = 90.0;

/// How far behind what has arrived the text shown may fall: the rate rises so what waits is
/// revealed within this.
const MOST_BEHIND: f32 = 0.25;

/// The longest time one frame moves the pace on by, so text that arrives after a pause is
/// not all revealed at once.
const LONGEST_STEP: Duration = Duration::from_millis(50);

/// How long the words just revealed take to fade in.
const FADE: Duration = Duration::from_millis(150);

/// How many characters at the end of what is shown fade in at most.
const FADING_CHARACTERS: usize = 40;

/// How a text being revealed stands: how much of it is shown, and when its newest pieces were.
#[derive(Clone, Debug, Default, PartialEq)]
struct Pace {
    /// How many bytes of the text are shown.
    revealed: usize,
    /// Where each piece revealed within the last fade starts, in bytes, with when it was.
    pieces: VecDeque<(usize, Duration)>,
    /// The time of the latest frame the pace moved on at.
    now: Duration,
    /// How many bytes the rate has earned and not yet spent on revealing words; less than
    /// nought after a word longer than was earned.
    credit: f32,
}

/// How much of a text being streamed is shown now and how its newest characters fade.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    /// How many bytes from the start are shown.
    pub len: usize,
    /// The newest characters that are still fading in, as how many characters from the end
    /// each run of them starts, the oldest first, with how opaque the run is, from 0 to 1.
    pub fades: Vec<(usize, f32)>,
}

impl Shown {
    /// All of `text`, with nothing fading.
    fn all(text: &str) -> Self {
        Self {
            len: text.len(),
            fades: Vec::new(),
        }
    }
}

/// How much of `text` to show while `streaming`: all of it, with nothing fading, once the
/// stream ends or for what had arrived when the text was first shown; otherwise what the pace
/// has revealed of it, moved on frame by frame by an effect that asks for frames only while
/// text waits to be revealed or fades in.
#[track_caller]
pub fn paced(text: &str, streaming: bool) -> Shown {
    let pace = rustle::remember(|| Pace {
        revealed: text.len(),
        ..Pace::default()
    });
    let target = text.len();
    let start = pace.with(|pace| floor_boundary(text, pace.revealed.min(target)));
    let waiting = text[start..].to_owned();
    let driven = pace.clone();
    rustle::launched_effect((target, streaming), move |scope| async move {
        let settled = driven.with(|pace| pace.revealed >= target && pace.pieces.is_empty());
        if !streaming || settled {
            return;
        }
        loop {
            let now = scope.next_frame().await;
            let mut next = driven.with(Pace::clone);
            let step = now.saturating_sub(next.now).min(LONGEST_STEP);
            next.revealed = next.revealed.clamp(start, target);
            let behind = target - next.revealed;
            if behind > 0 {
                let rate = STEADY_RATE.max(behind as f32 / MOST_BEHIND);
                next.credit = (next.credit + rate * step.as_secs_f32()).min(behind as f32);
                if next.credit >= 1.0 {
                    let reach = next.credit as usize;
                    let to = word_end(&waiting, next.revealed - start + reach) + start;
                    next.credit -= (to - next.revealed) as f32;
                    next.pieces.push_back((next.revealed, now));
                    next.revealed = to;
                }
            } else {
                next.credit = 0.0;
            }
            while next
                .pieces
                .front()
                .is_some_and(|(_, at)| now.saturating_sub(*at) >= FADE)
            {
                next.pieces.pop_front();
            }
            next.now = now;
            let done = next.revealed >= target && next.pieces.is_empty();
            driven.set(next);
            if done {
                return;
            }
        }
    });
    if !streaming {
        return Shown::all(text);
    }
    pace.with(|pace| shown(text, pace))
}

/// What of `text` `pace` shows, with the alpha of each piece still fading in.
fn shown(text: &str, pace: &Pace) -> Shown {
    let len = floor_boundary(text, pace.revealed.min(text.len()));
    let mut fades = Vec::new();
    let mut from_end = 0;
    let mut ends = len;
    for (start, at) in pace.pieces.iter().rev() {
        let start = floor_boundary(text, (*start).min(ends));
        from_end += text[start..ends].chars().count();
        ends = start;
        if from_end > FADING_CHARACTERS {
            break;
        }
        let alpha = (pace.now.saturating_sub(*at).as_secs_f32() / FADE.as_secs_f32()).min(1.0);
        if alpha < 1.0 {
            fades.push((from_end, alpha));
        }
    }
    fades.reverse();
    Shown { len, fades }
}

/// The greatest character boundary of `text` at or before `at`.
fn floor_boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// Where the word of `text` that byte `at` falls in ends, with the white space after it, so
/// what is revealed never stops part way through a word; the end of `text` when no word
/// ends after `at`.
fn word_end(text: &str, at: usize) -> usize {
    if at >= text.len() {
        return text.len();
    }
    let rest = &text[floor_boundary(text, at)..];
    let Some(space) = rest.find(char::is_whitespace) else {
        return text.len();
    };
    let after = &rest[space..];
    let blank = after.len() - after.trim_start().len();
    text.len() - after.len() + blank
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A word is revealed whole, with the white space after it.
    #[test]
    fn words_are_revealed_whole() {
        assert_eq!(word_end("hello world again", 2), 6);
        assert_eq!(word_end("hello world again", 7), 12);
        assert_eq!(word_end("hello wor", 7), 9);
        assert_eq!(word_end("héllo wörld", 2), 7);
    }

    /// The pieces revealed within the fade are faded by their age, the newest the faintest.
    #[test]
    fn the_newest_pieces_fade() {
        let text = "one two three";
        let pace = Pace {
            revealed: text.len(),
            credit: 0.0,
            pieces: VecDeque::from([
                (4, Duration::from_millis(1000)),
                (8, Duration::from_millis(1100)),
            ]),
            now: Duration::from_millis(1120),
        };
        let shown = shown(text, &pace);
        assert_eq!(shown.len, text.len());
        assert_eq!(shown.fades.len(), 2);
        assert_eq!(shown.fades[1].0, 5);
        assert_eq!(shown.fades[0].0, 9);
        assert!(shown.fades[1].1 < shown.fades[0].1);
    }
}
