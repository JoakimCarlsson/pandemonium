//! Pictures decoded away from the window.
//!
//! Reading a photograph off the disk and decoding it takes as long as a
//! dozen frames, and a transcript restored with a handful of screenshots in
//! it takes as long as a hundred. So no picture is decoded where a frame is
//! drawn: a pane asks for one, is told it is on its way, and draws it the
//! frame after the pool has woken the window with it.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

use pm_gfx::Image;

/// How many pictures are decoded at once, at most.
const WORKERS: usize = 4;

/// One piece of work for the pool.
type Job = Box<dyn FnOnce() + Send>;

/// Where the pool's threads take their work from.
static POOL: OnceLock<Sender<Job>> = OnceLock::new();

/// What wakes the window once a picture has been decoded.
static WAKE: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Has the window woken through `wake` whenever a picture is decoded.
pub fn wake_with(wake: Arc<dyn Fn() + Send + Sync>) {
    let _ = WAKE.set(wake);
}

/// Runs `job` on the pool, starting the pool the first time.
fn spawn(job: impl FnOnce() + Send + 'static) {
    let pool = POOL.get_or_init(|| {
        let (send, receive) = mpsc::channel::<Job>();
        let receive = Arc::new(Mutex::new(receive));
        let workers =
            std::thread::available_parallelism().map_or(1, |cores| cores.get().min(WORKERS));
        for _ in 0..workers {
            let receive = receive.clone();
            std::thread::spawn(move || work(&receive));
        }
        send
    });
    let _ = pool.send(Box::new(job));
}

/// Takes jobs off `receive` and runs them until the pool is gone, a job
/// that panics costing that job and not the thread.
fn work(receive: &Mutex<Receiver<Job>>) {
    loop {
        let job = match receive.lock() {
            Ok(receive) => receive.recv(),
            Err(_) => return,
        };
        match job {
            Ok(job) => {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            }
            Err(_) => return,
        }
    }
}

/// Wakes the window, when something has said how.
fn wake() {
    if let Some(wake) = WAKE.get() {
        wake();
    }
}

/// Where one picture has got to.
#[derive(Clone, Debug)]
pub enum Decoding {
    /// Still being read or decoded.
    Pending,
    /// Decoded.
    Ready(Image),
    /// Could not be read or decoded, and why.
    Failed(String),
}

impl Decoding {
    /// The picture, once it is decoded.
    pub fn ready(self) -> Option<Image> {
        match self {
            Self::Ready(image) => Some(image),
            _ => None,
        }
    }
}

/// One picture's place: where it has got to, and which asking it answers.
struct Slot {
    /// Counts the times it was asked for, so an answer to an earlier asking
    /// that comes back late is not taken for this one.
    asked: u64,
    /// Where it has got to.
    state: Decoding,
}

/// Pictures by `K`, each decoded on the pool the first time it is asked for.
///
/// Asking takes `&self`, so a screen built from the window's state without
/// changing it can still start the pictures it draws.
pub struct Decodes<K> {
    /// Every picture asked for and not forgotten.
    slots: Arc<Mutex<HashMap<K, Slot>>>,
}

impl<K> Default for Decodes<K> {
    /// Nothing asked for.
    fn default() -> Self {
        Self {
            slots: Arc::default(),
        }
    }
}

impl<K: Clone + Eq + Hash + Send + 'static> Decodes<K> {
    /// Where the picture `key` names has got to, starting it with the bytes
    /// `read` comes back with when it was never asked for.
    pub fn get(
        &self,
        key: &K,
        read: impl FnOnce() -> Result<Vec<u8>, String> + Send + 'static,
    ) -> Decoding {
        if let Some(slot) = self.lock().get(key) {
            return slot.state.clone();
        }
        self.start(key.clone(), read);
        Decoding::Pending
    }

    /// Where the picture `key` names has got to, if it was ever asked for.
    pub fn peek(&self, key: &K) -> Option<Decoding> {
        self.lock().get(key).map(|slot| slot.state.clone())
    }

    /// Decodes the bytes `read` comes back with as the picture `key` names,
    /// whether or not it was asked for before.
    pub fn start(&self, key: K, read: impl FnOnce() -> Result<Vec<u8>, String> + Send + 'static) {
        let asked = {
            let mut slots = self.lock();
            let slot = slots.entry(key.clone()).or_insert(Slot {
                asked: 0,
                state: Decoding::Pending,
            });
            slot.asked += 1;
            slot.state = Decoding::Pending;
            slot.asked
        };
        let slots = self.slots.clone();
        spawn(move || {
            let state = match read() {
                Ok(bytes) => Image::decode(&bytes).map_or_else(
                    || {
                        Decoding::Failed(
                            "This file is not a picture the editor can read".to_owned(),
                        )
                    },
                    Decoding::Ready,
                ),
                Err(error) => Decoding::Failed(error),
            };
            if let Ok(mut slots) = slots.lock()
                && let Some(slot) = slots.get_mut(&key)
                && slot.asked == asked
            {
                slot.state = state;
                drop(slots);
                wake();
            }
        });
    }

    /// Keeps `image` as the picture `key` names, decoded already.
    pub fn put(&self, key: K, image: Image) {
        let mut slots = self.lock();
        let asked = slots.get(&key).map_or(0, |slot| slot.asked + 1);
        slots.insert(
            key,
            Slot {
                asked,
                state: Decoding::Ready(image),
            },
        );
    }

    /// Forgets every picture `keep` does not hold on to.
    pub fn retain(&self, keep: impl Fn(&K) -> bool) {
        self.lock().retain(|key, _| keep(key));
    }

    /// Forgets every picture.
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// The pictures, locked; a pool thread that panicked holding them leaves
    /// them as it found them, since it only ever writes a whole slot.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<K, Slot>> {
        self.slots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Reads the file at `path`, saying why when it cannot.
pub fn read_file(
    path: std::path::PathBuf,
) -> impl FnOnce() -> Result<Vec<u8>, String> + Send + 'static {
    move || std::fs::read(&path).map_err(|error| error.to_string())
}
