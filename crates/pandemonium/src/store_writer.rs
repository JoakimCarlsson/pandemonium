//! Writes the window's files, such as its layout and settings, on a thread
//! of their own, so the UI thread never waits on the disk, and skips texts
//! that are what was last written.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// How long the writer waits for more texts after one arrives, so a burst
/// of changes, such as a sash dragged, is written once.
const SETTLE: Duration = Duration::from_millis(150);

thread_local! {
    /// The way to the writer thread, started on first use.
    static WRITER: RefCell<Option<Sender<(String, String)>>> = const { RefCell::new(None) };
    /// The text last handed over for each file, by name.
    static LAST: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// Has the file `name` of the store hold `text`, written soon on the writer
/// thread; does nothing when `text` is what was last handed over or what
/// the file held when first asked.
pub fn save(name: &str, text: String) {
    let unchanged = LAST.with(|last| {
        let mut last = last.borrow_mut();
        let held = last
            .entry(name.to_owned())
            .or_insert_with(|| pm_store::load_text(name).unwrap_or_default());
        if *held == text {
            return true;
        }
        held.clone_from(&text);
        false
    });
    if unchanged {
        return;
    }
    WRITER.with(|writer| {
        let mut writer = writer.borrow_mut();
        let sender = writer.get_or_insert_with(start);
        if sender.send((name.to_owned(), text.clone())).is_err() {
            *sender = start();
            let _ = sender.send((name.to_owned(), text));
        }
    });
}

/// Starts the writer thread and returns the way to it.
fn start() -> Sender<(String, String)> {
    let (sender, receiver) = channel();
    std::thread::Builder::new()
        .name("store-writer".to_owned())
        .spawn(move || write_all(&receiver))
        .ok();
    sender
}

/// Writes each text `receiver` brings to its file, taking only the latest
/// of those that arrive close together, until the window drops its end.
fn write_all(receiver: &Receiver<(String, String)>) {
    while let Ok((name, text)) = receiver.recv() {
        let mut waiting = HashMap::from([(name, text)]);
        while let Ok((name, text)) = receiver.recv_timeout(SETTLE) {
            waiting.insert(name, text);
        }
        for (name, text) in waiting {
            if let Err(error) = pm_store::save_text(&name, &text) {
                eprintln!("pandemonium: could not save {name}: {error}");
            }
        }
    }
}

/// Writes `text` to the file `name` of the store at once, on this thread,
/// for the last write as the window closes.
///
/// # Errors
///
/// What kept the file from being written.
pub fn save_now(name: &str, text: &str) -> std::io::Result<()> {
    LAST.with(|last| last.borrow_mut().insert(name.to_owned(), text.to_owned()));
    pm_store::save_text(name, text)
}
