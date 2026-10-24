//! Ordered background persistence with a single slot for the latest pending view.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use super::View;

/// The state shared with the single persistence worker.
#[derive(Default)]
struct Pending {
    /// The newest view not yet written, replacing any older queued one.
    view: Option<View>,
    /// Whether serialization or a disk write is in progress.
    busy: bool,
    /// Whether the worker should finish after its last pending write.
    closing: bool,
}

/// Writes window state away from the event loop, preserving submission order.
pub struct Writer {
    /// Latest pending state and worker status.
    pending: Arc<(Mutex<Pending>, Condvar)>,
    /// The worker, joined on shutdown so the final draft is saved.
    thread: Option<JoinHandle<()>>,
}

impl Default for Writer {
    /// Starts one worker for window-state serialization and disk writes.
    fn default() -> Self {
        Self::new(|view| view.save())
    }
}

impl Writer {
    /// Runs `save` sequentially, keeping at most one additional view pending.
    fn new(save: impl Fn(View) + Send + 'static) -> Self {
        let pending = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let shared = pending.clone();
        let thread = std::thread::spawn(move || {
            let (lock, ready) = &*shared;
            loop {
                let mut state = lock.lock().expect("view writer lock");
                while state.view.is_none() && !state.closing {
                    state = ready.wait(state).expect("view writer wait");
                }
                let Some(view) = state.view.take() else {
                    return;
                };
                state.busy = true;
                drop(state);
                save(view);
                let mut state = lock.lock().expect("view writer lock");
                state.busy = false;
                ready.notify_all();
            }
        });
        Self {
            pending,
            thread: Some(thread),
        }
    }

    /// Replaces the pending view without waiting for disk or growing a queue.
    pub fn queue(&self, view: View) {
        let (lock, ready) = &*self.pending;
        lock.lock().expect("view writer lock").view = Some(view);
        ready.notify_all();
    }

    /// Waits for the latest submitted view when the window exits.
    pub fn flush(&self) {
        let (lock, ready) = &*self.pending;
        let mut state = lock.lock().expect("view writer lock");
        while state.view.is_some() || state.busy {
            state = ready.wait(state).expect("view writer wait");
        }
    }
}

impl Drop for Writer {
    /// Finishes queued work and joins the worker before process shutdown.
    fn drop(&mut self) {
        let (lock, ready) = &*self.pending;
        lock.lock().expect("view writer lock").closing = true;
        ready.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Tests use an in-memory sink instead of the reader's data directory.
#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    /// A blocked disk write does not block queuing; intermediate drafts are coalesced in order.
    #[test]
    fn coalesces_and_flushes_latest_view() {
        let (written, observed) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let writer = Writer::new(move |view| {
            let first = view.prompt == "first";
            written.send(view.prompt).unwrap();
            if first {
                blocked.recv().unwrap();
            }
        });
        writer.queue(View {
            prompt: "first".to_owned(),
            ..View::default()
        });
        assert_eq!(observed.recv().unwrap(), "first");
        writer.queue(View {
            prompt: "second".to_owned(),
            ..View::default()
        });
        writer.queue(View {
            prompt: "last".to_owned(),
            ..View::default()
        });
        release.send(()).unwrap();
        writer.flush();
        assert_eq!(observed.recv().unwrap(), "last");
        assert!(observed.try_recv().is_err());
    }
}
