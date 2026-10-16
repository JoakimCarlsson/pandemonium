//! The work behind writing code: grammars fetched from the server that holds a file and compiled away from the window, open files kept in step with their language servers, and the extensions installed on this machine.

use std::sync::Arc;

use pm_pages::{Outgoing, Screen};
use pm_protocol::{GrammarBundle, Request, Response};
use pm_state::State;
use pm_treesitter::{Editing, Grammar, Queries};

use super::{Host, Job, Pending, Work};

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
        },
        Editing {
            indent: bundle.indent,
            tabs: bundle.tabs,
            line_comment: bundle.line_comment.clone(),
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
        let ids = model.read(|screen| match screen {
            Screen::Workspace(state) => state
                .extensions
                .iter()
                .filter(|language| language.installed && !language.hidden)
                .map(|language| language.id.clone())
                .collect(),
            Screen::Onboarding(_) => Vec::new(),
        });
        if !ids.is_empty() {
            self.ask(host, Request::InstallExtensions { ids }, Pending::Nothing);
        }
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
            Outgoing::Completion {
                session,
                path,
                version,
                position,
                trigger,
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
            Outgoing::InstallExtension { path } => {
                self.ask(&None, Request::InstallExtension { path }, Pending::Nothing);
            }
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
                if pm_treesitter::registry::loaded(&bundle.id).is_some() {
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
                    Response::Opened { triggers } => Ok(triggers),
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
            (Pending::Extensions, Response::Extensions { extensions }) => {
                model.update(|screen| {
                    *screen = std::mem::take(screen).with_extensions(extensions);
                });
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
