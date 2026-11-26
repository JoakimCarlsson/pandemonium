//! A thread of its own that runs the work handed to it one piece after another, so that slow work on a text does not hold up the thread that draws it.

use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

/// A piece of work to run.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// A named thread, started on first use, that runs work in the order it was handed over.
pub struct Worker {
    /// What the thread is called.
    name: &'static str,
    /// Where work for the thread goes, once it runs.
    queue: OnceLock<Option<Sender<Job>>>,
}

impl Worker {
    /// A worker whose thread is called `name`, not started until work is first handed to it.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            queue: OnceLock::new(),
        }
    }

    /// Hands `job` to the thread, after the work handed to it before; runs it on this thread when no thread could be started.
    pub fn run(&self, job: impl FnOnce() + Send + 'static) {
        let queue = self.queue.get_or_init(|| {
            let (sender, receiver) = channel::<Job>();
            std::thread::Builder::new()
                .name(self.name.to_owned())
                .spawn(move || {
                    for job in receiver {
                        job();
                    }
                })
                .ok()
                .map(|_| sender)
        });
        let job: Job = Box::new(job);
        match queue {
            Some(sender) => {
                if let Err(returned) = sender.send(job) {
                    (returned.0)();
                }
            }
            None => job(),
        }
    }
}
