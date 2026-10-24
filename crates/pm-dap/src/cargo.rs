//! The `cargo` key of a CodeLLDB configuration, which no adapter here reads.
//!
//! A Rust `launch.json` often does not name the program: it names the cargo
//! command that builds it, and CodeLLDB runs that command and finds the
//! executable in its output. `lldb-dap` knows only `program`, so the build is
//! run here and the executable it made is put where `program` goes.

use std::path::Path;
use std::process::Command;

use serde_json::{Map, Value};

/// How many lines of cargo's own complaint are kept when the build fails.
const COMPLAINT_LINES: usize = 12;

/// Replaces the `cargo` key of `config` with the `program` it builds.
///
/// A configuration that names a `program` of its own, or has no `cargo` key,
/// is left as it is. The build runs in the configuration's `cwd`, or in `root`
/// where it names none.
pub(crate) fn resolve(config: &mut Map<String, Value>, root: &Path) -> Result<(), String> {
    let Some(Value::Object(cargo)) = config.get("cargo").cloned() else {
        return Ok(());
    };
    if config.contains_key("program") {
        config.remove("cargo");
        return Ok(());
    }
    let directory = config
        .get("cwd")
        .and_then(Value::as_str)
        .map_or_else(|| root.to_path_buf(), |own| root.join(own));
    let program = build(&cargo, &directory)?;
    config.remove("cargo");
    config.insert("program".into(), program.into());
    Ok(())
}

/// Runs the cargo command `cargo` describes in `directory`, answering the
/// executable it built that the `filter` picks out.
fn build(cargo: &Map<String, Value>, directory: &Path) -> Result<String, String> {
    let arguments = cargo
        .get("args")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    let mut command = Command::new("cargo");
    command
        .args(arguments)
        .arg("--message-format=json-render-diagnostics")
        .current_dir(
            cargo
                .get("cwd")
                .and_then(Value::as_str)
                .map_or_else(|| directory.to_path_buf(), |own| directory.join(own)),
        );
    if let Some(Value::Object(env)) = cargo.get("env") {
        command.envs(
            env.iter()
                .filter_map(|(name, value)| Some((name, value.as_str()?))),
        );
    }
    let output = command
        .output()
        .map_err(|error| format!("cargo could not be run: {error}"))?;
    if !output.status.success() {
        return Err(complaint(&String::from_utf8_lossy(&output.stderr)));
    }
    let filter = cargo.get("filter").and_then(Value::as_object);
    let built = executables(&String::from_utf8_lossy(&output.stdout), filter);
    match built.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err("cargo built no executable the filter picks out".to_owned()),
        many => Err(format!(
            "cargo built {} executables; narrow them with a `filter`",
            many.len()
        )),
    }
}

/// The executables named in cargo's JSON `output` that `filter` picks out,
/// with `name` and `kind` compared to the artifact's target.
fn executables(output: &str, filter: Option<&Map<String, Value>>) -> Vec<String> {
    let wanted = |key: &str| {
        filter
            .and_then(|filter| filter.get(key))
            .and_then(Value::as_str)
    };
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|message| {
            message.get("reason").and_then(Value::as_str) == Some("compiler-artifact")
        })
        .filter(|message| {
            let target = message.get("target");
            let name = target
                .and_then(|target| target.get("name"))
                .and_then(Value::as_str);
            let kinds = target
                .and_then(|target| target.get("kind"))
                .and_then(Value::as_array);
            wanted("name").is_none_or(|wanted| name == Some(wanted))
                && wanted("kind").is_none_or(|wanted| {
                    kinds.is_some_and(|kinds| kinds.iter().any(|kind| kind == wanted))
                })
        })
        .filter_map(|message| {
            message
                .get("executable")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

/// The tail of what cargo said to standard error, which is the part that
/// holds the error.
fn complaint(said: &str) -> String {
    let lines = said.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(COMPLAINT_LINES);
    format!("cargo failed:\n{}", lines[start..].join("\n"))
}
