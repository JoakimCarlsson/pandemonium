//! A conversation with one agent: what the reader says goes in, what the agent does comes out.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use omc_wire::{Command, Opening, ToolServer};

use crate::inprocess::{Harness, Heard, Stopper};

use crate::agent::Agent;
use crate::event::{Event, Reply, Verdict};
use crate::language::{HubLanguages, LanguageServers};

/// How long the driver waits for the harness before looking at its commands again.
const LISTEN: Duration = Duration::from_millis(20);

/// What the reader hands an agent, before it is sent to the harness.
enum Request {
    /// Starts a turn with these words and these files.
    Prompt {
        /// What the reader wrote.
        text: String,
        /// The files attached, by path.
        files: Vec<PathBuf>,
        /// The id the turn is saved under.
        turn: Option<String>,
        /// Words the agent is told before the reader's, which are not saved as theirs.
        context: Option<String>,
    },
    /// Answers the question under `ticket` with the choice `option`, or declines it.
    Choose {
        /// The question being answered.
        ticket: u64,
        /// The choice made, or nothing to decline.
        option: Option<String>,
    },
    /// Answers the form under `ticket` with `replies`, or abandons it.
    Answer {
        /// The form being answered.
        ticket: u64,
        /// What the reader answered, or nothing to abandon the form.
        replies: Option<Vec<Reply>>,
    },
    /// Answers the trust request under `ticket` with `verdict`.
    Trust {
        /// The request being answered.
        ticket: u64,
        /// What the reader decided.
        verdict: Verdict,
    },
    /// Names the conversation.
    Rename {
        /// The new name.
        title: String,
    },
    /// Stops the turn that is running.
    Cancel,
    /// Stops one subagent, and the subagents it started, while the turn goes on.
    StopSubagent {
        /// The id a `Subagent` event gave it.
        id: String,
    },
    /// Gives the setting `setting` the value `value`.
    Set {
        /// The setting.
        setting: String,
        /// The value.
        value: String,
    },
}

impl Request {
    /// The command that carries out the request.
    fn command(self) -> Command {
        match self {
            Self::Prompt {
                text,
                files,
                turn,
                context,
            } => Command::Prompt {
                text,
                files,
                turn,
                context,
            },
            Self::Choose { ticket, option } => Command::Choose {
                ticket,
                option,
                note: None,
            },
            Self::Answer { ticket, replies } => Command::Answer { ticket, replies },
            Self::Trust { ticket, verdict } => Command::Trust { ticket, verdict },
            Self::Rename { title } => Command::Rename { title },
            Self::Cancel => Command::Cancel,
            Self::StopSubagent { id } => Command::StopSubagent { id },
            Self::Set { setting, value } => Command::Set { setting, value },
        }
    }
}

/// The way to end the conversation once it is started, and whether the chat was stopped, shared by a chat and its driver thread.
#[derive(Default)]
struct Ending {
    /// The way to end the conversation, once it is started.
    stopper: Option<Stopper>,
    /// Whether the chat was stopped, so a conversation started after that is ended at once.
    stopped: bool,
}

/// What a [`Chat`] calls each time its agent has something for [`Chat::events`], so the owner looks at once instead of on a timer.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// What an agent is given besides its own tools, and what it is kept from.
#[derive(Clone, Debug, Default)]
pub struct Given {
    /// The tool servers whose tools it may call.
    pub tools: Vec<ToolServer>,
    /// The names of the tools it is not given, whoever offers them.
    pub denied: Vec<String>,
    /// The instruction files (by path), skills and agent definitions (by name) the reader turned off.
    pub disabled: Vec<String>,
    /// Why the agent must not be started, when what the reader turned off cannot be kept off; the chat then fails with this instead of starting it.
    pub blocked: Option<String>,
    /// The id the harness keeps the conversation's saved history under, and loads it from when the conversation is opened again; nothing is saved without one.
    pub conversation: Option<String>,
    /// The folder the harness keeps saved histories in, or its own when there is none.
    pub history: Option<PathBuf>,
    /// The id of the turn to go back to before when the saved history is loaded, or all of it when there is none.
    pub before: Option<String>,
    /// The values to give settings the saved conversation has no choice for.
    pub defaults: Vec<(String, String)>,
    /// The app's language servers, which format and check the files the agent writes; when there are none, the harness runs its own when it can.
    pub languages: Option<LanguageServers>,
}

/// A running agent and the conversation held with it.
///
/// Everything the agent does happens on a thread of its own, so nothing it
/// takes time over holds a frame up; [`Chat::events`] collects what it has
/// said since the last look. Dropping the chat ends the agent.
pub struct Chat {
    /// Where the reader's requests go to the driver thread.
    requests: Sender<Request>,
    /// What the driver thread reports.
    events: Receiver<Event>,
    /// The way to end the conversation.
    ending: Arc<Mutex<Ending>>,
    /// The driver thread.
    driver: JoinHandle<()>,
}

impl Chat {
    /// Starts `agent` in the folder `directory`, keeping its login in the folder `home` or in the harness's own when there is none, and opens a conversation with it in which it is `given` its tools; `notify` is called whenever the agent has something for [`Chat::events`].
    pub fn start(
        agent: Agent,
        directory: PathBuf,
        home: Option<PathBuf>,
        given: Given,
        notify: Notify,
    ) -> Self {
        let (requests, received) = mpsc::channel();
        let (queue, events) = mpsc::channel();
        let ending = Arc::new(Mutex::new(Ending::default()));
        let shared = Arc::clone(&ending);
        let driver = std::thread::spawn(move || {
            let report = |event: Event| {
                let sent = queue.send(event).is_ok();
                notify();
                sent
            };
            if let Err(reason) = drive(agent, directory, home, given, &shared, &received, &report) {
                report(Event::Failed(reason));
            }
        });
        Self {
            requests,
            events,
            ending,
            driver,
        }
    }

    /// Ends the conversation and waits until it and the driver thread are gone, so nothing the agent runs is left writing into its folder.
    pub fn stop(self) {
        if let Ok(mut ending) = self.ending.lock() {
            ending.stopped = true;
            if let Some(stopper) = &ending.stopper {
                stopper.stop();
            }
        }
        drop(self.requests);
        let _ = self.driver.join();
    }

    /// Hands the agent `text` and the files at `files` as the next turn, which is saved under the id `turn`; `context` is what the agent is told first and is not saved as the reader's words.
    pub fn prompt(&self, text: &str, files: &[String], turn: u64, context: &str) {
        let _ = self.requests.send(Request::Prompt {
            text: text.to_owned(),
            files: files.iter().map(PathBuf::from).collect(),
            turn: Some(turn.to_string()),
            context: Some(context.to_owned()).filter(|context| !context.is_empty()),
        });
    }

    /// Answers the question under `ticket` with the choice `option`, or declines it.
    pub fn choose(&self, ticket: u64, option: Option<String>) {
        let _ = self.requests.send(Request::Choose { ticket, option });
    }

    /// Answers the form of questions under `ticket` with `replies`, or abandons it.
    pub fn answer(&self, ticket: u64, replies: Option<Vec<Reply>>) {
        let _ = self.requests.send(Request::Answer { ticket, replies });
    }

    /// Answers the trust request under `ticket` with `verdict`: [`Verdict::Always`] trusts the project for good, [`Verdict::Allow`] for this conversation only, and [`Verdict::Reject`] records that it is not trusted. What a trusted project brings is used from the next turn on.
    pub fn trust(&self, ticket: u64, verdict: Verdict) {
        let _ = self.requests.send(Request::Trust { ticket, verdict });
    }

    /// Names the conversation, which the harness saves with it and tells back as a title.
    pub fn rename(&self, title: &str) {
        let _ = self.requests.send(Request::Rename {
            title: title.to_owned(),
        });
    }

    /// Stops the turn that is running.
    pub fn cancel(&self) {
        let _ = self.requests.send(Request::Cancel);
    }

    /// Stops the subagent a `Subagent` event named `id`, and the subagents it started; the others and the turn go on, and the agent is told the reader stopped it. Nothing happens when it has ended already.
    pub fn stop_subagent(&self, id: &str) {
        let _ = self
            .requests
            .send(Request::StopSubagent { id: id.to_owned() });
    }

    /// Gives the setting `setting` the value `value`.
    pub fn set(&self, setting: &str, value: &str) {
        let _ = self.requests.send(Request::Set {
            setting: setting.to_owned(),
            value: value.to_owned(),
        });
    }

    /// What the agent has done since the last call.
    pub fn events(&self) -> Vec<Event> {
        let mut found = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => found.push(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return found,
            }
        }
    }
}

/// Starts a conversation with the harness, opens it and carries it until the chat is dropped or the harness stops.
///
/// # Errors
///
/// Fails with the reason when the conversation cannot be started, or ends badly.
fn drive(
    agent: Agent,
    directory: PathBuf,
    home: Option<PathBuf>,
    given: Given,
    ending: &Mutex<Ending>,
    requests: &Receiver<Request>,
    report: &dyn Fn(Event) -> bool,
) -> Result<(), String> {
    if !directory.is_dir() {
        return Err(format!("{} is not a folder", directory.display()));
    }
    if let Some(reason) = given.blocked {
        return Err(reason);
    }
    let mut harness = Harness::start()?;
    {
        let mut ending = ending.lock().map_err(|_| "the chat was dropped")?;
        if ending.stopped {
            return Ok(());
        }
        ending.stopper = Some(harness.stopper());
    }
    let mut watch = crate::limits::Watch::of(agent, home.clone());
    let (opening, languages) = opening(agent, directory, home, given);
    let (answers, answered) = mpsc::channel();
    harness.send(&Command::Open(opening))?;
    if let Some(languages) = &languages {
        warm(languages.clone());
    }
    loop {
        if let Some(watch) = &mut watch {
            watch.tick();
            for event in watch.heard() {
                if !report(event) {
                    return Ok(());
                }
            }
        }
        loop {
            match requests.try_recv() {
                Ok(request) => harness.send(&request.command())?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        while let Ok(answer) = answered.try_recv() {
            harness.send(&answer)?;
        }
        match harness.listen(LISTEN) {
            Heard::Event(omc_wire::Event::Language(request)) => {
                if let Some(languages) = &languages {
                    answer_language(languages.clone(), request, answers.clone());
                }
            }
            Heard::Event(event) => {
                if matches!(event, omc_wire::Event::Finished(_))
                    && let Some(watch) = &mut watch
                {
                    watch.turn_ended();
                }
                if let Some(event) = Event::from_harness(event)
                    && !report(event)
                {
                    return Ok(());
                }
            }
            Heard::Silence => {}
            Heard::Closed(reason) => return Err(reason),
        }
    }
}

/// The request that opens the conversation of `agent` in `directory`, keeping its login in `home`, with what it is `given`, and the app's language servers seen from that folder when it is given them; the harness is told it is offered language services, which answer its questions about code too, exactly when it is, so that it starts no servers of its own beside the app's.
fn opening(
    agent: Agent,
    directory: PathBuf,
    home: Option<PathBuf>,
    given: Given,
) -> (Opening, Option<Arc<HubLanguages>>) {
    let languages = given
        .languages
        .map(|servers| Arc::new(HubLanguages::new(servers.0, directory.clone())));
    let opening = Opening {
        languages: languages.is_some(),
        navigation: languages.is_some(),
        directory,
        home,
        provider: agent.provider.to_owned(),
        model: None,
        yolo: false,
        tools: given.tools,
        denied: given.denied,
        disabled: given.disabled,
        conversation: given.conversation,
        history: given.history,
        keep: None,
        before: given.before,
        defaults: given.defaults,
    };
    (opening, languages)
}

/// Starts the language servers of the agent's folder on a thread of its own, so that they are ready when it first writes a file.
fn warm(languages: Arc<HubLanguages>) {
    let spawned = std::thread::Builder::new()
        .name("harness-languages".to_owned())
        .spawn(move || languages.warm());
    if let Err(error) = spawned {
        eprintln!("could not start the language servers: {error}");
    }
}

/// The command that answers the harness's language `request` from `languages`, which may wait on the servers as long as the request allows.
fn answered(languages: &HubLanguages, request: omc_wire::LanguageRequest) -> Command {
    Command::Language {
        ticket: request.ticket,
        answer: languages.answer(&request.ask),
    }
}

/// Answers the harness's language `request` from `languages` on a thread of its own, since it may wait on the servers, and sends the answer as a command to `answers`.
fn answer_language(
    languages: Arc<HubLanguages>,
    request: omc_wire::LanguageRequest,
    answers: Sender<Command>,
) {
    let spawned = std::thread::Builder::new()
        .name("harness-languages".to_owned())
        .spawn(move || {
            let _ = answers.send(answered(&languages, request));
        });
    if let Err(error) = spawned {
        eprintln!("could not answer a language request: {error}");
    }
}

#[cfg(test)]
/// Tests that hold a conversation with the real harness.
mod tests {
    use super::*;
    use crate::agent::agents;

    /// A hub with no files open whose servers are found on this program's `PATH`.
    fn hub() -> Arc<pm_lsp::Hub> {
        pm_lsp::Hub::new(
            Arc::new(|_| {}),
            Arc::new(|_: &std::path::Path, _: &str, _: &pm_lsp::ServerSpec| {
                std::env::var("PATH")
                    .map(|path| std::collections::BTreeMap::from([("PATH".to_owned(), path)]))
                    .unwrap_or_default()
            }),
        )
    }

    /// Given the app's language servers, the harness is told it is offered language services, which keeps it from starting servers of its own; given none, it is told it is not, and may start its own.
    #[test]
    fn the_harness_is_told_when_the_app_offers_language_services() {
        let given = Given {
            languages: Some(crate::LanguageServers(hub())),
            ..Given::default()
        };
        let (offered, languages) = opening(agents()[0], PathBuf::from("/w"), None, given);
        assert!(offered.languages && offered.navigation);
        assert!(languages.is_some());
        let (alone, languages) = opening(agents()[0], PathBuf::from("/w"), None, Given::default());
        assert!(!alone.languages && !alone.navigation);
        assert!(languages.is_none());
        let line = omc_wire::encode(&Command::Open(offered)).unwrap();
        assert!(line.contains("\"languages\":true"), "{line}");
        assert!(line.contains("\"navigation\":true"), "{line}");
    }

    /// A language request the harness sends as a line of its wire protocol is answered from the app's hub with a line the harness reads back as the answer under the same ticket: the hub formats the file and, once its server has caught up with the new text, reports the error in it. Skipped when Rust is not installed in the app.
    #[test]
    fn answers_a_language_request_over_the_wire() {
        if pm_lsp::language_of("src/lib.rs").is_none() {
            eprintln!("skipped: Rust is not installed in the app");
            return;
        }
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/tests/pm-harness/wire");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"wire\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .unwrap();
        let dir = dir.canonicalize().unwrap();
        let file = dir.join("src/lib.rs");
        let given = Given {
            languages: Some(crate::LanguageServers(hub())),
            ..Given::default()
        };
        let (_, languages) = opening(agents()[0], dir.clone(), None, given);
        let languages = languages.unwrap();
        let round_trip = |event: omc_wire::Event| {
            let line = omc_wire::encode(&event).unwrap();
            let omc_wire::Event::Language(request) = omc_wire::decode(&line).unwrap() else {
                panic!("not a language request: {line}");
            };
            let line = omc_wire::encode(&answered(&languages, request)).unwrap();
            let Command::Language { ticket, answer } = omc_wire::decode(&line).unwrap() else {
                panic!("not a language answer: {line}");
            };
            (ticket, answer)
        };
        std::fs::write(&file, "pub fn  answer( )->u32{ 42 }\n").unwrap();
        let (ticket, answer) = round_trip(omc_wire::Event::Language(omc_wire::LanguageRequest {
            ticket: 41,
            ask: omc_wire::LanguageAsk::Format { path: file.clone() },
        }));
        assert_eq!(ticket, 41);
        let omc_wire::LanguageAnswer::Formatted { text: Some(text) } = answer else {
            panic!("the file was not formatted: {answer:?}");
        };
        assert!(text.contains("pub fn answer() -> u32 {"), "{text}");
        std::fs::write(&file, "pub fn answer() -> u32 {\n    \"text\"\n}\n").unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(120);
        let mut ticket = 42;
        loop {
            let (answered_ticket, answer) =
                round_trip(omc_wire::Event::Language(omc_wire::LanguageRequest {
                    ticket,
                    ask: omc_wire::LanguageAsk::Diagnostics {
                        paths: vec![file.clone()],
                        wait_ms: 5000,
                    },
                }));
            assert_eq!(answered_ticket, ticket);
            let omc_wire::LanguageAnswer::Diagnostics { diagnostics } = answer else {
                panic!("not a diagnostics answer");
            };
            if let Some(error) = diagnostics
                .iter()
                .find(|diagnostic| diagnostic.severity == omc_wire::Severity::Error)
            {
                assert_eq!((&error.path, error.line), (&file, 2));
                break;
            }
            assert!(std::time::Instant::now() < until, "no error was reported");
            ticket += 1;
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// A trust answer becomes the harness's trust command under the same ticket and verdict, in the shape the harness reads off its wire.
    #[test]
    fn a_trust_answer_is_sent_as_the_harness_trust_command() {
        for verdict in [Verdict::Always, Verdict::Allow, Verdict::Reject] {
            let command = Request::Trust { ticket: 9, verdict }.command();
            assert_eq!(command, Command::Trust { ticket: 9, verdict });
            let line = omc_wire::encode(&command).unwrap();
            assert_eq!(omc_wire::decode::<Command>(&line).unwrap(), command);
        }
        let line = omc_wire::encode(
            &Request::Trust {
                ticket: 3,
                verdict: Verdict::Always,
            }
            .command(),
        )
        .unwrap();
        assert_eq!(
            line.trim_end(),
            r#"{"Trust":{"ticket":3,"verdict":"Always"}}"#
        );
    }

    /// A prompt that needs a tool is answered with a question, and the answer lets the turn finish.
    #[test]
    #[ignore = "needs the harness installed and a login"]
    fn live_turn_asks_before_running_a_command() {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/pm-harness-live");
        let _ = std::fs::create_dir_all(&folder);
        let chat = Chat::start(
            agents()[0],
            folder.clone(),
            None,
            Given::default(),
            Arc::new(|| {}),
        );
        chat.set("model", "claude-sonnet-5-5");
        chat.prompt(
            "Run the bash command `echo hi`, then reply with just: done",
            &[],
            1,
            "",
        );
        let mut seen = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(90);
        let mut finished = false;
        while !finished && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            for event in chat.events() {
                if let Event::Ask(question) = &event {
                    chat.choose(question.ticket, Some("allow".to_owned()));
                }
                finished |= matches!(event, Event::Finished(_));
                seen.push(event);
            }
        }
        chat.stop();
        let _ = std::fs::remove_dir_all(&folder);
        assert!(
            seen.iter().any(|event| matches!(event, Event::Ask(_))),
            "{seen:?}"
        );
        assert!(
            seen.iter()
                .any(|event| matches!(event, Event::Finished(None))),
            "{seen:?}"
        );
    }

    /// Given the app's language servers, a real agent's write comes back formatted by them and with the error they find in it, and the agent fixes it; nothing of the harness's own servers is involved.
    #[test]
    #[ignore = "needs a login and Rust installed in the app"]
    fn live_write_is_checked_by_the_apps_language_servers() {
        let folder =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/pm-harness-live-languages");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(folder.join("src")).unwrap();
        std::fs::write(
            folder.join("Cargo.toml"),
            "[package]\nname = \"live\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .unwrap();
        std::fs::write(folder.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
        let folder = folder.canonicalize().unwrap();
        let chat = Chat::start(
            agents()[0],
            folder.clone(),
            None,
            Given {
                languages: Some(crate::LanguageServers(hub())),
                ..Given::default()
            },
            Arc::new(|| {}),
        );
        chat.set("mode", "yolo");
        chat.prompt(
            "Use the write tool to overwrite src/lib.rs with exactly: 'pub fn  answer( )->u32{ \"text\" }' and a newline. If its result reports an error, fix it with the edit tool so answer returns 42. Run no shell commands. Then reply: done",
            &[],
            1,
            "",
        );
        let mut outputs = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(240);
        let mut finished = false;
        while !finished && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            for event in chat.events() {
                if let Event::Ask(question) = &event {
                    chat.choose(question.ticket, Some("allow".to_owned()));
                }
                if let Event::Tool(tool) = &event
                    && let Some(output) = &tool.output
                {
                    outputs.push(output.clone());
                }
                finished |= matches!(event, Event::Finished(_));
            }
        }
        chat.stop();
        let written = std::fs::read_to_string(folder.join("src/lib.rs")).unwrap();
        let _ = std::fs::remove_dir_all(&folder);
        assert!(finished, "{outputs:#?}");
        assert!(
            outputs
                .iter()
                .any(|output| output.contains("formatted by the language server")
                    && output.contains("src/lib.rs:2:5 error:")),
            "{outputs:#?}"
        );
        assert_eq!(written, "pub fn answer() -> u32 {\n    42\n}\n");
    }
    /// Given the app's language servers, a real agent's `lsp code_actions`, `lsp format` and `lsp rename_file` are answered by them over the harness's language requests: the import of a name used without its `use` is offered and applied, a badly laid out file is formatted, and a file is moved with its module declaration updated, each change approved through the harness's question and written by the harness from the edits the app's server made. Prints what the agent did.
    #[test]
    #[ignore = "needs a login and Rust installed in the app"]
    fn live_lsp_changes_are_answered_by_the_apps_language_servers() {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/pm-harness-live-lsp-changes");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(folder.join("src")).unwrap();
        std::fs::write(
            folder.join("Cargo.toml"),
            "[package]\nname = \"livechanges\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .unwrap();
        std::fs::write(
            folder.join("src/lib.rs"),
            "pub mod messy;\npub mod util;\n\npub fn total() -> usize {\n    crate::util::names().len()\n}\n",
        )
        .unwrap();
        std::fs::write(
            folder.join("src/util.rs"),
            "pub fn names() -> HashMap<u32, String> {\n    HashMap::new()\n}\n",
        )
        .unwrap();
        std::fs::write(
            folder.join("src/messy.rs"),
            "pub fn  add( a:u32,b:u32 )->u32{a+b}\n",
        )
        .unwrap();
        let folder = folder.canonicalize().unwrap();
        let chat = Chat::start(
            agents()[0],
            folder.clone(),
            None,
            Given {
                languages: Some(crate::LanguageServers(hub())),
                ..Given::default()
            },
            Arc::new(|| {}),
        );
        chat.set("model", "claude-haiku-5-5");
        chat.prompt(
            "Use only the lsp tool, one call at a time, in this order. 1) Call lsp code_actions with path src/util.rs, line 2 and symbol HashMap to list the actions, then call it again with apply set to the number of the action that imports std::collections::HashMap. 2) Call lsp format on src/messy.rs. 3) Call lsp rename_file to move src/util.rs to src/helpers.rs. Then reply with one short line.",
            &[],
            1,
            "",
        );
        let mut outputs = Vec::new();
        let mut asked = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(300);
        let mut finished = false;
        while !finished && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            for event in chat.events() {
                if let Event::Ask(question) = &event {
                    eprintln!("ASK {}", question.title);
                    asked.push(question.title.clone());
                    chat.choose(question.ticket, Some("allow".to_owned()));
                }
                if let Event::Tool(tool) = &event
                    && let Some(output) = &tool.output
                {
                    eprintln!(
                        "TOOL {}\n{output}\n",
                        tool.title.clone().unwrap_or_default()
                    );
                    outputs.push(output.clone());
                }
                if let Event::Said(text) = &event {
                    eprint!("{text}");
                }
                finished |= matches!(event, Event::Finished(_));
            }
        }
        chat.stop();
        let lib = std::fs::read_to_string(folder.join("src/lib.rs")).unwrap();
        let helpers = std::fs::read_to_string(folder.join("src/helpers.rs")).unwrap_or_default();
        let messy = std::fs::read_to_string(folder.join("src/messy.rs")).unwrap();
        assert!(finished, "{outputs:#?}");
        assert!(
            outputs
                .iter()
                .any(|output| output.starts_with("Applied the code action")),
            "{outputs:#?}"
        );
        assert!(
            asked.len() >= 3,
            "the changes were not approved first: {asked:?}"
        );
        assert!(
            helpers.contains("use std::collections::HashMap;"),
            "{helpers}"
        );
        assert!(!folder.join("src/util.rs").exists());
        assert!(lib.contains("pub mod helpers;"), "{lib}");
        assert!(
            messy.contains("pub fn add(a: u32, b: u32) -> u32 {"),
            "{messy}"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Given the app's language servers, a real agent's `lsp references` and `lsp rename` in a project of two files are answered by them over the harness's language requests: the references name both files, the rename is approved through the harness's question and written by the harness from the edits the app's server made, and the files on disk end up renamed; nothing of the harness's own servers is involved. Prints what the agent did.
    #[test]
    #[ignore = "needs a login and Rust installed in the app"]
    fn live_lsp_is_answered_by_the_apps_language_servers() {
        let folder =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/pm-harness-live-lsp");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(folder.join("src")).unwrap();
        std::fs::write(
            folder.join("Cargo.toml"),
            "[package]\nname = \"livelsp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .unwrap();
        std::fs::write(
            folder.join("src/lib.rs"),
            "pub mod util;\n\n/// Two ones.\npub fn total() -> u32 {\n    util::helper() + util::helper()\n}\n",
        )
        .unwrap();
        std::fs::write(
            folder.join("src/util.rs"),
            "/// Gives one.\npub fn helper() -> u32 {\n    1\n}\n",
        )
        .unwrap();
        let folder = folder.canonicalize().unwrap();
        let chat = Chat::start(
            agents()[0],
            folder.clone(),
            None,
            Given {
                languages: Some(crate::LanguageServers(hub())),
                ..Given::default()
            },
            Arc::new(|| {}),
        );
        chat.set("model", "claude-haiku-5-5");
        chat.prompt(
            "First call the lsp tool with action references for the symbol helper on line 2 of src/util.rs. Then call the lsp tool with action rename to rename that same helper to assist. Use no other tools. Then reply with one line saying how many references you found.",
            &[],
            1,
            "",
        );
        let mut outputs = Vec::new();
        let mut asked = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(300);
        let mut finished = false;
        while !finished && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            for event in chat.events() {
                if let Event::Ask(question) = &event {
                    eprintln!("ASK {}", question.title);
                    asked.push(question.title.clone());
                    chat.choose(question.ticket, Some("allow".to_owned()));
                }
                if let Event::Tool(tool) = &event
                    && let Some(output) = &tool.output
                {
                    eprintln!(
                        "TOOL {}\n{output}\n",
                        tool.title.clone().unwrap_or_default()
                    );
                    outputs.push(output.clone());
                }
                if let Event::Said(text) = &event {
                    eprint!("{text}");
                }
                finished |= matches!(event, Event::Finished(_));
            }
        }
        chat.stop();
        let lib = std::fs::read_to_string(folder.join("src/lib.rs")).unwrap();
        let util = std::fs::read_to_string(folder.join("src/util.rs")).unwrap();
        assert!(finished, "{outputs:#?}");
        assert!(
            outputs.iter().any(|output| output.contains("src/lib.rs")
                && output.contains("src/util.rs")
                && output.contains("reference")),
            "{outputs:#?}"
        );
        assert!(
            outputs
                .iter()
                .any(|output| output.starts_with("Renamed") && output.contains("tag")),
            "{outputs:#?}"
        );
        assert!(!asked.is_empty(), "the rename was not approved first");
        assert_eq!(lib.matches("util::assist()").count(), 2, "{lib}");
        assert!(util.contains("pub fn assist() -> u32"), "{util}");
        let _ = std::fs::remove_dir_all(&folder);
    }
    /// Serves, on a thread of its own, a tool server at `/mcp` on a free local port that wants an OAuth sign-in, with the metadata that leads to its sign-in page, until the program ends; returns its address.
    fn fake_signing_in_tool_server() -> String {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let served = base.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream);
                let mut first = String::new();
                if reader.read_line(&mut first).is_err() {
                    continue;
                }
                let mut length = 0;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                        break;
                    }
                    if let Some((name, value)) = header.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; length];
                let _ = reader.read_exact(&mut body);
                let target = first.split_whitespace().nth(1).unwrap_or_default();
                let json = |status: &str, body: String| {
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                };
                let answer = if target.starts_with("/.well-known/oauth-protected-resource") {
                    json(
                        "200 OK",
                        format!(
                            r#"{{"resource":"{served}/mcp","authorization_servers":["{served}"]}}"#
                        ),
                    )
                } else if target.starts_with("/.well-known/oauth-authorization-server") {
                    json(
                        "200 OK",
                        format!(
                            r#"{{"issuer":"{served}","authorization_endpoint":"{served}/authorize","token_endpoint":"{served}/token","registration_endpoint":"{served}/register","response_types_supported":["code"],"code_challenge_methods_supported":["S256"]}}"#
                        ),
                    )
                } else if target.starts_with("/register") {
                    json(
                        "201 Created",
                        r#"{"client_id":"pandemonium-test"}"#.to_owned(),
                    )
                } else {
                    format!(
                        "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Bearer resource_metadata=\"{served}/.well-known/oauth-protected-resource\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                };
                let _ = reader.into_inner().write_all(answer.as_bytes());
            }
        });
        base
    }

    /// A tool server that wants an OAuth sign-in while a real conversation opens makes the harness send its sign-in page, which the app keeps as its tool sign-in event, the address leading to the fake server's own authorization page; no model is asked.
    #[test]
    #[ignore = "starts the tool servers the user's own files set up beside the fake one"]
    fn live_tool_sign_in_reaches_the_app() {
        let base = fake_signing_in_tool_server();
        let folder =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/pm-harness-live-sign-in");
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(folder.join("home")).unwrap();
        let chat = Chat::start(
            agents()[0],
            folder.clone(),
            Some(folder.join("home")),
            Given {
                tools: vec![ToolServer {
                    name: "fake".to_owned(),
                    reach: omc_wire::Reach::Http {
                        url: format!("{base}/mcp"),
                        headers: Vec::new(),
                    },
                }],
                ..Given::default()
            },
            Arc::new(|| {}),
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        let mut address = None;
        while address.is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            for event in chat.events() {
                eprintln!("EVENT {event:?}");
                if let Event::ToolSignIn(page) = event {
                    address = Some(page);
                }
            }
        }
        chat.stop();
        let _ = std::fs::remove_dir_all(&folder);
        let address = address.expect("no tool sign-in reached the app");
        assert!(
            address.starts_with(&format!("{base}/authorize?")),
            "{address}"
        );
        assert!(address.contains("code_challenge_method=S256"), "{address}");
    }
}
