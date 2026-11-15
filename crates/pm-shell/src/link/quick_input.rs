//! The link's part in the quick input: opens and closes it, asks the server
//! for symbols, outlines, lines and branches, fills its lists of choices,
//! and does what the row picked stands for.

use pm_protocol::{Request, Response};

use super::browse::char_column;
use super::{Ask, Link};
use crate::conversation::{Block, Speaker};
use crate::model::RightTab;
use crate::page::Page;
use crate::quick::{
    Choosing, Command, QuickAction, QuickInput, QuickItem, QuickKind, QuickMode, QuickOutcome,
    QuickPick, quick_items,
};
use crate::settings_catalog::Setting;
use crate::settings_change::SettingsChange;
use crate::settings_section::Section;
use crate::settings_values::Control;

impl Link {
    /// Does what the person asked of the quick input, answering with what the window must do beyond it.
    pub fn quick(&mut self, action: QuickAction) -> Option<QuickOutcome> {
        match action {
            QuickAction::Open(query) => {
                self.workspace.quick = Some(QuickInput {
                    query,
                    ..QuickInput::default()
                });
                if !self.workspace.tree.loaded {
                    self.load_files();
                }
                self.quick_ask();
            }
            QuickAction::Choose(choosing) => self.open_choices(choosing),
            QuickAction::Query(query) => {
                if let Some(quick) = &mut self.workspace.quick {
                    if quick.query == query {
                        return None;
                    }
                    quick.query = query;
                    quick.selected = 0;
                }
                self.quick_ask();
            }
            QuickAction::Move(by) => {
                let rows = quick_items(&self.workspace).len();
                if let Some(quick) = &mut self.workspace.quick
                    && rows > 0
                {
                    let place = quick.selected as isize + by;
                    quick.selected = place.rem_euclid(rows as isize) as usize;
                }
            }
            QuickAction::Pick(place) => {
                let items = quick_items(&self.workspace);
                let chosen = place.or(self.workspace.quick.as_ref().map(|quick| quick.selected));
                let item = chosen.and_then(|place| items.into_iter().nth(place))?;
                self.workspace.quick = None;
                return self.pick(item);
            }
            QuickAction::Close => self.workspace.quick = None,
        }
        None
    }

    /// Opens the quick input on the list of choices `choosing`.
    pub(super) fn open_choices(&mut self, choosing: Choosing) {
        let found = self.choices(choosing);
        self.workspace.quick = Some(QuickInput {
            choosing: Some(choosing),
            found,
            ..QuickInput::default()
        });
        if choosing == Choosing::Branch
            && let Some(directory) = self.browse_root()
        {
            self.ask(Request::Branches { directory }, Ask::Branches);
        }
        self.ask_listing(choosing);
    }

    /// The rows of the list of choices `choosing` that the link knows without asking.
    fn choices(&self, choosing: Choosing) -> Vec<QuickItem> {
        let choice = |place: usize, title: String, detail: String, current: bool| QuickItem {
            title,
            detail,
            kind: if current {
                QuickKind::Current
            } else {
                QuickKind::Choice
            },
            keys: None,
            matched: Vec::new(),
            pick: QuickPick::Choose(choosing, place),
        };
        let setting_choices = |setting: Setting| {
            let (options, current) = match setting.control(&self.settings) {
                Control::Picker(name) => {
                    let options = setting.options(&self.settings);
                    let current = options.iter().position(|option| *option == name);
                    (options, current)
                }
                Control::Choice(labels, picked) => (
                    labels.iter().map(|label| (*label).to_owned()).collect(),
                    Some(picked),
                ),
                _ => (setting.options(&self.settings), None),
            };
            options
                .into_iter()
                .enumerate()
                .map(|(place, title)| choice(place, title, String::new(), Some(place) == current))
                .collect()
        };
        match choosing {
            Choosing::Session
            | Choosing::Branch
            | Choosing::References
            | Choosing::Actions
            | Choosing::History => Vec::new(),
            Choosing::Problems => self.problem_choices(),
            Choosing::Theme => setting_choices(Setting::ThemeMode),
            Choosing::Keymap => setting_choices(Setting::BaseKeymap),
            Choosing::Setting => Section::ALL
                .iter()
                .enumerate()
                .map(|(place, section)| {
                    choice(
                        place,
                        section.title().to_owned(),
                        section.blurb().to_owned(),
                        false,
                    )
                })
                .collect(),
            Choosing::Rewind | Choosing::Fork | Choosing::Restore => self
                .workspace
                .conversation
                .iter()
                .filter(|message| message.speaker == Speaker::User)
                .enumerate()
                .map(|(turn, message)| {
                    let said = message
                        .blocks
                        .iter()
                        .find_map(|block| match block {
                            Block::Markdown { source, .. } => Some(source.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    choice(turn, said, format!("Turn {}", turn + 1), false)
                })
                .collect(),
        }
    }

    /// Asks the server what the quick input lists for its query, when what it lists comes from the server.
    fn quick_ask(&mut self) {
        let Some(quick) = &mut self.workspace.quick else {
            return;
        };
        if quick.choosing.is_some() {
            return;
        }
        let mode = quick.mode();
        let needle = quick.needle().to_owned();
        let query = quick.query.clone();
        if matches!(mode, QuickMode::Files | QuickMode::Commands) {
            return;
        }
        quick.found.clear();
        if needle.is_empty() {
            quick.waiting = None;
            return;
        }
        let Some(directory) = self.browse_root() else {
            return;
        };
        let file = self.symbol_file();
        let request = match mode {
            QuickMode::Text => Request::SearchFiles {
                directory,
                query: needle,
                regex: false,
                case: false,
            },
            _ => {
                let Some(path) = file else {
                    return;
                };
                Request::WorkspaceSymbols {
                    directory,
                    path,
                    query: needle,
                }
            }
        };
        if let Some(quick) = &mut self.workspace.quick {
            quick.waiting = Some(query.clone());
        }
        self.ask(request, Ask::Quick(query));
    }

    /// The file whose language server is asked for symbols: the one open, the one chosen in the tree, or the first listed.
    fn symbol_file(&self) -> Option<String> {
        self.workspace
            .open_files
            .shown()
            .map(|file| file.path.clone())
            .or_else(|| {
                self.workspace
                    .tree
                    .selected
                    .clone()
                    .filter(|path| !self.workspace.tree.is_folder(path))
            })
            .or_else(|| self.workspace.tree.paths.first().cloned())
    }

    /// Handles the `response` to a request the quick input made for `ask`.
    pub(super) fn answer_quick(&mut self, ask: Ask, response: Response) {
        let outline = self
            .workspace
            .open_files
            .shown()
            .map(|file| file.path.clone());
        let Some(quick) = &mut self.workspace.quick else {
            return;
        };
        match (ask, response) {
            (Ask::Quick(query), Response::Definitions { locations })
                if quick.waiting.as_deref() == Some(query.as_str()) =>
            {
                quick.waiting = None;
                let mode = quick.mode();
                quick.found = locations
                    .into_iter()
                    .filter(|location| {
                        mode != QuickMode::Outline || Some(&location.path) == outline.as_ref()
                    })
                    .map(|location| {
                        let line = location.span.start.line as usize + 1;
                        let text = location.line.clone().unwrap_or_default();
                        let (title, kind, matched) = match mode {
                            QuickMode::Text => {
                                let start = char_column(&text, location.span.start.character);
                                let lead = text
                                    .chars()
                                    .take_while(|letter| letter.is_whitespace())
                                    .count();
                                let length = query.chars().count().saturating_sub(1);
                                let matched = (start.saturating_sub(lead)
                                    ..start.saturating_sub(lead) + length)
                                    .collect();
                                (text.trim().to_owned(), QuickKind::Text, matched)
                            }
                            _ => (
                                location
                                    .name
                                    .clone()
                                    .unwrap_or_else(|| text.trim().to_owned()),
                                QuickKind::Symbol,
                                Vec::new(),
                            ),
                        };
                        QuickItem {
                            title,
                            detail: format!("{}:{line}", location.path),
                            kind,
                            keys: None,
                            matched,
                            pick: QuickPick::Open {
                                path: location.path,
                                line: Some(line),
                            },
                        }
                    })
                    .collect();
                quick.selected = 0;
            }
            (Ask::Quick(query), Response::Failed { message })
                if quick.waiting.as_deref() == Some(query.as_str()) =>
            {
                quick.waiting = None;
                self.workspace.status = Some(message);
            }
            (Ask::Branches, Response::Branches { names })
                if quick.choosing == Some(Choosing::Branch) =>
            {
                quick.found = names
                    .into_iter()
                    .enumerate()
                    .map(|(place, name)| QuickItem {
                        title: name,
                        detail: String::new(),
                        kind: if place == 0 {
                            QuickKind::Current
                        } else {
                            QuickKind::Choice
                        },
                        keys: None,
                        matched: Vec::new(),
                        pick: QuickPick::Choose(Choosing::Branch, place),
                    })
                    .collect();
            }
            _ => {}
        }
    }

    /// Does what the row `item` stands for.
    fn pick(&mut self, item: QuickItem) -> Option<QuickOutcome> {
        match item.pick {
            QuickPick::Open { path, line } => Some(QuickOutcome::OpenFile(path, line)),
            QuickPick::Session(at) => {
                self.open_session(at);
                None
            }
            QuickPick::Command(command) => self.run_command(command),
            QuickPick::Choose(choosing, place) => self.choose_quick(choosing, place, item.title),
            QuickPick::Version(version) => {
                self.restore_version(version);
                None
            }
        }
    }

    /// Runs `command`.
    fn run_command(&mut self, command: Command) -> Option<QuickOutcome> {
        let workspace = &mut self.workspace;
        match command {
            Command::ToggleSidebar => workspace.sidebar_open = !workspace.sidebar_open,
            Command::ToggleDock => workspace.dock_open = !workspace.dock_open,
            Command::ToggleRight => workspace.right_open = !workspace.right_open,
            Command::NewChat => self.new_chat(),
            Command::Settings => return Some(QuickOutcome::Settings),
            Command::Back => self.back(),
            Command::Forward => self.forward(),
            Command::History => self.navigate(Page::History),
            Command::Extensions => self.navigate(Page::Extensions),
            Command::Files | Command::Search | Command::Git => {
                workspace.right_open = true;
                self.choose_right_tab(match command {
                    Command::Files => RightTab::Files,
                    Command::Search => RightTab::Search,
                    _ => RightTab::Git,
                });
            }
            Command::SwitchSession => self.open_choices(Choosing::Session),
            Command::Theme => self.open_choices(Choosing::Theme),
            Command::Keymap => self.open_choices(Choosing::Keymap),
            Command::Setting => self.open_choices(Choosing::Setting),
            Command::Rewind => self.open_choices(Choosing::Rewind),
            Command::Branch => self.open_choices(Choosing::Branch),
            Command::Symbols => {
                let _ = self.quick(QuickAction::Open("#".to_owned()));
            }
            Command::Outline => {
                let _ = self.quick(QuickAction::Open("@".to_owned()));
            }
            Command::FindText => {
                let _ = self.quick(QuickAction::Open("%".to_owned()));
            }
            Command::References => self.open_choices(Choosing::References),
            Command::CodeActions => self.open_choices(Choosing::Actions),
            Command::LocalHistory => self.open_choices(Choosing::History),
            Command::Problems => self.open_choices(Choosing::Problems),
        }
        None
    }

    /// Picks the choice at `place` of the list `choosing`, which reads `title`.
    fn choose_quick(
        &mut self,
        choosing: Choosing,
        place: usize,
        title: String,
    ) -> Option<QuickOutcome> {
        match choosing {
            Choosing::Session => None,
            Choosing::Theme => {
                self.change_settings(SettingsChange::Press(Setting::ThemeMode, place));
                Some(QuickOutcome::SettingsChanged)
            }
            Choosing::References | Choosing::Problems => None,
            Choosing::Actions => {
                self.take_action(place);
                None
            }
            Choosing::History => None,
            Choosing::Keymap => {
                self.change_settings(SettingsChange::Pick(Setting::BaseKeymap, place));
                Some(QuickOutcome::SettingsChanged)
            }
            Choosing::Setting => {
                let section = *Section::ALL.get(place)?;
                self.change_settings(SettingsChange::Open(section));
                Some(QuickOutcome::Settings)
            }
            Choosing::Rewind => {
                let session = self.watched?;
                self.ask(
                    Request::Rewind {
                        session,
                        turn: place as u64,
                    },
                    Ask::Nothing,
                );
                None
            }
            Choosing::Fork => {
                let session = self.watched?;
                self.ask(
                    Request::ForkSession {
                        session,
                        turn: place as u64,
                    },
                    Ask::Fork,
                );
                None
            }
            Choosing::Restore => {
                let session = self.watched?;
                self.ask(
                    Request::Restore {
                        session,
                        turn: place as u64,
                    },
                    Ask::Nothing,
                );
                None
            }
            Choosing::Branch => {
                let directory = self.browse_root()?;
                self.workspace.git.branch = title.clone();
                self.ask(
                    Request::Checkout {
                        directory,
                        branch: title,
                    },
                    Ask::Nothing,
                );
                None
            }
        }
    }
}
