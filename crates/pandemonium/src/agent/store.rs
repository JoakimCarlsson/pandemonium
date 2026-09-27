//! The conversations the window is holding, listed per project.
//!
//! [`Talks`] is the one seam an agent is started and ended through, and it
//! is keyed by project id the way the shells are: an agent works in the
//! worktree of the project it was started from, and a conversation that is
//! not attached to a project is a bug. Which worktree that is — the
//! project's own checkout, or a session cut beside it — is the window's to
//! say, and it says it by where the agent is started.
//!
//! [`Talk`] is one of them — the agent, everything said to it and by it, the
//! prompt being typed and whatever it is waiting to be allowed to do.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use pm_gfx::Image;

use crate::agent::pane::Wrapped;
use crate::agent::transcript::Transcript;
use crate::image::{Decodes, read_file};
use crate::input::Input;
use pm_acp::{
    About, Agent, Answer, Ask, Attachment, Command, Event, History, Knob, Method, Mode, Notify,
    Request, Session, Setting, Stop, Usage, Voice,
};
use pm_core::{ProjectId, Scope, SessionId};
use pm_gfx::Point;
use pm_ui::{Bounds, Placements};

/// A conversation's identity for as long as it is running.
///
/// Ids are handed out by [`Talks`] and are unique across the window, so a
/// tab, a keybinding and a pane all name the same conversation without
/// knowing which project it belongs to.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TalkId(u64);

/// A place in the text of the conversation: a row of the pane, and how many
/// characters into that row.
///
/// Rows are counted from the top of the whole conversation rather than of
/// the view, so what is picked out stays put as the pane scrolls.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Spot {
    /// Which row, from the first.
    pub row: usize,
    /// How many characters into it.
    pub column: usize,
}

/// How a conversation is doing, as a reader deciding where to look reads it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Standing {
    /// The agent's process has gone.
    Stopped,
    /// It is waiting on the reader to allow something.
    Waiting,
    /// A turn is running.
    Working,
    /// A turn has ended that the reader has not looked at since.
    Done,
    /// It is doing nothing and waiting on nobody.
    Idle,
}

/// How many of the window's conversations stand each way.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Tally {
    /// How many have stopped.
    pub stopped: usize,
    /// How many are waiting on the reader.
    pub waiting: usize,
    /// How many are in the middle of a turn.
    pub working: usize,
    /// How many have finished a turn nobody has read yet.
    pub done: usize,
    /// How many are doing nothing.
    pub idle: usize,
}

/// One conversation: what is running, what has been said, what it is owed.
pub struct Talk {
    /// Which session this is.
    id: TalkId,
    /// The project whose worktree it is working in.
    project: ProjectId,
    /// The session it is working in, where it was started in one.
    session: Option<SessionId>,
    /// The conversation itself, as the protocol carries it.
    conversation: Session,
    /// What the worktree adds to the environment the agent was started in,
    /// kept so that it is started the same way again after a login.
    env: Vec<(String, String)>,
    /// The ways of logging in the agent offered when it would not open a
    /// conversation without one, until it has opened one.
    logins: Vec<Method>,
    /// Everything said so far.
    transcript: Transcript,
    /// The buffer the next prompt is written in.
    ///
    /// A prompt is several lines as often as it is one, so it is written in
    /// the editor the window is made of rather than in a line of its own.
    prompt: Input,
    /// Files and images to send with the next prompt.
    attachments: Vec<Attachment>,
    /// The preview of each attachment, in the same order, by its key in
    /// `previews` where the attachment is a picture.
    attachment_previews: Vec<Option<u64>>,
    /// The attachments' pictures, decoded away from the window.
    previews: Decodes<u64>,
    /// The key the next attached picture is given in `previews`.
    next_preview: u64,
    /// Pasted images saved as files for agents without image prompt support.
    clipboard_files: Vec<PathBuf>,
    /// The permission requests waiting on the reader, oldest first.
    asks: Vec<Ask>,
    /// The commands the agent has said it takes, as it last said them.
    commands: Vec<Command>,
    /// Skills installed for this agent, invoked with a dollar sign.
    skills: Vec<Command>,
    /// Saved sessions returned by the agent's history listing.
    history: Vec<History>,
    /// Whether the agent is still fetching the history pages.
    listing: bool,
    /// Why the agent could not list saved sessions, where it failed.
    history_error: Option<String>,
    /// The worktree's files as a mention offers them, relative to its root,
    /// listed away from the window when a mention starts and kept until the
    /// next one does.
    mentionable: Arc<Mutex<Mentionable>>,
    /// How the listing of those files wakes the window once it is done.
    notify: Notify,
    /// Which of the commands a slash narrows to is selected.
    chosen: usize,
    /// Whether the reader has waved that list away for what is typed now.
    dismissed: bool,
    /// Whether the conversation is open and will take prompts.
    ready: bool,
    /// Whether a turn is running.
    busy: bool,
    /// When the current turn began, for its visible activity timer.
    busy_since: Option<Instant>,
    /// Whether a turn has ended since the reader last looked at the pane.
    unseen: bool,
    /// The mode the agent says it is in, where it has modes.
    mode: Option<String>,
    /// The title the agent has given the conversation, where it has.
    title: Option<String>,
    /// How much of the model's context the conversation fills, where the
    /// agent says.
    usage: Option<Usage>,
    /// How far down the conversation the pane is scrolled, in logical pixels.
    scroll: f32,
    /// Where the conversation was last drawn, which is how much of it a
    /// pane holds.
    view: Bounds,
    /// How tall the conversation came to when it was last drawn.
    drawn_height: Rc<Cell<f32>>,
    /// How wide the pane was when it last wrapped the conversation, which is
    /// what its rows, and so the places in them, were counted against.
    drawn_width: Rc<Cell<f32>>,
    /// Where each link the pane last drew leads, in the order it drew them,
    /// which is how a press on one names it.
    drawn_links: Rc<RefCell<Vec<String>>>,
    /// Where each run of text the pane last drew came out, keyed by its
    /// place in `drawn_spots`.
    drawn_text: Placements,
    /// Where in the conversation each run of text the pane last drew begins.
    drawn_spots: Rc<RefCell<Vec<Spot>>>,
    /// What the reader has picked out of the conversation: where the drag
    /// began, and where it has got to.
    selection: Option<(Spot, Spot)>,
    /// Whether the pane follows the end of the conversation as it grows.
    following: bool,
    /// The tool and thought blocks the reader has opened.
    expanded_details: BTreeSet<usize>,
    /// The last lines each terminal the agent started has written, by the
    /// name the agent knows it by, for the tool calls that show one.
    terminals: BTreeMap<String, String>,
    /// Counts the changes to what the pane shows that the transcript does
    /// not hold: details opened or closed, and a terminal's latest lines.
    shown_revision: u64,
    /// The conversation as the pane last wrapped it.
    wrapped: RefCell<Wrapped>,
}

/// One completion offered by a prompt prefix.
pub struct Offered {
    /// The prefix the agent expects for this entry.
    pub prefix: char,
    /// The command or skill name.
    pub name: String,
    /// What the entry does.
    pub description: String,
}

impl Talk {
    /// Which session this is.
    pub fn id(&self) -> TalkId {
        self.id
    }

    /// Which agent is running.
    pub fn agent(&self) -> Agent {
        self.conversation.agent()
    }

    /// The worktree the conversation belongs to, as the panes name it.
    pub fn scope(&self) -> Scope {
        match self.session {
            Some(session) => Scope::of(self.project, session),
            None => Scope::checkout(self.project),
        }
    }

    /// The worktree the agent is working in.
    pub fn root(&self) -> &Path {
        self.conversation.root()
    }

    /// Whether this agent can list and load saved conversations.
    pub fn can_list(&self) -> bool {
        self.conversation.can_list()
    }

    /// Asks this agent to refresh its saved conversations.
    pub fn list_history(&mut self) {
        self.history.clear();
        self.history_error = None;
        self.listing = true;
        self.conversation.list_sessions();
    }

    /// The saved conversations this agent has returned so far.
    pub fn history(&self) -> &[History] {
        &self.history
    }

    /// Whether more saved conversations are being fetched.
    pub fn is_listing(&self) -> bool {
        self.listing
    }

    /// Why the saved conversations could not be fetched.
    pub fn history_error(&self) -> Option<&str> {
        self.history_error.as_deref()
    }

    /// What the agent calls this conversation, once it has opened one.
    pub fn resumable(&self) -> Option<String> {
        self.conversation.id()
    }

    /// Everything said so far.
    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// The box the next prompt is written in.
    pub fn prompt(&self) -> &Input {
        &self.prompt
    }

    /// That box, to write in.
    pub fn prompt_mut(&mut self) -> &mut Input {
        &mut self.prompt
    }

    /// Files and images waiting beside the prompt.
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    /// The preview of the attachment at `place`, once it is decoded.
    pub fn attachment_preview(&self, place: usize) -> Option<Image> {
        let key = self.attachment_previews.get(place).copied().flatten()?;
        self.previews.peek(&key)?.ready()
    }

    /// Whether the agent takes images inside a prompt rather than as files.
    pub fn can_image(&self) -> bool {
        self.conversation.can_image()
    }

    /// Adds a chosen file to the next prompt, decoding its preview away from
    /// the window when it is a picture.
    pub fn attach_file(&mut self, path: PathBuf) {
        let preview = crate::image::Images::is_picture(&path).then(|| {
            let key = self.preview_key();
            self.previews.start(key, read_file(path.clone()));
            key
        });
        self.attachment_previews.push(preview);
        self.attachments.push(Attachment::File(path));
    }

    /// Adds an image pasted and made ready away from the window.
    pub fn attach_pasted(&mut self, pasted: Pasted) {
        let key = self.preview_key();
        if let Some(preview) = pasted.preview {
            self.previews.put(key, preview);
        }
        self.attachment_previews.push(Some(key));
        self.attachments.push(pasted.attachment);
        self.clipboard_files.extend(pasted.saved);
    }

    /// The key the next attached picture is kept under.
    fn preview_key(&mut self) -> u64 {
        self.next_preview += 1;
        self.next_preview
    }

    /// Adds lines `first` to `last` of the file at `path`, which say `text`,
    /// to the next prompt.
    pub fn attach_selection(&mut self, path: PathBuf, first: usize, last: usize, text: String) {
        self.attachment_previews.push(None);
        self.attachments.push(Attachment::Selection {
            path,
            first,
            last,
            text,
        });
    }

    /// Removes the attachment at `place` before it is sent.
    pub fn remove_attachment(&mut self, place: usize) {
        if place < self.attachments.len() {
            self.attachments.remove(place);
            if let Some(key) = self.attachment_previews.remove(place) {
                self.previews.retain(|kept| *kept != key);
            }
        }
    }

    /// Answers the file or terminal request raised under `ticket`.
    pub fn answer_request(&self, ticket: u64, answer: Answer) {
        self.conversation.answer_request(ticket, answer);
    }

    /// The last lines the terminal the agent calls `terminal` has written.
    pub fn terminal_tail(&self, terminal: &str) -> Option<&str> {
        self.terminals.get(terminal).map(String::as_str)
    }

    /// Takes down the last lines the terminal the agent calls `terminal` has
    /// written, answering whether they are new.
    pub fn show_terminal(&mut self, terminal: &str, tail: String) -> bool {
        if self.terminals.get(terminal) == Some(&tail) {
            return false;
        }
        self.terminals.insert(terminal.to_owned(), tail);
        self.shown_revision += 1;
        true
    }

    /// The ways of logging in the agent is waiting on the reader to choose
    /// between, while it will not open a conversation without one.
    pub fn logins(&self) -> &[Method] {
        &self.logins
    }

    /// Adds something the editor itself has to say to the conversation.
    pub fn note(&mut self, note: impl Into<String>) {
        self.transcript.note(note);
    }

    /// The environment the worktree adds to the agent's.
    pub fn env(&self) -> &[(String, String)] {
        &self.env
    }

    /// Asks the agent to log in by the method `method` names, which it does
    /// by a flow of its own before opening the conversation.
    pub fn log_in(&mut self, method: &Method) {
        self.transcript
            .note(format!("Logging in with {}…", method.name));
        self.conversation.login(&method.id);
    }

    /// The permission requests waiting on the reader.
    pub fn asks(&self) -> &[Ask] {
        &self.asks
    }

    /// What the prompt is naming after a slash, when that is what it holds,
    /// or after an at sign, when it ends in a file being mentioned.
    ///
    /// A command is only being named while it is the whole of the prompt: a
    /// slash with an argument after it has been named already, and a slash
    /// in the middle of a sentence is a slash. A mention is being named while
    /// it is the word the prompt ends in.
    pub fn naming(&self) -> Option<(char, String)> {
        let typed = self.prompt.value();
        if let Some(mention) = mentioning(&typed) {
            return Some((MENTION, mention.to_lowercase()));
        }
        let prefix = typed.chars().next()?;
        let named = match prefix {
            '/' => typed.strip_prefix('/')?,
            '$' if self.agent().id == "codex" => typed.strip_prefix('$')?,
            _ => return None,
        };
        match named.contains(char::is_whitespace) {
            true => None,
            false => Some((prefix, named.to_lowercase())),
        }
    }

    /// The commands the prompt is narrowing to, while it is naming one.
    ///
    /// An agent says what it takes when the conversation opens and says it
    /// again whenever that changes, so this is what it offers now — the
    /// skills, the slash commands and whatever else it has put on the list.
    pub fn offered(&self) -> Vec<Offered> {
        let Some((prefix, named)) = self.naming().filter(|_| !self.dismissed) else {
            return Vec::new();
        };
        if prefix == MENTION {
            return self.mentions(&named);
        }
        let source = match prefix {
            '$' => &self.skills,
            _ => &self.commands,
        };
        source
            .iter()
            .filter(|command| command.name.to_lowercase().starts_with(&named))
            .map(|command| Offered {
                prefix,
                name: command.name.clone(),
                description: command.description.clone(),
            })
            .collect()
    }

    /// The worktree's files `named` narrows a mention to, those whose name
    /// starts with it first and shorter paths before longer ones.
    fn mentions(&self, named: &str) -> Vec<Offered> {
        let Some(files) = self.mentionable_files() else {
            return Vec::new();
        };
        let mut found = files
            .iter()
            .filter(|(_, lowered)| lowered.contains(named))
            .map(|(file, _)| file)
            .collect::<Vec<_>>();
        found.sort_by_key(|file| {
            let name = file.rsplit('/').next().unwrap_or(file).to_lowercase();
            (!name.starts_with(named), file.len())
        });
        found
            .into_iter()
            .take(MENTIONED)
            .map(|file| Offered {
                prefix: MENTION,
                name: file.clone(),
                description: String::new(),
            })
            .collect()
    }

    /// Which of the offered commands is selected.
    ///
    /// The list is rebuilt on every keystroke and can grow shorter as it
    /// narrows, so the selection is held against what is offered now rather
    /// than trusted: a row that is no longer there selects the last one that
    /// is.
    pub fn chosen(&self) -> usize {
        self.chosen.min(self.offered().len().saturating_sub(1))
    }

    /// Moves the selection `by` rows through the offered commands.
    ///
    /// The ends are joined: a list a reader is stepping through is shorter
    /// than the reach of the key, and going up from the first row to the
    /// last is what every other list in the window does.
    pub fn step_command(&mut self, by: isize) {
        let offered = self.offered().len();
        if offered == 0 {
            return;
        }
        let at = self.chosen() as isize + by;
        self.chosen = at.rem_euclid(offered as isize) as usize;
    }

    /// Puts the selected command into the prompt.
    pub fn take_chosen(&mut self) {
        self.take_command(self.chosen());
    }

    /// Puts a slash in the prompt, which is what offers the commands.
    pub fn start_command(&mut self) {
        self.prompt.set("/");
        self.chosen = 0;
        self.dismissed = false;
    }

    /// Starts naming an installed skill in the prompt.
    pub fn start_skill(&mut self) {
        self.prompt.set("$");
        self.chosen = 0;
        self.dismissed = false;
    }

    /// Takes the list of commands away, leaving what has been typed alone.
    ///
    /// The list is a suggestion over the prompt, not a thing the prompt is
    /// in: waving it away leaves the slash, the rest of the line and the
    /// keyboard where they were. It comes back at the next keystroke.
    ///
    /// The answer says whether there was a list to take away.
    pub fn dismiss_commands(&mut self) -> bool {
        if self.offered().is_empty() {
            return false;
        }
        self.dismissed = true;
        true
    }

    /// The worktree's files a mention offers, each beside its lowercase
    /// form, once they are listed; the first asking starts the listing.
    fn mentionable_files(&self) -> Option<Arc<[(String, String)]>> {
        let mut mentionable = self.mentionable.lock().ok()?;
        if mentionable.files.is_none() && !mentionable.listing {
            mentionable.listing = true;
            let root = self.root().to_path_buf();
            let shared = self.mentionable.clone();
            let notify = self.notify.clone();
            std::thread::spawn(move || {
                let files = pm_core::walk(&root)
                    .into_iter()
                    .filter_map(|path| {
                        let relative = path.strip_prefix(&root).ok()?;
                        let file = relative.to_string_lossy().replace('\\', "/");
                        let lowered = file.to_lowercase();
                        Some((file, lowered))
                    })
                    .collect();
                if let Ok(mut mentionable) = shared.lock() {
                    mentionable.files = Some(files);
                    mentionable.listing = false;
                    mentionable.fresh = true;
                }
                notify();
            });
        }
        mentionable.files.clone()
    }

    /// Whether the files a mention offers have been listed since this was
    /// last asked.
    fn take_listed(&self) -> bool {
        self.mentionable
            .lock()
            .is_ok_and(|mut mentionable| std::mem::take(&mut mentionable.fresh))
    }

    /// Starts the selection again, for a prompt that has been typed into.
    ///
    /// A mention just begun lists the worktree afresh, so a file the agent
    /// wrote a moment ago is there to be mentioned.
    pub fn retyped(&mut self) {
        self.chosen = 0;
        self.dismissed = false;
        if self.naming() == Some((MENTION, String::new()))
            && let Ok(mut mentionable) = self.mentionable.lock()
        {
            mentionable.files = None;
        }
    }

    /// Puts the command in `place` of what is offered into the prompt.
    ///
    /// The command is left with a space after it and the turn is not sent:
    /// most of them take something, and the ones that do not are one more
    /// key away.
    pub fn take_command(&mut self, place: usize) {
        let offered = self.offered();
        let Some(command) = offered.get(place) else {
            return;
        };
        if command.prefix == MENTION {
            let typed = self.prompt.value();
            let before = typed.rfind(MENTION).map_or("", |at| &typed[..at]);
            self.prompt
                .set(&format!("{before}{MENTION}{} ", command.name));
            self.chosen = 0;
            self.dismissed = false;
            return;
        }
        self.prompt
            .set(&format!("{}{} ", command.prefix, command.name));
        self.chosen = 0;
        self.dismissed = false;
    }

    /// Whether the conversation is open and will take prompts.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Whether a turn is running.
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    /// How long the current turn has been running.
    pub fn working_for(&self) -> Option<Duration> {
        self.busy_since.map(|since| since.elapsed())
    }

    /// The mode the agent is in, where it has modes.
    pub fn mode(&self) -> Option<&str> {
        self.mode.as_deref()
    }

    /// The title the agent has given the conversation, where it has.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Names the conversation `title` until its agent names it, for one
    /// taken up again under the title it had before.
    pub fn entitle(&mut self, title: &str) {
        if self.title.is_none() && !title.is_empty() {
            self.title = Some(title.to_owned());
        }
    }

    /// How much of the model's context the conversation fills, where the
    /// agent says.
    pub fn usage(&self) -> Option<&Usage> {
        self.usage.as_ref()
    }

    /// The modes the agent takes, in the order it offered them.
    ///
    /// An agent says what its modes are when the conversation opens, and an
    /// agent that has none says nothing: an empty list is a session that is
    /// only ever in one mode, not one whose modes have not arrived.
    pub fn modes(&self) -> Vec<Mode> {
        self.conversation.modes()
    }

    /// What the mode it is in is called, which is what a reader is shown.
    ///
    /// A mode the agent has not named is shown as the agent named it: a
    /// session that has been put into a mode nobody listed is still in it.
    pub fn mode_name(&self) -> Option<String> {
        let mode = self.mode.as_deref()?;
        let named = self
            .modes()
            .into_iter()
            .find(|offered| offered.id == mode)
            .map(|offered| offered.name);
        Some(named.unwrap_or_else(|| mode.to_owned()))
    }

    /// Puts the session into the mode `mode` names.
    pub fn set_mode(&self, mode: &str) {
        self.conversation.set_mode(mode);
    }

    /// What the session can be set to, as the agent now offers it.
    ///
    /// A model, how hard the agent is made to think, a switch it offers and,
    /// for an agent that says so this way, the mode: the protocol has one
    /// shape for all of them, and so has the window.
    pub fn knobs(&self) -> Vec<Knob> {
        self.conversation.knobs()
    }

    /// The knob `id` names.
    pub fn knob(&self, id: &str) -> Option<Knob> {
        self.knobs().into_iter().find(|knob| knob.id == id)
    }

    /// The knob that is about `about`, where the agent offers one.
    pub fn knob_about(&self, about: About) -> Option<Knob> {
        self.knobs().into_iter().find(|knob| knob.about == about)
    }

    /// Sets the knob `id` names to the value `value` names.
    pub fn set_knob(&self, id: &str, value: &str) {
        self.conversation.set_knob(id, value);
    }

    /// Puts the switch `id` names the other way.
    pub fn toggle_knob(&self, id: &str) {
        let Some(Setting::Switched(on)) = self.knob(id).map(|knob| knob.setting) else {
            return;
        };
        self.conversation.switch_knob(id, !on);
    }

    /// Sets the knob `id` names to the value after the one it is set to.
    pub fn cycle_knob(&self, id: &str) {
        let Some(knob) = self.knob(id) else {
            return;
        };
        let Setting::Picked { value, picks } = knob.setting else {
            return self.toggle_knob(id);
        };
        if picks.is_empty() {
            return;
        }
        let at = picks
            .iter()
            .position(|pick| pick.id == value)
            .map_or(0, |at| (at + 1) % picks.len());
        self.set_knob(id, &picks[at].id);
    }

    /// Puts it into the mode after the one it is in, wrapping round.
    ///
    /// One chord walks the modes because there are two or three of them and
    /// a reader changing mode is usually going to the next one; the list is
    /// there for the times they are not.
    pub fn cycle_mode(&self) {
        let modes = self.modes();
        if modes.is_empty() {
            return;
        }
        let at = modes
            .iter()
            .position(|offered| Some(offered.id.as_str()) == self.mode())
            .map_or(0, |at| (at + 1) % modes.len());
        self.set_mode(&modes[at].id);
    }

    /// Whether the agent's process is still there.
    pub fn is_running(&self) -> bool {
        self.conversation.is_running()
    }

    /// How the conversation is doing.
    ///
    /// A stopped agent outranks a question it left behind, and a question
    /// outranks a running turn: each is the more urgent thing to read.
    pub fn standing(&self) -> Standing {
        match (
            self.is_running(),
            self.asks.is_empty(),
            self.busy,
            self.unseen,
        ) {
            (false, ..) => Standing::Stopped,
            (_, false, ..) => Standing::Waiting,
            (_, _, true, _) => Standing::Working,
            (_, _, _, true) => Standing::Done,
            _ => Standing::Idle,
        }
    }

    /// Marks what the conversation has done as read.
    pub fn see(&mut self) {
        self.unseen = false;
    }

    /// How far down the conversation the pane is scrolled, in logical pixels.
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// Where the conversation was last drawn, shared with the pane drawing it.
    pub fn view(&self) -> Bounds {
        self.view.clone()
    }

    /// How tall the conversation came to when it was last drawn, shared with
    /// the pane drawing it.
    pub fn drawn_height(&self) -> Rc<Cell<f32>> {
        self.drawn_height.clone()
    }

    /// How wide the pane was when it last wrapped the conversation, shared
    /// with the pane drawing it.
    pub fn drawn_width(&self) -> Rc<Cell<f32>> {
        self.drawn_width.clone()
    }

    /// Where the links the pane last drew lead, shared with the pane
    /// drawing them.
    pub fn drawn_links(&self) -> Rc<RefCell<Vec<String>>> {
        self.drawn_links.clone()
    }

    /// Where the link the pane drew in `place` leads.
    pub fn drawn_link(&self, place: usize) -> Option<String> {
        self.drawn_links.borrow().get(place).cloned()
    }

    /// Where the runs of text the pane last drew came out, shared with the
    /// pane drawing them.
    pub fn drawn_text(&self) -> Placements {
        self.drawn_text.clone()
    }

    /// Where in the conversation the runs of text the pane last drew begin,
    /// shared with the pane drawing them.
    pub fn drawn_spots(&self) -> Rc<RefCell<Vec<Spot>>> {
        self.drawn_spots.clone()
    }

    /// The place in the conversation's text nearest `point`, as the pane
    /// last drew it.
    pub fn spot_at(&self, point: Point) -> Option<Spot> {
        let placements = self.drawn_text.borrow();
        let placed = pm_ui::nearest(&placements, point)?;
        let start = *self.drawn_spots.borrow().get(placed.key)?;
        Some(Spot {
            row: start.row,
            column: start.column + placed.caret_at(point.x),
        })
    }

    /// What the reader has picked out of the conversation, first place
    /// first, when it is anything at all.
    pub fn selection(&self) -> Option<(Spot, Spot)> {
        let (anchor, head) = self.selection?;
        (anchor != head).then(|| (anchor.min(head), anchor.max(head)))
    }

    /// Picks out the text from `anchor` to `head`.
    pub fn select(&mut self, anchor: Spot, head: Spot) {
        self.selection = Some((anchor, head));
    }

    /// Lets go of what was picked out of the conversation.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Scrolls the pane `pixels` down, or up when negative, no further than
    /// `end`, where the last row sits against the foot of the pane.
    ///
    /// Scrolling back is also what stops the pane following the end of the
    /// conversation: a reader who has gone up to read something is not
    /// dragged down again by the next thing the agent says.
    pub fn scroll_by(&mut self, pixels: f32, end: f32) {
        let end = end.max(0.0);
        self.scroll = (self.scroll + pixels).clamp(0.0, end);
        self.following = self.scroll >= end;
    }

    /// Puts the pane `pixels` down, which is what following the end comes to.
    pub fn scroll_to(&mut self, pixels: f32) {
        self.scroll = pixels.max(0.0);
    }

    /// Whether the pane follows the end of the conversation as it grows.
    pub fn is_following(&self) -> bool {
        self.following
    }

    /// Whether the details starting at `block` are open.
    pub fn details_expanded(&self, block: usize) -> bool {
        self.expanded_details.contains(&block)
    }

    /// Opens or closes the details starting at `block`.
    pub fn toggle_details(&mut self, block: usize) {
        self.selection = None;
        if !self.expanded_details.insert(block) {
            self.expanded_details.remove(&block);
        }
        self.shown_revision += 1;
    }

    /// Counts the changes to what the pane shows that the transcript does
    /// not hold, so the pane can tell when its wrapped rows still stand.
    pub fn shown_revision(&self) -> u64 {
        self.shown_revision
    }

    /// The conversation as the pane last wrapped it, kept between frames.
    pub(super) fn wrapped(&self) -> &RefCell<Wrapped> {
        &self.wrapped
    }

    /// Sends what is in the prompt buffer, and empties it.
    ///
    /// The prompt goes into the transcript here rather than when the agent
    /// echoes it: an agent is not obliged to say back what it was told, and
    /// a reader who has pressed Enter should see what they sent.
    pub fn send(&mut self) {
        let text = self.prompt.value().trim().to_owned();
        if text.is_empty() && self.attachments.is_empty() {
            return;
        }
        self.prompt.clear();
        let attachments = std::mem::take(&mut self.attachments);
        let previews = std::mem::take(&mut self.attachment_previews)
            .into_iter()
            .map(|key| key.and_then(|key| self.previews.peek(&key)?.ready()))
            .collect::<Vec<_>>();
        self.previews.clear();
        let labels = attachments
            .iter()
            .zip(&previews)
            .filter(|(_, preview)| preview.is_none())
            .map(|(attachment, _)| format!("[{}]", attachment.label()))
            .collect::<Vec<_>>()
            .join(" ");
        let shown = if labels.is_empty() {
            text.clone()
        } else if text.is_empty() {
            labels
        } else {
            format!("{text}\n{labels}")
        };
        if !shown.is_empty() {
            self.transcript.say(Voice::Reader, &shown);
        }
        let mut attachments = attachments;
        for path in mentioned(self.root(), &text) {
            if !attachments
                .iter()
                .any(|attached| matches!(attached, Attachment::File(file) if *file == path))
            {
                attachments.push(Attachment::File(path));
            }
        }
        for preview in previews.into_iter().flatten() {
            self.transcript.picture(preview);
        }
        self.conversation.prompt(&text, attachments);
        self.chosen = 0;
        self.dismissed = false;
        self.busy = true;
        self.busy_since = Some(Instant::now());
        self.unseen = false;
        self.following = true;
    }

    /// Stops the turn that is running.
    pub fn cancel(&self) {
        self.conversation.cancel();
    }

    /// Answers the permission request `ask` with the choice in `place`.
    ///
    /// A request the reader has answered leaves the list whichever way they
    /// answered it: the agent is waiting on one reply and gets one.
    pub fn answer(&mut self, ask: u64, place: usize) {
        let Some(at) = self.asks.iter().position(|waiting| waiting.id == ask) else {
            return;
        };
        let waiting = self.asks.remove(at);
        match waiting.choices.get(place) {
            Some(choice) => self.conversation.allow(ask, &choice.id),
            None => self.conversation.refuse(ask),
        }
    }

    /// Takes in one thing the agent said.
    fn take(&mut self, event: Event) {
        match event {
            Event::Ready => {
                self.ready = true;
                self.logins.clear();
            }
            Event::Listed(page, more) => {
                self.history.extend(page);
                self.listing = more;
            }
            Event::ListFailed(error) => {
                self.history_error = Some(error);
                self.listing = false;
            }
            Event::Login(methods) => {
                let note = match methods.is_empty() {
                    true => format!(
                        "{} needs logging in, and offers no way the editor can do it.",
                        self.agent().name
                    ),
                    false => format!("{} needs logging in.", self.agent().name),
                };
                self.transcript.note(note);
                self.logins = methods;
            }
            Event::Said(voice, text) => self.transcript.say(voice, &text),
            Event::Ran(call) => self.transcript.ran(call),
            Event::Planned(steps) => self.transcript.planned(steps),
            Event::Offers(commands) => self.commands = commands,
            Event::Mode(mode) => self.mode = Some(mode),
            Event::Knobs(_) => {}
            Event::Titled(title) => self.title = Some(title).filter(|title| !title.is_empty()),
            Event::Used(usage) => self.usage = Some(usage),
            Event::Asked(ask) => self.asks.push(ask),
            Event::Requested(..) => {}
            Event::Stopped(stop) => {
                self.busy = false;
                self.busy_since = None;
                self.unseen = stop != Stop::Cancelled;
                if stop != Stop::EndTurn {
                    self.transcript.note(note(stop));
                }
            }
            Event::Failed(trouble) => {
                self.busy = false;
                self.busy_since = None;
                self.unseen = true;
                self.transcript.note(trouble);
            }
            Event::Ended => {
                self.busy = false;
                self.busy_since = None;
                self.ready = false;
                self.transcript.note(ended(&self.conversation));
            }
        }
    }
}

/// The files a mention offers, as far as they have been listed.
#[derive(Default)]
struct Mentionable {
    /// The files, relative to the worktree, each beside its lowercase form.
    files: Option<Arc<[(String, String)]>>,
    /// Whether they are being listed now.
    listing: bool,
    /// Whether a listing has come back that the window has not drawn yet.
    fresh: bool,
}

/// An image off the clipboard, made ready to attach away from the window.
pub struct Pasted {
    /// What goes with the prompt.
    attachment: Attachment,
    /// What is shown of it beside the prompt.
    preview: Option<Image>,
    /// Where it was saved, for an agent that takes it as a file.
    saved: Option<PathBuf>,
}

impl Pasted {
    /// Makes `width` by `height` RGBA `pixels` off the clipboard into what is
    /// attached: the image itself for an agent that `can_image`, a file of it
    /// for any other.
    ///
    /// Encoding a screenshot takes long enough to be felt, so this is run
    /// away from the window.
    pub fn prepare(width: u32, height: u32, pixels: Vec<u8>, can_image: bool) -> Option<Self> {
        let png = crate::desktop::encode_png(width, height, &pixels)?;
        let preview = Image::from_rgba(width, height, pixels);
        let (attachment, saved) = match can_image {
            true => (
                Attachment::Image {
                    data: base64::engine::general_purpose::STANDARD.encode(png),
                    mime_type: "image/png".to_owned(),
                },
                None,
            ),
            false => {
                let path = crate::desktop::save_pasted_image(&png)?;
                (Attachment::File(path.clone()), Some(path))
            }
        };
        Some(Self {
            attachment,
            preview,
            saved,
        })
    }
}

impl Drop for Talk {
    /// Removes pasted images kept for an agent without image prompt support.
    fn drop(&mut self) {
        for path in &self.clipboard_files {
            let _ = fs::remove_file(path);
        }
    }
}

/// What starts a mention of a file in a prompt.
const MENTION: char = '@';

/// The most files a mention offers at once.
const MENTIONED: usize = 50;

/// What trails a mention in prose without being part of the path.
const AFTER_MENTION: &[char] = &['.', ',', ';', ':', '!', '?', ')', '\'', '"'];

/// The mention `typed` ends in, without its at sign, when it ends in one.
fn mentioning(typed: &str) -> Option<&str> {
    typed
        .rsplit(char::is_whitespace)
        .next()?
        .strip_prefix(MENTION)
}

/// The files of the worktree at `root` that `text` mentions, in the order
/// it mentions them.
///
/// A mention is an at sign and a path from the root, and only one naming a
/// file that is there, inside the worktree, is taken: an address in an
/// email is an at sign too.
fn mentioned(root: &Path, text: &str) -> Vec<PathBuf> {
    let Ok(inside) = root.canonicalize() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for word in text.split_whitespace() {
        let Some(named) = word.strip_prefix(MENTION) else {
            continue;
        };
        let path = root.join(named.trim_end_matches(AFTER_MENTION));
        let there = path
            .canonicalize()
            .is_ok_and(|resolved| resolved.starts_with(&inside) && resolved.is_file());
        if there && !found.contains(&path) {
            found.push(path);
        }
    }
    found
}

/// Finds skills Codex can invoke from the user's and worktree's skill folders.
fn installed_skills(root: &Path, agent: Agent) -> Vec<Command> {
    if agent.id != "codex" {
        return Vec::new();
    }
    let mut folders = Vec::new();
    if let Some(home) = env::home_dir() {
        folders.push(home.join(".codex/skills"));
        folders.push(home.join(".agents/skills"));
    }
    if let Some(home) = env::var_os("CODEX_HOME") {
        folders.push(std::path::PathBuf::from(home).join("skills"));
    }
    folders.push(root.join(".codex/skills"));
    folders.push(root.join(".agents/skills"));
    let mut found = BTreeMap::new();
    for folder in folders {
        let Ok(entries) = fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path().join("SKILL.md");
            let Ok(contents) = fs::read_to_string(path) else {
                continue;
            };
            let name = contents
                .lines()
                .find_map(|line| line.strip_prefix("name: "))
                .map(str::trim);
            let Some(name) = name.filter(|name| !name.is_empty()) else {
                continue;
            };
            let description = contents
                .lines()
                .find_map(|line| line.strip_prefix("description: "))
                .unwrap_or_default()
                .trim()
                .to_owned();
            found.insert(
                name.to_owned(),
                Command {
                    name: name.to_owned(),
                    description,
                },
            );
        }
    }
    found.into_values().collect()
}

/// How a conversation is opened or taken up again.
enum Opening<'a> {
    /// Start a new conversation.
    New,
    /// Restore a saved pane, falling back to a fresh conversation.
    Restore(&'a str),
    /// Load a chosen saved conversation exactly.
    Exact(&'a str),
}

/// Every agent session the window is running.
#[derive(Default)]
pub struct Talks {
    /// The sessions, by the id each was handed.
    talks: BTreeMap<TalkId, Talk>,
    /// The id the next session started will be given.
    next: TalkId,
    /// How an agent wakes the window once it has something to say.
    notify: Option<Notify>,
    /// Whether a session has opened or retitled its conversation since this
    /// was asked.
    renamed: bool,
    /// The sessions whose agent went away on its own since this was asked.
    ended: Vec<TalkId>,
    /// The file and terminal requests the agents have raised and the window
    /// has not yet taken, with the ticket each is answered under.
    requests: Vec<(TalkId, u64, Request)>,
}

impl Talks {
    /// Wakes the window through `notify` whenever an agent says something.
    pub fn set_notify(&mut self, notify: Notify) {
        self.notify = Some(notify);
    }

    /// Starts `agent` in `root` for `project`, and says which talk it is.
    pub fn start(
        &mut self,
        project: ProjectId,
        session: Option<SessionId>,
        root: &Path,
        env: &[(String, String)],
        agent: Agent,
    ) -> Option<TalkId> {
        self.open(project, session, root, env, agent, Opening::New)
    }

    /// Takes the conversation `resume` names up again, in a session of its own.
    pub fn resume(
        &mut self,
        project: ProjectId,
        session: Option<SessionId>,
        root: &Path,
        env: &[(String, String)],
        agent: Agent,
        resume: &str,
    ) -> Option<TalkId> {
        self.open(project, session, root, env, agent, Opening::Restore(resume))
    }

    /// Loads a saved conversation without substituting a new one if it fails.
    pub fn load(
        &mut self,
        project: ProjectId,
        session: Option<SessionId>,
        root: &Path,
        env: &[(String, String)],
        agent: Agent,
        saved: &str,
    ) -> Option<TalkId> {
        self.open(project, session, root, env, agent, Opening::Exact(saved))
    }

    /// Opens an agent conversation with the requested load behavior.
    fn open(
        &mut self,
        project: ProjectId,
        session: Option<SessionId>,
        root: &Path,
        env: &[(String, String)],
        agent: Agent,
        opening: Opening<'_>,
    ) -> Option<TalkId> {
        let notify = self.notify.clone()?;
        let started = match opening {
            Opening::Exact(saved) => Session::load(agent, root, env, saved, notify.clone()),
            Opening::Restore(saved) => Session::resume(agent, root, env, saved, notify.clone()),
            Opening::New => Session::start(agent, root, env, notify.clone()),
        };
        let conversation = match started {
            Ok(conversation) => conversation,
            Err(error) => {
                eprintln!(
                    "could not start {} in {}: {error}",
                    agent.name,
                    root.display()
                );
                return None;
            }
        };

        let id = self.next;
        self.next = TalkId(id.0 + 1);
        self.talks.insert(
            id,
            Talk {
                id,
                project,
                session,
                conversation,
                env: env.to_vec(),
                logins: Vec::new(),
                transcript: Transcript::default(),
                prompt: Input::many_lines("Prompt"),
                attachments: Vec::new(),
                attachment_previews: Vec::new(),
                previews: Decodes::default(),
                next_preview: 0,
                clipboard_files: Vec::new(),
                asks: Vec::new(),
                commands: Vec::new(),
                skills: installed_skills(root, agent),
                history: Vec::new(),
                listing: false,
                history_error: None,
                mentionable: Arc::default(),
                notify: notify.clone(),
                chosen: 0,
                dismissed: false,
                ready: false,
                busy: false,
                busy_since: None,
                unseen: false,
                mode: None,
                title: None,
                usage: None,
                scroll: 0.0,
                view: Bounds::default(),
                drawn_height: Rc::default(),
                drawn_width: Rc::default(),
                drawn_links: Rc::default(),
                drawn_text: Rc::default(),
                drawn_spots: Rc::default(),
                selection: None,
                following: true,
                expanded_details: BTreeSet::new(),
                shown_revision: 0,
                wrapped: RefCell::default(),
                terminals: BTreeMap::new(),
            },
        );
        Some(id)
    }

    /// Starts the agent of the conversation `id` names again, in a new
    /// conversation, answering whether it started.
    ///
    /// This is what follows a login the agent had the reader do outside it:
    /// the agent reads what the login left behind only when it starts.
    pub fn restart(&mut self, id: TalkId) -> bool {
        let Some(notify) = self.notify.clone() else {
            return false;
        };
        let Some(talk) = self.talks.get_mut(&id) else {
            return false;
        };
        match Session::start(talk.agent(), talk.root(), &talk.env, notify) {
            Ok(conversation) => {
                talk.conversation = conversation;
                talk.ready = false;
                talk.busy = false;
                talk.busy_since = None;
                talk.logins.clear();
                talk.asks.clear();
                talk.transcript.note("Logged in. Starting the agent again…");
                true
            }
            Err(error) => {
                talk.transcript
                    .note(format!("The agent would not start again: {error}"));
                false
            }
        }
    }

    /// How many sessions `project` has running.
    pub fn count(&self, project: ProjectId) -> usize {
        self.talks
            .values()
            .filter(|talk| talk.project == project)
            .count()
    }

    /// The conversation running in the session `session` names, if one is.
    pub fn of_session(&self, session: SessionId) -> Option<TalkId> {
        self.talks
            .values()
            .find(|talk| talk.session == Some(session))
            .map(Talk::id)
    }

    /// Every conversation in the window, across all its projects.
    pub fn iter(&self) -> impl Iterator<Item = &Talk> {
        self.talks.values()
    }

    /// The conversation `id` names.
    pub fn get(&self, id: TalkId) -> Option<&Talk> {
        self.talks.get(&id)
    }

    /// The conversation `id` names, to act on.
    pub fn get_mut(&mut self, id: TalkId) -> Option<&mut Talk> {
        self.talks.get_mut(&id)
    }

    /// Finds a conversation already open for this agent, worktree and saved id.
    pub fn find_saved(&self, scope: Scope, agent: Agent, saved: &str) -> Option<TalkId> {
        self.talks.values().find_map(|talk| {
            (talk.scope() == scope
                && talk.agent() == agent
                && talk.resumable().as_deref() == Some(saved))
            .then_some(talk.id())
        })
    }

    /// Ends every session but the ones `held` names.
    ///
    /// An agent is a process, and the tab is the whole of what is holding it:
    /// a session nothing shows any more is one nobody can read, answer or
    /// stop, so it is ended rather than left running unseen.
    pub fn retain(&mut self, held: &BTreeSet<TalkId>) {
        self.talks.retain(|id, _| held.contains(id));
    }

    /// Ends every session of `project`, for a project leaving the window.
    pub fn close_project(&mut self, project: ProjectId) {
        self.talks.retain(|_, talk| talk.project != project);
    }

    /// How many of the window's conversations stand each way, across every
    /// project.
    pub fn tally(&self) -> Tally {
        self.talks
            .values()
            .fold(Tally::default(), |mut tally, talk| {
                match talk.standing() {
                    Standing::Stopped => tally.stopped += 1,
                    Standing::Waiting => tally.waiting += 1,
                    Standing::Working => tally.working += 1,
                    Standing::Done => tally.done += 1,
                    Standing::Idle => tally.idle += 1,
                }
                tally
            })
    }

    /// The sessions whose agent went away on its own since this was asked.
    ///
    /// Ending a session is closing its tab, which drops it without a word:
    /// an agent that is heard ending is one that stopped by itself.
    pub fn take_ended(&mut self) -> Vec<TalkId> {
        std::mem::take(&mut self.ended)
    }

    /// How many of the conversations are in the middle of a turn.
    pub fn working(&self) -> usize {
        self.talks.values().filter(|talk| talk.is_busy()).count()
    }

    /// Applies what every agent has said, and says whether anything changed.
    ///
    /// A session whose agent has gone is kept rather than dropped: what was
    /// said is still worth reading, and the tab closes when the reader closes
    /// it.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        for talk in self.talks.values_mut() {
            changed |= talk.take_listed();
            for event in talk.conversation.drain() {
                if let Event::Requested(ticket, request) = event {
                    self.requests.push((talk.id, ticket, request));
                    changed = true;
                    continue;
                }
                self.renamed |= matches!(event, Event::Ready | Event::Titled(_));
                if matches!(event, Event::Ended) {
                    self.ended.push(talk.id);
                }
                talk.take(event);
                changed = true;
            }
        }
        changed
    }

    /// The file and terminal requests raised since this was last asked, each
    /// with the conversation and the ticket it is answered through.
    pub fn take_requests(&mut self) -> Vec<(TalkId, u64, Request)> {
        std::mem::take(&mut self.requests)
    }

    /// Whether a conversation has been opened or retitled since this was
    /// last asked.
    ///
    /// The window writes down which conversation each pane is holding, and
    /// under what title, so that the next launch can take it up again; the
    /// name and the title are the agent's, and neither exists until the agent
    /// has said it.
    pub fn take_renamed(&mut self) -> bool {
        std::mem::take(&mut self.renamed)
    }
}

/// What the transcript says about a turn that ended some other way.
fn note(stop: Stop) -> &'static str {
    match stop {
        Stop::EndTurn => "",
        Stop::MaxTokens => "The agent ran out of tokens.",
        Stop::MaxRequests => "The agent ran out of turns.",
        Stop::Refusal => "The agent refused to go on.",
        Stop::Cancelled => "Stopped.",
    }
}

/// What the transcript says about an agent whose process has gone.
///
/// An agent that will not start says why on its error pipe and nowhere else,
/// so what it wrote there is what the reader is shown.
fn ended(session: &Session) -> String {
    let trouble = session.trouble();
    let last = trouble.lines().next_back().unwrap_or_default().trim();
    match last.is_empty() {
        true => "The agent has stopped.".to_owned(),
        false => format!("The agent has stopped: {last}"),
    }
}
