//! The controls and areas of the window, each named by what it is: the typed target whose
//! rustle focus the app holds, finds a control by in tests and snapshots, and hands to its
//! reducers when the control is pressed.
//!
//! Every control is named once, in the enum of the area it belongs to in
//! [`areas`](crate::areas); a target is that control wrapped in its area, such as
//! `Target::Composer(ComposerEvent::Prompt)` or `Target::Sidebar(SidebarEvent::Session(2))`
//! carrying the session's index. A target's
//! [`test_tag`](rustle::Modifier::test_tag) is derived from it in one place,
//! [`Target::tag`], and read back with [`Target::from_tag`].

use serde::{Deserialize, Serialize};

use crate::areas::{
    BrowserEvent, ChatEvent, ComposerEvent, DebugEvent, DialogEvent, DiffEvent, EditorEvent,
    GitEvent, MenuEvent, SearchEvent, SettingsEvent, SidebarEvent, TerminalEvent, TreeEvent,
    WindowEvent,
};

/// One control or area of the window the app gives focus to, acts on, or finds in tests:
/// the control, named in the enum of its area, wrapped in that area.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Target {
    /// A control of the sidebar: its pages, projects and sessions.
    Sidebar(SidebarEvent),
    /// A control of the conversation: its messages, steps, questions and queued prompts.
    Chat(ChatEvent),
    /// A control of the prompt box and the chips and menus around it.
    Composer(ComposerEvent),
    /// A control of the review of the changes and the comments on them.
    Diff(DiffEvent),
    /// A control of the project's files.
    Tree(TreeEvent),
    /// A control of the file shown and what is laid over it.
    Editor(EditorEvent),
    /// A control of the project search, the excerpts and the conflict resolver.
    Search(SearchEvent),
    /// A control of source control: branches, commits, stashes, pull requests and the history.
    Git(GitEvent),
    /// A control of the dock: its shells and problems.
    Terminal(TerminalEvent),
    /// A control of the debug tab.
    Debug(DebugEvent),
    /// A control of the browser pane.
    Browser(BrowserEvent),
    /// A control of the settings window and the extensions page.
    Settings(SettingsEvent),
    /// A control of the menus opened over fields, tabs, messages, sessions and projects.
    Menu(MenuEvent),
    /// A control of the dialogs laid over the window.
    Dialog(DialogEvent),
    /// A control of the window's frame: its title bar, panes and the handles between panels.
    Window(WindowEvent),
}

/// The names of the areas a target is wrapped in, as its JSON writes them.
const AREAS: [&str; 15] = [
    "Sidebar", "Chat", "Composer", "Diff", "Tree", "Editor", "Search", "Git", "Terminal", "Debug",
    "Browser", "Settings", "Menu", "Dialog", "Window",
];

/// The prefix of every target's test tag, which keeps them apart from other tags.
const TAG_PREFIX: &str = "target:";

impl Target {
    /// The [`test_tag`](rustle::Modifier::test_tag) of the control this target names,
    /// which tests and snapshots find it by: the target written as JSON after a prefix.
    pub fn tag(self) -> String {
        format!(
            "{TAG_PREFIX}{}",
            serde_json::to_string(&self).unwrap_or_default()
        )
    }

    /// The target whose [`tag`](Self::tag) is `tag`, or nothing when `tag` is no target's.
    pub fn from_tag(tag: &str) -> Option<Self> {
        serde_json::from_str(tag.strip_prefix(TAG_PREFIX)?).ok()
    }

    /// The target `by` places further along the list this target is a row of, such as the
    /// link `by` links after it; a target that is no row of a list is itself.
    pub fn offset(self, by: usize) -> Self {
        match self {
            Self::Chat(ChatEvent::Link {
                message,
                piece,
                link,
            }) => Self::Chat(ChatEvent::Link {
                message,
                piece,
                link: link + by,
            }),
            Self::Editor(EditorEvent::HoverLink { prose, link }) => {
                Self::Editor(EditorEvent::HoverLink {
                    prose,
                    link: link + by,
                })
            }
            Self::Editor(EditorEvent::PreviewLink(link)) => {
                Self::Editor(EditorEvent::PreviewLink(link + by))
            }
            Self::Chat(ChatEvent::Details { message, place }) => Self::Chat(ChatEvent::Details {
                message,
                place: place + by,
            }),
            Self::Composer(ComposerEvent::Effort(stop)) => {
                Self::Composer(ComposerEvent::Effort(stop + by))
            }
            other => other,
        }
    }

    /// The target named `name`: a target written as JSON such as
    /// `{"Sidebar":{"Session":2}}`, or the control alone as its area's enum writes it, such
    /// as `Prompt` or `{"Session":2}`, as the snapshot command line takes one.
    pub fn from_name(name: &str) -> Option<Self> {
        serde_json::from_str(name).ok().or_else(|| {
            let control = serde_json::from_str::<serde_json::Value>(name)
                .unwrap_or_else(|_| serde_json::Value::String(name.to_owned()));
            AREAS.iter().find_map(|area| {
                let mut wrapped = serde_json::Map::new();
                wrapped.insert((*area).to_owned(), control.clone());
                serde_json::from_value(serde_json::Value::Object(wrapped)).ok()
            })
        })
    }
}

#[cfg(test)]
/// Tests for naming targets.
mod tests {
    use super::*;

    /// A target's tag reads back as the same target, and other tags read as none.
    #[test]
    fn a_tag_reads_back() {
        for target in [
            Target::Composer(ComposerEvent::Prompt),
            Target::Sidebar(SidebarEvent::Session(3)),
        ] {
            assert_eq!(Target::from_tag(&target.tag()), Some(target));
        }
        assert_eq!(Target::from_tag("something-else"), None);
    }

    /// A target is found by its control's name, by its control's JSON or by its own JSON.
    #[test]
    fn a_name_reads_as_a_target() {
        assert_eq!(
            Target::from_name("Prompt"),
            Some(Target::Composer(ComposerEvent::Prompt))
        );
        assert_eq!(
            Target::from_name("{\"Session\":2}"),
            Some(Target::Sidebar(SidebarEvent::Session(2)))
        );
        assert_eq!(
            Target::from_name("{\"Sidebar\":{\"Session\":2}}"),
            Some(Target::Sidebar(SidebarEvent::Session(2)))
        );
        assert_eq!(
            Target::from_name("Terminal"),
            Some(Target::Terminal(TerminalEvent::Terminal))
        );
        assert_eq!(Target::from_name("Nothing"), None);
    }
}
