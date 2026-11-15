//! The sections of the settings window: what each is called, which one it
//! sits inside, and what it says about itself under its heading.

/// A group of settings, named down the left side of the window; some sit inside another.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Section {
    /// How the window opens, closes and keeps itself up to date.
    #[default]
    General,
    /// How the window looks: its theme, its fonts and how closely it sets things.
    Appearance,
    /// How files are written in, with the groups below it.
    Editor,
    /// How files look.
    EditorAppearance,
    /// How files are edited.
    EditorEditing,
    /// What saving does.
    EditorSaving,
    /// What the language server adds.
    EditorIntelligence,
    /// Finding and scrolling.
    EditorSearch,
    /// Editing with vim's keys.
    EditorVim,
    /// How each language is edited, and the languages installed and offered.
    Languages,
    /// The keys that work the editor.
    Keymap,
    /// The shell the terminal starts and how it is drawn.
    Terminal,
    /// The embedded browser: turning it on, opening dev servers in it and what agents may do with it.
    Browser,
    /// How git is run and what it marks.
    Git,
    /// How sessions get their branches and worktrees.
    Worktrees,
    /// What agents are started with, with the groups below it.
    Agents,
    /// The agents that can be started, and the ones added by hand.
    AgentServers,
    /// The tool servers every agent is started with.
    McpServers,
    /// The OpenAI-compatible model servers the reader declared, each offered as an agent.
    ModelServers,
    /// The model slots: which model each kind of work asks, with backups, and the presets that set them all.
    ModelSlots,
    /// How many of each kind of customization agents are given in the project.
    Customizations,
    /// The skills agents are given.
    Skills,
    /// The instruction files agents read.
    Instructions,
    /// The agent definitions agents can hand work to.
    AgentDefinitions,
    /// The commands agents run when something happens.
    Hooks,
    /// The tools agents are given, in sets by where they come from.
    Tools,
    /// The servers the window knows and where new projects go.
    Servers,
    /// The settings a search found, on whatever page they are.
    Results,
}

impl Section {
    /// Every section, in the order the list shows them, each followed by the ones inside it.
    pub const ALL: [Section; 27] = [
        Section::General,
        Section::Appearance,
        Section::Editor,
        Section::EditorAppearance,
        Section::EditorEditing,
        Section::EditorSaving,
        Section::EditorIntelligence,
        Section::EditorSearch,
        Section::EditorVim,
        Section::Languages,
        Section::Keymap,
        Section::Terminal,
        Section::Browser,
        Section::Git,
        Section::Worktrees,
        Section::Agents,
        Section::AgentServers,
        Section::McpServers,
        Section::ModelServers,
        Section::ModelSlots,
        Section::Customizations,
        Section::Skills,
        Section::Instructions,
        Section::AgentDefinitions,
        Section::Hooks,
        Section::Tools,
        Section::Servers,
    ];

    /// The pages the Editor overview shows one under another.
    pub const EDITOR_PAGES: [Section; 6] = [
        Section::EditorAppearance,
        Section::EditorEditing,
        Section::EditorSaving,
        Section::EditorIntelligence,
        Section::EditorSearch,
        Section::EditorVim,
    ];

    /// What the list and the heading call it.
    pub fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Languages => "Languages",
            Self::Terminal => "Terminal",
            Self::Browser => "Browser",
            Self::Git => "Git",
            Self::Worktrees => "Worktrees",
            Self::Servers => "Servers",
            Self::Results => "Search Results",
            Self::Editor => "Editor",
            Self::EditorAppearance => "Appearance",
            Self::EditorEditing => "Editing",
            Self::EditorSaving => "Saving",
            Self::EditorIntelligence => "Code Intelligence",
            Self::EditorSearch => "Search & Scrolling",
            Self::EditorVim => "Vim",
            Self::Keymap => "Keymap",
            Self::Agents => "Agents",
            Self::AgentServers => "Agents and accounts",
            Self::McpServers => "MCP Servers",
            Self::ModelServers => "Model Servers",
            Self::ModelSlots => "Models",
            Self::Customizations => "Overview",
            Self::Skills => "Skills",
            Self::Instructions => "Instructions",
            Self::AgentDefinitions => "Agents",
            Self::Hooks => "Hooks",
            Self::Tools => "Tools",
        }
    }

    /// The name used for it on the command line and in snapshot names, such as `editor-vim`.
    pub fn slug(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Editor => "editor",
            Self::EditorAppearance => "editor-appearance",
            Self::EditorEditing => "editor-editing",
            Self::EditorSaving => "editor-saving",
            Self::EditorIntelligence => "editor-intelligence",
            Self::EditorSearch => "editor-search",
            Self::EditorVim => "editor-vim",
            Self::Languages => "languages",
            Self::Keymap => "keymap",
            Self::Terminal => "terminal",
            Self::Browser => "browser",
            Self::Git => "git",
            Self::Worktrees => "worktrees",
            Self::Agents => "agents",
            Self::AgentServers => "agent-servers",
            Self::McpServers => "mcp-servers",
            Self::ModelServers => "model-servers",
            Self::ModelSlots => "models",
            Self::Customizations => "customizations",
            Self::Skills => "skills",
            Self::Instructions => "instructions",
            Self::AgentDefinitions => "agent-definitions",
            Self::Hooks => "hooks",
            Self::Tools => "tools",
            Self::Servers => "servers",
            Self::Results => "results",
        }
    }

    /// The section whose [`Section::slug`] is `slug`.
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .chain([Self::Results])
            .find(|section| section.slug() == slug)
    }

    /// The section it sits inside, if it sits inside one.
    pub fn parent(self) -> Option<Section> {
        match self {
            Self::AgentServers
            | Self::McpServers
            | Self::ModelServers
            | Self::ModelSlots
            | Self::Customizations
            | Self::Skills
            | Self::Instructions
            | Self::AgentDefinitions
            | Self::Hooks
            | Self::Tools => Some(Self::Agents),
            Self::EditorAppearance
            | Self::EditorEditing
            | Self::EditorSaving
            | Self::EditorIntelligence
            | Self::EditorSearch
            | Self::EditorVim => Some(Self::Editor),
            Self::Worktrees => Some(Self::Git),
            Self::General
            | Self::Appearance
            | Self::Languages
            | Self::Terminal
            | Self::Browser
            | Self::Git
            | Self::Servers
            | Self::Results
            | Self::Agents
            | Self::Editor
            | Self::Keymap => None,
        }
    }

    /// The kind of customization it lists, when it lists one.
    pub fn kind(self) -> Option<crate::settings_model::Kind> {
        use crate::settings_model::Kind;
        match self {
            Self::Skills => Some(Kind::Skill),
            Self::Instructions => Some(Kind::Instruction),
            Self::AgentDefinitions => Some(Kind::Agent),
            Self::Hooks => Some(Kind::Hook),
            _ => None,
        }
    }

    /// What the card of one of the customization pages on the overview says it holds.
    pub fn blurb(self) -> &'static str {
        match self {
            Self::Skills => "Folders of instructions loaded when a task calls for them.",
            Self::Instructions => "Files read for the folders they apply to.",
            Self::AgentDefinitions => "Agents the agent can hand work to.",
            Self::Hooks => "Commands run when something happens.",
            Self::Tools => "Its own tools, and each tool server's.",
            _ => "",
        }
    }

    /// What it says about itself under its heading.
    pub fn summary(self) -> &'static str {
        match self {
            Self::General => "How the window opens, closes and keeps itself up to date.",
            Self::Appearance => {
                "The theme, the fonts of the interface and of code, and how closely things are set."
            }
            Self::Languages => {
                "How each language is indented, formatted and read by its language server, and the languages installed on this machine."
            }
            Self::Keymap => {
                "Pick the editor your hands know, or press a binding to give an action keys of your own."
            }
            Self::Terminal => {
                "The shell the terminal starts, and how its text and cursor are drawn."
            }
            Self::Browser => {
                "The browser beside your work: turn it on, have it open the dev servers you and your agents start, and limit where agents may take it."
            }
            Self::Git => {
                "How the server fetches and keeps the history of files, and what git marks in files."
            }
            Self::Worktrees => {
                "How the server names the branches of sessions, where it makes their worktrees and what it copies into them."
            }
            Self::Editor => {
                "How files look, how they are edited and saved, and what the language server adds as you write."
            }
            Self::EditorAppearance => {
                "The line numbers, what is marked beside and over the lines, and the caret."
            }
            Self::EditorEditing => {
                "Wrapping, indentation, brackets and quotes, and what happens when a line is broken."
            }
            Self::EditorSaving => {
                "What is done to a file as it is saved, and when it saves itself."
            }
            Self::EditorIntelligence => {
                "Completions, hovers, signatures and the problems the language server finds."
            }
            Self::EditorSearch => {
                "Finding in a file, and how the view and its scrollbar follow it."
            }
            Self::EditorVim => {
                "Modal editing with vim's keys: whether it is on, what it shares with the clipboard, the bindings you add, and how replayed typing and syntax objects behave."
            }
            Self::Agents => {
                "Tailor how agents work in your projects: what new sessions start with, what happens when an agent finishes, the agents that can be started and the tools they are given."
            }
            Self::AgentServers => {
                "Every agent the harness offers. New sessions offer the ones signed in; switch one off to leave it out even then. Its sessions and accounts stay. Skills, instructions, tools, memory and conversations are shared by every account and every agent."
            }
            Self::McpServers => {
                "An open standard that lets AI use external tools and services. MCP servers provide tools for file operations, databases, APIs, and more."
            }
            Self::ModelServers => {
                "OpenAI-compatible servers, such as a company gateway or a local vLLM, LM Studio or llama.cpp server. Each one is offered as an agent, with its models in the model picker."
            }
            Self::ModelSlots => {
                "The models agents ask for each kind of work, for each provider. A provider's slot left unset uses the one set for all providers, else the conversation's own model. Name a slot as @name wherever a model is named, such as @fast in an agent definition."
            }
            Self::Servers => {
                "The servers the window works with: the one on this machine, and ones reached at their address, over ssh, inside WSL or a dev container, or through a tunnel."
            }
            Self::Customizations => {
                "What agents are given in the project besides the tool servers: the skills, instructions, agent definitions and hooks in the folders the agent reads, and the tools it may call. Turning one off leaves it out of sessions started afterwards; the files are never changed."
            }
            Self::Skills => {
                "Skills the agent loads when a task calls for them, each a folder with a SKILL.md."
            }
            Self::Instructions => {
                "Files of instructions the agent reads for the folders they apply to."
            }
            Self::AgentDefinitions => {
                "Agent definitions the agent can hand work to, each with the model and tools it names."
            }
            Self::Hooks => {
                "Commands the agent runs when something happens, as its settings files list them."
            }
            Self::Tools => {
                "The tools the agent may call: its own, and each tool server's. A tool turned off is not offered to new sessions; turning a set off turns off every tool in it."
            }
            Self::Results => "",
        }
    }
}

#[cfg(test)]
/// Tests for the names of the sections.
mod tests {
    use super::*;

    /// Every section is found again by its slug.
    #[test]
    fn slugs_round_trip() {
        for section in Section::ALL {
            assert_eq!(Section::from_slug(section.slug()), Some(section));
        }
    }
}
