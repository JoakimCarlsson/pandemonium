//! The form on the pages of skills, instruction files and agent definitions that writes a new one from a template, in the project's folder or the reader's own, and opens it to be filled in.

use crate::areas::SettingsEvent;
use crate::kit::{self, Frame};
use crate::target::Target;
use crate::theme;
use pm_protocol::library::{
    APPENDS_PROMPT, Item, Kind, PromptFile, REPLACES_PROMPT, Scope, valid_name,
};
use rustle::Modifier;
use rustle::TextFieldState;

use crate::theme::{Radius, Role};

use crate::button::Variant;
use crate::parts::line;
use crate::settings_machines::{labelled, labelled_choices};
use crate::settings_rows::DRAFT_SCOPES;
use crate::settings_servers::{boxed, flat_row, wrapped_note};
use crate::state::{Outgoing, WorkspaceState};

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
    pub name: TextFieldState,
    /// What it is for.
    pub description: TextFieldState,
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
            name: TextFieldState::default(),
            description: TextFieldState::default(),
            error: None,
            busy: false,
            in_project,
        }
    }

    /// The name as written, without spaces around it.
    fn name(&self) -> String {
        self.name.text().trim().to_owned()
    }

    /// Whether the form holds enough to be written.
    fn complete(&self) -> bool {
        valid_name(&self.name()) && !self.busy
    }
}

/// The choice that puts a new customization in `scope`, when the form offers it.
pub fn draft_scope(scope: Scope) -> Option<Target> {
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
pub(crate) fn is_draft_field(id: Target) -> bool {
    id == Target::Settings(SettingsEvent::DraftName)
        || id == Target::Settings(SettingsEvent::DraftDescription)
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
    pub(crate) fn draft_pressed(&mut self, action: Target, kind: Option<Kind>) -> bool {
        match action {
            Target::Settings(SettingsEvent::DraftNew) => {
                if let Some(kind) = kind.filter(|kind| draftable(*kind)) {
                    self.open_draft(kind);
                }
            }
            Target::Settings(SettingsEvent::DraftCancel) => self.settings_page.library.draft = None,
            Target::Settings(SettingsEvent::DraftCreate) => self.create_draft(),
            _ if let Some(at) = DRAFT_SCOPES.index_of(action) => {
                let form = self.settings_page.library.draft.as_mut();
                if let (Some(form), Some(scope)) =
                    (form.filter(|form| form.in_project), SCOPES.get(at))
                {
                    form.scope = *scope;
                }
            }
            _ => return false,
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

    /// Takes in what the reader wrote in a box of the form: the trouble shown before is let go.
    pub(crate) fn draft_typed(&mut self) {
        if let Some(form) = self.settings_page.library.draft.as_mut() {
            form.error = None;
        }
    }

    /// The box `id` of the form, if it is open and `id` names one.
    pub(crate) fn draft_field(&self, id: Target) -> Option<&TextFieldState> {
        let form = self.settings_page.library.draft.as_ref()?;
        match id {
            Target::Settings(SettingsEvent::DraftName) => Some(&form.name),
            Target::Settings(SettingsEvent::DraftDescription) => Some(&form.description),
            _ => None,
        }
    }
}

impl WorkspaceState {
    /// The state after `action`, which may name one of the buttons of crab's prompt files, was pressed; whether it named one. The file in force is opened; one not there yet is made empty in the reader's own folder, then opened.
    pub(crate) fn prompt_pressed(&mut self, action: Target) -> bool {
        let file = match action {
            Target::Settings(SettingsEvent::PromptReplace) => PromptFile::Replace,
            Target::Settings(SettingsEvent::PromptAppend) => PromptFile::Append,
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

/// Emits the box of crab's own prompt files on the page of instructions, `room` logical pixels across: the one replacing its built-in instructions and the one added to every prompt, each with the button that opens or makes it.
pub(crate) fn prompt_node(state: &WorkspaceState, room: f32) {
    let prompt_row = |file: PromptFile, title: &str, about: &str, id: Target| {
        let found = prompt_item(state, file);
        let said = match found {
            Some(_) => format!("{about} Written."),
            None => format!("{about} Not written yet."),
        };
        flat_row(3, || {
            kit::column(Modifier::new().weight(1.0), 1, || {
                line(title, 13, 1.4, Role::Text);
                wrapped_note(&said, (room - 160.0).max(120.0));
            });
            kit::small_button(
                if found.is_some() { "Open" } else { "Write" },
                id,
                Variant::Outline,
            )
            .disabled(state.settings_page.library.of.is_none())
            .build();
        });
    };
    kit::column(Modifier::new().fill_max_width(), 2, || {
        kit::row(Modifier::new().height(theme::sp(9)), 0, || {
            kit::text(
                Modifier::new(),
                "System prompt",
                kit::strong(15, Role::Text),
            );
        });
        boxed(|| {
            prompt_row(
                PromptFile::Replace,
                "Replace crab's instructions",
                "SYSTEM.md takes the place of crab's built-in instructions; its tools, skills and the project's details are still added.",
                Target::Settings(SettingsEvent::PromptReplace),
            );
            prompt_row(
                PromptFile::Append,
                "Add to every prompt",
                "APPEND_SYSTEM.md is added to the end of every prompt, subagents' too.",
                Target::Settings(SettingsEvent::PromptAppend),
            );
        });
    });
}

/// Emits the button that opens the form for a new customization of `kind`, turned off while the form is open or nothing can be written.
pub(crate) fn new_button(state: &WorkspaceState, kind: Kind) {
    let library = &state.settings_page.library;
    kit::small_button(
        new_label(kind),
        Target::Settings(SettingsEvent::DraftNew),
        Variant::Outline,
    )
    .disabled(library.draft.is_some() || library.of.is_none())
    .build();
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

/// Emits the form for a new customization `form`, `room` logical pixels across.
pub(crate) fn draft_node(form: &DraftForm, room: f32) {
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
    let message = match (&form.error, name.is_empty() || valid_name(&name)) {
        (Some(reason), _) => Some(reason.clone()),
        (None, false) => {
            Some("Use lowercase letters, digits and hyphens, up to 64 characters.".to_owned())
        }
        (None, true) => None,
    };
    kit::column(
        Modifier::new()
            .fill_max_width()
            .panel(Radius::Lg)
            .padding(theme::sp(4)),
        3,
        || {
            labelled(
                "Name",
                &form.name,
                Target::Settings(SettingsEvent::DraftName),
                name_hint,
            );
            labelled(
                description_label,
                &form.description,
                Target::Settings(SettingsEvent::DraftDescription),
                description_hint,
            );
            if form.in_project {
                labelled_choices(
                    "Where",
                    &["This project", "Every project"],
                    picked,
                    |place| DRAFT_SCOPES.id(place),
                );
            }
            wrapped_note(&place_note(form.kind, form.scope), room);
            if let Some(message) = message {
                line(message, 12, 1.4, Role::Danger);
            }
            kit::confirm_row(
                if form.busy { "Creating…" } else { "Create" },
                Target::Settings(SettingsEvent::DraftCreate),
                form.complete(),
                Target::Settings(SettingsEvent::DraftCancel),
            );
        },
    );
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
        form.name = TextFieldState::new("Not Valid");
        assert!(!form.complete());
        form.name = TextFieldState::new(" ship-it ");
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
        assert!(state.draft_pressed(Target::Settings(SettingsEvent::DraftNew), Some(Kind::Skill)));
        state = state
            .typed(Target::Settings(SettingsEvent::DraftName), "ship-it")
            .typed(
                Target::Settings(SettingsEvent::DraftDescription),
                "Ships it",
            );
        assert!(state.draft_pressed(draft_scope(Scope::User).unwrap(), Some(Kind::Skill)));
        assert!(state.draft_pressed(
            Target::Settings(SettingsEvent::DraftCreate),
            Some(Kind::Skill)
        ));
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
        assert!(state.draft_pressed(Target::Settings(SettingsEvent::DraftNew), Some(Kind::Skill)));
        assert!(state.draft_pressed(draft_scope(Scope::Workspace).unwrap(), Some(Kind::Skill)));
        state = state.typed(Target::Settings(SettingsEvent::DraftName), "ship-it");
        assert!(state.draft_pressed(
            Target::Settings(SettingsEvent::DraftCreate),
            Some(Kind::Skill)
        ));
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
        assert!(state.prompt_pressed(Target::Settings(SettingsEvent::PromptReplace)));
        assert!(state.take_outbox().iter().any(|outgoing| matches!(
            outgoing,
            Outgoing::CreatePromptFile {
                file: PromptFile::Replace,
                scope: Scope::User,
                ..
            }
        )));
        assert!(state.prompt_pressed(Target::Settings(SettingsEvent::PromptAppend)));
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
        assert!(!state.prompt_pressed(Target::Settings(SettingsEvent::DraftNew)));
    }

    #[test]
    fn hooks_are_not_drafted() {
        assert!(draftable(Kind::Skill));
        assert!(draftable(Kind::Instruction));
        assert!(draftable(Kind::Agent));
        assert!(!draftable(Kind::Hook));
    }
}
