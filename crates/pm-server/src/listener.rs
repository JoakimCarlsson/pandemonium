//! Listening for clients and running until the process ends.

use std::net::TcpListener;
use std::time::Duration;

use crate::connection::serve;
use crate::core::{BELL, Core};
use crate::credential::write_credential;
use pm_protocol::{Build, Credential};

/// How long the pump waits for the bell before it looks at the agents anyway.
const PUMP: Duration = Duration::from_millis(500);

/// How the server is run.
#[derive(Clone, Debug)]
pub struct Settings {
    /// The address to listen on, such as `127.0.0.1:6868`; port zero lets the system pick one.
    pub listen: String,
    /// The token clients must send, or nothing to make one up.
    pub token: Option<String>,
}

/// A token made of random bytes, as hexadecimal.
fn random_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Listens and serves clients, for as long as the process lives.
pub fn run(settings: Settings) -> std::io::Result<()> {
    let listener = TcpListener::bind(&settings.listen)?;
    let token = match settings.token {
        Some(token) => token,
        None => random_token()?,
    };
    write_credential(&Credential {
        port: listener.local_addr()?.port(),
        token: token.clone(),
        pid: std::process::id(),
        build: Build::current(),
    })?;
    let core = Core::new();
    core.load();
    crate::machine::start_fetching(core.clone());
    core.extensions_changed();
    crate::machine::start_sweeping(core.clone());
    crate::folders::start(core.clone());
    let watching = core.clone();
    std::thread::spawn(move || {
        loop {
            watching.pump();
            BELL.wait(PUMP);
        }
    });
    for stream in listener.incoming().flatten() {
        let (core, token) = (core.clone(), token.clone());
        std::thread::spawn(move || serve(&core, stream, &token));
    }
    Ok(())
}
