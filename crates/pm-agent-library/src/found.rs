//! What the harness gives an agent working in a project, as crab finds it, written as the items the settings list.

use std::path::Path;

use omc_context::{AgentDefinition, Instruction, Locations, Origin, Skill};

use crate::item::{Item, Kind, Scope};

/// What an instruction file of the reader's applies to.
const EVERY_PROJECT: &str = "every project";

/// What an instruction file of the project's applies to.
const THIS_PROJECT: &str = "this project";

/// Every instruction file, skill and agent definition the harness gives an agent working in `project`, turned off or not, in the order crab finds them.
pub fn discover(project: &Path) -> Vec<Item> {
    let found = omc_context::discover(&Locations::detect(project), &[]);
    found
        .skills
        .iter()
        .map(skill)
        .chain(found.instructions.iter().map(instruction))
        .chain(found.agents.iter().map(agent))
        .collect()
}

/// Where the harness reads customizations of `kind` from, written for a reader; none for a kind it does not read.
pub fn locations(kind: Kind) -> Vec<String> {
    match kind {
        Kind::Skill => omc_context::skill_places(),
        Kind::Instruction => omc_context::instruction_places(),
        Kind::Agent => omc_context::agent_places(),
        Kind::Hook => Vec::new(),
    }
}

/// The scope a row shows for what came from `origin`.
fn scope(origin: Origin) -> Scope {
    match origin {
        Origin::Project => Scope::Workspace,
        Origin::User => Scope::User,
        Origin::Plugin | Origin::BuiltIn => Scope::BuiltIn,
    }
}

/// `found` as a row of skills.
fn skill(found: &Skill) -> Item {
    Item {
        kind: Kind::Skill,
        scope: scope(found.origin),
        name: found.name.clone(),
        description: found.description.clone(),
        path: found.path.display().to_string(),
        ..Item::default()
    }
}

/// `found` as a row of instruction files, named after its file and applying to every project or this one.
fn instruction(found: &Instruction) -> Item {
    let path = found.path.display().to_string();
    Item {
        kind: Kind::Instruction,
        scope: if found.user {
            Scope::User
        } else {
            Scope::Workspace
        },
        name: found
            .path
            .file_name()
            .map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned()),
        applies: vec![
            if found.user {
                EVERY_PROJECT
            } else {
                THIS_PROJECT
            }
            .to_owned(),
        ],
        path,
        ..Item::default()
    }
}

/// `found` as a row of agent definitions; a built-in one has no file.
fn agent(found: &AgentDefinition) -> Item {
    Item {
        kind: Kind::Agent,
        scope: scope(found.origin),
        name: found.name.clone(),
        description: found.description.clone(),
        path: found
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        tools: found.tools.clone().unwrap_or_default(),
        ..Item::default()
    }
}
