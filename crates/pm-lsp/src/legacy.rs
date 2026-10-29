//! The older spellings of an install, `go` and `npm`, written out as the `command` install they stand for, so that language folders written before `command` existed still install.

use crate::manifest::{Install, InstallKind};

/// `install` as a `command` install: the older `go` and `npm` kinds with the program, steps, environment and files they imply filled in where the folder left them out, and any other install as it is.
pub(crate) fn command_form(install: &Install) -> Install {
    let mut form = install.clone();
    match install.kind {
        InstallKind::Go => {
            form.tool.get_or_insert_with(|| "go".to_owned());
            form.needs.get_or_insert_with(|| "Go".to_owned());
            if form.steps.is_empty() {
                form.steps = install
                    .package
                    .iter()
                    .map(|package| vec!["install".to_owned(), package.clone()])
                    .collect();
            }
            for (name, value) in [
                ("GOBIN", "{folder}"),
                ("GOSUMDB", "sum.golang.org"),
                ("GOFLAGS", "-mod=readonly"),
            ] {
                form.env
                    .entry(name.to_owned())
                    .or_insert_with(|| value.to_owned());
            }
            form.env_remove.extend(
                ["GONOSUMDB", "GONOSUMCHECK", "GOPRIVATE", "GOINSECURE"].map(str::to_owned),
            );
        }
        InstallKind::Npm => {
            form.tool.get_or_insert_with(|| "npm".to_owned());
            form.needs
                .get_or_insert_with(|| "Node.js and npm".to_owned());
            if form.steps.is_empty() {
                form.steps = vec![
                    [
                        "ci",
                        "--ignore-scripts",
                        "--no-audit",
                        "--no-fund",
                        "--loglevel=error",
                    ]
                    .map(str::to_owned)
                    .to_vec(),
                ];
            }
            if form.copy.is_empty() {
                form.copy = ["package.json", "package-lock.json"]
                    .map(str::to_owned)
                    .to_vec();
            }
            form.bin
                .get_or_insert_with(|| "node_modules/.bin".to_owned());
        }
        InstallKind::Download | InstallKind::Command => {}
    }
    form
}
