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
    found_in(&Locations::detect(project), Some(project))
}

/// The instruction files, skills and agent definitions the harness gives an agent in every project, read from the reader's own folders and its built-in ones, without any project's.
pub fn discover_own() -> Vec<Item> {
    let Some(home) = std::env::home_dir() else {
        return Vec::new();
    };
    let locations = Locations {
        project: false,
        ..Locations::detect(&home)
    };
    found_in(&locations, None)
        .into_iter()
        .filter(|item| item.scope != Scope::Workspace)
        .collect()
}

/// What a row says the file that replaces the harness's built-in instructions applies to.
pub const REPLACES_PROMPT: &str = "replaces crab's built-in instructions";

/// What a row says the file added to the end of every prompt applies to.
pub const APPENDS_PROMPT: &str = "added to the end of every prompt";

/// What crab finds in `locations`, as rows, with instruction files named from `project` when there is one, and each row saying whose folder it is kept in.
fn found_in(locations: &Locations, project: Option<&Path>) -> Vec<Item> {
    let found = omc_context::discover(locations, &[]);
    let system = [
        (found.system.as_ref(), REPLACES_PROMPT),
        (found.append_system.as_ref(), APPENDS_PROMPT),
    ]
    .into_iter()
    .filter_map(|(file, applies)| file.map(|file| prompt_file(file, project, applies)));
    found
        .skills
        .iter()
        .map(skill)
        .chain(system)
        .chain(
            found
                .instructions
                .iter()
                .map(|found| instruction(found, project)),
        )
        .chain(found.agents.iter().map(agent))
        .map(|mut item| {
            item.tool = tool_of(&item, locations).to_owned();
            item
        })
        .collect()
}

/// `found`, a file that replaces or adds to the harness's prompt, as a row of instruction files saying `applies`.
fn prompt_file(found: &Instruction, project: Option<&Path>, applies: &str) -> Item {
    Item {
        applies: vec![applies.to_owned()],
        ..instruction(found, project)
    }
}

/// The tool whose folder `item` is kept in: crab for its own folders and what is built into it, Claude Code and Codex for theirs and their plugins, and nothing for a file every tool reads.
fn tool_of(item: &Item, locations: &Locations) -> &'static str {
    let path = Path::new(&item.path);
    let under = |folder: &Option<std::path::PathBuf>| {
        folder
            .as_deref()
            .is_some_and(|folder| path.starts_with(folder))
    };
    let named = |part: &str| {
        path.components()
            .any(|component| component.as_os_str() == part)
    };
    let file = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if item.path.is_empty() || under(&locations.crab) || named(omc_context::PROJECT_FOLDER) {
        "crab"
    } else if under(&locations.claude)
        || named(".claude")
        || file == "CLAUDE.md"
        || file == "CLAUDE.local.md"
    {
        "Claude Code"
    } else if under(&locations.codex) || named(".codex") || file == "AGENTS.override.md" {
        "Codex"
    } else {
        ""
    }
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
        Origin::Plugin => Scope::Plugin,
        Origin::BuiltIn => Scope::BuiltIn,
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

/// `found` as a row of instruction files, applying to every project or the whole of `project`, when there is one.
///
/// It is named by where it is, as `shown` writes it, so that two files of one name never share the key that turning one off is kept under, and keys kept before still match.
fn instruction(found: &Instruction, project: Option<&Path>) -> Item {
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

/// How a row names the file `file`: from `project` when there is one and it is inside it, after `~/` when it is in the home folder, and in full otherwise.
fn shown(file: &Path, project: Option<&Path>) -> String {
    if let Some(inside) = project.and_then(|project| file.strip_prefix(project).ok()) {
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
        model: model_named(found.model.as_deref()),
        ..Item::default()
    }
}

/// The model a row of an agent definition names, from the definition's `model`: nothing for `inherit`, the model of the session that starts it; `@subagent` when it names none, as the harness then asks the `subagent` slot; otherwise the model as written, such as `haiku` or `@fast`.
fn model_named(model: Option<&str>) -> String {
    match model {
        Some("inherit") => String::new(),
        Some(model) => model.to_owned(),
        None => "@subagent".to_owned(),
    }
}

#[cfg(test)]
/// Tests for how found items are written as rows.
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A definition that inherits shows the session's model, one with none the `subagent` slot, and one naming a model or slot shows it.
    #[test]
    fn models_are_named_as_the_harness_reads_them() {
        assert_eq!(model_named(Some("inherit")), "");
        assert_eq!(model_named(None), "@subagent");
        assert_eq!(model_named(Some("@fast")), "@fast");
    }

    /// The files that replace or add to crab's prompt are listed with the instructions, and every row says whose folder it is kept in.
    #[test]
    fn rows_say_whose_folder_they_are_in() {
        let project =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/test-scratch/found-tools");
        let _ = std::fs::remove_dir_all(&project);
        let put = |path: &str, text: &str| {
            let at = project.join(path);
            std::fs::create_dir_all(at.parent().expect("parent")).expect("folder");
            std::fs::write(at, text).expect("write");
        };
        put(".oh-my-crab/SYSTEM.md", "Be terse.");
        put(".oh-my-crab/APPEND_SYSTEM.md", "Say done.");
        put(
            ".oh-my-crab/skills/mine/SKILL.md",
            "---\nname: mine\ndescription: Mine\n---\nDo it.",
        );
        put(
            ".claude/skills/theirs/SKILL.md",
            "---\nname: theirs\ndescription: Theirs\n---\nDo it.",
        );
        put("CLAUDE.md", "Claude only.");
        put("AGENTS.md", "Everyone.");
        let locations = Locations {
            directory: project.clone(),
            root: None,
            home: None,
            claude: None,
            codex: None,
            crab: None,
            project: true,
        };
        let found = found_in(&locations, Some(&project));
        let tool = |name: &str| {
            found
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.tool.clone())
                .unwrap_or_else(|| panic!("{name} not found"))
        };
        assert_eq!(tool("mine"), "crab");
        assert_eq!(tool("theirs"), "Claude Code");
        assert_eq!(tool("CLAUDE.md"), "Claude Code");
        assert_eq!(tool("AGENTS.md"), "");
        let system = found
            .iter()
            .find(|item| item.name == ".oh-my-crab/SYSTEM.md")
            .expect("SYSTEM.md listed");
        assert_eq!(system.applies, [REPLACES_PROMPT]);
        assert_eq!(system.tool, "crab");
        assert!(
            found
                .iter()
                .any(|item| item.name == ".oh-my-crab/APPEND_SYSTEM.md"
                    && item.applies == [APPENDS_PROMPT])
        );
    }

    /// Instruction files are named by where they are, so that two of one file name get different keys.
    #[test]
    fn instructions_are_named_by_where_they_are() {
        let project = Some(Path::new("/work/app"));
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
