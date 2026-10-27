//! A build an adapter's scenario asks for in place of a `program`, as the language that brings the adapter says how it is run.
//!
//! A configuration written for one editor often does not name the program: it
//! names the command that builds it, under a key of its own, and the adapter
//! finds the executable in the command's output. An adapter that knows only
//! `program` is helped by running the command here and putting the executable
//! it made where `program` goes. What the command is, how its output is read
//! and what narrows it down are all the language's to say.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde_json::{Map, Value};

/// How many lines of the command's own complaint are kept when the build fails.
const COMPLAINT_LINES: usize = 12;

/// How a build is run and read.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Build {
    /// The key of a configuration that holds the build, an object.
    pub key: String,
    /// The program that builds.
    pub program: String,
    /// The key of the build that holds its arguments, a list.
    pub args: String,
    /// What is put after the arguments.
    pub append: Vec<String>,
    /// The key of the build that holds the folder it runs in.
    pub cwd: String,
    /// The key of the build that holds the variables it runs with.
    pub env: String,
    /// The key of the build that holds what narrows down the executables it made.
    pub filter: String,
    /// What a line of its output, a JSON object, must have to say that an executable was made: each key with the value it holds.
    pub matching: Map<String, Value>,
    /// The key of such a line that holds the executable.
    pub take: String,
    /// For each key of the filter, where in such a line the value it is compared to stands, as keys joined by dots; a list holds it when it has it among its items.
    pub compare: BTreeMap<String, String>,
}

/// The value at the path `path`, keys joined by dots, of `message`.
fn at<'a>(message: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(message, |held, key| held.get(key))
}

impl Build {
    /// Replaces the build `config` holds under the build's key with the `program` it makes.
    ///
    /// A configuration that names a `program` of its own, or has no such key,
    /// is left as it is. The build runs in the configuration's `cwd`, or in
    /// `root` where it names none.
    pub(crate) fn resolve(
        &self,
        config: &mut Map<String, Value>,
        root: &Path,
    ) -> Result<(), String> {
        let Some(Value::Object(build)) = config.get(&self.key).cloned() else {
            return Ok(());
        };
        if config.contains_key("program") {
            config.remove(&self.key);
            return Ok(());
        }
        let directory = config
            .get("cwd")
            .and_then(Value::as_str)
            .map_or_else(|| root.to_path_buf(), |own| root.join(own));
        let program = self.run(&build, &directory)?;
        config.remove(&self.key);
        config.insert("program".into(), program.into());
        Ok(())
    }

    /// Runs the build `build` describes in `directory`, answering the executable it made that the filter picks out.
    fn run(&self, build: &Map<String, Value>, directory: &Path) -> Result<String, String> {
        let arguments = build
            .get(&self.args)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        let mut command = Command::new(&self.program);
        command.args(arguments).args(&self.append).current_dir(
            build
                .get(&self.cwd)
                .and_then(Value::as_str)
                .map_or_else(|| directory.to_path_buf(), |own| directory.join(own)),
        );
        if let Some(Value::Object(env)) = build.get(&self.env) {
            command.envs(
                env.iter()
                    .filter_map(|(name, value)| Some((name, value.as_str()?))),
            );
        }
        let output = command
            .output()
            .map_err(|error| format!("{} could not be run: {error}", self.program))?;
        if !output.status.success() {
            return Err(self.complaint(&String::from_utf8_lossy(&output.stderr)));
        }
        let filter = build.get(&self.filter).and_then(Value::as_object);
        let made = self.executables(&String::from_utf8_lossy(&output.stdout), filter);
        match made.as_slice() {
            [one] => Ok(one.clone()),
            [] => Err(format!(
                "{} built no executable the filter picks out",
                self.program
            )),
            many => Err(format!(
                "{} built {} executables; narrow them with a `filter`",
                self.program,
                many.len()
            )),
        }
    }

    /// The executables named in the JSON `output` that `filter` picks out.
    fn executables(&self, output: &str, filter: Option<&Map<String, Value>>) -> Vec<String> {
        output
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|message| {
                self.matching
                    .iter()
                    .all(|(key, wanted)| message.get(key) == Some(wanted))
            })
            .filter(|message| {
                self.compare.iter().all(|(key, path)| {
                    let Some(wanted) = filter.and_then(|filter| filter.get(key)) else {
                        return true;
                    };
                    match at(message, path) {
                        Some(Value::Array(held)) => held.contains(wanted),
                        Some(held) => held == wanted,
                        None => false,
                    }
                })
            })
            .filter_map(|message| {
                message
                    .get(&self.take)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect()
    }

    /// The tail of what the build said to standard error, which is the part that holds the error.
    fn complaint(&self, said: &str) -> String {
        let lines = said.lines().collect::<Vec<_>>();
        let start = lines.len().saturating_sub(COMPLAINT_LINES);
        format!("{} failed:\n{}", self.program, lines[start..].join("\n"))
    }
}
