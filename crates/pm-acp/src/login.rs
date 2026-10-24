//! Probing an agent, and signing one in.

use std::process::Command;
use std::time::Duration;

use serde_json::json;

use crate::agent::Agent;
use crate::client::{Answer, Client, LOGIN_REQUIRED, logins};
use crate::standing::{Login, Standing, Way};

/// How long an agent that is already installed has to answer.
const QUICK: Duration = Duration::from_secs(30);

/// How long an agent that has to be fetched first has to answer.
const FETCH: Duration = Duration::from_secs(180);

/// How long a login in a browser has to finish.
const BROWSER: Duration = Duration::from_secs(300);

/// How long a login in a terminal window has to finish.
#[cfg(target_os = "macos")]
const TERMINAL_LOGIN: Duration = Duration::from_secs(600);

/// The terminals tried when `TERMINAL` names none, with what each takes before the command.
#[cfg(not(target_os = "macos"))]
const TERMINALS: [(&str, &[&str]); 8] = [
    ("x-terminal-emulator", &["-e"]),
    ("kitty", &[]),
    ("alacritty", &["-e"]),
    ("wezterm", &["start", "--"]),
    ("gnome-terminal", &["--wait", "--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("xterm", &["-e"]),
];

impl Agent {
    /// Starts the agent and finds out where it stands: it answers and opens
    /// a conversation, wants a login first, cannot be started, or fails.
    pub fn probe(self) -> Standing {
        if !self.startable() {
            return Standing::Missing;
        }
        let patience = if self.fetches() { FETCH } else { QUICK };
        let mut client = match Client::start(self.command(&[])) {
            Ok(client) => client,
            Err(error) => return Standing::Failed(error),
        };
        let shaken = match client.initialize(patience) {
            Ok(result) => result,
            Err(error) => return Standing::Failed(error),
        };
        let home = std::env::home_dir().unwrap_or_default();
        let opened = client.ask(
            "session/new",
            &json!({ "cwd": home, "mcpServers": [] }),
            patience,
        );
        match opened {
            Ok(Answer::Result(_)) => Standing::Ready,
            Ok(Answer::Error(LOGIN_REQUIRED, _)) => {
                Standing::SignedOut(logins(&shaken["authMethods"]))
            }
            Ok(Answer::Error(_, message)) | Err(message) => Standing::Failed(message),
        }
    }

    /// Signs the agent in by `login`, and returns once that is over.
    ///
    /// A login the agent carries out itself is asked of it and may open a
    /// browser. One that needs a terminal runs the agent in the reader's
    /// terminal, and returns when that terminal is closed.
    pub fn log_in(self, login: &Login) -> Result<(), String> {
        self.log_in_in(login, self.home())
    }

    /// Signs in through the existing login method using an explicit account folder.
    pub(crate) fn log_in_in(
        self,
        login: &Login,
        home: Option<std::path::PathBuf>,
    ) -> Result<(), String> {
        match &login.way {
            Way::Asked => {
                let mut client = Client::start(self.command_in(&[], home))?;
                client.initialize(QUICK.max(FETCH))?;
                match client.ask("authenticate", &json!({ "methodId": login.id }), BROWSER)? {
                    Answer::Result(_) => Ok(()),
                    Answer::Error(_, message) => Err(message),
                }
            }
            Way::Terminal { args } => run_in_terminal(self.command_in(args, home)),
        }
    }
}

/// Runs `command`, with its environment, in a terminal window, and waits for the window to close.
pub(crate) fn run_in_terminal(program: Command) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return run_in_terminal_app(&program);
    #[cfg(not(target_os = "macos"))]
    run_in_terminal_elsewhere(program)
}

/// Runs `program`, with its environment, in the first terminal emulator found, and waits for it to finish.
#[cfg(not(target_os = "macos"))]
fn run_in_terminal_elsewhere(program: Command) -> Result<(), String> {
    let wanted = std::env::var("TERMINAL").ok();
    let candidates = wanted
        .iter()
        .map(|name| (name.as_str(), &["-e"][..]))
        .chain(TERMINALS);
    for (terminal, before) in candidates {
        let mut command = Command::new(terminal);
        command.args(before).arg("env");
        for (name, value) in program.get_envs() {
            if let Some(value) = value {
                let mut pair = name.to_os_string();
                pair.push("=");
                pair.push(value);
                command.arg(pair);
            }
        }
        command.arg(program.get_program()).args(program.get_args());
        if let Ok(mut child) = command.spawn() {
            child.wait().map_err(|error| error.to_string())?;
            return Ok(());
        }
    }
    Err("no terminal to log in with".to_owned())
}

/// Quotes `text` as one word of a shell script.
#[cfg(target_os = "macos")]
fn shell_word(text: &std::ffi::OsStr) -> String {
    format!("'{}'", text.to_string_lossy().replace('\'', "'\\''"))
}

/// Runs `program`, with its environment, in a Terminal window, and waits for it to finish.
#[cfg(target_os = "macos")]
fn run_in_terminal_app(program: &Command) -> Result<(), String> {
    let folder = std::env::home_dir()
        .ok_or("no home folder to keep the login script in")?
        .join(".cache/pandemonium");
    std::os::unix::fs::DirBuilderExt::mode(std::fs::DirBuilder::new().recursive(true), 0o700)
        .create(&folder)
        .map_err(|error| error.to_string())?;
    let script = folder.join(format!("login-{}.command", std::process::id()));
    let finished = script.with_extension("done");
    let mut words = vec!["env".to_owned()];
    for (name, value) in program.get_envs() {
        if let Some(value) = value {
            let mut pair = name.to_os_string();
            pair.push("=");
            pair.push(value);
            words.push(shell_word(&pair));
        }
    }
    words.push(shell_word(program.get_program()));
    words.extend(program.get_args().map(shell_word));
    let lines = [
        "#!/bin/sh".to_owned(),
        words.join(" "),
        format!("touch {}", shell_word(finished.as_os_str())),
        "exit".to_owned(),
    ];
    let _ = std::fs::remove_file(&script);
    let mut file = std::os::unix::fs::OpenOptionsExt::mode(
        std::fs::OpenOptions::new().write(true).create_new(true),
        0o700,
    )
    .open(&script)
    .map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut file, (lines.join("\n") + "\n").as_bytes())
        .map_err(|error| error.to_string())?;
    drop(file);
    let opened = Command::new("open")
        .arg("-a")
        .arg("Terminal")
        .arg(&script)
        .status();
    let outcome = match opened {
        Ok(status) if status.success() => {
            let deadline = std::time::Instant::now() + TERMINAL_LOGIN;
            while !finished.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(500));
            }
            Ok(())
        }
        _ => Err("could not open Terminal to log in with".to_owned()),
    };
    let _ = std::fs::remove_file(&script);
    let _ = std::fs::remove_file(&finished);
    outcome
}
