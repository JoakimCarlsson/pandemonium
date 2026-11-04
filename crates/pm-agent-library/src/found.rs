//! What the harness gives an agent working in a project, as crab finds it, written as the items the settings list.

use std::path::Path;

use omc_context::{AgentDefinition, Instruction, Locations, Origin, Skill};

use crate::item::{Item, Kind, Scope};

/// What an instruction file of the reader's applies to.
const EVERY_PROJECT: &str = "every project";

/// What an instruction file of the project's applies to.
const WHOLE_PROJECT: &str = "the whole project";

/// Every instruction file, skill and agent definition the harness gives an agent working in `project`, turned off or not, in the order crab finds them.
pub fn discover(project: &Path) -> Vec<Item> {
    let found = omc_context::discover(&Locations::detect(project), &[]);
    found
        .skills
        .iter()
        .map(skill)
        .chain(
            found
                .instructions
                .iter()
                .map(|found| instruction(found, project)),
        )
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

/// `found` as a row of instruction files, applying to every project or the whole of `project`.
///
/// It is named by where it is, as `shown` writes it, so that two files of one name never share the key that turning one off is kept under, and keys kept before still match.
fn instruction(found: &Instruction, project: &Path) -> Item {
    Item {
        kind: Kind::Instruction,
        scope: if found.user {
            Scope::User
        } else {
            Scope::Workspace
        },
        name: shown(&found.path, project),
        applies: vec![
            if found.user {
                EVERY_PROJECT
            } else {
                WHOLE_PROJECT
            }
            .to_owned(),
        ],
        path: found.path.display().to_string(),
        ..Item::default()
    }
}

/// How a row names the file `file`: from `project` when it is inside it, after `~/` when it is in the home folder, and in full otherwise.
fn shown(file: &Path, project: &Path) -> String {
    if let Ok(inside) = file.strip_prefix(project) {
        return inside.to_string_lossy().into_owned();
    }
    if let Some(home) = std::env::home_dir()
        && let Ok(inside) = file.strip_prefix(home)
    {
        return format!("~/{}", inside.to_string_lossy());
    }
    file.to_string_lossy().into_owned()
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

#[cfg(test)]
/// Tests for how found items are written as rows.
mod tests {
    use super::*;

    /// Instruction files are named by where they are, so that two of one file name get different keys.
    #[test]
    fn instructions_are_named_by_where_they_are() {
        let project = Path::new("/work/app");
        let at = |path: &str, user: bool| Instruction {
            path: path.into(),
            content: String::new(),
            user,
        };
        let top = instruction(&at("/work/app/CLAUDE.md", false), project);
        let inner = instruction(&at("/work/app/.claude/CLAUDE.md", false), project);
        let above = instruction(&at("/work/CLAUDE.md", false), project);
        assert_eq!(top.name, "CLAUDE.md");
        assert_eq!(inner.name, ".claude/CLAUDE.md");
        assert_eq!(above.name, "/work/CLAUDE.md");
        assert_ne!(top.key(), inner.key());
        assert_eq!(top.key(), "instruction/workspace/CLAUDE.md");
        if let Some(home) = std::env::home_dir() {
            let mine = instruction(
                &at(&home.join(".claude/CLAUDE.md").to_string_lossy(), true),
                project,
            );
            assert_eq!(mine.name, "~/.claude/CLAUDE.md");
            assert_eq!(mine.applies, vec![EVERY_PROJECT.to_owned()]);
        }
    }
}
