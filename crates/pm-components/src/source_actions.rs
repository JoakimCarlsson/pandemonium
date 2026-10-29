//! Changes of a whole file that a language server offers by kind: organizing its imports and applying its safe fixes, by hand and as the first part of saving.

use pm_protocol::CodeAction;

use crate::listing::{Items, Listing};
use crate::state::{Outgoing, WorkspaceState};

/// The kind of change that sorts and prunes the imports of a file.
pub const ORGANIZE_IMPORTS: &str = "source.organizeImports";

/// The kind of change that applies every fix the server marks safe to apply at once.
pub const FIX_ALL: &str = "source.fixAll";

/// Where one kind of change to a whole file stands in a run of them: the file, the kinds still to take, the first being the one asked for now, and what comes after the last.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceStep {
    /// The session, by position.
    pub session: usize,
    /// The file.
    pub path: String,
    /// The kinds to take, one after another, the first being the one asked for now.
    pub kinds: Vec<String>,
    /// Whether the file is formatted and written after the last of them.
    pub save: bool,
    /// Whether the reader is told when the server offers nothing.
    pub notify: bool,
}

/// What came of asking the language server for one kind of change to a file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceOutcome {
    /// The server made changes to files.
    Applied,
    /// The server offered nothing to change.
    Nothing,
    /// The server could not say, for this reason.
    Failed(String),
}

/// The place among `offered` of the change to take: the one the server prefers, or the first that can be taken; nothing when none can.
pub fn source_action_to_take(offered: &[CodeAction]) -> Option<usize> {
    let usable = |action: &CodeAction| action.disabled.is_none();
    offered
        .iter()
        .position(|action| usable(action) && action.preferred)
        .or_else(|| offered.iter().position(usable))
}

/// What the reader is told when the server offers nothing of `kind`.
fn nothing_offered(kind: &str) -> &'static str {
    if kind == FIX_ALL {
        "The language server offers no fixes to apply"
    } else {
        "The language server offers no import changes"
    }
}

impl WorkspaceState {
    /// Asks the language server to organize the imports of the file shown, a list hanging at `at` telling the reader when it offers nothing.
    pub fn organize_imports_requested(self, at: (f32, f32)) -> Self {
        self.source_requested(ORGANIZE_IMPORTS, at)
    }

    /// Asks the language server to apply the fixes it marks safe to the file shown, a list hanging at `at` telling the reader when it offers nothing.
    pub fn fix_all_requested(self, at: (f32, f32)) -> Self {
        self.source_requested(FIX_ALL, at)
    }

    /// Asks the language server for the change of `kind` to the file shown, a list hanging at `at` while it answers.
    fn source_requested(mut self, kind: &str, at: (f32, f32)) -> Self {
        let Some((session, path)) = self.coding_file() else {
            return self;
        };
        let Some(document) = self
            .shown_document()
            .filter(|document| !document.field.is_read_only())
        else {
            return self;
        };
        let (version, span) = (document.version, document.whole_span());
        self.outbox.push(Outgoing::SourceActions {
            session,
            path: path.clone(),
            version,
            span,
            kinds: vec![kind.to_owned()],
            save: false,
            notify: true,
        });
        self.listing = Some(Listing::waiting(
            session,
            Items::Actions {
                path,
                offered: Vec::new(),
            },
            at,
        ));
        self
    }

    /// Starts the changes the settings ask for before the file shown is written, the fixes first and the imports after them, and says whether it did; the file is formatted and written when the last of them is done.
    pub(crate) fn sources_before_save(&mut self) -> bool {
        let Some(language) = self
            .shown_document()
            .filter(|document| document.synced && !document.field.is_read_only())
            .map(|document| document.language.clone())
        else {
            return false;
        };
        let mut kinds = Vec::new();
        if self.fixes_on_save(language.as_deref()) {
            kinds.push(FIX_ALL.to_owned());
        }
        if self.organizes_on_save(language.as_deref()) {
            kinds.push(ORGANIZE_IMPORTS.to_owned());
        }
        let (Some(session), Some(path), Some((version, span))) = (
            self.review_index(),
            self.shown_viewer().map(|viewer| viewer.path.clone()),
            self.shown_document()
                .map(|document| (document.version, document.whole_span())),
        ) else {
            return false;
        };
        if kinds.is_empty() {
            return false;
        }
        self.outbox.push(Outgoing::SourceActions {
            session,
            path,
            version,
            span,
            kinds,
            save: true,
            notify: false,
        });
        true
    }

    /// The state after one kind of change, the first of the `step`, came to `outcome`: the reader is told when it was asked for by hand and nothing was offered, and the kinds after it are asked for next, or the file is formatted and written when the step is part of saving and none are left.
    pub fn with_source_done(mut self, step: SourceStep, outcome: SourceOutcome) -> Self {
        let SourceStep {
            session,
            path,
            mut kinds,
            save,
            notify,
        } = step;
        let kind = if kinds.is_empty() {
            String::new()
        } else {
            kinds.remove(0)
        };
        if notify {
            match outcome {
                SourceOutcome::Applied => self.listing = None,
                SourceOutcome::Nothing => {
                    self = self.listing_failed(nothing_offered(&kind).to_owned());
                }
                SourceOutcome::Failed(problem) => self = self.listing_failed(problem),
            }
        }
        let current = self.review_index() == Some(session)
            && self
                .shown_viewer()
                .is_some_and(|viewer| viewer.path == path && !viewer.head);
        if !current {
            return self;
        }
        if !kinds.is_empty() {
            if let Some((version, span)) = self
                .shown_document()
                .map(|document| (document.version, document.whole_span()))
            {
                self.outbox.push(Outgoing::SourceActions {
                    session,
                    path,
                    version,
                    span,
                    kinds,
                    save,
                    notify,
                });
            }
        } else if save {
            self.format_and_save(true, None);
        }
        self
    }
}
