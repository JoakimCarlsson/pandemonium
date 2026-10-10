//! Per-channel credit windows, keeping one slow stream from stopping replies.

use std::io;
use std::sync::{Condvar, Mutex};

/// The number of raw chunks permitted in flight on one channel.
const WINDOW: usize = 64;

/// A stream's remaining send credits and disconnect state.
pub(crate) struct Flow {
    /// Credits left and whether the connection has closed.
    state: Mutex<(usize, bool)>,
    /// Wakes producers for credits or disconnect.
    ready: Condvar,
}

impl Flow {
    /// Opens a stream with a bounded initial receive window.
    pub fn new() -> Self {
        Self {
            state: Mutex::new((WINDOW, false)),
            ready: Condvar::new(),
        }
    }

    /// Waits for room for one frame, failing when the transport closes.
    pub fn acquire(&self) -> io::Result<()> {
        let mut state = self.state.lock().unwrap();
        while state.0 == 0 && !state.1 {
            state = self.ready.wait(state).unwrap();
        }
        if state.1 {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "Disconnected"));
        }
        state.0 -= 1;
        Ok(())
    }

    /// Returns one consumed frame's credit to its producer.
    pub fn give(&self) {
        let mut state = self.state.lock().unwrap();
        state.0 = (state.0 + 1).min(WINDOW);
        self.ready.notify_one();
    }

    /// Releases all producers when the connection or stream ends.
    pub fn close(&self) {
        self.state.lock().unwrap().1 = true;
        self.ready.notify_all();
    }
}
