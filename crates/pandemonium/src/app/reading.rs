//! Asking git about the worktrees without holding a frame up.
//!
//! An agent writes into its worktree and runs git in it several times a
//! second, and every one of those is news for the review of that worktree
//! and the drift of every session. The asking is a subprocess or more a
//! file, so it runs on a thread of its own and wakes the window with the
//! answer; one worktree is read at most once at a time, and news that comes
//! in while it is being read is read again once that reading is back.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use pm_core::{Scope, SessionId, Summary};

use crate::app::{App, Wake};
use crate::review::Reading;

/// What one read away from the window came back with.
enum Read {
    /// A worktree's review, read whole.
    Review(Scope, Reading),
    /// How far every session has drifted.
    Drift(Vec<(SessionId, Summary)>),
}

/// The reads running away from the window, and what they came back with.
#[derive(Default)]
pub(super) struct Readings {
    /// What has come back and not been taken in yet.
    done: Arc<Mutex<Vec<Read>>>,
    /// The worktrees whose review is being read now.
    reading: BTreeSet<Scope>,
    /// The worktrees whose review was asked for again while it was read.
    again: BTreeSet<Scope>,
    /// Whether the sessions' drift is being read now.
    drifting: bool,
    /// Whether it was asked for again while it was read.
    drift_again: bool,
}

impl App {
    /// Asks git again what `scope`'s worktree holds, and shows it once git
    /// has answered.
    pub(super) fn reread_review_later(&mut self, scope: Scope) {
        if self.readings.reading.contains(&scope) {
            self.readings.again.insert(scope);
            return;
        }
        let Some(read) = self
            .reviews
            .get(&scope)
            .map(crate::review::Review::read_later)
        else {
            return;
        };
        self.readings.reading.insert(scope);
        self.spawn_read(move || Read::Review(scope, read()));
    }

    /// Asks git again how far every session has drifted, and shows it once
    /// git has answered.
    pub(super) fn reread_drift_later(&mut self) {
        if self.readings.drifting {
            self.readings.drift_again = true;
            return;
        }
        let read = self.sessions.read_drift();
        self.readings.drifting = true;
        self.spawn_read(move || Read::Drift(read()));
    }

    /// Runs `read` on a thread of its own, waking the window with what it
    /// came back with.
    fn spawn_read(&self, read: impl FnOnce() -> Read + Send + 'static) {
        let done = self.readings.done.clone();
        let wake = self.waker(Wake::Reading);
        std::thread::spawn(move || {
            let read = read();
            if let Ok(mut done) = done.lock() {
                done.push(read);
            }
            wake();
        });
    }

    /// Takes in every read that has come back, answering whether any did,
    /// and starts again the ones asked for while they ran.
    pub(super) fn take_readings(&mut self) -> bool {
        let done = self
            .readings
            .done
            .lock()
            .map(|mut done| std::mem::take(&mut *done))
            .unwrap_or_default();
        let any = !done.is_empty();
        for read in done {
            match read {
                Read::Review(scope, reading) => self.take_review(scope, reading),
                Read::Drift(drifts) => self.take_drift(drifts),
            }
        }
        any
    }

    /// Puts `reading` into `scope`'s review and everything drawn from it.
    fn take_review(&mut self, scope: Scope, reading: Reading) {
        self.readings.reading.remove(&scope);
        if self
            .reviews
            .get_mut(&scope)
            .is_some_and(|review| review.take(reading))
        {
            self.repaint_reviews();
            self.refresh_excerpts_of(scope);
        }
        if self.readings.again.remove(&scope) {
            self.reread_review_later(scope);
        }
    }

    /// Puts how far the sessions have drifted into their rows.
    fn take_drift(&mut self, drifts: Vec<(SessionId, Summary)>) {
        self.readings.drifting = false;
        self.sessions.drifted(drifts);
        if std::mem::take(&mut self.readings.drift_again) {
            self.reread_drift_later();
        }
    }
}
