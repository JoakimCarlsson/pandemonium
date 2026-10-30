//! The work behind writing code: grammars fetched from the server that holds a file and compiled away from the window, open files kept in step with their language servers, and the extensions installed on this machine.

use std::sync::Arc;

use pm_pages::{Outgoing, Screen};
use pm_protocol::{GrammarBundle, Request, Response};
use pm_state::State;
use pm_treesitter::{Editing, Grammar, Queries};

use super::{Host, Job, Pending, Work};

/// The manifest of the language `bundle` was made from, as far as it says how the language is edited.
fn manifest_of(bundle: &GrammarBundle) -> pm_lsp::Manifest {
    pm_lsp::Manifest {
        indent: bundle.indent,
        tabs: bundle.tabs,
        line_comment: bundle.line_comment.clone(),
        pairs: bundle
            .pairs
            .clone()
            .unwrap_or_else(pm_treesitter::default_pairs),
        scopes: bundle.scopes.clone(),
        tags: bundle.tags.clone(),
        emmet_dialect: bundle.more.emmet_dialect.clone(),
        word_characters: bundle.more.word_characters.clone(),
        brackets: bundle.more.bracket_pairs.clone(),
        increase_indent_pattern: bundle.more.increase_indent_pattern.clone(),
        decrease_indent_pattern: bundle.more.decrease_indent_pattern.clone(),
        decrease_indent_patterns: bundle.more.decrease_indent_patterns.clone(),
        ..pm_lsp::Manifest::default()
    }
}

/// The grammar of `bundle`, compiled.
fn compile(bundle: &GrammarBundle) -> Result<Arc<Grammar>, String> {
    Grammar::load(
        &bundle.id,
        &bundle.symbol,
        &bundle.wasm,
        &Queries {
            highlights: bundle.highlights.clone(),
            indents: bundle.indents.clone(),
            outline: bundle.outline.clone(),
            injections: bundle.injections.clone(),
            brackets: bundle.brackets.clone(),
            runnables: bundle.runnables.clone(),
            overrides: bundle.more.overrides.clone(),
            redactions: bundle.more.redactions.clone(),
            textobjects: bundle.more.textobjects.clone(),
            debugger: bundle.more.debugger.clone(),
            dependencies: String::new(),
        },
        Editing {
            run_lenses: bundle
                .more
                .lenses
                .iter()
                .map(|lens| (lens.tags.clone(), lens.label.clone()))
                .collect(),
            ..pm_lsp::editing_of(&manifest_of(bundle))
        },
    )
}

impl Work {
    /// Every machine whose server is reached or being reached, this one always among them.
    fn hosts(&self) -> Vec<Host> {
        let mut hosts: Vec<Host> = self.links.keys().cloned().collect();
        if !hosts.contains(&None) {
            hosts.push(None);
        }
        hosts
    }

    /// Has the server of another machine, `host`, install the extensions installed on this one, so the files of its projects are read in them too.
    pub(super) fn share_extensions(&mut self, model: &State<Screen>, host: &Host) {
        if host.is_none() {
            return;
        }
        let ids: Vec<String> = model.read(|screen| match screen {
            Screen::Workspace(state) => state
                .extensions
                .iter()
                .filter(|language| {
                    language.installed && !language.hidden && language.kind != "extension"
                })
                .map(|language| language.id.clone())
                .collect(),
        });
        if !ids.is_empty() {
            self.ask(host, Request::InstallExtensions { ids }, Pending::Nothing);
        }
    }

    /// Asks the server of `host` what its extensions bring that the window uses itself.
    pub(super) fn ask_extension_assets(&mut self, host: &Host) {
        self.ask(
            host,
            Request::ExtensionAssets,
            Pending::ExtensionAssets(host.clone()),
        );
    }

    /// Asks the server of the session at `session` for the grammar of `language`, unless it was asked for it before.
    fn ask_grammar(&mut self, model: &State<Screen>, session: usize, language: String) {
        let Some(placement) = Self::session_of(model, session) else {
            return;
        };
        if !self
            .grammars
            .insert((placement.host.clone(), language.clone()))
        {
            return;
        }
        self.ask(
            &placement.host,
            Request::Grammar {
                language: language.clone(),
            },
            Pending::Grammar(language),
        );
    }

    /// Asks for the grammars of the languages stretches of the open files are written in that no grammar here answers to yet.
    pub(super) fn ask_wanted_grammars(&mut self, model: &State<Screen>) {
        let wanted = pm_treesitter::registry::take_wanted();
        if !wanted.is_empty() {
            model.update(|screen| *screen = std::mem::take(screen).grammars_wanted(wanted));
        }
    }

    /// Carries out what the window asked about writing code.
    pub(super) fn deliver_code(&mut self, model: &State<Screen>, outgoing: Outgoing) {
        match outgoing {
            Outgoing::Grammar { session, language } => self.ask_grammar(model, session, language),
            Outgoing::OpenDocument {
                session,
                path,
                language,
                version,
                text,
            } => {
                let opened = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::OpenDocument {
                        directory,
                        path,
                        language,
                        version,
                        text,
                    },
                    Pending::Opened(session, opened),
                );
            }
            Outgoing::ChangeDocument {
                session,
                path,
                version,
                changes,
            } => self.ask_in_folder(
                model,
                session,
                |directory| Request::ChangeDocument {
                    directory,
                    path,
                    version,
                    changes,
                },
                Pending::Nothing,
            ),
            Outgoing::CloseDocument { session, path } => self.ask_in_folder(
                model,
                session,
                |directory| Request::CloseDocument { directory, path },
                Pending::Nothing,
            ),
            Outgoing::ResolveCompletion {
                session,
                path,
                item,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::ResolveCompletion {
                        directory,
                        path,
                        item,
                    },
                    Pending::CompletionResolved(session, asked),
                );
            }
            Outgoing::Completion {
                session,
                path,
                version,
                position,
                trigger,
                scope,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Completion {
                        directory,
                        path,
                        version,
                        position,
                        trigger,
                        scope,
                    },
                    Pending::Completions(session, asked, version),
                );
            }
            Outgoing::Hover {
                session,
                path,
                version,
                position,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Hover {
                        directory,
                        path,
                        version,
                        position,
                    },
                    Pending::Hovered(session, asked, position),
                );
            }
            Outgoing::Format {
                session,
                path,
                version,
                save,
                range,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Format {
                        directory,
                        path,
                        version,
                        range,
                    },
                    Pending::Formatted(session, asked, version, save),
                );
            }
            Outgoing::FormatOnType {
                session,
                path,
                version,
                position,
                character,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::FormatOnType {
                        directory,
                        path,
                        version,
                        position,
                        character,
                    },
                    Pending::Formatted(session, asked, version, false),
                );
            }
            Outgoing::FoldingRanges {
                session,
                path,
                version,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::FoldingRanges {
                        directory,
                        path,
                        version,
                    },
                    Pending::FoldingRanges(session, asked, version),
                );
            }
            Outgoing::LinkedRanges {
                session,
                path,
                version,
                position,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::LinkedRanges {
                        directory,
                        path,
                        version,
                        position,
                    },
                    Pending::LinkedRanges(session, asked, version),
                );
            }
            Outgoing::Definition {
                session,
                path,
                version,
                position,
                seek,
            } => self.ask_in_folder(
                model,
                session,
                |directory| Request::Definition {
                    directory,
                    path,
                    version,
                    position,
                    seek,
                },
                Pending::Definitions(session, seek),
            ),
            Outgoing::Blame {
                session,
                path,
                line,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Blame {
                        directory,
                        path,
                        line,
                    },
                    Pending::Blame(session, asked, line),
                );
            }
            Outgoing::Problems { session } => self.ask_in_folder(
                model,
                session,
                |directory| Request::Problems { directory },
                Pending::Problems(session),
            ),
            Outgoing::SearchProject {
                session,
                query,
                case,
                regex,
            } => {
                let asked = query.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::SearchFiles {
                        directory,
                        query,
                        regex,
                        case,
                    },
                    Pending::ProjectSearch(session, asked, case, regex),
                );
            }
            Outgoing::SearchFiles { session, query } => {
                let asked = query.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::SearchFiles {
                        directory,
                        query,
                        regex: false,
                        case: false,
                    },
                    Pending::Symbols(session, asked),
                );
            }
            Outgoing::WorkspaceSymbols {
                session,
                path,
                query,
            } => {
                let asked = query.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::WorkspaceSymbols {
                        directory,
                        path,
                        query,
                    },
                    Pending::Symbols(session, asked),
                );
            }
            Outgoing::ReadBase { session, path } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::ReadFile {
                        directory,
                        path,
                        head: true,
                        revision: None,
                    },
                    Pending::Base(session, asked),
                );
            }
            Outgoing::Highlights {
                session,
                path,
                version,
                position,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Highlights {
                        directory,
                        path,
                        version,
                        position,
                    },
                    Pending::Highlights(session, asked, version),
                );
            }
            Outgoing::Signature {
                session,
                path,
                version,
                position,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Signature {
                        directory,
                        path,
                        version,
                        position,
                    },
                    Pending::Signature(session, asked, version),
                );
            }
            Outgoing::PrepareRename {
                session,
                path,
                version,
                position,
            } => self.ask_in_folder(
                model,
                session,
                |directory| Request::PrepareRename {
                    directory,
                    path,
                    version,
                    position,
                },
                Pending::RenameTarget(session),
            ),
            Outgoing::RenameSymbol {
                session,
                path,
                version,
                position,
                name,
            } => self.ask_in_folder(
                model,
                session,
                |directory| Request::Rename {
                    directory,
                    path,
                    version,
                    position,
                    name,
                },
                Pending::Renamed(session),
            ),
            Outgoing::CodeActions {
                session,
                path,
                version,
                span,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::CodeActions {
                        directory,
                        path,
                        version,
                        span,
                        only: None,
                    },
                    Pending::CodeActions(session, asked),
                );
            }
            Outgoing::ReadToCompare {
                session,
                active,
                path,
                saved,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::ReadFile {
                        directory,
                        path,
                        head: false,
                        revision: None,
                    },
                    Pending::Compared(session, active, asked, saved),
                );
            }
            Outgoing::Lightbulb {
                session,
                path,
                version,
                line,
                span,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::CodeActions {
                        directory,
                        path,
                        version,
                        span,
                        only: None,
                    },
                    Pending::Lightbulb(session, asked, version, line),
                );
            }
            Outgoing::SemanticTokens {
                session,
                path,
                version,
                range,
            } => {
                let asked = path.clone();
                let rules = model.read(|screen| match screen {
                    Screen::Workspace(state) => state.editor.semantic_token_rules.clone(),
                });
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::SemanticTokens {
                        directory,
                        path,
                        version,
                        rules,
                        range,
                    },
                    Pending::SemanticTokens(session, asked, version, range),
                );
            }
            Outgoing::ResolveLink {
                session,
                path,
                link,
            } => self.ask_in_folder(
                model,
                session,
                |directory| Request::ResolveLink {
                    directory,
                    path,
                    link,
                },
                Pending::LinkResolved(session),
            ),
            Outgoing::ColorPresentations {
                session,
                path,
                version,
                span,
                color,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::ColorPresentations {
                        directory,
                        path,
                        version,
                        span,
                        color,
                    },
                    Pending::ColorPresentations(session, asked, color),
                );
            }
            Outgoing::DocumentLinks {
                session,
                path,
                version,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::DocumentLinks {
                        directory,
                        path,
                        version,
                    },
                    Pending::DocumentLinks(session, asked, version),
                );
            }
            Outgoing::Colors {
                session,
                path,
                version,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::Colors {
                        directory,
                        path,
                        version,
                    },
                    Pending::Colors(session, asked, version),
                );
            }
            Outgoing::InlayHints {
                session,
                path,
                version,
                span,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::InlayHints {
                        directory,
                        path,
                        version,
                        span,
                    },
                    Pending::InlayHints(session, asked, version),
                );
            }
            Outgoing::InlineValues {
                session,
                path,
                version,
                span,
                frame,
                stopped,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::InlineValues {
                        directory,
                        path,
                        version,
                        span,
                        frame,
                        stopped,
                    },
                    Pending::InlineValues(session, asked, version),
                );
            }
            Outgoing::CodeLenses {
                session,
                path,
                version,
                dependencies,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::CodeLenses {
                        directory,
                        path,
                        version,
                        dependencies,
                    },
                    Pending::CodeLenses(session, asked, version),
                );
            }
            Outgoing::PressCodeLens {
                session,
                path,
                index,
            } => self.ask_in_folder(
                model,
                session,
                |directory| Request::PressCodeLens {
                    directory,
                    path,
                    index,
                },
                Pending::LensPressed(session),
            ),
            Outgoing::SourceActions {
                session,
                path,
                version,
                span,
                kinds,
                save,
                notify,
            } => {
                let step = pm_components::source_actions::SourceStep {
                    session,
                    path: path.clone(),
                    kinds: kinds.clone(),
                    save,
                    notify,
                };
                let only = kinds.first().cloned().map(|kind| vec![kind]);
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::CodeActions {
                        directory,
                        path,
                        version,
                        span,
                        only,
                    },
                    Pending::SourceOffered(step),
                );
            }
            Outgoing::TakeCodeAction {
                session,
                path,
                index,
            } => self.ask_in_folder(
                model,
                session,
                |directory| Request::TakeCodeAction {
                    directory,
                    path,
                    index,
                },
                Pending::TookAction(session),
            ),
            Outgoing::RevealFile { session, path } => {
                if let Some(placement) = Self::session_of(model, session)
                    && placement.host.is_none()
                {
                    let root = if placement.directory.is_empty() {
                        placement.repository
                    } else {
                        placement.directory
                    };
                    reveal(&std::path::Path::new(&root).join(path));
                }
            }
            Outgoing::Permalink {
                session,
                path,
                line,
                end_line,
                open,
            } => self.ask_in_folder(
                model,
                session,
                |directory| Request::Permalink {
                    directory,
                    path,
                    line,
                    end_line,
                },
                Pending::Permalink(open),
            ),
            Outgoing::FileHistory { session, path } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::FileHistory { directory, path },
                    Pending::FileHistory(session, asked),
                );
            }
            Outgoing::LocalHistory { session, path } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::LocalHistory { directory, path },
                    Pending::LocalHistory(session, asked),
                );
            }
            Outgoing::ReadVersion {
                session,
                path,
                version,
                when,
            } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::ReadVersion {
                        directory,
                        path,
                        version,
                    },
                    Pending::Version(session, asked, when),
                );
            }
            Outgoing::ReadConflict { session, path } => {
                let asked = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::ReadConflict { directory, path },
                    Pending::Conflict(session, asked),
                );
            }
            Outgoing::ResolveConflict {
                session,
                path,
                text,
            } => {
                let written = path.clone();
                self.ask_in_folder(
                    model,
                    session,
                    |directory| Request::ResolveConflict {
                        directory,
                        path,
                        text,
                    },
                    Pending::Write(session, written),
                );
            }
            Outgoing::RevisionChanges {
                session,
                from,
                to,
                label,
            } => self.ask_in_folder(
                model,
                session,
                |directory| Request::RevisionChanges {
                    directory,
                    from,
                    to,
                },
                Pending::Revisions(session, label),
            ),
            Outgoing::ReadRevision {
                session,
                path,
                revision,
                label,
            } => self.ask_in_folder(
                model,
                session,
                |directory| Request::ReadFile {
                    directory,
                    path,
                    head: false,
                    revision: Some(revision),
                },
                Pending::Revision(session, label),
            ),
            Outgoing::Extensions => self.ask(&None, Request::Extensions, Pending::Extensions),
            Outgoing::InstallExtensions { ids } => {
                for host in self.hosts() {
                    self.ask(
                        &host,
                        Request::InstallExtensions { ids: ids.clone() },
                        Pending::Nothing,
                    );
                }
            }
            Outgoing::InspectExtension { path } => {
                self.ask(
                    &None,
                    Request::InspectExtension { path: path.clone() },
                    Pending::ExtensionPreview(path),
                );
            }
            Outgoing::InstallExtension { path, grants } => {
                self.ask(
                    &None,
                    Request::InstallExtension { path, grants },
                    Pending::Nothing,
                );
            }
            Outgoing::RemovePackage { id } => {
                self.ask(&None, Request::RemoveExtension { id }, Pending::Nothing);
            }
            Outgoing::GrantExtension {
                id,
                capability,
                granted,
            } => {
                self.ask(
                    &None,
                    Request::GrantExtension {
                        id,
                        capability,
                        granted,
                    },
                    Pending::Nothing,
                );
            }
            Outgoing::ExtensionAssets { host } => self.ask_extension_assets(&host),
            Outgoing::RunExtensionCommand {
                host,
                extension,
                command,
                argument,
                directory,
                session,
            } => self.ask(
                &host,
                Request::RunExtensionCommand {
                    extension,
                    command,
                    argument,
                    directory,
                },
                Pending::ExtensionCommand(session),
            ),
            Outgoing::RemoveExtension { id } => {
                for host in self.hosts() {
                    self.ask(
                        &host,
                        Request::RemoveExtension { id: id.clone() },
                        Pending::Nothing,
                    );
                }
            }
            _ => {}
        }
    }

    /// Does what an answer about writing code calls for.
    pub(super) fn answered_code(
        &mut self,
        model: &State<Screen>,
        host: &Host,
        pending: Pending,
        response: Response,
    ) {
        match (pending, response) {
            (Pending::Grammar(_), Response::Grammar(bundle)) => {
                if pm_treesitter::registry::called(&bundle.id).is_some() {
                    let id = bundle.id.clone();
                    model.update(|screen| *screen = std::mem::take(screen).with_grammar(&id));
                    return;
                }
                self.spawn(move |reporter| {
                    let outcome = compile(&bundle).map(|grammar| {
                        let mut aliases = bundle.aliases.clone();
                        aliases.push(bundle.symbol.clone());
                        pm_treesitter::registry::add(grammar, &aliases);
                    });
                    let _ = reporter.send(Job::Grammar(bundle.id.clone(), outcome));
                });
            }
            (Pending::Grammar(language), Response::Failed { message }) => {
                if host.is_none() {
                    eprintln!("no grammar for {language}: {message}");
                }
            }
            (Pending::Opened(session, path), response) => {
                let outcome = match response {
                    Response::Opened {
                        triggers,
                        signature,
                        format,
                        formats_ranges,
                        folds,
                        links,
                        semantic_tokens,
                        inlay_hints,
                        code_lens,
                        inline_values,
                        document_links,
                        colors,
                        semantic_ranges,
                    } => Ok(pm_protocol::Triggers {
                        completion: triggers,
                        signature,
                        format,
                        formats_ranges,
                        folds,
                        links,
                        semantic_tokens,
                        inlay_hints,
                        code_lens,
                        inline_values,
                        document_links,
                        colors,
                        semantic_ranges,
                    }),
                    other => Err(Self::trouble_of(other)),
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_opened(session, &path, outcome);
                });
            }
            (Pending::Completions(session, path, version), Response::Completions { items }) => {
                model.update(|screen| {
                    *screen =
                        std::mem::take(screen).with_completions(session, &path, version, items);
                });
            }
            (Pending::CompletionResolved(session, path), Response::CompletionResolved { item }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_completion_resolved(session, &path, item);
                });
            }
            (Pending::CompletionResolved(..), _) => {}
            (Pending::Completions(session, path, version), _) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_completions(
                        session,
                        &path,
                        version,
                        Vec::new(),
                    );
                });
            }
            (Pending::Hovered(session, path, position), Response::Hovered { hover }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_hover(session, &path, position, hover);
                });
            }
            (Pending::Hovered(session, path, position), _) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_hover(session, &path, position, None);
                });
            }
            (Pending::Formatted(session, path, version, save), response) => {
                let edits = match response {
                    Response::Formatted { edits } => edits,
                    _ => Vec::new(),
                };
                model.update(|screen| {
                    *screen =
                        std::mem::take(screen).with_formatted(session, &path, version, edits, save);
                });
            }
            (
                Pending::FoldingRanges(session, path, version),
                Response::FoldingRanges { ranges },
            ) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_folds(session, &path, version, ranges);
                });
            }
            (Pending::LinkedRanges(session, path, version), response) => {
                let spans = match response {
                    Response::LinkedRanges { spans } => spans,
                    _ => Vec::new(),
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_links(session, &path, version, spans);
                });
            }
            (Pending::FoldingRanges(..), _) => {}
            (Pending::Definitions(session, seek), Response::Definitions { locations }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_places(session, seek, locations);
                });
            }
            (
                Pending::Renamed(session) | Pending::TookAction(session),
                Response::Edited { files },
            ) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_edited_files(session, files);
                });
            }
            (Pending::TookAction(session), Response::ActionTaken { files, shown }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_action_shown(session, files, shown);
                });
            }
            (Pending::RenameTarget(session), Response::RenameTarget { target }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_rename_target(session, target);
                });
            }
            (Pending::RenameTarget(_), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| *screen = std::mem::take(screen).rename_unprepared(problem));
            }
            (Pending::Renamed(_), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| *screen = std::mem::take(screen).rename_failed(problem));
            }
            (Pending::TookAction(session), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| {
                    *screen = std::mem::take(screen).noted(session, &problem);
                });
            }
            (Pending::SourceOffered(step), Response::CodeActions { actions }) => {
                match pm_components::source_actions::source_action_to_take(&actions) {
                    Some(index) => {
                        let (session, path) = (step.session, step.path.clone());
                        self.ask_in_folder(
                            model,
                            session,
                            |directory| Request::TakeCodeAction {
                                directory,
                                path,
                                index,
                            },
                            Pending::SourceTaken(step),
                        );
                    }
                    None => model.update(|screen| {
                        *screen = std::mem::take(screen).with_source_done(
                            step,
                            pm_components::source_actions::SourceOutcome::Nothing,
                        );
                    }),
                }
            }
            (Pending::SourceOffered(step), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_source_done(
                        step,
                        pm_components::source_actions::SourceOutcome::Failed(problem),
                    );
                });
            }
            (Pending::SourceTaken(step), Response::Edited { files }) => {
                let outcome = if files.is_empty() {
                    pm_components::source_actions::SourceOutcome::Nothing
                } else {
                    pm_components::source_actions::SourceOutcome::Applied
                };
                model.update(|screen| {
                    let edited = std::mem::take(screen).with_source_edited(step.session, files);
                    *screen = edited.with_source_done(step, outcome);
                });
            }
            (Pending::SourceTaken(step), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_source_done(
                        step,
                        pm_components::source_actions::SourceOutcome::Failed(problem),
                    );
                });
            }
            (Pending::CodeActions(session, path), Response::CodeActions { actions }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_code_actions(session, &path, actions);
                });
            }
            (
                Pending::Compared(session, active, path, saved),
                Response::File {
                    text: Some(text), ..
                },
            ) => {
                model.update(|screen| {
                    *screen =
                        std::mem::take(screen).with_compared(session, &active, &path, saved, &text);
                });
            }
            (Pending::Compared(session, _, path, _), other) => {
                let problem = format!("{path}: {}", Self::trouble_of(other));
                model.update(|screen| *screen = std::mem::take(screen).noted(session, &problem));
            }
            (Pending::Lightbulb(session, path, version, line), response) => {
                let offered = match response {
                    Response::CodeActions { actions } => actions
                        .iter()
                        .filter(|action| action.disabled.is_none())
                        .count(),
                    _ => 0,
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen)
                        .with_lightbulb(session, &path, version, line, offered);
                });
            }
            (Pending::CodeActions(..), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| *screen = std::mem::take(screen).listing_failed(problem));
            }
            (Pending::Blame(session, path, line), Response::Blamed { blame }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_blame(session, &path, line, blame);
                });
            }
            (Pending::Problems(session), response) => {
                let problems = match response {
                    Response::Problems { problems } => problems,
                    _ => Vec::new(),
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_problems(session, problems)
                });
            }
            (Pending::ProjectSearch(session, query, case, regex), response) => {
                let hits = match response {
                    Response::Definitions { locations } => Ok(locations),
                    other => Err(Self::trouble_of(other)),
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen)
                        .with_project_search(session, &query, case, regex, hits);
                });
            }
            (Pending::Symbols(session, query), response) => {
                let locations = match response {
                    Response::Definitions { locations } => locations,
                    _ => Vec::new(),
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_symbols(session, &query, locations);
                });
            }
            (Pending::Base(session, path), response) => {
                let base = match response {
                    Response::File { text, .. } => text,
                    _ => None,
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_base(session, &path, base)
                });
            }
            (Pending::Highlights(session, path, version), response) => {
                let spans = match response {
                    Response::Highlights { spans } => spans,
                    _ => Vec::new(),
                };
                model.update(|screen| {
                    *screen =
                        std::mem::take(screen).with_highlights(session, &path, version, spans);
                });
            }
            (Pending::SemanticTokens(session, path, version, range), response) => {
                let (tokens, partial) = match response {
                    Response::SemanticTokens { tokens, partial } => (tokens, partial),
                    _ => (Vec::new(), false),
                };
                model.update(|screen| {
                    let taken = std::mem::take(screen);
                    *screen = match range.filter(|_| partial) {
                        Some(range) => {
                            taken.with_semantic_range(session, &path, version, range, tokens)
                        }
                        None => taken.with_semantic_tokens(session, &path, version, tokens),
                    };
                });
            }
            (Pending::LinkResolved(session), Response::LinkResolved { target }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_link_resolved(session, target);
                });
            }
            (Pending::LinkResolved(session), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| {
                    *screen = std::mem::take(screen).noted(session, &problem);
                });
            }
            (
                Pending::ColorPresentations(session, path, color),
                Response::ColorPresentations { presentations },
            ) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_color_presentations(
                        session,
                        &path,
                        color,
                        presentations,
                    );
                });
            }
            (Pending::ColorPresentations(..), other) => {
                let problem = Self::trouble_of(other);
                model
                    .update(|screen| *screen = std::mem::take(screen).color_picker_failed(problem));
            }
            (Pending::DocumentLinks(session, path, version), response) => {
                let links = match response {
                    Response::DocumentLinks { links } => links,
                    _ => Vec::new(),
                };
                model.update(|screen| {
                    *screen =
                        std::mem::take(screen).with_document_links(session, &path, version, links);
                });
            }
            (Pending::Colors(session, path, version), response) => {
                let colors = match response {
                    Response::Colors { colors } => colors,
                    _ => Vec::new(),
                };
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_colors(session, &path, version, colors);
                });
            }
            (Pending::InlayHints(session, path, version), response) => {
                let hints = match response {
                    Response::InlayHints { hints } => hints,
                    _ => Vec::new(),
                };
                model.update(|screen| {
                    *screen =
                        std::mem::take(screen).with_inlay_hints(session, &path, version, hints);
                });
            }
            (Pending::InlineValues(session, path, version), response) => {
                let values = match response {
                    Response::InlineValues { values } => values,
                    _ => Vec::new(),
                };
                model.update(|screen| {
                    *screen =
                        std::mem::take(screen).with_inline_values(session, &path, version, values);
                });
            }
            (Pending::CodeLenses(session, path, version), response) => {
                let lenses = match response {
                    Response::CodeLenses { lenses } => lenses,
                    _ => Vec::new(),
                };
                model.update(|screen| {
                    *screen =
                        std::mem::take(screen).with_code_lenses(session, &path, version, lenses);
                });
            }
            (Pending::LensPressed(session), Response::LensPressed { places, files }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_lens_pressed(session, places, files);
                });
            }
            (Pending::LensPressed(session), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| {
                    let taken = std::mem::take(screen);
                    *screen = taken
                        .listing_failed(problem.clone())
                        .noted(session, &problem);
                });
            }
            (Pending::Signature(session, path, version), response) => {
                let signature = match response {
                    Response::Signature { signature } => signature,
                    _ => None,
                };
                model.update(|screen| {
                    *screen =
                        std::mem::take(screen).with_signature(session, &path, version, signature);
                });
            }
            (Pending::Permalink(open), Response::Address { url }) => {
                model.update(|screen| {
                    let taken = std::mem::take(screen);
                    *screen = if open {
                        taken.with_address_opened(url)
                    } else {
                        taken.with_copied(url)
                    };
                });
            }
            (Pending::FileHistory(session, path), Response::FileHistory { commits }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_file_history(session, &path, commits);
                });
            }
            (Pending::Definitions(..) | Pending::FileHistory(..), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| *screen = std::mem::take(screen).listing_failed(problem));
            }
            (
                Pending::Revision(session, label),
                Response::File {
                    text: Some(text),
                    language,
                    ..
                },
            ) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_revision(session, label, text, language);
                });
            }
            (Pending::Revision(session, label), other) => {
                let problem = format!("{label}: {}", Self::trouble_of(other));
                model.update(|screen| *screen = std::mem::take(screen).noted(session, &problem));
            }
            (Pending::LocalHistory(session, path), Response::LocalHistory { versions }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_local_history(session, &path, versions);
                });
            }
            (Pending::LocalHistory(..), _) => {}
            (
                Pending::Version(session, path, when),
                Response::File {
                    text: Some(text), ..
                },
            ) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).timeline_opened(session, &path, when, text);
                });
            }
            (Pending::Version(session, path, _), other) => {
                let problem = format!("{path}: {}", Self::trouble_of(other));
                model.update(|screen| *screen = std::mem::take(screen).noted(session, &problem));
            }
            (Pending::Conflict(session, path), Response::Conflict { files }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_conflict(session, &path, files);
                });
            }
            (Pending::Conflict(session, path), other) => {
                let problem = format!("{path}: {}", Self::trouble_of(other));
                model.update(|screen| *screen = std::mem::take(screen).noted(session, &problem));
            }
            (Pending::Revisions(session, label), Response::RevisionChanges { changes }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).revision_opened(session, label, changes);
                });
            }
            (Pending::Revisions(session, label), other) => {
                let problem = format!("{label}: {}", Self::trouble_of(other));
                model.update(|screen| *screen = std::mem::take(screen).noted(session, &problem));
            }
            (Pending::Permalink(_), other) => {
                eprintln!("no permalink: {}", Self::trouble_of(other));
            }
            (Pending::ExtensionPreview(path), Response::ExtensionPreview { extension }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_extension_preview(path, extension);
                });
            }
            (Pending::ExtensionPreview(_), other) => {
                let problem = Self::trouble_of(other);
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_extension_problem(problem)
                });
            }
            (Pending::ExtensionAssets(host), Response::ExtensionAssets(assets)) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_extension_assets(&host, assets);
                });
            }
            (
                Pending::ExtensionCommand(session),
                Response::ExtensionCommand { prompt, message },
            ) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_command_result(prompt);
                    if let (Some(session), Some(message)) = (session, &message) {
                        *screen = std::mem::take(screen).noted(session, message);
                    }
                });
            }
            (Pending::ExtensionCommand(session), other) => {
                let problem = Self::trouble_of(other);
                match session {
                    Some(session) => model.update(|screen| {
                        *screen = std::mem::take(screen).noted(session, &problem);
                    }),
                    None => eprintln!("extension command failed: {problem}"),
                }
            }
            (Pending::Extensions, Response::Extensions { extensions }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_extensions(extensions);
                });
                self.ask_extension_assets(&None);
                for remote in self.hosts().into_iter().filter(Option::is_some) {
                    self.share_extensions(model, &remote);
                }
            }
            _ => {}
        }
    }

    /// Takes in the grammar of the language `language` compiled, or why it could not be.
    pub(super) fn grammar_compiled(
        &mut self,
        model: &State<Screen>,
        language: &str,
        outcome: Result<(), String>,
    ) {
        match outcome {
            Ok(()) => {
                model.update(|screen| *screen = std::mem::take(screen).with_grammar(language))
            }
            Err(reason) => eprintln!("the grammar of {language} could not be loaded: {reason}"),
        }
    }
}

/// Shows `file` in the file manager of this machine, picked out among the files beside it where the file manager can do that, and its folder opened otherwise.
fn reveal(file: &std::path::Path) {
    let quiet = |command: &mut std::process::Command| {
        command
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    };
    let shown = if cfg!(target_os = "macos") {
        quiet(std::process::Command::new("open").arg("-R").arg(file))
    } else if cfg!(target_os = "windows") {
        quiet(std::process::Command::new("explorer").arg(format!("/select,{}", file.display())))
    } else {
        let address = format!("file://{}", file.display());
        quiet(std::process::Command::new("dbus-send").args([
            "--session",
            "--print-reply",
            "--dest=org.freedesktop.FileManager1",
            "--type=method_call",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1.ShowItems",
            &format!("array:string:{address}"),
            "string:",
        ]))
    };
    if !shown && let Some(folder) = file.parent() {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "explorer"
        } else {
            "xdg-open"
        };
        let _ = std::process::Command::new(opener)
            .arg(folder)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}
