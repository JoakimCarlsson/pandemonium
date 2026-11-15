//! The numbers the window gives sessions, shells and debugging sessions of
//! other machines, so they never clash with this machine's, and the
//! rewriting of requests and answers between those numbers and the ones
//! each machine's server knows, with addresses on another machine sent to
//! that machine as the folders its server knows.

use std::collections::HashMap;

use pm_protocol::{Request, Response};
use serde_json::Value;

/// The first number given to something of another machine; this machine's servers count far below it.
pub(super) const REMOTE_BASE: u64 = 1 << 48;

/// The fields of requests and answers that number something a server keeps.
const NUMBERED: [&str; 3] = ["session", "terminal", "debug"];

/// The fields of requests that name a folder or file on a server's machine.
const ADDRESSED: [&str; 4] = ["directory", "repository", "project", "path"];

/// The numbers given to things of other machines, both ways.
#[derive(Default)]
pub(super) struct Routes {
    /// The window's number of each thing, by its machine's host, its kind and its number there.
    local: HashMap<(String, &'static str, u64), u64>,
    /// The machine's host and the number there of each thing, by the window's number.
    remote: HashMap<u64, (String, u64)>,
}

impl Routes {
    /// The window's number for the `kind` numbered `number` on the server of `host`, given one the first time.
    pub(super) fn local(&mut self, host: &str, kind: &'static str, number: u64) -> u64 {
        let next = REMOTE_BASE + self.remote.len() as u64;
        let id = *self
            .local
            .entry((host.to_owned(), kind, number))
            .or_insert(next);
        self.remote
            .entry(id)
            .or_insert_with(|| (host.to_owned(), number));
        id
    }

    /// The host and the number there of what the window numbers `id`, when it belongs to another machine.
    pub(super) fn remote(&self, id: u64) -> Option<(&str, u64)> {
        self.remote
            .get(&id)
            .map(|(host, number)| (host.as_str(), *number))
    }

    /// The host of the machine `request` is for and the request as its server knows it, when it names something of another machine; `None` when it is for this machine.
    pub(super) fn outgoing(&self, request: &Request) -> Option<(String, Request)> {
        let mut value = serde_json::to_value(request).ok()?;
        let fields = value.as_object_mut()?;
        let mut host = None;
        for field in NUMBERED {
            if let Some(Value::Number(number)) = fields.get(field)
                && let Some((owner, there)) = number.as_u64().and_then(|id| self.remote(id))
            {
                host = Some(owner.to_owned());
                fields.insert(field.to_owned(), Value::from(there));
            }
        }
        for field in ADDRESSED {
            if let Some(Value::String(address)) = fields.get(field)
                && let Some((owner, folder)) = pm_protocol::host_of(address)
            {
                let folder = format!("/{}", folder.trim_start_matches('/'));
                host = Some(owner.to_owned());
                fields.insert(field.to_owned(), Value::String(folder));
            }
        }
        let host = host?;
        Some((host, serde_json::from_value(value).ok()?))
    }

    /// `response` from the server of `host` with what it numbers given the window's numbers.
    pub(super) fn incoming(&mut self, host: &str, response: Response) -> Response {
        let Ok(mut value) = serde_json::to_value(&response) else {
            return response;
        };
        let Some(fields) = value.as_object_mut() else {
            return response;
        };
        for field in NUMBERED {
            if let Some(number) = fields.get(field).and_then(Value::as_u64) {
                let id = self.local(host, field, number);
                fields.insert(field.to_owned(), Value::from(id));
            }
        }
        serde_json::from_value(value).unwrap_or(response)
    }
}

/// The address of `folder` on the machine with `host`, as projects on it are named.
pub(super) fn address_on(host: &str, folder: &str) -> String {
    let folder = folder.trim_start_matches('/');
    match pm_protocol::route_of(Some(host)) {
        pm_protocol::Route::Ssh(_) => format!("{}{host}/{folder}", pm_protocol::SSH_SCHEME),
        _ => format!("{host}/{folder}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Numbers of another machine get the window's own, and requests naming them go back to that machine with its numbers.
    #[test]
    fn requests_for_other_machines_go_there() {
        let mut routes = Routes::default();
        let id = routes.local("devbox", "session", 4);
        assert!(id >= REMOTE_BASE);
        assert_eq!(routes.local("devbox", "session", 4), id);
        assert_ne!(routes.local("devbox", "terminal", 4), id);
        let (host, request) = routes
            .outgoing(&Request::Cancel { session: id })
            .expect("routed");
        assert_eq!(host, "devbox");
        assert_eq!(request, Request::Cancel { session: 4 });
        assert!(routes.outgoing(&Request::Cancel { session: 4 }).is_none());
    }

    /// A folder on another machine is sent to it as the folder its server knows.
    #[test]
    fn addresses_go_to_their_machine() {
        let routes = Routes::default();
        let address = address_on("devbox", "/home/me/app");
        assert_eq!(address, "ssh://devbox/home/me/app");
        let (host, request) = routes
            .outgoing(&Request::Tasks { directory: address })
            .expect("routed");
        assert_eq!(host, "devbox");
        assert_eq!(
            request,
            Request::Tasks {
                directory: "/home/me/app".to_owned()
            }
        );
    }

    /// What a machine's server numbers in an answer is given the window's number.
    #[test]
    fn answers_take_the_windows_numbers() {
        let mut routes = Routes::default();
        let answered = routes.incoming("devbox", Response::Terminal { terminal: 2 });
        let Response::Terminal { terminal } = answered else {
            panic!("a terminal answer");
        };
        assert_eq!(routes.remote(terminal), Some(("devbox", 2)));
    }
}
