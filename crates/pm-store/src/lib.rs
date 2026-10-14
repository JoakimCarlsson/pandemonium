//! Persistence for what the reader has set up: the projects they added and the choices they made for each agent.

use std::fs;
use std::io;
use std::path::PathBuf;

/// The folder the app keeps its data in, under the desktop's data directory.
fn directory() -> Option<PathBuf> {
    dirs::data_dir().map(|data| data.join("pandemonium"))
}

/// The file the project list is kept in.
fn file() -> Option<PathBuf> {
    directory().map(|directory| directory.join("projects"))
}

/// The paths of the projects saved by [`save_projects`], oldest first, or
/// nothing when none was ever saved: the app has not been set up yet.
pub fn load_projects() -> Option<Vec<String>> {
    let contents = fs::read_to_string(file()?).ok()?;
    Some(
        contents
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// Saves `paths` as the projects to open with next time, one per line.
pub fn save_projects(paths: &[String]) -> io::Result<()> {
    let (Some(directory), Some(file)) = (directory(), file()) else {
        return Err(io::Error::other("the desktop has no data directory"));
    };
    fs::create_dir_all(directory)?;
    let mut contents = paths.join("\n");
    contents.push('\n');
    fs::write(file, contents)
}

/// The file the choices made for each agent are kept in.
fn choices_file() -> Option<PathBuf> {
    directory().map(|directory| directory.join("choices"))
}

/// A setting of an agent the reader chose a value for: the agent, the setting and the value, each as the agent names it.
pub type Choice = (String, String, String);

/// The choices saved by [`save_choices`], or none when none was ever saved.
pub fn load_choices() -> Vec<Choice> {
    let Some(contents) = choices_file().and_then(|file| fs::read_to_string(file).ok()) else {
        return Vec::new();
    };
    contents
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some((
                parts.next()?.to_owned(),
                parts.next()?.to_owned(),
                parts.next()?.to_owned(),
            ))
        })
        .collect()
}

/// Saves `choices` to be chosen again next time, one per line.
pub fn save_choices(choices: &[Choice]) -> io::Result<()> {
    let (Some(directory), Some(file)) = (directory(), choices_file()) else {
        return Err(io::Error::other("the desktop has no data directory"));
    };
    fs::create_dir_all(directory)?;
    let mut contents = String::new();
    for (agent, setting, value) in choices {
        contents.push_str(&format!("{agent}\t{setting}\t{value}\n"));
    }
    fs::write(file, contents)
}

/// The text saved under `name` by [`save_text`], if any.
pub fn load_text(name: &str) -> Option<String> {
    fs::read_to_string(directory()?.join(name)).ok()
}

/// Saves `text` under `name`, replacing what was saved there.
pub fn save_text(name: &str, text: &str) -> io::Result<()> {
    let Some(directory) = directory() else {
        return Err(io::Error::other("the desktop has no data directory"));
    };
    fs::create_dir_all(&directory)?;
    fs::write(directory.join(name), text)
}

/// Saves `text` under `name` so that only the reader can read it, replacing what was saved there.
pub fn save_private(name: &str, text: &str) -> io::Result<()> {
    use std::io::Write;
    let Some(directory) = directory() else {
        return Err(io::Error::other("the desktop has no data directory"));
    };
    fs::create_dir_all(&directory)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options
        .open(directory.join(name))?
        .write_all(text.as_bytes())
}

/// Makes the folder at `path`, and the ones above it, so that only the reader can open the ones made.
pub fn create_private_directory(path: &std::path::Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(path)
}

/// Opens the file at `path` for writing, making it when it is not there so that only the reader can read it, and adding to its end when `append` is set and emptying it otherwise.
pub fn open_private(path: &std::path::Path, append: bool) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// The folder the app keeps its data in.
pub fn data_directory() -> Option<PathBuf> {
    directory()
}

/// A named login of an agent kept apart from the others: the agent, the folder name it is kept under and what it is called.
pub type Account = (String, String, String);

/// The accounts saved by [`save_accounts`] and, for each agent, the one in use, if any.
pub fn load_accounts() -> (Vec<Account>, Vec<(String, String)>) {
    let mut accounts = Vec::new();
    let mut selected = Vec::new();
    let Some(contents) = load_text("account-list") else {
        return (accounts, selected);
    };
    for line in contents.lines() {
        let mut parts = line.split('\t');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some("account"), Some(agent), Some(id), Some(name)) => {
                accounts.push((agent.to_owned(), id.to_owned(), name.to_owned()));
            }
            (Some("selected"), Some(agent), Some(id), _) => {
                selected.push((agent.to_owned(), id.to_owned()));
            }
            _ => {}
        }
    }
    (accounts, selected)
}

/// Saves `accounts` and the `selected` one of each agent, and makes sure each account has a private folder.
pub fn save_accounts(accounts: &[Account], selected: &[(String, String)]) -> io::Result<()> {
    let mut contents = String::new();
    for (agent, id, name) in accounts {
        account_directory(agent, id)?;
        contents.push_str(&format!("account\t{agent}\t{id}\t{name}\n"));
    }
    for (agent, id) in selected {
        contents.push_str(&format!("selected\t{agent}\t{id}\n"));
    }
    save_text("account-list", &contents)
}

/// The folder an account keeps its login in, created private to the reader when it is not there yet.
pub fn account_directory(agent: &str, id: &str) -> io::Result<PathBuf> {
    let safe = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    };
    if !safe(agent) || !safe(id) {
        return Err(io::Error::other(
            "the account name is not a safe folder name",
        ));
    }
    let Some(directory) = directory() else {
        return Err(io::Error::other("the desktop has no data directory"));
    };
    let folder = directory.join("accounts").join(agent).join(id);
    if !folder.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&folder)?;
        seed(agent, &folder)?;
    }
    Ok(folder)
}

/// Puts into a new account folder what the agent needs to sign in on its own and keep its login there.
fn seed(agent: &str, folder: &std::path::Path) -> io::Result<()> {
    match agent {
        "claude-code" => fs::write(
            folder.join("settings.json"),
            r#"{"forceLoginMethod":"claudeai"}"#,
        ),
        "codex" => fs::write(
            folder.join("config.toml"),
            "cli_auth_credentials_store = \"file\"\n",
        ),
        _ => Ok(()),
    }
}
