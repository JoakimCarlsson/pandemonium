//! Signing an agent in: the harness runs the login and says which address the reader opens.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use omc_wire::{Command, Event, Opening};

use crate::inprocess::{Harness, Heard};

use crate::agent::Agent;

/// How long a login in a browser has to finish.
const BROWSER: Duration = Duration::from_secs(300);

/// How long the harness has to say it is ready to sign in.
const OPENING: Duration = Duration::from_secs(30);

impl Agent {
    /// Signs the agent in with its credentials kept in `home`, or in the harness's own folder when there is none, opening the login page in the reader's browser and returning once the login is over.
    ///
    /// When the provider signs in with a code entered on a web page, `code` is told the page and the code, so that the reader can be shown them.
    ///
    /// # Errors
    ///
    /// Fails with the reason when the conversation cannot be started, or the login fails.
    pub fn sign_in_in(
        self,
        home: Option<PathBuf>,
        code: &dyn Fn(&str, &str),
    ) -> Result<(), String> {
        let mut harness = Harness::start()?;
        let directory = home
            .clone()
            .or_else(std::env::home_dir)
            .unwrap_or_else(|| ".".into());
        harness.send(&Command::Open(Opening {
            directory,
            home,
            provider: self.provider.to_owned(),
            model: None,
            yolo: false,
            tools: Vec::new(),
            denied: Vec::new(),
            disabled: Vec::new(),
            conversation: None,
            history: None,
            keep: None,
            before: None,
            defaults: Vec::new(),
            languages: false,
            navigation: false,
        }))?;
        harness.send(&Command::Login {
            provider: self.provider.to_owned(),
        })?;
        let deadline = Instant::now() + BROWSER.max(OPENING);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match harness.listen(left) {
                Heard::Event(Event::LoginUrl(address)) => crate::web::open(&address)?,
                Heard::Event(Event::LoginCode { url, code: shown }) => {
                    code(&url, &shown);
                    crate::web::open(&url)?;
                }
                Heard::Event(Event::SignedIn) => return Ok(()),
                Heard::Event(Event::Failed(reason)) => return Err(reason),
                Heard::Event(_) | Heard::Silence => {}
                Heard::Closed(reason) => return Err(reason),
            }
        }
        Err("the login did not finish in time".to_owned())
    }
}
