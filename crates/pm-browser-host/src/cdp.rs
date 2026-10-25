//! The DevTools protocol of one page, spoken inside this process without any port.
//!
//! Commands this program sends carry ids from [`INTERNAL`] up and have their
//! answers handed to a function; everything else the engine reports, the
//! answers to the DevTools page's own commands and the events, goes to the
//! subscribers, which is how the DevTools page is shown a page.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use cef::*;
use serde_json::{Value, json};

/// The first id of the commands this program sends; the DevTools page counts up from one.
const INTERNAL: i64 = 1 << 30;

/// What is done with the answer to a command.
type Reply = Box<dyn FnOnce(&Value) + Send>;

/// What is done with an event: its method and its parameters.
type Listener = Box<dyn Fn(&str, &Value) + Send>;

/// The protocol of one page.
pub(crate) struct Cdp {
    /// The id the next command gets.
    next: AtomicI64,
    /// The commands waiting for their answers.
    waiting: Mutex<HashMap<i64, Reply>>,
    /// Who hears every event.
    listeners: Mutex<Vec<Listener>>,
    /// Where messages for the DevTools page go.
    subscribers: Mutex<Vec<Sender<String>>>,
    /// Keeps the engine reporting to this layer while the page lives.
    registration: Mutex<Option<Registration>>,
}

wrap_dev_tools_message_observer! {
    struct Tap {
        cdp: Arc<Cdp>,
    }

    impl DevToolsMessageObserver {
        fn on_dev_tools_message(
            &self,
            _browser: Option<&mut Browser>,
            message: Option<&[u8]>,
        ) -> ::std::os::raw::c_int {
            if let Some(message) = message {
                self.cdp.deliver(message);
            }
            true as _
        }
    }
}

impl Cdp {
    /// A layer with nobody attached.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            next: AtomicI64::new(INTERNAL),
            waiting: Mutex::new(HashMap::new()),
            listeners: Mutex::new(Vec::new()),
            subscribers: Mutex::new(Vec::new()),
            registration: Mutex::new(None),
        })
    }

    /// Starts hearing what the engine says about the page of `host`.
    pub(crate) fn attach(self: &Arc<Self>, host: &BrowserHost) {
        let mut tap = Tap::new(self.clone());
        let registration = host.add_dev_tools_message_observer(Some(&mut tap));
        if let Ok(mut held) = self.registration.lock() {
            *held = registration;
        }
    }

    /// Sends `method` with `params` to the page of `host` and hands the answer, with its `result` or `error`, to `reply`.
    pub(crate) fn call(
        &self,
        host: &BrowserHost,
        method: &str,
        params: &Value,
        reply: impl FnOnce(&Value) + Send + 'static,
    ) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut waiting) = self.waiting.lock() {
            waiting.insert(id, Box::new(reply));
        }
        let message = json!({ "id": id, "method": method, "params": params }).to_string();
        host.send_dev_tools_message(Some(message.as_bytes()));
    }

    /// Sends a message the DevTools page wrote, as it is.
    pub(crate) fn send_raw(host: &BrowserHost, message: &str) {
        host.send_dev_tools_message(Some(message.as_bytes()));
    }

    /// Has `sender` given everything the DevTools page should be shown.
    pub(crate) fn subscribe(&self, sender: Sender<String>) {
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.push(sender);
        }
    }

    /// Has `listener` told of every event the page reports.
    pub(crate) fn listen(&self, listener: impl Fn(&str, &Value) + Send + 'static) {
        if let Ok(mut listeners) = self.listeners.lock() {
            listeners.push(Box::new(listener));
        }
    }

    /// Takes in one message from the engine.
    fn deliver(&self, message: &[u8]) {
        let text = String::from_utf8_lossy(message).into_owned();
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            return;
        };
        if let Some(id) = value["id"].as_i64() {
            if id >= INTERNAL {
                let reply = self.waiting.lock().ok().and_then(|mut w| w.remove(&id));
                if let Some(reply) = reply {
                    reply(&value);
                }
                return;
            }
        } else if let Some(method) = value["method"].as_str()
            && let Ok(listeners) = self.listeners.lock()
        {
            for listener in listeners.iter() {
                listener(method, &value["params"]);
            }
        }
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.retain(|sender| sender.send(text.clone()).is_ok());
        }
    }
}
