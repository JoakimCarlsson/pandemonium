//! Reading the customizations a product is given in a project from the places it keeps them.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::front_matter::{field, fields, list};
use crate::item::{Item, Kind, Scope};
use crate::place::{Place, Root, Shape, places};
use crate::product::Product;

/// How many folders deep files are looked for inside a folder of files, and instructions inside the project.
const DEPTH: usize = 4;

/// How many folders of the project are looked through for instructions, at most.
const FOLDERS: usize = 2000;

/// Folders inside a project that hold what was fetched or built rather than written, which are not looked through for instructions.
const SKIPPED: [&str; 4] = ["build", "dist", "vendor", "out"];

/// Every customization `product` is given in the project at `project`, with `own` as the folder the product keeps its config in, kind by kind in the order its places are listed.
pub fn discover(product: Option<Product>, project: &Path, own: Option<&Path>) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    for kind in Kind::ALL {
        for place in places(product).iter().filter(|place| place.kind == kind) {
            let root = match place.root {
                Root::Project => Some(project.to_path_buf()),
                Root::Own => own.map(Path::to_path_buf),
                Root::Home => std::env::home_dir(),
            };
            let Some(root) = root else {
                continue;
            };
            for item in read(place, &root, project) {
                if !items
                    .iter()
                    .any(|held| held.path == item.path && held.name == item.name)
                {
                    items.push(item);
                }
            }
        }
    }
    items
}

/// The skills `product` is given in the project at `project`, with `own` as its config folder.
pub fn skills(product: Option<Product>, project: &Path, own: Option<&Path>) -> Vec<Item> {
    discover(product, project, own)
        .into_iter()
        .filter(|item| item.kind == Kind::Skill)
        .collect()
}

/// What `place`, under `root`, holds; `project` is the project's folder, which instructions name the folders they apply to from.
fn read(place: &Place, root: &Path, project: &Path) -> Vec<Item> {
    let at = root.join(place.path);
    match place.shape {
        Shape::Skills => skill_folders(&at, place.scope, ""),
        Shape::Files(suffix) => files(&at, suffix, DEPTH)
            .into_iter()
            .filter_map(|file| markdown(place, &file, suffix, root, project))
            .collect(),
        Shape::File => markdown(place, &at, "", root, project)
            .into_iter()
            .collect(),
        Shape::Nested => nested(project, place.path)
            .into_iter()
            .filter_map(|file| markdown(place, &file, "", root, project))
            .collect(),
        Shape::Hooks => hooks(&at, place.scope),
        Shape::Toml => files(&at, ".toml", 1)
            .into_iter()
            .filter_map(|file| toml_agent(&file, place.scope))
            .collect(),
        Shape::Plugins => plugins(&at)
            .into_iter()
            .flat_map(|(plugin, folder)| match place.kind {
                Kind::Skill => skill_folders(&folder.join("skills"), Scope::BuiltIn, &plugin),
                _ => files(&folder.join("agents"), ".md", 1)
                    .into_iter()
                    .filter_map(|file| {
                        let mut item = markdown(place, &file, ".md", root, project)?;
                        item.name = format!("{plugin}:{}", item.name);
                        Some(item)
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The skills in the folders inside `folder` that each hold a `SKILL.md`, from `scope`, their names after `plugin:` when a plugin brings them.
fn skill_folders(folder: &Path, scope: Scope, plugin: &str) -> Vec<Item> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found: Vec<Item> = entries
        .flatten()
        .filter_map(|entry| {
            let file = entry.path().join("SKILL.md");
            let text = fs::read_to_string(&file).ok()?;
            let held = fields(&text);
            let name = field(&held, "name")
                .map(str::to_owned)
                .or_else(|| Some(entry.file_name().to_str()?.to_owned()))?;
            Some(Item {
                kind: Kind::Skill,
                scope,
                name: if plugin.is_empty() {
                    name
                } else {
                    format!("{plugin}:{name}")
                },
                description: field(&held, "description").unwrap_or_default().to_owned(),
                path: file.to_string_lossy().into_owned(),
                ..Item::default()
            })
        })
        .collect();
    found.sort_by(|left, right| left.name.cmp(&right.name));
    found
}

/// The files under `folder`, at most `depth` folders down, whose names end in `suffix`, in the order of their paths.
fn files(folder: &Path, suffix: &str, depth: usize) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 1 {
                found.extend(files(&path, suffix, depth - 1));
            }
        } else if entry.file_name().to_string_lossy().ends_with(suffix) {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// The files named `name` in the folders inside `project`, its root left out, looking no further than [`DEPTH`] folders down and through no more than [`FOLDERS`] folders.
fn nested(project: &Path, name: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut waiting = vec![(project.to_path_buf(), 0)];
    let mut looked = 0;
    while let Some((folder, depth)) = waiting.pop() {
        looked += 1;
        if looked > FOLDERS {
            break;
        }
        if depth > 0 && folder.join(name).is_file() {
            found.push(folder.join(name));
        }
        if depth == DEPTH {
            continue;
        }
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let named = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir()
                && !named.starts_with('.')
                && !SKIPPED.contains(&named.as_str())
                && !pm_tools::generated().contains(&named)
            {
                waiting.push((entry.path(), depth + 1));
            }
        }
    }
    found.sort();
    found
}

/// The customization the markdown file `file` of `place` is, named from its front matter or from its file name less `suffix`; `root` is the folder of the place and `project` the project's.
fn markdown(place: &Place, file: &Path, suffix: &str, root: &Path, project: &Path) -> Option<Item> {
    let text = fs::read_to_string(file).ok()?;
    let held = fields(&text);
    let file_name = file.file_name()?.to_string_lossy().into_owned();
    let stem = file_name.strip_suffix(suffix).unwrap_or(&file_name);
    let name = match place.kind {
        Kind::Instruction => shown(file, root, project),
        _ => field(&held, "name").unwrap_or(stem).to_owned(),
    };
    let applies = match (
        place.kind,
        field(&held, "paths").or(field(&held, "applyTo")),
    ) {
        (Kind::Instruction, Some(patterns)) => list(patterns),
        (Kind::Instruction, None) if place.root == Root::Project => {
            let folder = file.parent()?.strip_prefix(project).ok()?;
            let folder = folder.to_string_lossy();
            vec![match folder.as_ref() {
                "" | ".claude" | ".github" => "the whole project".to_owned(),
                inner => format!("{inner}/"),
            }]
        }
        (Kind::Instruction, None) => vec!["every project".to_owned()],
        _ => Vec::new(),
    };
    Some(Item {
        kind: place.kind,
        scope: place.scope,
        name,
        description: field(&held, "description").unwrap_or_default().to_owned(),
        path: file.to_string_lossy().into_owned(),
        applies,
        model: field(&held, "model").unwrap_or_default().to_owned(),
        tools: field(&held, "tools").map(list).unwrap_or_default(),
        command: String::new(),
    })
}

/// How a row names the file `file`: from the project's folder when it is inside it, from the home folder after `~` when it is there, and as it is otherwise; `root` is the folder of its place.
fn shown(file: &Path, root: &Path, project: &Path) -> String {
    if let Ok(inside) = file.strip_prefix(project) {
        return inside.to_string_lossy().into_owned();
    }
    if let Some(home) = std::env::home_dir()
        && let Ok(inside) = file.strip_prefix(home)
    {
        return format!("~/{}", inside.to_string_lossy());
    }
    file.strip_prefix(root.parent().unwrap_or(root))
        .unwrap_or(file)
        .to_string_lossy()
        .into_owned()
}

/// The hooks the settings file `file` lists under `hooks`, each event's commands one by one, from `scope`.
fn hooks(file: &Path, scope: Scope) -> Vec<Item> {
    let Some(held) = fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    else {
        return Vec::new();
    };
    let Some(events) = held["hooks"].as_object() else {
        return Vec::new();
    };
    events
        .iter()
        .flat_map(|(event, groups)| {
            groups
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(move |group| {
                    let matcher = group["matcher"].as_str().unwrap_or_default().to_owned();
                    group["hooks"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(move |hook| {
                            Some(Item {
                                kind: Kind::Hook,
                                scope,
                                name: event.clone(),
                                description: matcher.clone(),
                                path: file.to_string_lossy().into_owned(),
                                command: hook["command"]
                                    .as_str()
                                    .or_else(|| hook["url"].as_str())
                                    .or_else(|| hook["prompt"].as_str())?
                                    .to_owned(),
                                ..Item::default()
                            })
                        })
                })
        })
        .collect()
}

/// The agent definition the TOML file `file` describes, from `scope`, named from its `name` or its file name.
fn toml_agent(file: &Path, scope: Scope) -> Option<Item> {
    let table: toml::Table = fs::read_to_string(file).ok()?.parse().ok()?;
    let text = |key: &str| {
        table
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
    };
    Some(Item {
        kind: Kind::Agent,
        scope,
        name: text("name").or_else(|| Some(file.file_stem()?.to_string_lossy().into_owned()))?,
        description: text("description").unwrap_or_default(),
        path: file.to_string_lossy().into_owned(),
        model: text("model").unwrap_or_default(),
        ..Item::default()
    })
}

/// The plugins installed for a product whose plugin folder is `folder`, by name, each with the folder it is installed in, as the folder's record of them lists them.
fn plugins(folder: &Path) -> Vec<(String, PathBuf)> {
    let Some(held) = fs::read_to_string(folder.join("installed_plugins.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    else {
        return Vec::new();
    };
    let Some(installed) = held["plugins"].as_object() else {
        return Vec::new();
    };
    installed
        .iter()
        .filter_map(|(id, installs)| {
            let name = id.split('@').next().unwrap_or(id).to_owned();
            let path = installs.as_array()?.first()?["installPath"].as_str()?;
            Some((name, PathBuf::from(path)))
        })
        .collect()
}
