//! The form on the pages of skills, instruction files and agent definitions that writes a new one from a template, in the project's folder or the reader's own, and opens it to be filled in.

use pm_input::FocusId;
use pm_protocol::library::{
    APPENDS_PROMPT, Item, Kind, PromptFile, REPLACES_PROMPT, Scope, valid_name,
};
use pm_style::{
    Radius, Role, bg, border, border_color, flex, flex_1, flex_col, flex_none, gap, items_center,
    min_w_px, p, rounded,
};
use pm_text::{Edit, Field};
use pm_ui::{Node, node};

use crate::button::Variant;
use crate::parts::line;
use crate::settings_ids::{DRAFT_SCOPES, SINGLES};
use crate::settings_row::choices;
use crate::settings_servers::{boxed, flat_row, wrapped_note};
use crate::size::Size;
use crate::state::{Outgoing, WorkspaceState};
use crate::{Button, Input};

/// Opens crab's `SYSTEM.md`, making it first when it is not there.
pub const PROMPT_REPLACE: FocusId = SINGLES.at(83);

/// Opens crab's `APPEND_SYSTEM.md`, making it first when it is not there.
pub const PROMPT_APPEND: FocusId = SINGLES.at(84);

/// Opens the form for a new customization of the page showing.
pub const DRAFT_NEW: FocusId = SINGLES.at(93);

/// The box the new customization's name is written in.
pub const DRAFT_NAME: FocusId = SINGLES.at(94);

/// The box what the new customization is for is written in.
pub const DRAFT_DESCRIPTION: FocusId = SINGLES.at(95);

/// Writes the new customization.
pub const DRAFT_CREATE: FocusId = SINGLES.at(96);

/// Closes the form without writing anything.
pub const DRAFT_CANCEL: FocusId = SINGLES.at(97);

/// Where a new customization can go, in the order the form offers them.
const SCOPES: [Scope; 2] = [Scope::Workspace, Scope::User];

const _: () = assert!(
    SCOPES.len() as u64 <= DRAFT_SCOPES.len(),
    "DRAFT_SCOPES has no room for every place a customization can go"
);

/// A new customization being described.
#[derive(Clone, Debug, PartialEq)]
pub struct DraftForm {
    /// What it is.
    pub kind: Kind,
    /// Whether it goes in the project's folder or the reader's own.
    pub scope: Scope,
    /// Its name.
    pub name: Field,
    /// What it is for.
    pub description: Field,
    /// Why the server could not write the last one asked for.
    pub error: Option<String>,
    /// Whether the server is writing it.
    pub busy: bool,
    /// Whether a project is open for it to go in; when none is, it goes in the reader's own folder.
    pub in_project: bool,
}

impl DraftForm {
    /// An empty form for a customization of `kind`, going in the project's folder when `in_project` says one is open and in the reader's own otherwise.
    fn new(kind: Kind, in_project: bool) -> Self {
        Self {
            kind,
            scope: if in_project {
                Scope::Workspace
            } else {
                Scope::User
            },
            name: Field::default(),
            description: Field::default(),
            error: None,
            busy: false,
            in_project,
        }
    }

    /// The name as written, without spaces around it.
    fn name(&self) -> &str {
        self.name.text().trim()
    }

    /// Whether the form holds enough to be written.
    fn complete(&self) -> bool {
        valid_name(self.name()) && !self.busy
    }
}

/// The choice that puts a new customization in `scope`, when the form offers it.
pub fn draft_scope(scope: Scope) -> Option<FocusId> {
    SCOPES
        .iter()
        .position(|offered| *offered == scope)
        .and_then(|place| DRAFT_SCOPES.id(place))
}

/// Whether customizations of `kind` can be written from a template here; hooks live in settings files and cannot.
pub(crate) fn draftable(kind: Kind) -> bool {
    kind != Kind::Hook
}

/// What the button opening the form says for `kind`.
fn new_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Skill => "New skill",
        Kind::Instruction => "New instructions",
        Kind::Agent => "New agent",
        Kind::Hook => "",
    }
}

/// Whether `id` names a box of the form.
pub(crate) fn is_draft_field(id: FocusId) -> bool {
    id == DRAFT_NAME || id == DRAFT_DESCRIPTION
}

/// Whether `id` names a control of the form.
fn is_draft_control(id: FocusId) -> bool {
    [DRAFT_NEW, DRAFT_CREATE, DRAFT_CANCEL].contains(&id) || DRAFT_SCOPES.contains(id)
}

impl WorkspaceState {
    /// Opens the form for a new customization of `kind`.
    pub(crate) fn open_draft(&mut self, kind: Kind) {
        let in_project = self
            .settings_page
            .library
            .of
            .is_some_and(|(project, _)| project.is_some());
        self.settings_page.library.draft = Some(DraftForm::new(kind, in_project));
    }

    /// The state after `action`, which may name a control of the form, was pressed on the page of `kind`; whether it named one.
    pub(crate) fn draft_pressed(&mut self, action: FocusId, kind: Option<Kind>) -> bool {
        if !is_draft_control(action) {
            return false;
        }
        match action {
            DRAFT_NEW => {
                if let Some(kind) = kind.filter(|kind| draftable(*kind)) {
                    self.open_draft(kind);
                }
            }
            DRAFT_CANCEL => self.settings_page.library.draft = None,
            DRAFT_CREATE => self.create_draft(),
            _ => {
                let scope = DRAFT_SCOPES
                    .index_of(action)
                    .and_then(|place| SCOPES.get(place));
                if let (Some(form), Some(scope)) = (
                    self.settings_page
                        .library
                        .draft
                        .as_mut()
                        .filter(|form| form.in_project),
                    scope,
                ) {
                    form.scope = *scope;
                }
            }
        }
        true
    }

    /// Has the server of the project the pages are about write what the form holds.
    fn create_draft(&mut self) {
        let Some((project, _)) = self.settings_page.library.of else {
            return;
        };
        let Some((host, directory)) = self.library_place(project) else {
            return;
        };
        let Some(form) = self
            .settings_page
            .library
            .draft
            .as_mut()
            .filter(|form| form.complete())
        else {
            return;
        };
        form.busy = true;
        form.error = None;
        self.outbox.push(Outgoing::CreateCustomization {
            project,
            host,
            directory,
            kind: form.kind,
            scope: form.scope,
            name: form.name().to_owned(),
            description: form.description.text().trim().to_owned(),
        });
    }

    /// The state after the server wrote the new customization at `path` for the project at `project`: the form closes, the pages are read again and the file opens, even when the form was put away while the server wrote it; then the file is taken to be the project's when it lies in the project's folder.
    pub fn with_customization_created(mut self, project: Option<usize>, path: String) -> Self {
        let scope = self
            .settings_page
            .library
            .draft
            .take()
            .map_or(Scope::Workspace, |form| form.scope);
        self.read_library();
        self.open_customization(project, path, scope);
        self
    }

    /// The state after the server could not write the new customization, for `reason`.
    pub fn with_customization_failed(mut self, reason: String) -> Self {
        if let Some(form) = self.settings_page.library.draft.as_mut() {
            form.busy = false;
            form.error = Some(reason);
        }
        self
    }

    /// Writes `edit` into the box `id` of the form.
    pub(crate) fn draft_edited(&mut self, id: FocusId, edit: Edit) {
        let Some(form) = self.settings_page.library.draft.as_mut() else {
            return;
        };
        form.error = None;
        match id {
            DRAFT_NAME => form.name.apply(edit),
            DRAFT_DESCRIPTION => form.description.apply(edit),
            _ => {}
        }
    }

    /// The box `id` of the form, if it is open and `id` names one.
    pub(crate) fn draft_field(&self, id: FocusId) -> Option<&Field> {
        let form = self.settings_page.library.draft.as_ref()?;
        match id {
            DRAFT_NAME => Some(&form.name),
            DRAFT_DESCRIPTION => Some(&form.description),
            _ => None,
        }
    }
}

impl WorkspaceState {
    /// The state after `action`, which may name one of the buttons of crab's prompt files, was pressed; whether it named one. The file in force is opened; one not there yet is made empty in the reader's own folder, then opened.
    pub(crate) fn prompt_pressed(&mut self, action: FocusId) -> bool {
        let file = match action {
            PROMPT_REPLACE => PromptFile::Replace,
            PROMPT_APPEND => PromptFile::Append,
            _ => return false,
        };
        let Some((project, _)) = self.settings_page.library.of else {
            return true;
        };
        if let Some(item) = prompt_item(self, file) {
            let (path, scope) = (item.path.clone(), item.scope);
            self.open_customization(project, path, scope);
            return true;
        }
        let Some((host, directory)) = self.library_place(project) else {
            return true;
        };
        self.outbox.push(Outgoing::CreatePromptFile {
            project,
            host,
            directory,
            file,
            scope: Scope::User,
        });
        true
    }
}

/// The instruction row of the prompt file `file` in force, when there is one.
fn prompt_item(state: &WorkspaceState, file: PromptFile) -> Option<&Item> {
    let said = match file {
        PromptFile::Replace => REPLACES_PROMPT,
        PromptFile::Append => APPENDS_PROMPT,
    };
    state
        .settings_page
        .library
        .items
        .as_ref()?
        .iter()
        .find(|item| {
            item.kind == Kind::Instruction && item.applies.first().map(String::as_str) == Some(said)
        })
}

/// The box of crab's own prompt files on the page of instructions: the one replacing its built-in instructions and the one added to every prompt, each with the button that opens or makes it.
pub(crate) fn prompt_node(state: &WorkspaceState, room: f32) -> Node {
    let row = |file: PromptFile, title: &str, about: &str, id: FocusId| {
        let found = prompt_item(state, file);
        let said = match found {
            Some(_) => format!("{about} Written."),
            None => format!("{about} Not written yet."),
        };
        flat_row()
            .child(
                node()
                    .styles([flex(), flex_col(), gap(1), flex_1(), min_w_px(0)])
                    .child(line(title, 13, 1.4, Role::Text))
                    .child(wrapped_note(&said, (room - 160.0).max(120.0))),
            )
            .child(
                Button::new(if found.is_some() { "Open" } else { "Write" }, id)
                    .variant(Variant::Outline)
                    .size(Size::Sm)
                    .disabled(state.settings_page.library.of.is_none())
                    .build(),
            )
    };
    node()
        .styles([flex(), flex_col(), gap(2), flex_none()])
        .child(
            node()
                .styles([flex(), items_center(), flex_none(), pm_style::h(9)])
                .child(line("System prompt", 15, 1.4, Role::Text).styles([pm_style::font_medium()])),
        )
        .child(boxed(vec![
            row(
                PromptFile::Replace,
                "Replace crab's instructions",
                "SYSTEM.md takes the place of crab's built-in instructions; its tools, skills and the project's details are still added.",
                PROMPT_REPLACE,
            ),
            row(
                PromptFile::Append,
                "Add to every prompt",
                "APPEND_SYSTEM.md is added to the end of every prompt, subagents' too.",
                PROMPT_APPEND,
            ),
        ]))
}

/// The button that opens the form for a new customization of `kind`, turned off while the form is open or nothing can be written.
pub(crate) fn new_button(state: &WorkspaceState, kind: Kind) -> Node {
    let library = &state.settings_page.library;
    Button::new(new_label(kind), DRAFT_NEW)
        .variant(Variant::Outline)
        .size(Size::Sm)
        .disabled(library.draft.is_some() || library.of.is_none())
        .build()
}

/// What the form says about where `scope` puts a new customization of `kind`.
fn place_note(kind: Kind, scope: Scope) -> String {
    let folder = match scope {
        Scope::User => "crab's config folder (~/.config/oh-my-crab)",
        _ => "the project's .oh-my-crab folder",
    };
    let file = match kind {
        Kind::Skill => "skills/<name>/SKILL.md",
        Kind::Instruction => "rules/<name>.md",
        _ => "agents/<name>.md",
    };
    let reach = match scope {
        Scope::User => "every project",
        _ => "this project",
    };
    format!(
        "Written to {file} in {folder}, for {reach}, then opened to be filled in. Only crab reads it; Claude Code's and Codex's folders are left alone."
    )
}

/// The form for a new customization, `room` logical pixels across.
pub(crate) fn draft_node(form: &DraftForm, room: f32) -> Node {
    let (name_hint, description_label, description_hint) = match form.kind {
        Kind::Skill => (
            "release-notes",
            "Description: when the agent should use it",
            "Writes release notes from the commits since the last tag",
        ),
        Kind::Instruction => (
            "code-style",
            "First line of the file",
            "How code in this project is written",
        ),
        _ => (
            "reviewer",
            "Description: when the agent should hand work to it",
            "Reviews a change for bugs before it ships",
        ),
    };
    let picked = SCOPES
        .iter()
        .position(|scope| *scope == form.scope)
        .unwrap_or(0);
    let name = form.name();
    let message = match (&form.error, name.is_empty() || valid_name(name)) {
        (Some(reason), _) => Some(reason.clone()),
        (None, false) => {
            Some("Use lowercase letters, digits and hyphens, up to 64 characters.".to_owned())
        }
        (None, true) => None,
    };
    node()
        .styles([
            flex(),
            flex_col(),
            gap(3),
            p(4),
            rounded(Radius::Lg),
            border(),
            border_color(Role::Border),
            bg(Role::Surface),
            flex_none(),
        ])
        .child(labelled("Name", &form.name, DRAFT_NAME, name_hint))
        .child(labelled(
            description_label,
            &form.description,
            DRAFT_DESCRIPTION,
            description_hint,
        ))
        .children(form.in_project.then(|| {
            node()
                .styles([flex(), flex_col(), gap(1)])
                .child(line("Where", 12, 1.4, Role::Muted))
                .child(node().styles([flex()]).child(choices(
                    &["This project", "Every project"],
                    picked,
                    |place| DRAFT_SCOPES.id(place),
                )))
        }))
        .child(wrapped_note(&place_note(form.kind, form.scope), room))
        .children(message.map(|message| line(message, 12, 1.4, Role::Danger)))
        .child(
            node()
                .styles([flex(), items_center(), gap(2)])
                .child(
                    Button::new(if form.busy { "Creating…" } else { "Create" }, DRAFT_CREATE)
                        .size(Size::Sm)
                        .disabled(!form.complete())
                        .build(),
                )
                .child(
                    Button::new("Cancel", DRAFT_CANCEL)
                        .variant(Variant::Ghost)
                        .size(Size::Sm)
                        .build(),
                ),
        )
}

/// A box under the words that say what is written in it.
fn labelled(label: &str, field: &Field, id: FocusId, placeholder: &str) -> Node {
    node()
        .styles([flex(), flex_col(), gap(1)])
        .child(line(label, 12, 1.4, Role::Muted))
        .child(
            Input::new(field, id)
                .size(Size::Sm)
                .placeholder(placeholder)
                .build(),
        )
}

#[cfg(test)]
/// Tests for the form for a new customization.
mod tests {
    use super::*;

    /// A form is complete only with a valid name and while nothing is being written.
    #[test]
    fn a_form_needs_a_valid_name() {
        let mut form = DraftForm::new(Kind::Skill, true);
        assert!(!form.complete());
        form.name = Field::new("Not Valid");
        assert!(!form.complete());
        form.name = Field::new(" ship-it ");
        assert!(form.complete());
        form.busy = true;
        assert!(!form.complete());
    }

    /// The form opens on a skills page, sends what it holds to the project's server, and closes once the server wrote the file; a refusal keeps it open with the reason.
    #[test]
    fn a_new_skill_is_sent_to_the_server() {
        let mut state = WorkspaceState::new(&[]).with_project("/work/app");
        state.read_library();
        state.take_outbox();
        assert!(state.draft_pressed(DRAFT_NEW, Some(Kind::Skill)));
        state.draft_edited(DRAFT_NAME, Edit::Paste("ship-it".to_owned()));
        state.draft_edited(DRAFT_DESCRIPTION, Edit::Paste("Ships it".to_owned()));
        assert!(state.draft_pressed(draft_scope(Scope::User).unwrap(), Some(Kind::Skill)));
        assert!(state.draft_pressed(DRAFT_CREATE, Some(Kind::Skill)));
        let sent = state.take_outbox();
        assert!(sent.iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::CreateCustomization { kind: Kind::Skill, scope: Scope::User, name, description, directory, .. }
                if name == "ship-it" && description == "Ships it" && directory == "/work/app"
        )));
        let state = state.with_customization_failed("already there".to_owned());
        let form = state.settings_page.library.draft.clone().unwrap();
        assert_eq!(form.error.as_deref(), Some("already there"));
        assert!(!form.busy);
        let mut state = state.with_customization_created(Some(0), "/work/app/x".to_owned());
        assert!(state.settings_page.library.draft.is_none());
        assert!(
            state
                .take_outbox()
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::Library { .. }))
        );
    }

    /// With no project open the pages still read the reader's own customizations on this machine, and a new one can only go in the reader's own folder.
    #[test]
    fn without_a_project_the_readers_own_are_read_and_written() {
        let mut state = WorkspaceState::new(&[]);
        state.read_library();
        let sent = state.take_outbox();
        assert!(sent.iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::Library { project: None, host: None, directory, .. } if directory.is_empty()
        )));
        assert!(state.draft_pressed(DRAFT_NEW, Some(Kind::Skill)));
        assert!(state.draft_pressed(draft_scope(Scope::Workspace).unwrap(), Some(Kind::Skill)));
        state.draft_edited(DRAFT_NAME, Edit::Paste("ship-it".to_owned()));
        assert!(state.draft_pressed(DRAFT_CREATE, Some(Kind::Skill)));
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::CreateCustomization { project: None, scope: Scope::User, directory, .. }
                if directory.is_empty()
        )));
    }

    /// Hooks get no form.
    /// The prompt file not written yet is made in the reader's own folder, and the one in force is opened rather than made again.
    #[test]
    fn prompt_files_are_made_or_opened() {
        let mut state = WorkspaceState::new(&[]);
        state.read_library();
        let (project, agent) = state.settings_page.library.of.unwrap();
        let written = Item {
            kind: Kind::Instruction,
            scope: Scope::User,
            name: "~/.config/oh-my-crab/APPEND_SYSTEM.md".to_owned(),
            path: "/home/me/.config/oh-my-crab/APPEND_SYSTEM.md".to_owned(),
            applies: vec![APPENDS_PROMPT.to_owned()],
            ..Item::default()
        };
        let mut state = state.with_library(project, agent, None, vec![written], Vec::new());
        state.take_outbox();
        assert!(state.prompt_pressed(PROMPT_REPLACE));
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::CreatePromptFile {
                file: PromptFile::Replace,
                scope: Scope::User,
                ..
            }
        )));
        assert!(state.prompt_pressed(PROMPT_APPEND));
        let sent = state.take_outbox();
        assert!(
            !sent
                .iter()
                .any(|outgoing| matches!(outgoing, Outgoing::CreatePromptFile { .. }))
        );
        assert!(sent.iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::OpenPath { path } if path.ends_with("APPEND_SYSTEM.md")
        )));
        assert!(!state.prompt_pressed(DRAFT_NEW));
    }

    #[test]
    fn hooks_are_not_drafted() {
        assert!(draftable(Kind::Skill));
        assert!(draftable(Kind::Instruction));
        assert!(draftable(Kind::Agent));
        assert!(!draftable(Kind::Hook));
    }
}
