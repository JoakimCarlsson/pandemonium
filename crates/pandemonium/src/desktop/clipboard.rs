//! Ordered desktop clipboard operations with one persistent owner.

use std::path::PathBuf;
use std::sync::{OnceLock, mpsc};

/// An operation executed by the clipboard's owning thread.
type Operation = Box<dyn FnOnce(Option<&mut arboard::Clipboard>) + Send>;

/// The ordered queue shared by copying and every kind of paste.
static OPERATIONS: OnceLock<mpsc::Sender<Operation>> = OnceLock::new();

/// Queues an operation, keeping the clipboard alive between requests on Linux.
fn submit(operation: impl FnOnce(Option<&mut arboard::Clipboard>) + Send + 'static) {
    let sender = OPERATIONS.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<Operation>();
        std::thread::spawn(move || {
            let mut clipboard = None;
            for operation in receiver {
                if clipboard.is_none() {
                    clipboard = arboard::Clipboard::new().ok();
                }
                operation(clipboard.as_mut());
            }
        });
        sender
    });
    let _ = sender.send(Box::new(operation));
}

/// Reads after earlier writes have completed, returning no value on failure.
fn read<T: Send + 'static>(
    operation: impl FnOnce(&mut arboard::Clipboard) -> Option<T> + Send + 'static,
) -> Option<T> {
    let (sender, receiver) = mpsc::channel();
    submit(move |clipboard| {
        let _ = sender.send(clipboard.and_then(operation));
    });
    receiver.recv().ok().flatten()
}

/// Puts text on the clipboard without blocking the window on its owner.
pub fn copy(text: String) {
    submit(move |clipboard| {
        if let Some(clipboard) = clipboard {
            let _ = clipboard.set_text(text);
        }
    });
}

/// Puts HTML and its plain-text alternative on the same ordered clipboard.
pub fn copy_html(html: String, alt_text: String) {
    submit(move |clipboard| {
        if let Some(clipboard) = clipboard {
            let _ = clipboard.set().html(html, Some(alt_text));
        }
    });
}

/// Returns clipboard text after pending copies have been published.
pub fn paste() -> Option<String> {
    read(|clipboard| clipboard.get_text().ok())
}

/// Returns existing clipboard files in clipboard order.
pub fn paste_files() -> Option<Vec<PathBuf>> {
    read(|clipboard| {
        let files: Vec<_> = clipboard
            .get()
            .file_list()
            .ok()?
            .into_iter()
            .filter(|path| path.is_file())
            .collect();
        (!files.is_empty()).then_some(files)
    })
}

/// Returns the clipboard image's width, height and straight-alpha RGBA pixels.
pub fn paste_image() -> Option<(u32, u32, Vec<u8>)> {
    read(|clipboard| {
        let image = clipboard.get_image().ok()?;
        Some((
            u32::try_from(image.width).ok()?,
            u32::try_from(image.height).ok()?,
            image.bytes.into_owned(),
        ))
    })
}
