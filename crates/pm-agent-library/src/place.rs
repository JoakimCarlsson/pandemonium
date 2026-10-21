//! Where each product keeps each kind of customization, as the folders it reads.

use crate::item::{Kind, Scope};
use crate::product::Product;

use Kind::{Agent, Hook, Instruction, Skill};
use Root::{Home, Own, Project};
use Scope::{BuiltIn, User, Workspace};
use Shape::{File, Files, Hooks, Nested, Plugins, Skills, Toml};

/// The folder a place is found under.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Root {
    /// The project's folder.
    Project,
    /// The product's own config folder.
    Own,
    /// The reader's home folder.
    Home,
}

/// How the files of a place are laid out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Shape {
    /// Folders that each hold a `SKILL.md`.
    Skills,
    /// The files ending in this, in the folder and the folders inside it.
    Files(&'static str),
    /// The one file at the path.
    File,
    /// The file named so in every folder inside the project but its root.
    Nested,
    /// The hooks a settings file in JSON lists.
    Hooks,
    /// Agent definitions written as `.toml` files.
    Toml,
    /// What the plugins installed for the product bring.
    Plugins,
}

/// One place a product reads a kind of customization from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Place {
    /// What it holds.
    pub(crate) kind: Kind,
    /// Where what it holds comes from.
    pub(crate) scope: Scope,
    /// The folder its path is under.
    pub(crate) root: Root,
    /// Its path under that folder.
    pub(crate) path: &'static str,
    /// How its files are laid out.
    pub(crate) shape: Shape,
}

/// A place holding `kind` from `scope`, at `path` under `root`, laid out as `shape`.
const fn at(kind: Kind, scope: Scope, root: Root, path: &'static str, shape: Shape) -> Place {
    Place {
        kind,
        scope,
        root,
        path,
        shape,
    }
}

/// Where Claude Code reads its customizations.
const CLAUDE: &[Place] = &[
    at(Skill, User, Own, "skills", Skills),
    at(Skill, Workspace, Project, ".claude/skills", Skills),
    at(Skill, BuiltIn, Own, "plugins", Plugins),
    at(Instruction, User, Own, "CLAUDE.md", File),
    at(Instruction, User, Own, "rules", Files(".md")),
    at(Instruction, Workspace, Project, "CLAUDE.md", File),
    at(Instruction, Workspace, Project, ".claude/CLAUDE.md", File),
    at(Instruction, Workspace, Project, "CLAUDE.local.md", File),
    at(
        Instruction,
        Workspace,
        Project,
        ".claude/rules",
        Files(".md"),
    ),
    at(Instruction, Workspace, Project, "CLAUDE.md", Nested),
    at(Agent, User, Own, "agents", Files(".md")),
    at(Agent, Workspace, Project, ".claude/agents", Files(".md")),
    at(Agent, BuiltIn, Own, "plugins", Plugins),
    at(Hook, User, Own, "settings.json", Hooks),
    at(Hook, Workspace, Project, ".claude/settings.json", Hooks),
    at(
        Hook,
        Workspace,
        Project,
        ".claude/settings.local.json",
        Hooks,
    ),
];

/// Where Codex reads its customizations.
const CODEX: &[Place] = &[
    at(Skill, User, Home, ".agents/skills", Skills),
    at(Skill, User, Own, "skills", Skills),
    at(Skill, Workspace, Project, ".codex/skills", Skills),
    at(Skill, Workspace, Project, ".agents/skills", Skills),
    at(Instruction, User, Own, "AGENTS.md", File),
    at(Instruction, User, Own, "AGENTS.override.md", File),
    at(Instruction, Workspace, Project, "AGENTS.md", File),
    at(Instruction, Workspace, Project, "AGENTS.override.md", File),
    at(Instruction, Workspace, Project, "AGENTS.md", Nested),
    at(Agent, User, Own, "agents", Toml),
    at(Agent, Workspace, Project, ".codex/agents", Toml),
    at(Hook, User, Own, "hooks.json", Hooks),
    at(Hook, Workspace, Project, ".codex/hooks.json", Hooks),
];

/// Where Grok Build reads its customizations.
const GROK: &[Place] = &[
    at(Skill, User, Own, "skills", Skills),
    at(Skill, User, Home, ".agents/skills", Skills),
    at(Skill, Workspace, Project, ".grok/skills", Skills),
    at(Skill, Workspace, Project, ".agents/skills", Skills),
    at(Instruction, User, Own, "GROK.md", File),
    at(Instruction, User, Own, "AGENTS.md", File),
    at(Instruction, Workspace, Project, "GROK.md", File),
    at(Instruction, Workspace, Project, "AGENTS.md", File),
    at(Instruction, Workspace, Project, "AGENTS.md", Nested),
    at(Agent, User, Own, "agents", Files(".md")),
    at(Agent, Workspace, Project, ".grok/agents", Files(".md")),
    at(Hook, User, Own, "settings.json", Hooks),
    at(Hook, Workspace, Project, ".grok/settings.json", Hooks),
];

/// Where GitHub Copilot reads its customizations.
const COPILOT: &[Place] = &[
    at(Skill, User, Own, "skills", Skills),
    at(Skill, Workspace, Project, ".github/skills", Skills),
    at(Instruction, User, Own, "copilot-instructions.md", File),
    at(
        Instruction,
        Workspace,
        Project,
        ".github/copilot-instructions.md",
        File,
    ),
    at(
        Instruction,
        Workspace,
        Project,
        ".github/instructions",
        Files(".instructions.md"),
    ),
    at(Instruction, Workspace, Project, "AGENTS.md", File),
    at(Agent, User, Own, "agents", Files(".agent.md")),
    at(
        Agent,
        Workspace,
        Project,
        ".github/agents",
        Files(".agent.md"),
    ),
];

/// Every place `product` reads its customizations from; none for a product the app does not know.
pub(crate) fn places(product: Option<Product>) -> &'static [Place] {
    match product {
        Some(Product::Claude) => CLAUDE,
        Some(Product::Codex) => CODEX,
        Some(Product::Grok) => GROK,
        Some(Product::Copilot) => COPILOT,
        None => &[],
    }
}

/// Where `product` would read customizations of `kind` from, written for a reader: a path in the project, or one under `~`.
pub fn locations(product: Option<Product>, kind: Kind) -> Vec<String> {
    places(product)
        .iter()
        .filter(|place| place.kind == kind)
        .map(|place| {
            let folder = match place.root {
                Project => String::new(),
                Own => format!("~/.{}/", product.map_or("", Product::slug)),
                Home => "~/".to_owned(),
            };
            match place.shape {
                Skills => format!("{folder}{}/<name>/SKILL.md", place.path),
                Files(suffix) => format!("{folder}{}/<name>{suffix}", place.path),
                Toml => format!("{folder}{}/<name>.toml", place.path),
                File | Hooks => format!("{folder}{}", place.path),
                Nested => format!("<folder>/{}", place.path),
                Plugins => "installed plugins".to_owned(),
            }
        })
        .collect()
}
