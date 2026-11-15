//! What the reader does in the open session's composer, sent to the server:
//! sending a prompt with what is attached, stopping the turn, changing the
//! agent's settings, and handling the prompts queued behind the turn; and
//! the files, pictures, browser elements and captures attached to the next
//! prompt.

use std::time::{SystemTime, UNIX_EPOCH};

use pm_protocol::{Attachment, Request, Response};

use super::{Ask, Link};
use crate::composer::{
    Capture, CaptureKind, ComposerAction, PickedElement, PromptFile, allowance_gauge, prompt_text,
};
use crate::open_files::is_picture;

/// A prompt sent and not yet taken by the server: what it was sent to, what the reader typed and what was attached, given back to the composer when it fails.
pub(super) struct Unsent {
    /// The session it was sent to.
    pub(super) session: u64,
    /// What the reader typed.
    text: String,
    /// The files attached.
    files: Vec<PromptFile>,
    /// The browser elements attached.
    elements: Vec<PickedElement>,
    /// The browser captures attached.
    captures: Vec<Capture>,
}

impl Link {
    /// Does what the reader did in the composer, `action`.
    pub fn composer_action(&mut self, action: ComposerAction) {
        match action {
            ComposerAction::SendNow(text) => self.prompt(text, true),
            ComposerAction::Stop => self.stop_turn(),
            ComposerAction::Set(setting, value) => self.set_agent_setting(setting, value),
            ComposerAction::Attach(paths) => self.attach_paths(paths),
            ComposerAction::Picture(width, height, rgba) => {
                self.attach_picture(width, height, rgba)
            }
            ComposerAction::DetachFile(index) => self.detach_file(index),
            ComposerAction::DetachElement(index) => self.detach_element(index),
            ComposerAction::DetachCapture(index) => self.detach_capture(index),
            ComposerAction::Hurry(queued) => self.hurry_queued(queued),
            ComposerAction::RemoveQueued(queued) => self.remove_queued(queued),
            ComposerAction::HoldQueue(held) => self.hold_queue(held),
            ComposerAction::EditQueued(queued, text) => self.edit_queued(queued, text),
            ComposerAction::Place(worktree, container) => self.choose_place(worktree, container),
            ComposerAction::Browse => {}
            ComposerAction::Mention => {
                if !self.workspace.tree.loaded
                    || self.browse_root().as_deref() != Some(self.workspace.tree.root.as_str())
                {
                    self.load_files();
                }
            }
            ComposerAction::Account(login) => self.choose_account(login),
            ComposerAction::ManageAccounts => self.manage_accounts(),
            ComposerAction::CycleMode => {
                if let Some((setting, value)) =
                    crate::composer::next_mode(&self.workspace.extras.composer.settings)
                {
                    self.set_agent_setting(setting, value);
                }
            }
        }
    }

    /// Sends `text` to the open session with what is attached: in place of
    /// the turn the agent is on when `now`, otherwise as its next turn or
    /// queued behind the one it is on.
    pub fn prompt(&mut self, text: String, now: bool) {
        let Some(session) = self.watched else {
            self.prefill(text);
            return;
        };
        self.workspace.extras.prefill = None;
        let composer = &mut self.workspace.extras.composer;
        let typed = text.clone();
        let text = prompt_text(&text, &composer.elements, &composer.captures);
        let mut attachments: Vec<Attachment> =
            composer.files.iter().filter_map(attachment_of).collect();
        attachments.extend(
            composer
                .captures
                .iter()
                .filter(|capture| capture.kind == CaptureKind::Screenshot)
                .enumerate()
                .map(|(place, capture)| Attachment {
                    name: format!("screenshot-{}.png", place + 1),
                    data: capture.data.clone(),
                }),
        );
        let unsent = Unsent {
            session,
            text: typed,
            files: std::mem::take(&mut composer.files),
            elements: std::mem::take(&mut composer.elements),
            captures: std::mem::take(&mut composer.captures),
        };
        let kept = self.next_unsent;
        self.next_unsent += 1;
        self.unsent.insert(kept, unsent);
        self.workspace.sending = true;
        self.ask(
            Request::Prompt {
                session,
                text,
                attachments,
                now,
            },
            Ask::Prompt(kept),
        );
    }

    /// Puts `text` in the open composer, even when it is what was put there last.
    pub(super) fn prefill(&mut self, text: String) {
        self.workspace.extras.prefill = Some(text);
        self.workspace.extras.prefills += 1;
    }

    /// Takes the server's `response` to the prompt whose attachments are kept under `kept`: forgets them once it was taken, and otherwise shows why it failed and, while its session is still open, puts its text and attachments back into the composer.
    pub(super) fn answer_prompt(&mut self, kept: u64, response: Response) {
        let Some(unsent) = self.unsent.remove(&kept) else {
            return;
        };
        self.workspace.sending = self
            .unsent
            .values()
            .any(|held| Some(held.session) == self.watched);
        let Response::Failed { message } = response else {
            return;
        };
        self.workspace.status = Some(message);
        if self.watched != Some(unsent.session) {
            return;
        }
        let composer = &mut self.workspace.extras.composer;
        composer.files.splice(0..0, unsent.files);
        composer.elements.splice(0..0, unsent.elements);
        composer.captures.splice(0..0, unsent.captures);
        self.prefill(unsent.text);
    }

    /// Puts the login the open session runs as into the account chip, with the named logins of its agent it offers; a session of another machine offers none, since its logins are that machine's.
    pub(super) fn show_accounts(&mut self) {
        let Some(session) = self.watched else {
            return;
        };
        let info = self.session_info(session);
        let login = info.as_ref().and_then(|info| info.login.clone());
        let agent = info.map(|info| info.agent).unwrap_or_default();
        let accounts: Vec<(String, String)> = if self.routes.remote(session).is_some() {
            Vec::new()
        } else {
            self.settings
                .forms
                .logins
                .iter()
                .filter(|held| held.agent == agent)
                .map(|held| (held.id.clone(), held.name.clone()))
                .collect()
        };
        self.workspace.account = match &login {
            Some(id) => accounts
                .iter()
                .find(|(held, _)| held == id)
                .map_or_else(|| id.clone(), |(_, name)| name.clone()),
            None if accounts.is_empty() => String::new(),
            None => "Default account".to_owned(),
        };
        self.workspace.login = login;
        self.workspace.accounts = accounts;
    }

    /// Has the open session run as the login `login`, or as its agent's own when nothing.
    pub fn choose_account(&mut self, login: Option<String>) {
        let Some(session) = self.watched else {
            return;
        };
        if let Some(info) = self.sessions.iter_mut().find(|info| info.session == session) {
            info.login = login.clone();
        }
        self.ask(Request::SetSessionAccount { session, login }, Ask::Nothing);
        self.show_accounts();
    }

    /// Readies the settings window on the open session's agent, its keys, sign-in and accounts, for the reader to add an account.
    pub fn manage_accounts(&mut self) {
        let agent = self
            .watched
            .and_then(|session| self.session_info(session))
            .map(|info| info.agent);
        self.settings.page.section = crate::settings_section::Section::AgentServers;
        let change = crate::settings_forms::FormChange::OpenAgent(agent);
        for request in self.settings.take_form(change) {
            self.ask(request, Ask::Form);
        }
    }

    /// Stops the turn the open session's agent is on and drops the prompts queued behind it.
    pub fn stop_turn(&mut self) {
        if let Some(session) = self.watched {
            self.ask(Request::Cancel { session }, Ask::Nothing);
        }
    }

    /// Gives the open session's agent setting `setting` the value `value`.
    pub fn set_agent_setting(&mut self, setting: String, value: String) {
        if self.workspace.starting.is_some() {
            self.set_start_setting(setting, value);
            return;
        }
        let Some(session) = self.watched else {
            return;
        };
        if let Some(thread) = self.threads.get_mut(&session) {
            for known in &mut thread.extras.composer.settings {
                if known.id == setting {
                    known.current = value.clone();
                }
            }
        }
        for known in &mut self.workspace.extras.composer.settings {
            if known.id == setting {
                known.current = value.clone();
            }
        }
        self.ask(
            Request::Set {
                session,
                setting,
                value,
            },
            Ask::Nothing,
        );
    }

    /// Stops the open session's turn and hands its agent the queued prompt `queued` next.
    pub fn hurry_queued(&mut self, queued: u64) {
        if let Some(session) = self.watched {
            self.ask(Request::Hurry { session, queued }, Ask::Nothing);
        }
    }

    /// Removes the queued prompt `queued` of the open session without sending it.
    pub fn remove_queued(&mut self, queued: u64) {
        if let Some(session) = self.watched {
            self.ask(Request::RemoveQueued { session, queued }, Ask::Nothing);
        }
    }

    /// Holds the open session's queue while a queued prompt is edited, or lets it go on.
    pub fn hold_queue(&mut self, held: bool) {
        if let Some(session) = self.watched {
            self.ask(Request::HoldQueue { session, held }, Ask::Nothing);
        }
    }

    /// Makes the queued prompt `queued` of the open session say `text`, and lets the queue go on.
    pub fn edit_queued(&mut self, queued: u64, text: String) {
        if let Some(session) = self.watched {
            self.ask(
                Request::EditQueued {
                    session,
                    queued,
                    text,
                },
                Ask::Nothing,
            );
        }
        self.hold_queue(false);
    }

    /// Attaches the files at `paths` on the reader's machine to the next prompt.
    pub fn attach_paths(&mut self, paths: Vec<String>) {
        let files = &mut self.workspace.extras.composer.files;
        for path in paths {
            if files.iter().any(|file| file.path.as_ref() == Some(&path)) {
                continue;
            }
            let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_owned();
            files.push(PromptFile {
                picture: is_picture(&name),
                name,
                path: Some(path),
                data: Vec::new(),
            });
        }
    }

    /// Attaches a pasted picture `width` by `height` pixels of RGBA rows `rgba` to the next prompt, as a PNG.
    pub fn attach_picture(&mut self, width: u32, height: u32, rgba: Vec<u8>) {
        let Some(data) = encode_png(width, height, &rgba) else {
            return;
        };
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        self.workspace.extras.composer.files.push(PromptFile {
            name: format!("pasted-{stamp}.png"),
            path: None,
            data,
            picture: true,
        });
    }

    /// Takes the file at position `index` off the next prompt.
    pub fn detach_file(&mut self, index: usize) {
        let files = &mut self.workspace.extras.composer.files;
        if index < files.len() {
            files.remove(index);
        }
    }

    /// Attaches the browser element `element` to the next prompt.
    pub fn attach_element(&mut self, element: PickedElement) {
        self.workspace.extras.composer.elements.push(element);
    }

    /// Takes the browser element at position `index` off the next prompt.
    pub fn detach_element(&mut self, index: usize) {
        let elements = &mut self.workspace.extras.composer.elements;
        if index < elements.len() {
            elements.remove(index);
        }
    }

    /// Attaches the browser capture `capture` to the next prompt.
    pub fn attach_capture(&mut self, capture: Capture) {
        self.workspace.extras.composer.captures.push(capture);
    }

    /// Takes the browser capture at position `index` off the next prompt.
    pub fn detach_capture(&mut self, index: usize) {
        let captures = &mut self.workspace.extras.composer.captures;
        if index < captures.len() {
            captures.remove(index);
        }
    }

    /// Asks the server for what the composer of session `session` shows
    /// beyond its events: the model slots of its agent, and how much of
    /// its account's allowance `agent` under `login` has used.
    pub(super) fn ask_composer(&mut self, session: u64, agent: String, login: Option<String>) {
        self.ask(
            Request::ModelSlots {
                session: Some(session),
                change: None,
            },
            Ask::SessionSlots(session),
        );
        if !agent.is_empty() {
            self.ask(
                Request::Allowance { agent, login },
                Ask::SessionAllowance(session),
            );
        }
    }

    /// Handles the `response` to a request the composer made for `ask`.
    pub(super) fn composer_answer(&mut self, ask: Ask, response: Response) {
        match (ask, response) {
            (Ask::SessionSlots(session), Response::ModelSlots { slots, .. }) => {
                let thread = self.threads.entry(session).or_default();
                thread.set_slots(slots);
                if self.watched == Some(session) {
                    self.workspace.extras.composer.settings =
                        thread.extras.composer.settings.clone();
                }
            }
            (Ask::SessionAllowance(session), Response::Allowance { limits }) => {
                let gauge = limits.and_then(|limits| allowance_gauge(&limits.0));
                let thread = self.threads.entry(session).or_default();
                thread.extras.composer.allowance = gauge.clone();
                if self.watched == Some(session) {
                    self.workspace.extras.composer.allowance = gauge;
                }
            }
            _ => {}
        }
    }
}

/// The attachment `file` is sent as: its bytes as pasted, or read from its path, or nothing when that cannot be read.
pub(super) fn attachment_of(file: &PromptFile) -> Option<Attachment> {
    let data = match &file.path {
        Some(path) => std::fs::read(path).ok()?,
        None => file.data.clone(),
    };
    Some(Attachment {
        name: file.name.clone(),
        data,
    })
}

/// `rgba`, `width` by `height` pixels, encoded as PNG, or nothing when it does not hold that many pixels.
fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if rgba.len() != width as usize * height as usize * 4 {
        return None;
    }
    let mut data = Vec::new();
    let mut encoder = png::Encoder::new(&mut data, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(rgba).ok()?;
    writer.finish().ok()?;
    Some(data)
}

#[cfg(test)]
/// Tests for the attachments of a prompt.
mod tests {
    use super::*;

    /// A picture is encoded only when it holds as many pixels as it says.
    #[test]
    fn pictures_are_encoded_when_whole() {
        let encoded = encode_png(1, 1, &[255, 0, 0, 255]).unwrap_or_default();
        assert!(encoded.starts_with(&[0x89, b'P', b'N', b'G']));
        assert!(encode_png(2, 2, &[0; 4]).is_none());
    }
}
