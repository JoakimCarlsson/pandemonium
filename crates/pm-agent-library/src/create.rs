//! Writing a new skill, instruction file or agent definition where the harness reads it, from a short template the reader then fills in.

use std::path::{Path, PathBuf};

use omc_context::Locations;

use crate::item::{Kind, Scope};

/// What a new customization is asked to be: its kind, where it goes, its name and what it is for.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Draft {
    /// What it is; hooks cannot be drafted.
    pub kind: Kind,
    /// Whether it goes in the project's folder or the reader's own.
    pub scope: Scope,
    /// Its name, which names its file or folder.
    pub name: String,
    /// What it is for, as its file says.
    pub description: String,
}

/// Whether `name` can name a new customization: lowercase letters, digits and hyphens, starting with a letter or digit, at most 64 characters.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && name
            .chars()
            .all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-')
}

/// Writes `draft` for an agent working in `project`, empty when no project is open, and returns the file written.
///
/// A skill becomes `skills/<name>/SKILL.md`, an instruction file `rules/<name>.md` and an agent definition `agents/<name>.md`, under the project's `.claude` folder or Claude Code's own config folder.
///
/// # Errors
///
/// Fails when the kind is a hook, the scope is built in, the scope is the project's and `project` is empty, the name is not [`valid_name`], the reader's config folder is not known, the file is already there, or it cannot be written.
pub fn create(draft: &Draft, project: &Path) -> Result<PathBuf, String> {
    if !valid_name(&draft.name) {
        return Err(format!(
            "\"{}\" is not a valid name: use lowercase letters, digits and hyphens",
            draft.name
        ));
    }
    let folder = folder_of(draft.scope, project)?;
    let path = folder.join(relative_path(draft.kind, &draft.name)?);
    if path.exists() {
        return Err(format!("{} is already there", path.display()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not make {}: {error}", parent.display()))?;
    }
    std::fs::write(&path, template(draft))
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    Ok(path)
}

/// The folder customizations of `scope` are written under for `project`: the project's `.claude` folder, or Claude Code's own config folder.
fn folder_of(scope: Scope, project: &Path) -> Result<PathBuf, String> {
    match scope {
        Scope::Workspace if project.as_os_str().is_empty() => {
            Err("no project is open to write it in".to_owned())
        }
        Scope::Workspace => Ok(project.join(".claude")),
        Scope::User => Locations::detect(project)
            .claude
            .ok_or_else(|| "the home folder is not known".to_owned()),
        Scope::BuiltIn => Err("built-in customizations cannot be written".to_owned()),
    }
}

/// Where a customization of `kind` called `name` is kept, under its scope's folder.
fn relative_path(kind: Kind, name: &str) -> Result<PathBuf, String> {
    match kind {
        Kind::Skill => Ok(Path::new("skills").join(name).join("SKILL.md")),
        Kind::Instruction => Ok(Path::new("rules").join(format!("{name}.md"))),
        Kind::Agent => Ok(Path::new("agents").join(format!("{name}.md"))),
        Kind::Hook => Err("hooks are written in a settings file, not created here".to_owned()),
    }
}

/// What the new file of `draft` starts with.
fn template(draft: &Draft) -> String {
    let description = one_line(&draft.description);
    match draft.kind {
        Kind::Skill => format!(
            "---\nname: {}\ndescription: {description}\n---\n\n# {}\n\nDescribe, step by step, what the agent does when this skill is used.\n",
            draft.name,
            title(&draft.name)
        ),
        Kind::Agent => format!(
            "---\nname: {}\ndescription: {description}\n---\n\nYou are {}. Describe the task this agent is handed and how it reports back.\n",
            draft.name, draft.name
        ),
        Kind::Instruction | Kind::Hook => match description.is_empty() {
            true => format!("# {}\n\n", title(&draft.name)),
            false => format!("# {}\n\n{description}\n", title(&draft.name)),
        },
    }
}

/// `text` on one line, so that it cannot end the front matter it is written in.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `name` with its hyphens as spaces and a capital first letter.
fn title(name: &str) -> String {
    let spaced = name.replace('-', " ");
    let mut letters = spaced.chars();
    letters
        .next()
        .map(|first| first.to_uppercase().chain(letters).collect())
        .unwrap_or_default()
}

#[cfg(test)]
/// Tests for writing new customizations.
mod tests {
    use super::*;

    /// A fresh folder under the build directory for one test.
    fn scratch(name: &str) -> PathBuf {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../build/test-scratch/create")
            .join(name);
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("scratch folder");
        folder
    }

    /// A draft of `kind` called `name` for the project.
    fn draft(kind: Kind, name: &str) -> Draft {
        Draft {
            kind,
            scope: Scope::Workspace,
            name: name.to_owned(),
            description: "Does the\nthing".to_owned(),
        }
    }

    /// Names are lowercase words joined by hyphens.
    #[test]
    fn names_are_checked() {
        assert!(valid_name("release-notes"));
        assert!(valid_name("v2"));
        assert!(!valid_name(""));
        assert!(!valid_name("-lead"));
        assert!(!valid_name("Upper"));
        assert!(!valid_name("../escape"));
        assert!(!valid_name(&"a".repeat(65)));
    }

    /// Each kind lands where the harness reads it, and a skill is found again by discovery.
    #[test]
    fn each_kind_lands_where_it_is_read() {
        let project = scratch("kinds");
        let skill = create(&draft(Kind::Skill, "ship-it"), &project).expect("skill");
        let rule = create(&draft(Kind::Instruction, "style"), &project).expect("rule");
        let agent = create(&draft(Kind::Agent, "reviewer"), &project).expect("agent");
        assert_eq!(skill, project.join(".claude/skills/ship-it/SKILL.md"));
        assert_eq!(rule, project.join(".claude/rules/style.md"));
        assert_eq!(agent, project.join(".claude/agents/reviewer.md"));
        let written = std::fs::read_to_string(&skill).expect("read");
        assert!(written.contains("description: Does the thing\n"));
        let found = crate::discover(&project);
        assert!(
            found
                .iter()
                .any(|item| item.kind == Kind::Skill && item.name == "ship-it")
        );
        assert!(
            found
                .iter()
                .any(|item| item.kind == Kind::Agent && item.name == "reviewer")
        );
    }

    /// A file that is already there is left alone.
    #[test]
    fn an_existing_file_is_not_replaced() {
        let project = scratch("existing");
        create(&draft(Kind::Instruction, "style"), &project).expect("first");
        assert!(create(&draft(Kind::Instruction, "style"), &project).is_err());
    }

    /// Hooks and built-in scope cannot be drafted.
    #[test]
    fn hooks_and_built_ins_are_refused() {
        let project = scratch("refused");
        assert!(create(&draft(Kind::Hook, "x"), &project).is_err());
        let built_in = Draft {
            scope: Scope::BuiltIn,
            ..draft(Kind::Skill, "x")
        };
        assert!(create(&built_in, &project).is_err());
    }
}
